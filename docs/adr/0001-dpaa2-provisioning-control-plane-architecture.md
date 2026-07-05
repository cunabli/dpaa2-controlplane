# ADR-0001: DPAA2 provisioning control-plane architecture

- **Status:** Accepted — board-validated on an LX2160A, 2026-07-05
- **Date:** 2026-07-05
- **Supersedes / relates to:** OpenSpec change `add-dpaa2-provisioning` (archived)

## Context

The LX2160A exposes its network datapath through the NXP Management Complex (MC): a
DPNI (network interface object) must be created and connected to a DPMAC (MAC/SerDes
lane) before `fsl_dpaa2_eth` binds it and a Linux netdev appears. The stock tooling
(`ls-addni`/`restool`) is imperative and stateful: it assigns DPNI indices that renumber
across reboots, leaves resources behind on failure, and produces kernel-default names
(`eth0`, `eth1`, …) whose ordering is not stable.

We wanted a **declarative, intent-based** control plane: the operator states *which
physical port (DPMAC) should carry which stably-named interface*, and the tool converges
the board to that intent idempotently, at every boot, with no persisted state.

This ADR records the architecture and — crucially — the assumptions the initial design
got wrong and how on-board reality reshaped them. Several decisions only *solidified*
once the code met the hardware.

## Decision

### 1. Hexagonal architecture with a pure functional core

The system is split into a backend/frontend-neutral **core** (`dpaa2-api`) and thin
**adapters**:

- `dpaa2-api` — the neutral topology model, the trait seams, and the pure
  `reconcile(desired, observed) -> Plan`. It depends on no transport (`restool`,
  ioctl, `serde`) and no config format. Dependencies point *inward*.
- `dpaa2-mc` — southbound adapter over `restool` v2.4 (`McControl`) and the fsl-mc
  sysfs bus (`KernelControl`).
- `dpaa2-config` — northbound adapter turning `topology.toml` into the neutral model.
- `dpaa2-tools` — the imperative shell (`dpaa2ctl`): observe → reconcile → act →
  re-observe, plus `systemd.link` naming.

The seams are **traits** (`McControl`, `KernelControl`, `ConfigSource`, `Runner`), which
let the entire convergence loop run against an in-memory `FakeBackend` with **zero
hardware**. This was not merely testing hygiene: it is what allowed the reconciler's
behaviour to be pinned down (idempotence, edge-matching, drift-refusal, teardown) before
a board was ever touched, and what made the two hardware corrections below cheap to
absorb — they were confined to the adapters, never the core.

### 2. Functional Core / Imperative Shell; level-triggered reconciliation

`reconcile` is total, deterministic, and I/O-free. The shell reads the *entire* observed
state each pass, computes a fresh plan, applies it, and re-observes — **level-triggered**,
never edge-triggered. Nothing is persisted between runs; the MC and the kernel are the
only sources of truth. A half-finished previous run self-heals on the next pass because
the plan is always recomputed from what *is*, not from what we *did*.

### 3. Identity is anchored on the DPMAC, derived by connection edge — never the DPNI index

The operator keys a port by its **DPMAC** (`dpmac.3`), which is fixed by the board's DPC
and never renumbers. A managed DPNI's identity is derived from its *connection edge* to
that DPMAC, not from its MC-assigned index. A DPNI that comes back as `dpni.42` instead
of `dpni.7` still matches the same port. Config that tries to pin a `dpni` index is
rejected outright.

### 4. Ownership is implicit; drift on immutable attributes is refused, not repaired

`reconcile` only ever iterates the *configured* ports, so foreign objects (e.g. the
DPL-provisioned `dpni.0` on the management port `dpmac.17`) are never enumerated, let
alone mutated. Create-time-immutable attribute mismatches are reported as **drift and
refused** — the tool never destroy-and-recreates a live interface to "fix" them.

### 5. Stable naming via runtime-generated `systemd.link`, applied after provisioning

Interface names come from stock `systemd.link` `[Match] MACAddress=` → `[Link] Name=`
files that the reconciler generates into `/run/systemd/network/` (volatile, regenerated
each boot, so they cannot drift from `topology.toml`). No custom udev helper, no marker
files, no persistence.

## What implementation taught us (the corrections)

Two load-bearing assumptions in the original design were **wrong on the board**. Both are
recorded here because they are the least obvious and most likely to be re-broken.

### C1. `dpaa2-eth` allocates a resource pool it does not create — a bare DPNI fails at probe

