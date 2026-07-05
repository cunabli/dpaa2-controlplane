## Context

The MC firmware on the LX2160A owns all DPAA2 objects. A network port becomes a
Linux netdev only after: (1) MC is loaded and responsive, (2) a DPNI is created
and connected to a DPMAC via the MC portal, (3) `fsl_dpaa2_eth` binds the DPNI.
This is hardware provisioning that must run before `networkd`, be idempotent, and
survive reboots. The MC firmware is the authoritative store of what exists; the
operator's intent lives in a declarative file. We have both halves of truth
without inventing a third.

Prior exploration (`dpaa2-init-design.md`) established the reconciler direction and
discarded systemd-oneshot-as-primary, pure-udev-shell, and gNMI-as-mechanism. This
document settles the architecture and the decisions surfaced while pressure-testing
it, and maps them onto the existing four-crate workspace.

Target scope: 2×10G SFP+ and 1×25G QSFP28-breakout on the LX2160A board,
point-to-point DPNI↔DPMAC, with room to extend.

## Goals / Non-Goals

**Goals:**
- Declarative, intent-based provisioning of DPNI↔DPMAC ports on the MC.
- Level-triggered, idempotent, hardware-authoritative reconciliation.
- Stable, meaningful interface names surviving reboots and index renumbering.
- Strongly-typed config and state; pure, testable reconciliation logic.
- Two swappable seams (config source; device backend) around a pure core.
- Clean netdev handoff to L3 consumers.

**Non-Goals:**
- The direct `/dev/mc_cmd` ioctl portal (future change; relaxes `unsafe_code` in
  `dpaa2-mc` only). Phase 1 uses the `restool` shim behind the same trait.
- DPSW (switch) topologies (model must accommodate later; not implemented now).
- A long-running daemon, gNMI/YANG northbound, or any external state DB.
- Setting link speed / SerDes protocol (fixed in the DPC at boot; verify-only here).
- L3 configuration (addresses, routing) — that is the consumer's job.

## Decisions

### D0. Architecture: level-triggered hexagonal reconciler with a functional core
The control plane is a **closed-loop controller** (Kubernetes-operator / control
theory). Desired state is declarative *intent* (IBN); observed state is read from
the MC every pass; a **pure** `reconcile(desired, observed) -> Plan` computes the
delta; an imperative shell actuates it. Ports & Adapters: a northbound config-source
port and a southbound device port are pluggable adapters; the core is transport-
and schema-agnostic (Functional Core, Imperative Shell).

