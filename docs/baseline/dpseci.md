# dpseci baseline

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

The DPSECI is the SEC (crypto accelerator) interface object: queue pairs
into and out of the SEC engine. Its consumer is either the kernel
`dpaa2_caam` driver (a Linux cryptodev) or the DPDK `dpaa2_sec` PMD via
VFIO (what VPP uses). The board runs both profiles side by side: a
16-queue/priority-1 dpseci in the kernel container and an
8-queue/priority-2 dpseci in the VPP child container [verified,
reference-environment.md]. The family anchors the crypto ADRs in
vpp-dpaa2-support (0005 queue-pair wedge, 0007 per-qp FLE isolation, 0008
raw-DP enqueue stub).

## Command surface

restool v2.4 exposes 4 verbs (`dpseci_commands.c:600-618`) [read]:

| Command | MC interaction | Notes |
|---|---|---|
| `--help` | none | |
| `info <dpseci.N> [--verbose]` | open, get_attributes, get_api_version, get_tx_queue ×N | prints id, plugged state, tx/rx queue counts, per-queue tx priorities. **Does not print the options mask** (fetched, discarded) — HAS_CG/HAS_OPR are unreadable through restool |
| `create --num-queues=<n> --priorities=<csv> [--options=<m>] [--container=<c>]` | `dpseci_create` (CMDID V3) | both `--num-queues` and `--priorities` are **mandatory** (no defaults); one `--num-queues` drives tx *and* rx (flib supports asymmetric counts, restool cannot express them) |
| `destroy <dpseci.N>` | `dpseci_destroy` | refuses if driver-bound |

No restool verb exists for enable/disable/reset, rx-queue config, SEC
attributes (era, accelerator counts), SEC counters, congestion, OPR, or
the queue-status commands — that whole surface belongs to consumers
[read]. restool's bundled flib is pinned to dpseci API 5.3 (the 10.32
era), one minor behind the 10.39 firmware's 5.4.

## Option inventory: used vs available

| Option | Constraint | Used by |
|---|---|---|
| `--num-queues` | 1–16 (`DPSECI_MAX_QUEUE_NUM`) | dprc-script: 8 [verified] |
| `--priorities` | comma-separated, count must equal num-queues exactly, each 1–8; help text wrongly caps the list at 8 entries | dprc-script: derived all-2 (restool rejects a count mismatch) [verified] |
| `--options` | `DPSECI_OPT_HAS_CG` (0x20), `HAS_OPR` (0x40), `OPR_SHARED` (0x80); unknown tokens fall back to raw hex — arbitrary bits reach MC | dprc-script: all three, mirroring the vendor default [verified] |
| `--container` | default root | dprc-script: child dprc [verified] |

**No ls-script creates or manages dpsecis.** The only reference in the
whole scripts corpus is ls-debug's generic dump glob (`ls-debug:266`);
ls-main has none; ls-append-dpl would fail on every shipped dpseci DPL
node (no `num_queues` property exists in DPL — queue count is implied by
the priorities array length — so no `--num-queues` flag is generated, and
`fdtget` renders priorities space-separated, which restool cannot parse)
[read].

DPL usage across NXP boards: LX2160A DPLs declare 16 priorities of 1 plus
`DPSECI_OPT_HAS_CG`; one legacy DPL uses priority 0 and a `sec_if_id`
property, both outside restool's accepted vocabulary [read].

## Attribute mutability

Create-time-immutable [read, `fsl_dpseci.h`]: `options`, `num_tx_queues`,
`num_rx_queues`, and the per-tx-queue `priorities[]` — there is no
`dpseci_set_tx_queue` at all; tx SEC-processing priority is fixed for the
object's life. `dpseci_attr` does not return the priorities array — it is
recoverable only queue-by-queue via `get_tx_queue`.