The design assumed the driver auto-allocated its buffer/queue infrastructure. It does
not. At probe, `fsl_dpaa2_eth` **allocates** (does not create) a DPBP, a DPMCP, and one
DPCON *per queue*, backed by a per-core DPIO pool — from objects that must **already
exist** in the container. A minimal `dpni create` + connect leaves the driver failing
with `fsl_mc_dprc: No more resources of type dpcon left`, a connected-but-unbound DPNI,
and leaked resources.

**Resolution:** `RestoolMc::create_dpni` now provisions the DPNI's private dependencies
before plugging it, mirroring `ls-addni`'s `create_dpni`: top up one DPIO per core (+ a
companion DPMCP each), then create one DPBP, one DPMCP, and `min(num_queues, nproc)`
DPCONs (`dpcon create --num-priorities=2`), each `dprc assign --plugged=1`. This was
containable precisely because it lives entirely in the southbound adapter.

### C2. The matchable MAC lives on the DPNI, and naming must follow provisioning

The design assumed the `.link` could match the **DPMAC's** MAC, read ahead of time, and
be written *before* provisioning so udev caught the netdev-add event. Two errors:

1. `restool dpmac info` reports **no usable MAC** on this board. The address lives on the
   **DPNI**, inherited from the DPMAC at connect (the board's globally-unique burned-in
   MAC, e.g. `d0:63:b4:04:96:25` — distinct from the DPNI's *random* locally-administered
   *permanent* MAC, which would drift per boot). Matching must source the MAC declared →
   connected-DPNI → DPMAC.
2. The DPNI (hence its MAC) does not exist until `create_dpni` plugs it, by which point
   the kernel has already named the netdev `eth1`. So `.link` generation must run **after**
   convergence, and the rename must be *force-applied* to the existing interface via a
   per-interface udev retrigger (`udevadm trigger --action=add --settle
   /sys/class/net/eth1`). The interface is admin-down at that point in boot (before
   `network-pre.target`), so the kernel accepts the rename.

**Board-confirmed chain (2026-07-05):** `Probed interface eth1` → `wrote link file …
name=wan0` (matching `d0:63:b4:04:96:25`) → `applying stable name via udev from=eth1
to=wan0` → `dpni.1 wan0: renamed from eth1` → networkd brings the 25G link up with DHCP.
No `ip link` fallback was needed.

### C3. MC readiness is probed by command round-trip, not a sysfs attribute

The root container `dprc.1` exposes no `firmware_version` attribute, so "is the MC ready"
can only be answered by issuing an MC command (`dprc show`/`dpni info`) and retrying on
failure. `wait_ready` polls via `observe`.

### C4. Idiomatic-Rust conventions adopted after review

- Newtypes (`MacAddr`, `DpmacId`, `DpniId`) **seal** their internals; access goes through
  `Display` (`dpni.N`/`dpmac.N`), `From`, and a last-resort `into_inner()`, so the raw
  integer never leaks and maps key on the strong type.
- Unit tests live **in-module** (`#[cfg(test)] mod tests`); dedicated test files are
  reserved for integration tests.
- Crate roots (`lib.rs`) are **thin barrels** — module wiring and re-exports only.
- `unsafe_code = "forbid"` workspace-wide; the `restool`/sysfs adapter needs none.

## Consequences

**Positive**

- The core is exhaustively testable without hardware; both hardware corrections landed in
  adapters without touching the reconciler or its tests.
- Convergence is idempotent and self-healing across reboots; no state to corrupt.
- Interface names are stable and operator-declared, decoupled from MC index churn.
- A future ioctl-based `McControl` (replacing the `restool` shim) drops in behind the
  same trait with no core changes.

**Negative / to watch**

- The southbound adapter must keep tracking `ls-addni`'s resource recipe (C1); an MC
  firmware update could change the pool the driver expects.
- Naming correctness depends on the DPNI's inherited MAC being the stable, globally-unique
  one (C2); a board/DPC that assigns only random MACs would need MAC *actuation* (already
  scaffolded as `MacMode::Actuate`) to guarantee stability.
- The per-interface udev retrigger relies on the interface being admin-down, which holds
  at early boot but not for a manual re-run against a live interface (documented; a
  down/rename/up fallback remains a known option if that use-case matters).

## References

- OpenSpec change `add-dpaa2-provisioning` (archived) — proposal, design, specs, tasks.
- `ls-main` — NXP restool wrapper; ground-truth reference for the resource recipe (C1).
- Board validation log, 2026-07-05 (`dpaa2ctl status`, `networkctl status wan0`,
  `journalctl -b`).
