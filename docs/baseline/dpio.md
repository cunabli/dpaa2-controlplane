# dpio baseline

<!-- Instantiated from _template.md; every section mandatory, empty sections
     state so explicitly (spec: object-baseline). -->

Claim markers, used per claim throughout: **[read]** = derived from source or
manual, not yet observed; **[verified]** = observed on the board against the
pinned reference environment (see `reference-environment.md`).

Findings are written to be provable: behavioral claims name their
observables and are distilled into the Invariant candidates section as
propositions a Quint model can carry — invariant-bearing or
invariant-breaking.

Cross-family relationships are mapped in [object-model.md](object-model.md);
the board scenarios that will settle this document's open items are
classified in [traffic-inventory.md](traffic-inventory.md).

The family's model parameters live in [models/families/dpio.qnt](../../models/families/dpio.qnt); every invariant candidate below has a disposition row in [models/COVERAGE.md](../../models/COVERAGE.md), and the board suites that settled them are indexed in [models/board/README.md](../../models/board/README.md).

The DPIO is a QBMan software portal: the per-CPU doorway for enqueue,
dequeue, buffer acquire/release, and notifications. Unlike
dpbp/dpcon/dpmcp it is **not an allocatable pool object** in the kernel
— there is no `FSL_MC_POOL_DPIO`; the kernel binds each dpio to its own
driver and offers a global per-CPU service that consumers borrow. The
DPDK/VPP model differs (two exclusive portals per thread), which is why
dpio sizing is the most miscounted number on this board (ADR-0012)
[verified].

## Command surface

restool v2.4: 4 verbs (`dpio_commands.c:514-532`) [read]. `info` prints
id, version, plugged state, cache-enabled/-inhibited area offsets (hex),
QBMan portal id (hex), channel mode, and num-priorities (**printed in
hex** — `0x8` means 8); `qbman_version` and `clk` are fetched and never
printed [read].

## Option inventory: used vs available

| Option | Values | Default | Used by |
|---|---|---|---|
| `--channel-mode` | exactly `DPIO_LOCAL_CHANNEL` \| `DPIO_NO_CHANNEL` (case-sensitive) | `DPIO_LOCAL_CHANNEL` | ls-addni: LOCAL; dprc-script: LOCAL [verified] |
| `--num-priorities` | 1–8 (client-side) | 8 | ls-addni: 8; dprc-script: 8 [verified] |
| `--container` | — | root | both |

Quirk: `--num-priorities` is sent even in `DPIO_NO_CHANNEL` mode, where
the flib documents it as irrelevant — no warning; and the kernel later
infers channel capability from the *reported* priority count, not the
mode (kernel-side section) [read].

## Attribute mutability

All create-time (`channel_mode`, `num_priorities`); nothing mutable at
runtime through the object API. The stashing destination
(`dpio_set_stashing_destination`) is the one runtime knob, exercised
only by the kernel driver [read].

## MC API notes

The dpio flib is **byte-identical between MC 10.32.0 and 10.39.0** — no
delta for the pinned pair [read, mc-utils diff].

## Kernel-side behavior (Linux 6.6.52)