Runtime-mutable (consumer flib only, not restool): rx-queue destination
(`set_rx_queue`: DPIO/DPCON/NONE dest, priority, user_ctx,
order-preservation, with `DPSECI_ALL_QUEUES` broadcast), congestion
notification (requires `HAS_CG` at create; entry threshold 0 disables),
OPR config, irq config, enable/disable/reset [read].

The asymmetry to encode in typestate: **rx queues are steerable, tx
queues are read-only**; and the congestion group either exists from birth
(`HAS_CG`) or can never be added.

## MC API notes

10.32→10.39 delta [read, mc-utils diff]: minimal — API 5.3 → 5.4, adding
exactly two commands, `GET_RX_QUEUE_STATUS`/`GET_TX_QUEUE_STATUS` (0x172/
0x173): per-FQ qbman scheduling state, XOFF/retirement/overflow flags,
frame and byte counts. No struct, option, or version changes to anything
pre-existing. restool (pinned 5.3) cannot reach them; they are the
natural observables for wedge detection (ADR-0005) in the Rust
southbound.

Neither flib validates anything at create — queue counts, priority
ranges, and option bits pass through unchecked; MC firmware is the sole
validator, and its rules are not in the corpus (unknown register) [read].

restool's `--options` handling is version-gated on `mc_fw_version.minor
>= 1` — on a firmware below x.1 the object is created with options 0 and
restool *then* exits non-zero on the unconsumed flag: same
created-then-failed shape as the dpni dead options [read]. restool's
`mc_v10/dpseci.c` also skips the `cpu_to_le32` on the options field that
mc-utils performs (a real divergence, moot on little-endian) [read].

`dpseci_get_sec_attr` (CMDID V2) returns SEC era plus per-algorithm
accelerator counts; `dpseci_get_sec_counters` returns seven u64 counters
that are **global to the SEC block, not per-dpseci** — two dpsecis (our
board's exact layout) read the same counters [read].

## Kernel-side behavior (Linux 6.6.52)

Consumer: `dpaa2_caam` (`drivers/crypto/caam/caamalg_qi2.c`),
type-only match, no version floor [read].

**Probe**: allocates exactly **1 DPMCP** (portal) from the parent
container's pool — no DPBP, no DPCON, no IRQs; DPIOs are borrowed from
the kernel's global DPIO service, not drawn per object.
`num_pairs = min(num_rx_queues, num_tx_queues, online CPUs)`; a clamp is
a warning only, and queues beyond `num_pairs` are **never given a
destination** — parked, silent capacity loss (same dark-queue shape as
the dpni's DPCON shortfall). Each of the first `num_pairs` CPUs gets an
rx queue steered `DEST_DPIO` at priority 0 (pull-mode comment: WQ
priority is irrelevant to volatile dequeues); every CPU may enqueue to
the tx FQIDs [read].

**Reset is version-gated and skips 5.3 exactly**: `dpseci_reset` runs
only when the reported API is *strictly greater* than 5.3, at probe and
at teardown. If the firmware reports 5.3, unbind leaves the rx-queue
steering (kernel DPIO ids, `user_ctx` = stale kernel pointers) and an
**armed congestion group whose `message_iova` points at freed kernel
memory** live in the MC. On >5.3 the ordering is correct. Board firmware
is 10.39/dpseci 5.4, so the reset path should be live — board-check
worth one dmesg line (unknown register) [read].

**Congestion**: configured only when the object has `HAS_CG` (and API ≥
5.1): byte units, entry 128 MiB, exit 90% of that, memory-write-only
(dest NONE — no DPIO notification), polled at enqueue. On congestion the
driver **drops with -EBUSY** — no backlog, no backpressure, a
rate-limited debug-level log. Without `HAS_CG` the check is skipped
entirely: unbounded enqueue with no log line [read].

**Whitelist** (`/dev/dprc.N`): the only dpseci-specific command allowed
is `GET_TX_QUEUE`; generic open/close/get-attr/get-api-version pass, and
create/destroy need `CAP_NET_ADMIN`. Everything else the kernel driver
does (enable, disable, reset, set_rx_queue, congestion, sec-attr,
sec-counters) is `-EACCES` from userspace via the root container [read].

**Ownership**: the dpseci API has no exclusivity primitive — multiple
tokens on multiple portals are structurally permitted; the only stated
rule is "all tokens closed before destroy". Exclusivity is enforced
solely by the Linux driver model (`driver_override`, VFIO's container
notifier — which *warns*, not refuses, when a kernel driver grabs an
object inside a VFIO container). VFIO `close_device` unconditionally
open→**reset**→close-es the object, wiping any other opener's rx-queue
and congestion config [read].

## Lifecycle ordering and dependencies

Creation (our deployed path, no vendor script exists): `dpseci create`
in the target container with queues+priorities+options →
`assign --plugged=1` alongside the container's dpio/dpbp/dpmcp → VFIO
binding with the rest of the child dprc; the PMD probes it as one
cryptodev with `num-queues` queue pairs [verified]. Kernel path: object
in the kernel container, `dpaa2_caam` binds on plug, draws its dpmcp,
steers rx queues to per-CPU DPIOs [read].

Consumer sizing rule [verified, vpp crypto baseline]: one queue pair per
crypto worker; 8 queues cover the 2-worker async + offload split with
headroom. In-flight per qp must be capped by the consumer (512, half the
FLE pool) — over-posting wedges the qp permanently (ADR-0005).

Teardown: kernel unbind disables and (API > 5.3) resets; destroy
requires all tokens closed and an unbound object. No ls-delete support —
dpseci is not an accepted entry type there [read].

## Intent mapping

The dpseci realizes the crypto-accelerator construct (ADR-0005 intent
vocabulary): an intent like "IPsec offload with W crypto workers"
derives one dpseci with `num_queues ≥ W` (+ headroom for a split
async/offload path), priorities uniform (all-2 deployed; the kernel DPL
convention is all-1 — priority semantics between the two are an unknown),
and `HAS_CG,HAS_OPR,OPR_SHARED` as the vendor-default safety set. The
`HAS_CG` bit is a **safety property**: without it the kernel consumer
runs with no congestion backstop at all. Congestion thresholds and
in-flight caps are consumer-side, not object-side, and belong to the
runtime model.

## Silent-failure notes

- restool `destroy` **overwrites the destroy error with the dprc_close
  result** in non-root containers — a failed destroy can report success
  while the object survives (`dpseci_commands.c:549-553`) [read].
- restool `info` cannot show the options mask; convergence checks on
  HAS_CG/HAS_OPR through restool are impossible — raw `GET_ATTR` (which
  the /dev/dprc.N whitelist permits) is the only observable [read].
- `--options` on pre-x.1 firmware: object created with options 0, then
  nonzero exit (create-then-fail) [read].
- Kernel: queue clamp beyond CPU count is a warning; parked queues are
  invisible. Zero registered algorithms is still a successful probe. A
  dpseci without HAS_CG silently loses all backpressure. Congestion hit
  = drop (-EBUSY) at debug verbosity [read].
- **Queue-pair wedge** [verified, ADR-0005]: over-driving one qp past
  the PMD's FLE pool wedges it permanently — enqueue returns 0 forever,
  no MC error, no counter imbalance on the consumer side; only the PMD's
  own qp stats localize it. The new 5.4 queue-status commands are the
  MC-side observable this family lacked.
- The per-cpu enable/disable loops index per-cpu structs by plain
  counter while setup uses the online-CPU mask — with a non-contiguous
  online mask, NAPIs are enabled/disabled on the wrong CPUs, quietly
  [read].

## Invariant candidates

| Id | Proposition | Observables | Status |
|---|---|---|---|
| DPSECI-I1 | `options`, tx/rx queue counts, and per-queue tx priorities are immutable post-create; no setter exists (tx queues have no set path at all) | `get_attributes` + per-queue `get_tx_queue` before/after suites | candidate |
| DPSECI-I2 | Create precondition (restool layer): priorities count = num-queues, each in 1–8; MC-layer validation is unknown and must not be assumed | restool exit on mismatch; MC status on out-of-range via DPL | restool layer board-anchored 2026-08-29 (V-DPSECI-1 rev 1: priority 0, a priority above 8, and a priority-count ≠ num-queues are each refused by restool's own parser, exit 234, before any MC command; also V-LIFE-DPSECI-1 rev 1 and production use); MC-layer validation unreachable through restool, board-pending → V-DPSECI-1 (MC layer) under `dpseci-typestate` (#8) |
| DPSECI-I3 | **Breaking:** the model must NOT treat restool `info` output as the convergence observable for this family — the options mask is not printed; only raw `GET_ATTR` observes it | info output vs GET_ATTR response | candidate |
| DPSECI-I4 | Safety: consumer backpressure exists iff `HAS_CG` was set at create; absent it, enqueue is unbounded (kernel consumer) | congestion config presence; enqueue behavior at saturation | candidate |
| DPSECI-I5 | **Breaking:** the model must NOT assume unbind ⇒ clean MC state: the kernel reset is gated on API > 5.3, and rx-queue steering + armed CG (with dangling iova) persist when skipped | `get_rx_queue`/`get_congestion_notification` after unbind | board-pending (board API is 5.4 → reset expected live) |
| DPSECI-I6 | Liveness ceiling: a queue pair over-posted past its FLE depth wedges permanently with no MC-visible error; consumers must self-cap in-flight (≤ half the FLE pool) | enqueue returns 0 with in-flight 0; qp stats; 5.4 queue-status flags | verified (ADR-0005) |
| DPSECI-I7 | **Breaking:** the model must NOT assume MC-enforced exclusivity: multiple open tokens are structurally permitted, and a VFIO close resets the object under any other opener; single-owner is a modeling assumption (ADR-0006), not an MC property | concurrent open success; config wiped after VFIO close | candidate |
| DPSECI-I8 | Destroy precondition: all tokens closed and no bound driver; restool's destroy result is unreliable in child containers (error overwritten) | object presence after "successful" destroy | candidate |
| DPSECI-I9 | SEC counters are block-global: two dpsecis observe one counter set; per-object accounting must come from queue-status/consumer stats, never `get_sec_counters` | counter deltas across both objects under single-object load | candidate |

## Unknown / unverified register

1. MC-side validation rules at create: does firmware reject priority 0,
   priorities ≠ queue count, or rx ≠ tx counts? (No validator in either
   flib; one legacy DPL ships priority 0.) Partially answered [board
   suite V-DPSECI-1 rev 1, 2026-08-29]: restool's own parser refuses all
   three (priority 0, a priority above 8, and a count ≠ num-queues) with
   exit 234 before any MC command is built, so the MC-side rule stays
   unreachable through restool — the ioctl portal is needed to reach it.
2. Board confirmation that dpseci API reports 5.4 (drives DPSECI-I5's
   reset path) — one dmesg/`restool dpseci info` line.
3. Whether the kernel container's dpseci carries `HAS_CG` (it predates
   our script; restool cannot show it — needs raw GET_ATTR).
4. Priority semantics 1 vs 2: kernel DPLs use all-1, our VPP profile
   all-2 [verified in use]; what the SEC scheduler does with the
   difference is undocumented in the corpus.
5. The legacy `sec_if_id` DPL property (single occurrence, no cfg field
   anywhere) — dead or MC-parsed?
6. `DPSECI_OPT_HAS_OPR`/`OPR_SHARED` observable behavior: no consumer in
   the corpus inspects them (kernel sets order_preservation_en=0; OPR
   wire code exists unused).
7. What `dpseci_reset` covers (per-field) — same gap as dpni_reset.
8. Board values of `sec_attr` (era, accelerator counts) — never logged
   by the kernel; relevant to algorithm capability modeling.
