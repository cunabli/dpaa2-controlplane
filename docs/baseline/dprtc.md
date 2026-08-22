# dprtc baseline

<!-- Instantiated from _template.md; every section mandatory, empty sections
     state so explicitly (spec: object-baseline). -->

Claim markers, used per claim throughout: **[read]** = derived from source or
manual, not yet observed; **[verified]** = observed on the board against the
pinned reference environment (see `reference-environment.md`).

Findings are written to be provable: behavioral claims name their
observables and are distilled into the Invariant candidates section as
propositions a Quint model can carry — invariant-bearing or
invariant-breaking.

The DPRTC fronts the physical IEEE-1588 real-time clock. It is the **only
Tier C family present on the reference DPC** (dprtc.0, DPL-instantiated
into the root container [verified]) — trivially board-exercisable, but
ownership is settled and is not ours: **firmware enforces a single dprtc
object system-wide** (MC 10.31.0, filed as a *Fix*: "Restrict the creation
of multiple dprtc objects to a single object only"; the manual concurs —
one DPRTC serves the GPP PTP stack), and the kernel owns it. The clock is
a singleton resource, not a per-consumer companion [read, MC changelog].

## Command surface

restool v2.4: 4 verbs (`dprtc_commands.c:439-457`) [read]. `create` takes
only `--container`; `dprtc_cfg.options` is a placeholder the flib
explicitly discards (`(void)(cfg)`) — zero create-time tunables, same
shape as dpbp. `info` prints exactly version/id/plugged/label — **no
time, no frequency**; a stopped or wildly-off clock reads identically to
a healthy one. `info --verbose` adds the IRQ mask, but restool's vendored
flib knows only ALARM and PPS cause bits — the PPS2/ETS1/ETS2 bits MC
10.39 uses decode as unknown [read].

The MC attr response carries `{paddr, id, little_endian}` (the 1588 block
base, exported at 10.16.0); restool reads `id` and **silently discards
the other two** [read].

## Option inventory: used vs available

**Used by ls-main/ls-append-dpl: nothing** (ls-debug's only hit is the
`dpdbg dump` glob — which the firmware has no dprtc handler for, see
dpdbg.md). Available-unused: the whole restool surface. The runtime API
that matters (set/get time, clock offset, **frequency compensation**,
alarms, ext-trigger timestamps, FIPER loopback, enable/disable/reset)
exists only in the MC flib — no restool verb [read].

## Attribute mutability

Through restool: nothing mutable, `id` the only readable field. Through
the MC flib: the clock is heavily runtime-mutable (time, offset, freq
compensation, alarms, IRQ config) — all runtime-only; nothing at all is
configured at create [read].

## MC API notes

The dprtc flib is **byte-identical between MC 10.32.0 and 10.39.0**
(frozen at 2.3 since ≤ 10.32; only the header-layout move) [read,
mc-utils diff]. Firmware behavior in and around the window
(`qoriq-mc-binary/CHANGELOG.md`): **10.31.0** imposed the single-object
restriction; **10.37.0** fixed blast radius — "in case the instantiation
of a DPRTC fails, no other objects are impacted", i.e. on pre-10.37
firmware a failed dprtc create could perturb *unrelated objects*.
The 10.36 timestamp fix (dpnis behind a dpsw stamped at the originating
dpmac) is filed under DPSW and names 1588 but not dprtc — timestamping
correctness is a WRIOP/dpmac property, not a dprtc one. Nothing at
10.38.x–10.40 [read, MC changelog].

## Kernel-side behavior (Linux 6.6.52)

`fsl-dpaa2-ptp` binds `obj_type "dprtc"` (default y, depends on
dpaa2-eth + qoriq PTP) and publishes **/dev/ptpN** via `ptp_qoriq_init`.
The split of responsibility is the family's defining quirk: the kernel's
dprtc flib is **IRQ-only** (open/close + 6 IRQ calls); all actual clock
reads/adjusts go through the **ioremapped 1588 register block from the
device tree** (`fsl,dpaa2-ptp` node), bypassing MC entirely. MC is the
interrupt plumber; MMIO is the clock [read].

dpaa2-eth linkage is via two exported globals (`dpaa2_ptp`,
`dpaa2_phc_index`), written by the PTP module at probe. If the DT node is
absent, probe bails `-ENODEV` **without a dev_err**, and every kernel
dpni silently degrades to software timestamping (`ethtool -T` shows
software-only, no error anywhere). Rx/Tx timestamps themselves ride the
**frame annotation**, stamped by WRIOP/dpmac — not fetched from dprtc
[read].