- The dpio driver probes each dpio: portal allocate (a **dpmcp** draw —
  the hidden dependency behind ls-addni's dpmcp-per-dpio rule) → open →
  reset → get_attributes → enable → map QBMan regions → IRQ → register
  in the global service. Portal-allocation failure is downgraded to a
  **silent** `-EPROBE_DEFER` loop [read].
- CPU assignment is **first-come-first-served in probe order** from a
  module-global unused-CPU mask — no id→CPU mapping; dpios beyond the
  CPU count are rejected with `-ERANGE` ("Number of DPIOs exceeds
  NR_CPUS") [read].
- The driver ignores the attribute's `channel_mode` entirely:
  notification capability = `num_priorities != 0`, 8-priority mode =
  `num_priorities == 8`. What MC reports for a NO_CHANNEL dpio decides
  behavior, and that value is not in the corpus (unknown register)
  [read].
- Consumers use `dpaa2_io_service_select(cpu)` /
  `dpaa2_io_service_register` — a single flat per-CPU registry. The
  "general vs ethernet-rx portal" split **does not exist in this
  kernel**; it is the DPDK bus model (ADR-0012's `2 × threads` rule is
  a DPDK-side fact, verified on the board via VPP) [read/verified].
- Stashing (SDEST) setup failure — unknown SoC or MC error — is
  **non-fatal and quiet**: the dpio looks probed, with a large silent
  throughput loss. LX2160A maps CPU→cluster as base 0, size 2 [read].
- Two latent memory-safety bugs: `service_select()` on an **empty dpio
  list** fabricates a bogus non-NULL pointer (defeating every `-ENODEV`
  check — the real zero-DPIO failure mode is corruption, not an
  error), and the region table is indexed without a count check [read].
- A DPIO shortfall at dpni probe degrades quietly: the only positive
  signal is a shorter core list in dmesg ("Cores %*pbl available for
  processing ingress traffic") [read].

## Lifecycle ordering and dependencies

Each dpio consumes **one dpmcp** at kernel probe (ls-addni encodes this
as one dpmcp created per dpio) [read/verified]. The dpio create itself
draws only **one `swp` and one `swpch.8wq`** (restool's default local
channel) from the MC pools and **no mcp** [verified 2026-08-30,
V-POOL-4]; the dpmcp is drawn only when a consumer probes the dpio, not
by the create. Create + plug before consumers; count rules differ by
regime:

- Kernel container: one dpio per online CPU is the ceiling and the
  useful maximum (the board's dprc.1 holds 16 for 16 cores) [verified].
- VPP child container (DPDK): **`NDPIO = 2 × (main + workers)`** — the
  DPDK bus keeps a general and an ethernet-rx portal per thread, each
  an exclusive dpio. Under-provisioning surfaces as DPDK's
  "No software portal resource left" (a DPDK-side string; it exists
  nowhere in the kernel or restool corpus) followed by buffer-free
  failures [verified, ADR-0012; board runs 10 for T=5].

## Intent mapping

Dpio count is a derived quantity of the consumer's thread model, never
user-specified: kernel intent → one per CPU; poll-mode intent → `2 × T`
(ADR-0012). The compiler must also emit the companion dpmcp per dpio
(kernel regime)
and keep dpio creation *before* consumer dpnis in the plan order.

## Silent-failure notes

- Portal (dpmcp) exhaustion at dpio probe: silent infinite defer
  [read].
- SDEST/stashing failures: probed-looking dpio, degraded performance,
  debug-level logs [read].
- ls-main counts existing dpios with an unanchored
  `dprc show | grep -c dpio` — labels containing "dpio" suppress
  creation [read].
- DPDK regime under-provisioning: portal exhaustion mid-run with
  errors that pattern-match to buffer/silicon faults (the ADR-0012
  finding that motivated the `2×T` rule) [verified].
- Kernel zero-dpio state: memory corruption, not a clean error [read].

## Invariant candidates

| Id | Proposition | Observables | Status |
|---|---|---|---|
| DPIO-I1 | dpio is not pool-allocatable: consumers bind/borrow, never draw it from the container resource pools; its own probe *draws a dpmcp* — the dependency arrow points dpio→dpmcp | allocator pool types; dpmcp free-count delta per dpio probe | candidate |
| DPIO-I2 | Regime-typed sizing: kernel container needs ≤ 1 dpio/CPU (extras rejected -ERANGE); a DPDK child container needs exactly 2 per consumer thread — the model must type dpio counts by consumer regime, not treat them as one pool | kernel probe of dpio N+1; DPDK portal-exhaustion error at 2T−1 | verified (ADR-0012; board 16 + 10) |
| DPIO-I3 | **Breaking:** the model must NOT derive kernel channel capability from the configured `channel_mode` — the driver reads only the reported `num_priorities`; the mode attribute is dead in the kernel | dpio info mode vs kernel notification behavior for a NO_CHANNEL dpio | candidate |
| DPIO-I4 | CPU affinity is probe-order-dependent, not identity-stable: dpio N's CPU can change across reboots/rescans; nothing may key on a dpio↔CPU pairing | per-boot dpio→CPU assignment from sysfs/dmesg | candidate |
| DPIO-I5 | **Breaking:** probe success ⇒ full function is false — stashing setup can fail quietly; convergence on performance-relevant state needs its own observable (none exists today: loudness gap) | SDEST MC status vs probe result | candidate |

## Unknown / unverified register

1. ~~What MC reports as `num_priorities` for a `DPIO_NO_CHANNEL` dpio
   (decides DPIO-I3's outcome).~~ — resolved: the requested count, `0x8`
   for a dpio created with `--num-priorities=8` [board-observed 2026-08-25, V-READBACK-1]; the mode does not
   fold the priorities away, so DPIO-I3's kernel half (what the driver
   does with them) is the part still open.
2. Whether `dpio_reset` at probe clears stashing/portal state fully
   (same reset-coverage gap as every family).
3. The DPDK-side portal-allocation algorithm's exact per-thread portal
   types (outside this corpus; ADR-0012's 2×T rule is board-verified
   behaviorally, not source-cited on the DPDK side here). **Narrowed**
   [V-POOL-4, 2026-08-30]: the bus keeps a single MC portal per process
   (`fslmc_vfio_process_group`), so the dpmcp side of the DPDK regime is
   one per process; the per-thread QBMan portal types stay as ADR-0012
   records them.
4. Whether the board's DPC constrains QBMan portal count below the
   dpio count restool would allow.
