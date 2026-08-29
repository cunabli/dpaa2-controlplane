# dpmcp baseline

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

The family's model parameters live in [models/families/dpmcp.qnt](../../models/families/dpmcp.qnt); every invariant candidate below has a disposition row in [models/COVERAGE.md](../../models/COVERAGE.md), and the board suites that settled them are indexed in [models/board/README.md](../../models/board/README.md).

The DPMCP is an MC command portal: a small MMIO window through which a
software context sends the 64-byte MC commands every other family runs
on. It carries no datapath function — it is the **control plane's own
resource**, and its pool sizing gates everything else: every consumer
driver draws one, every dpio draws one, and every concurrent
`/dev/dprc.N` opener beyond the first draws one [read].

## Command surface

restool v2.4: 4 verbs (`dpmcp_commands.c:483-501`) [read]. `info`
prints id ("object id/portal id"), version, plugged state, label, and a
decoded options mask — unless any unknown bit is set, in which case it
prints "Unrecognized options found..." and decodes **nothing** [read].

## Option inventory: used vs available

One option token exists: `--options=DPMCP_OPT_HIGH_PRIO_CMD_DIS`
(0x1, disables the high-priority command queue); default none [read].
`dpmcp_cfg.portal_id` is encodable in the flib but restool **hardwires
pool assignment** (−1) with no CLI to pin it — dead capability, same
shape as dprc's icid/portal pinning (DPL-only) [read]. Used by:
ls-addni (1 per dpio + 1 per dpni), ls-addsw (1), ls-addmux (1 — but
created in the **root** container even when the dpdmux goes to a child:
a placement bug), ls-append-dpl (generic); our dprc-script creates
`NDPMCP=3` in the VPP child [verified, ADR-0012].

## Attribute mutability

Create-time: options, portal id (pool-assigned). No runtime setters.
The kernel's MC surface for the family is open/close/reset only — it
never even reads dpmcp attributes [read].

## MC API notes

The dpmcp flib is **byte-identical between MC 10.32.0 and 10.39.0** —
no delta for the pinned pair [read, mc-utils diff]. Minimum version the
kernel accepts is 3.0 (older → `-ENOTSUPP`, loud) [read].

## Kernel-side behavior (Linux 6.6.52)

dpmcp is allocatable (shared pool mechanics: `dpbp.md`) but **special**:
`fsl_mc_object_allocate` explicitly rejects it — the only path is
`fsl_mc_portal_allocate`, which draws from the pool *and* opens the
object, maps its MMIO region, and wraps it in an `fsl_mc_io` with one
of two locking regimes: sleeping (mutex + usleep polling) or atomic
(spinlock + udelay), chosen per consumer at allocate time. dpaa2-eth,
dpaa2-switch and the child-DPRC portal are atomic; dpio, dpseci, dprtc,
dpmac-standalone, dpdmai, and the uapi are sleeping [read].

Consumers (complete in-tree census): one dpmcp per dpni, dpsw, dpio,
dprtc, standalone dpmac, dpseci, dpdmai, dpdmux(evb) — the per-dpio
draw is the one most often forgotten and is why ls-addni creates a
dpmcp inside its dpio loop [read/verified].

`/dev/dprc.N` (root only): the **first concurrent opener uses the root
DPRC's built-in portal** (zero pool draw); each additional simultaneous
opener draws one dpmcp, returned on close. So the cost is
concurrency-driven, not cumulative — N parallel restool invocations
were read as needing N−1 free dpmcps, with exhaustion surfacing as
`open()` failing with `ENXIO` [read].

**Single-opener mechanism** [verified 2026-08-29, V-POOL-3 rev 1]: the
board never reaches that exhaustion, because it admits only **one** opener
at a time. `fsl_mc_uapi_dev_open` calls `fsl_mc_portal_allocate` for the
root dprc, which draws a dpmcp from the root pool, opens it, and then
records the root dprc as that dpmcp's **consumer** with `device_link_add`
(`mc-io.c`). The dpmcp is the root dprc's own child, so the link would
make a device depend on its descendant; the device core refuses such a
cycle, the allocator maps the refusal to `-EINVAL`, and the second
`open()` fails `EINVAL` — not `ENXIO`, and with the pool nowhere near
empty (over a hundred free at the time). This holds for any process, not
just restool, and is the kernel enforcing ADR-0006's single-writer stance
one layer below the tool (ADR-0006 amendment 2026-08-29). The N−1 pool
draw the corpus predicted is unreachable through this device.

**DPMCPs are never reset in this kernel**: `fsl_mc_portal_reset` exists,
is exported, and has zero callers — a portal returns to the pool
carrying whatever state its previous owner left [read]. MC command
timeouts (15 s) and error statuses are logged at debug level only —
a stalled MC is invisible at default loglevel [read].

## Lifecycle ordering and dependencies

Dpmcps must exist and be plugged before *anything* that talks to the MC
through a pooled portal — including dpio probes. The dependency graph
bottoms out here: dpmcp depends only on its container. Counts:
kernel-side = one per consumer object (see census); VPP child =
`NDPMCP=3` (PMD portals) [verified]. Teardown: portal free → pool
return (no reset) → unbind → destroy.