Crate mapping (dependencies point inward to `dpaa2-api`, which depends on neither):
```
        dpaa2-tools ──▶ dpaa2-api ◀── dpaa2-mc
        (shell/CLI)   (core+traits)  (southbound)
             ▲              ▲
        dpaa2-config ───────┘   (northbound; toml now, dpaa2-gnmi later)
```
*Alternatives:* single binary (design doc's original) — rejected to keep the seams
crate-enforced; edge-triggered transactional commit (holo-style) — rejected because
MC state resets independently of our intent, so we must re-observe every pass.

### D1. Identity is anchored to the stable DPMAC, matched by edge
MC assigns DPNI indices at creation; the operator cannot pick `dpni.3`. Therefore
config is keyed by the **static DPMAC** (`dpmac.3`), and a managed DPNI's identity
is **derived from its connection edge** to that DPMAC, not from its index. Reconcile
matches desired↔observed by graph structure (endpoints), never by index equality.
*Alternative:* persist a symbolic-name→index map — rejected; it becomes a third
source of truth that lies after a crash or external `restool` use.

### D2. No persistence, no daemon, no DB
Intent (`topology.toml`) + observed (MC firmware) are the two sources of truth.
A DB (OVSDB-style) is unnecessary because — unlike OVS — our observed state is fully
recoverable from the hardware. Daemonizing is justified only by continuous
event-driven reconciliation (future gNMI plane), which is a *shell* change around
the same pure core, not a persistence requirement.

### D3. Stable naming via known MAC + stock `systemd.link` (C), DPMAC-anchored (A)
Each port's netdev inherits its DPMAC's burned MAC automatically (verified, E3), so
a stock `systemd.link` `[Match] MACAddress=` → `[Link] Name=` renames it with **no
MAC actuation at all**. The MAC is readable ahead of time from `restool dpmac info`.
No custom udev helper, no marker file, no persistence. MAC actuation (setting the
DPNI primary MAC) is retained only as an optional future capability; phase 1 is pure
assert.

The `.link` files are **generated at runtime by the reconciler into
`/run/systemd/network/`** (volatile tmpfs, regenerated each boot), not shipped as
static files — they are derived artifacts of `topology.toml` and cannot drift from
it. `10-dpaa2-<name>.link` (`[Match] MACAddress=<dpmac-mac>` → `[Link] Name=<stable>`)
sorts before the stock `99-default.link` (first match wins) and `/run` outranks
`/usr/lib` for same-named files, so it reliably wins. The zero-MAC `macN`
placeholders (E6) never match it. The reconciler writes the files and runs
`udevadm control --reload` **before** provisioning, so link config is loaded before
the `ethX` add event fires. *Not a systemd generator:* generators emit units, and
`.link` (udev config) is not searched under the generator directories; the reconciler
that already owns the config is the natural producer.
*Alternatives:* udev `IMPORT{program}` walking netdev→DPNI→DPMAC→name from live MC
state (kept as fallback if MAC-match proves unworkable); MC object labels (only if
labels are settable and persist — unverified).

**REVISED after board testing (2026-07-05).** Two assumptions above were wrong:
1. *The matchable MAC is not on the DPMAC.* `restool dpmac info dpmac.3` reports no
   usable MAC on this board; the address lives on the **DPNI** (inherited from the
   DPMAC at connect — the board's burned-in MAC, e.g. `d0:63:b4:04:96:25`, distinct
   from the DPNI's random locally-administered *permanent* MAC). `link::match_mac`
   now sources it declared → connected-DPNI → DPMAC.
2. *Generation cannot precede provisioning.* The DPNI (hence its MAC) does not exist
   until `create_dpni` plugs it, by which point the kernel has already named the
   netdev `eth1`. So generation moved to **after** `converge`, and `link::apply`
   re-triggers udev per interface (`udevadm trigger --action=add --settle
   /sys/class/net/eth1`) to rename it this boot. Board log confirms the chain:
   `Probed interface eth1` → `wrote link file … name=wan0` → `applying stable name
   via udev from=eth1 to=wan0` → `dpni.1 wan0: renamed from eth1`. The interface is
   admin-down at that point in boot (before `network-pre.target`), so the rename is
   accepted. The `udevadm control --reload` is retained; the per-interface trigger is
   the load-bearing step and needs no `ip link` fallback (6.6).

### D4. udev is presentation only, never the reconciliation trigger
The synthetic-event fanout DAG from the design doc is dropped. udev/`systemd.link`
does exactly one job: rename the netdev when it appears. The reconciler owns
provisioning end-to-end.

### D5. MC-ready trigger: a systemd unit gated on `dprc.1`, ordered before networkd
A `systemd` unit runs the reconciler to completion, gated on the appearance of
`dprc.1` (device dependency) plus a **liveness probe that issues an MC command**
(e.g. `dprc info dprc.1` / get-version) and retries until it responds — because MC
10.32.0 exposes **no `firmware_version` sysfs attribute** (E5). Ordered
`Before=network-pre.target`. A one-line udev rule may poke the unit on `dprc.1`
`add`; it triggers once and does not reconcile.
*Alternative:* stat a sysfs attribute — rejected, not present; `After=`-only temporal
ordering — rejected as guessing, not detecting.

### D6. Southbound is two ports: `McControl` + `KernelControl`
Creating/connecting objects is an MC portal concern; binding DPNI→`dpaa2-eth` is a
kernel sysfs concern; and `Bound` is frequently a state we *wait to observe* (the
kernel probes asynchronously), not an action we execute. The southbound is therefore
split into `McControl` (fsl-mc) and `KernelControl` (sysfs bind + netlink/sysfs
observe). `restool` shim implements `McControl` in phase 1.

### D7. Ownership: touch only our subgraph; never enumerate-and-delete
Reconcile may only create/modify/destroy objects **reachable from a DPMAC named in
our config**. It never lists all MC objects and deletes those absent from intent —
that would destroy DPL-provisioned or foreign objects. We assume we are *augmenting*
a possibly-empty DPL and own only our subgraph. Teardown of a port removed from
config is **opt-in** (`--prune`); default is to leave it in place.

### D8. Minimal managed config surface; refuse unsafe drift
We manage existence, connection, and (optionally) the primary MAC — nothing more.
Many DPNI attributes are create-time-only, so a change to an immutable attribute is
**reported as drift and refused**, not silently resolved by destroy+recreate of a
live interface. This keeps the drift surface tiny and blast radius bounded.

### D9. Actuatable vs assert-only intent
The desired model distinguishes fields we **actuate** (existence, connection,
optionally MAC) from fields we only **assert** (link speed / SerDes, board-burned
MAC). Assert-only fields are verified against reality and reported, never written.

### D10. Test strategy: hardware-free by construction, one HIL gate
Testability is the primary justification for D0 (functional core) and D6 (trait
seams), so the test approach follows directly and must be explicit:
- **Pure core (`dpaa2-api`):** exhaustively unit-tested against a **fake in-memory
  MC** — a test double implementing `McControl`/`KernelControl` over an in-memory
  `ObservedTopology`. Because the seam is a trait, the full observe → reconcile →
  act → re-observe loop runs with **zero hardware**: idempotence, ownership guard,
  drift refusal, renumber-stability, assert-only verification, convergence/requeue.
- **Config (`dpaa2-config`):** unit-tested parsing, neutral-model conversion, and
  validation errors.
- **restool shim (`dpaa2-mc`):** parsing tested with **recorded-output fixtures**
  (golden files captured from restool v2.4), since the `restool` binary cannot be
  cleanly mocked. Command construction asserted against the D-recipe.
- **CLI (`dpaa2-tools`):** convergence-loop and exit-code behavior tested by driving
  the shell against the fake backend (no board).
- **Hardware-in-the-loop:** a single end-to-end smoke test — the `ls-addni`-style
  create→connect→named-netdev flow already run by hand — as the on-board gate.
Each spec `#### Scenario:` is an acceptance test; the fake backend is what makes the
non-HIL ones runnable in CI.
*Alternative:* mock `restool` via PATH shims — rejected as more fragile than
fixture-based parse tests plus the fake trait backend.

## restool shim reference recipe (from `ls-main`, restool v2.4 / MC 10.32.0)

The phase-1 `McControl` shim reproduces the exact sequence `ls-addni`/`ls-listni`
use. Use `--script` for machine-parseable output (returns just the object id),
avoiding fragile table scraping.

Create + connect (our `create_dpni` + `connect`). **A DPNI is not usable alone**:
`dpaa2-eth` *allocates* a DPBP, a DPMCP, and one DPCON per queue from the container's
pool at probe, backed by a per-core DPIO pool. Those objects must already exist, so
`create_dpni` provisions them first exactly as `ls-addni`'s own `create_dpni` does
(corrects the original E4 assumption — the driver does **not** create them):
```
# private dependencies first (each created then `dprc assign --plugged=1`)
#   ensure one dpio per core (+ a companion dpmcp each) — idempotent top-up
#   dpbp create; dpmcp create; N× (dpcon create --num-priorities=2), N = num_queues
dpni=$(restool --script dpni create --num-queues=$nproc $dpni_args)
        # dpni_args: num_tcs, mac_entries, vlan_entries, qos_entries, fs_entries, …
[restool dpni update $dpni --mac-addr=$mac]                 # actuate mode only
restool dprc assign $container --object=$dpni --plugged=1   # plug → driver probes
restool dprc connect $container --endpoint1=$dpni --endpoint2=$dpmac
restool dprc sync                                           # force bus rescan
```

Observe (our `McControl::observe` + `KernelControl` netdev lookup):
```
restool dprc show $container | grep dpni                    # enumerate DPNIs
restool dpni info dpni.N | grep endpoint:                   # → connected dpmac
ls /sys/bus/fsl-mc/devices/$container/dpni.N/net/           # → linux netdev name
```

Notes: creation is `dpni create` then `dprc assign --plugged=1` (two steps, not
one); `dprc sync` after every mutation; endpoint string is
`endpoint: dpmac.7, link is up` (parse the object ref before the comma).

## Risks / Trade-offs

- **[E1: DPMAC is `LINK_TYPE_FIXED`]** → then `dpaa2-eth` never binds, no netdev
  appears, and MAC-match naming cannot fire. *Mitigation:* the design branches on
  link type; for FIXED ports, "provisioned" means connected (no netdev/rename
  stage), surfaced distinctly in `status`. Verify per-DPMAC on the board (E1 task).
- **[E4: minimal DPNI does not yield a working netdev]** (missing DPBP/DPIO/DPCON in
  the container) → *Mitigation:* verify container resources; extend provisioning to
  allocate them only if `dpaa2-eth` does not. Kept out of the managed surface unless
  proven necessary.
- **[restool absent or version-skewed vs MC firmware]** → phase-1 "fast to first
  light" advantage erodes. *Mitigation:* one-command check on the board; the trait
  boundary lets us move to the ioctl portal without touching the core.
- **[MAC-match naming unsupported on `dpaa2-eth` netdevs]** (E3) → *Mitigation:*
  fall back to the udev `IMPORT{program}` resolver (D3 alternative).
- **[Reconcile races networkd]** → *Mitigation:* ordering `Before=network-pre.target`
  and `.link` rename at netdev-add time before networkd claims the interface.

## Migration Plan

Phase 1 (this change): `restool` shim behind `McControl`; get the pure reconciler
correct and validated on hardware. Phase 2 (future change): direct ioctl portal as a
second `McControl` impl, relaxing `unsafe_code` in `dpaa2-mc` only; no core changes.
Phase 3 (optional): event-driven daemon + gNMI/YANG northbound reusing the same core.
Rollback: the tool is additive and idempotent; removing the unit stops provisioning,
and existing objects persist in the MC untouched.

## Board findings (verified 2026-07-04, MC 10.32.0, restool v2.4)

- **dpmac.3–6**: PHY, CAUI, 25000 Mbps → the QSFP28 breakout (25G lanes).
- **dpmac.7–10**: PHY, XFI, 10000 Mbps → the SFP+ cage (10G).
- **dpmac.17**: PHY, RGMII, 1000 Mbps → onboard 1G management, **already
  provisioned as `dpni.0` (link up)**. Pre-existing DPL object — out of scope,
  must not be touched (validates D7 ownership).
- The original `dpaa2-init-design.md` port mapping (dpmac.3/4 = 10G, dpmac.17 =
  25G) is **incorrect for this board** and is superseded by the table above.
- A created DPNI netdev **inherits the connected DPMAC's MAC** (verified: dpni.7→
  dpmac.7 gave MAC …29 = dpmac.7's MAC). MACs are sequential and readable from
  `restool dpmac info` ahead of time → naming needs no MAC actuation.
- **CORRECTION (verified on board 2026-07-05):** `dpaa2-eth` does **not** create
  DPBP/DPCON/DPMCP — it *allocates* them from objects that must already exist in the
  container. A minimal DPNI + connect fails at probe with
  `fsl_mc_dprc: No more resources of type dpcon left`. The earlier observation that
  "connecting a DPNI spawned dpbp.N + ~16 dpcon + dpmcp" was in fact `ls-addni`
  creating them. `create_dpni` must therefore provision a DPBP, a DPMCP, one DPCON
  per queue, and top up the per-core DPIO pool — exactly as `ls-addni`'s own
  `create_dpni` does (see `ls-main`). This is confined to the `dpaa2-mc` adapter; the
  core model is unchanged.
- `dprc.1` has **no `firmware_version` attribute** on MC 10.32.0.
- Unconnected DPMACs each appear as a placeholder netdev `macN`; connecting a DPNI
  replaces `macN` with the `dpaa2-eth` `ethX`. Origin of `macN` naming TBD (E6).

## Open Questions

Resolved:
- **E1 — RESOLVED (favorable):** all DPMACs are `DPMAC_LINK_TYPE_PHY`; none fixed.
  phylink path applies, netdevs appear, MAC-match naming viable.
- **E2 — RESOLVED:** indices per the board-findings table above.
- **E3 — RESOLVED (best case):** netdev inherits the DPMAC's stable MAC; naming keys
  on it with no MAC actuation. Assert mode is sufficient; MAC actuation deferred.
- **E4 — REOPENED then RESOLVED (unfavorable, 2026-07-05):** minimal DPNI + connect
  does **not** yield a working netdev — the driver allocates DPBP/DPCON/DPMCP/DPIO
  from a pool that must be pre-provisioned. `create_dpni` now creates a DPBP, a
  DPMCP, one DPCON per queue, and tops up the per-core DPIO pool (mirrors `ls-addni`).
- **E5 — RESOLVED (probe changed):** no `firmware_version` attr; readiness must be
  probed by issuing an MC command (e.g. `dprc info`) and retrying — see D5.

- **E7 — RESOLVED:** `ethtool -P` confirms the inherited DPMAC MAC is the netdev's
  *permanent* address, so `systemd.link [Match] MACAddress=` matches it.

- **E6 — RESOLVED:** `macN` is the `fsl_dpaa2_mac` driver's kernel name, preserved
  by the stock `99-default.link` (`NamePolicy=keep kernel …`); no custom rule owns
  it. Placeholders carry an all-zero permanent MAC, so our MAC-keyed
  `10-dpaa2-*.link` matches only provisioned `ethX` (no collision).

All empirical questions resolved.