DPDK (both trees): dprtc support is compiled out unless the manual
`-DRTE_LIBRTE_IEEE1588` c_arg is set (no meson option); default builds
have **zero** dprtc dependency. The IEEE1588 path carries real bugs: a
failed object-create leaves a dangling static pointer (use-after-free on
first timesync read); timesync ops are registered whether or not a dprtc
was found (NULL deref in a container without one); and `adjust_time` is a
get/add/set step — `dprtc_set_freq_compensation` exists in the flib and
is never called, so a servo steps rather than syntonizes [read].

## Lifecycle ordering and dependencies

Normal path: DPL-instantiated at boot (bare `dprtc@0` node, no
properties — matching restool's generate-dpl stub, which emits exactly
that) → fsl-mc bus enumerates → kernel probe → /dev/ptpN. Runtime create
is possible but capped at one object system-wide. Destroy requires the
creating context's token (manual §16.2.4) — whether the *MC-created*
(DPL) dprtc.0 can be destroyed by GPP software at all is an open item,
and a one-way trip for kernel PTP regardless [read].

## Intent mapping

**Not derived from topology — a fixed root-container singleton.** The
intent compiler pins dprtc.0 in the root container with the kernel
(DPRTC-I2) and never emits one for a consumer container: the VPP child
needs no dprtc (datapath timestamps come from frame annotations), and a
second object is unobtainable anyway. If VPP ever genuinely needs to
steer the RTC, ownership moves wholesale — kernel PTP goes away — it is
never shared.

## Silent-failure notes

- `dprtc info` is near-blind (version/id/plugged/label only) — clock
  health is invisible; `paddr`/`little_endian` are fetched and discarded
  [read].
- Missing DT node: kernel PTP silently absent, dpnis quietly fall back to
  software timestamping with no log line [read].
- **No arbitration on the clock**: DPDK writes time via MC
  (`dprtc_set_time`) while the kernel drives the same hardware via MMIO;
  nothing prevents both — a DPDK set_time steps the clock underneath a
  running kernel PTP servo with no diagnostic on either side [read].
- Pre-10.37 firmware only: a failed dprtc instantiation could impact
  unrelated objects (fixed on the pinned 10.39) [read, MC changelog].
- DPDK IEEE1588 builds: dangling-pointer/NULL-deref paths above; the bus
  discards object-create failures entirely [read].

## Invariant candidates

| Id | Proposition | Observables | Status |
|---|---|---|---|
| DPRTC-I1 | Singleton: at most one dprtc exists system-wide; a second create is refused loudly (MC ≥ 10.31) | MC status of `dprtc create` with dprtc.0 present | board-pending (exact status unknown) |
| DPRTC-I2 | Ownership exclusivity: the dprtc belongs to exactly one stack (kernel via MMIO+IRQ, or a DPDK IEEE1588 build via MC) — the model must treat "both configured" as an invalid state, since hardware allows the silent fight | /dev/ptpN presence vs DPDK build flags | candidate |
| DPRTC-I3 | **Breaking:** the model must NOT treat restool `info` success as clock health — the readable surface carries no time/frequency state | `info` on a stopped vs running clock | candidate |
| DPRTC-I4 | Datapath independence: packet timestamping needs no dprtc in the consumer container — stamps originate at WRIOP/dpmac and ride frame annotations | VPP child has no dprtc; timestamps still present kernel-side | verified (reference DPC + 10.36 changelog) |
| DPRTC-I5 | Create-config emptiness: like dpbp, two dprtcs would be interchangeable at create (options discarded) — identity is placement, not configuration | flib `(void)(cfg)` | candidate (unfalsifiable beyond one object) |

## Unknown / unverified register

1. Exact MC status string/byte for a second `dprtc create` (manual
   truncates mid-sentence; likely No-resources or Invalid-state).
2. Whether the refused create is clean post-10.37 — `dprc show` unchanged
   before/after (the 10.37 fix's observable).
3. Whether `dprtc destroy` on the DPL-created dprtc.0 is permitted at all
   (creator is the MC itself) — test only if the container can be
   re-DPL'd; it kills kernel PTP either way.
4. Reported API version on the board (flib says 2.3; restool's header
   says 2.0-compatible).
5. `paddr`/`little_endian` from MC vs the DT `ptp-timer` node — the
   kernel ioremaps from DT while MC reports its own view; agreement is
   assumed, unverified.
6. Manual Table 2-1 marks the DPRTC row "Two-step 1588 only", yet the
   changelog adds one-step/single-step APIs from 10.22.0 and the kernel
   carries a one-step SYNC path — stale manual comment or a real
   platform limit? Decides whether one-step timestamping is claimable
   here.
