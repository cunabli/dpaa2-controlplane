# dpaiop baseline

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

The DPAIOP represents an **AIOP tile** — the optional programmable packet
engine of LS1088/LS2088-class SoCs. Tier C question answered up front:
**not board-exercisable on this platform**, on two independent grounds:
LX2160A carries no AIOP silicon — stated directly by DPAA2UM rev 53
Table 2-1 (p. 2-8), which lists DPAIOP platforms as LS2088, LS1088,
LA1575 with LX2160 absent [read, manual], corroborated by the config
corpus (no `aiop` node in any lx2160a/lx2162a DPC, versus LS1088A's
explicit one) — and MC firmware has
**refused `DPAIOP_CREATE` on AIOP-less platforms since release 10.18.0**
("Disabled DPAIOP object creation on platforms which do not have an AIOP
block") — 21 releases before the pinned 10.39.0 [read, MC changelog].
Note the container-permission point: dprc.1 having OBJ_CREATE allowed is
not sufficient — the 10.18.0 gate is a platform check inside the MC's
create handler, orthogonal to DPRC permissions [read].

## Command surface

restool v2.4: 4 verbs (`dpaiop_commands.c:545-563`) [read]. `create`
requires `--aiop-container=<dprc.N>` (a *separate* DPRC created with
`DPRC_CFG_OPT_AIOP`), optional `--container` for the parent. `info` runs
open → get_attr → get_api_version → **get_sl_version + get_state** (the
AIOP boot state machine) → close.

restool **cannot load or run an AIOP image**: `RESET`, `LOAD`, `RUN`,
`SET_TIME_OF_DAY`, `SET_RESETABLE` exist in the MC flib but have no
restool verb — image load belongs to an out-of-tree "AIOP Tool" (the
qoriq DPDK 22.11 tree ships `dynamic_AIOP_dpl.sh` showing the split:
restool creates, the tool loads) [read].

## Option inventory: used vs available

**Used by ls-main/ls-append-dpl: nothing** (zero references; ls-debug's
only hit is the generic `dpdbg dump` glob). Available-unused: the whole
surface. `dpaiop_cfg` has two fields; restool hardcodes `aiop_id = 0` and
exposes only `aiop_container_id` [read]. Related but distinct:
`DPRC_CFG_OPT_AIOP` (0x20) is settable via `dprc create --options=` and
emitted by generate-dpl — also unused by any ls-* script [read].

## Attribute mutability

Create-time: `aiop_container_id` (user), `aiop_id` (hardcoded 0).
Runtime-readable: `id` (the **only** field in `dpaiop_attr` —
`aiop_container_id` is write-only and unreadable), service-layer version,
and the boot state (bit flags: RESET_DONE/ONGOING, LOAD_DONE/ONGOING/ERROR,
BOOT_ONGOING/ERROR, RUNNING). Runtime-mutable: nothing via restool;
load/run/reset/time-of-day only via the MC flib [read].

## MC API notes

The dpaiop flib is **byte-identical from MC 10.32.0 through 10.39.0**
(object version pinned 2.3; only the header-layout move) [read, mc-utils
diff]. The firmware changelog has **zero** dpaiop entries in the window;
all 11 AIOP entries predate 10.32, and the load-bearing one is 10.18.0's
platform gate on create (above). The `dpaiop_load_cfg.tpc` field is
documented in the flib literally as `TODO` [read, MC changelog].

## Kernel-side behavior (Linux 6.6.52)

No dpaiop driver exists — only bus plumbing (`fsl_mc_bus_dpaiop_type`,
open-cmd table entry): a dpaiop would enumerate unbound and nothing would
probe it. The `/dev/dpaa2-mcX` uapi allowlist sanctions exactly **two**
dpaiop commands — `GET_SL_VERSION` and `GET_STATE` — the two `restool
dpaiop info` depends on; anything else through that path is rejected by
the kernel, not MC, with an error pointing at the wrong layer.
`dpaa2-console.c` registers `/dev/dpaa2_aiop_console` **unconditionally**
even with no AIOP present (open checks for an AIOP magic in MC DDR) [read].

## Lifecycle ordering and dependencies

Manual ch. 20: (1) create an AIOP-flagged DPRC → (2) `DPAIOP_CREATE`
pointing at it → (3) open → (4) `RESET` (AIOP block must be in reset
before load) → (5) `LOAD` (ELF at `img_iova`) → (6) `RUN` (`cores_mask`)
→ (7) poll state to RUNNING. Error paths are strict: LOAD failure
requires RESET before retrying; RUN failure forbids resending RUN *or*
LOAD — restart from RESET. restool implements only step 2 and the read
side of step 7 [read].

## Intent mapping

None. No network construct derives a dpaiop on this platform; the intent
compiler refuses the family at compile time (DPAIOP-I1), same posture as
dpdcei.

## Silent-failure notes

- `info` prints the API version **before** checking the query error —
  on failure it prints uninitialized stack values as a version, then
  errors (`dpaiop_commands.c:229-236`) [read].
- `assert(false)` on an unknown state value; states are bit *flags*, so a
  plausible OR-combination aborts restool — or with NDEBUG prints
  `"DPAIOP state: "` with no value at all [read].
- **generate-dpl drops `aiop_container_id`**: the parse stub writes
  nothing (the attr struct can't read it back), so a regenerated DPL
  emits a dpaiop node missing the property a hand-written DPL requires —
  round-trip is lossy and fails only at the next boot [read].
- `destroy` against a non-root parent overwrites the destroy error with
  the `dprc_close` result — failed destroy, exit code 0 (same family-wide
  pattern as dpdcei) [read].
- `create` with `--container` leaks the opened DPRC handle on the failure
  path [read].

## Invariant candidates

| Id | Proposition | Observables | Status |
|---|---|---|---|
| DPAIOP-I1 | Platform-unsupported: LX2160 is absent from the DPAIOP row of manual Table 2-1, has no AIOP config surface, and MC ≥ 10.18.0 refuses create on AIOP-less silicon; the model refuses dpaiop intents | manual Table 2-1; `dpaiop create` probe MC status | corpus-confirmed (manual + gate); status code board-pending |
| DPAIOP-I2 | **Breaking:** the model must NOT assume DPRC permission ⇒ create permission — OBJ_CREATE gates the container check, the platform gate lives inside the MC create handler | create refused in a container whose options allow OBJ_CREATE | candidate |
| DPAIOP-I3 | **Breaking:** the model must NOT assume generate-dpl round-trips config — `aiop_container_id` is write-only and silently dropped | regenerated DPL node vs hand-written DPL | candidate |
| DPAIOP-I4 | Two-object coupling: a functional dpaiop requires a *pair* (AIOP-flagged dprc + dpaiop referencing it) created in order; neither alone is meaningful | DPDK 22.11 `dynamic_AIOP_dpl.sh` sequence | candidate (unfalsifiable here) |

## Unknown / unverified register

1. The exact MC status `DPAIOP_CREATE` returns on LX2160A (the 10.18.0
   gate names no errno) — needed for a precise negative-path assertion.
2. Whether the refusal fires at `dpaiop create` or already at
   `dprc create --options=...,DPRC_CFG_OPT_AIOP` — two separately
   observable steps.
3. What `/dev/dpaa2_aiop_console` open returns with no AIOP (the magic
   check depends on DDR contents the code alone can't settle).