## Intent mapping

Pure derived companion: `#dpmcp = #portal-consuming objects (+ dpio
count)` in the kernel regime; a fixed small count (3) in the VPP child
[verified]. Plus **concurrency headroom for the management plane
itself**: the reconciler must model its own portal draw when running
alongside other openers — a self-referential resource unique to this
family.

## Silent-failure notes

- Unknown option bits make restool `info` refuse to decode the whole
  mask [read].
- Options overflow past 32 bits truncates silently (debug-only
  diagnostic) [read].
- Never-reset portals circulate through the pool (loudness gap: no
  observable distinguishes a clean portal from a dirty one) [read].
- 15-second MC command timeouts are debug-level — the control plane's
  primary transport fails invisibly [read].
- ls-addmux's dpmcp lands in the root container regardless of
  `--container` — the consumer then draws from the *target* container's
  pool, which was never topped up: converged-looking create, deferred
  consumer [read].
- Portal exhaustion = `ENXIO` from `open("/dev/dprc.N")` — an error
  that pattern-matches to a device/permission problem, not a pool
  problem [read].

## Invariant candidates

| Id | Proposition | Observables | Status |
|---|---|---|---|
| DPMCP-I1 | Bottom of the dependency graph: every MC-command-issuing consumer holds ≥ 1 dpmcp (or the root's built-in portal); dpmcp itself depends only on container membership | consumer probe/open outcomes vs pool free-count | candidate |
| DPMCP-I2 | Concurrency law for the uapi: N simultaneous `/dev/dprc.N` openers require N−1 pool dpmcps; the first is free (built-in portal); closes return them | `open()` errno at exhaustion; free-count across open/close sequences | falsified as stated 2026-08-29 (V-POOL-3 rev 1): the uapi admits **one** opener at a time — the second `open()` fails `EINVAL` while the first is held, with over a hundred portals free, so exhaustion is never reached and `ENXIO` never appears (single-opener mechanism below; ADR-0006 amendment); rev 2 counted it — one held, 119 refused |
| DPMCP-I3 | **Breaking:** the model must NOT assume portal state is clean on allocate — no reset happens anywhere in the kernel lifecycle, ever | portal state after a re-allocation following an aborted owner | candidate |
| DPMCP-I4 | Placement law: the dpmcp must be in the *consumer's* container (pool locality, DPRC-I1); a top-up in any other container is a no-op for the consumer | consumer defer despite global dpmcp surplus | verified (DPRC-I1 class; ls-addmux bug demonstrates the violation [read]) |
| DPMCP-I5 | **Breaking:** transport failures are not observable at default verbosity — the model's fairness assumption ("commands eventually complete or fail loudly") does not hold; every convergence check needs its own timeout | absence of dmesg output during a forced MC stall | candidate |
| DPMCP-I6 | A destroyed dpmcp's MC portal is not returned to `mc.global --resources` for the rest of the boot — not on destroy, not when its container is destroyed; only a reboot restores the count | the `mcp` count across create, destroy, container destroy and reboot | verified 2026-08-29 (V-CEIL-1 rev 2 + the rev 2 snapshots): 200 → 138 after 64 creates, 138 after 64 destroys, 138 after the child's destroy, 203 after the reboot (ADR-0011 §3) |

## Unknown / unverified register

1. Semantics of `DPMCP_OPT_HIGH_PRIO_CMD_DIS` beyond its one-line
   gloss (what uses the high-priority queue at all — `MC_CMD_FLAG_PRI`
   is defined but no in-corpus caller sets it).
2. What state a dpmcp can actually carry across owners given no reset
   (is DPMCP-I3 vacuous because portals are stateless, or real?).
   **Re-anchored** [V-POOL-2 rev 1, 2026-08-29]: restool exposes no
   portal state at all — `dpmcp info` prints id, version, plugged state,
   label and the options mask, none of which a re-allocation would move —
   so this stays unobservable through restool and needs the raw portal
   backend (`mc-portal-backend`, #10) to settle.
3. Whether pinning `portal_id` via DPL has observable consequences
   (restool cannot express it).
4. The board's dpmcp id-14 hole in dprc.1 (52 dpmcps, one id gap) —
   boot-time consumption or DPL artifact? [carried from
   reference-environment capture].
5. **Does a destroyed dpmcp return its MC portal to the pool?** — No,
   not for the rest of the boot [answered, V-CEIL-1 rev 1 + rev 2 and
   the rev 2 snapshots, 2026-08-29; ADR-0011 §3; DPMCP-I6 below].
   Creating 64 dpmcps in a scratch child drew the MC portal count from
   200 to 138 (62 drawn for 64 creates), all 64 destroyed cleanly
   without the count moving, the checkpoint after the *child itself*
   was destroyed still read 138, and only the reboot restored 203. A
   firmware leak, not a container quota. The reconciler treats dpmcp
   churn as consuming a fixed 203-portal budget per boot and never
   recycles a portal through destroy. What stays open is the listing's
   arithmetic — 62 for 64, and 203 to 200 before the family ran in both
   sittings after five and six earlier create/destroy pairs.
