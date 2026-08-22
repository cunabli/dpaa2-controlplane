# dpdcei baseline

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

The DPDCEI fronts the DCE compression/decompression engine — an optional
SoC block that **this platform has**: DPAA2UM rev 53 Table 2-1 (p. 2-8)
lists DPDCEI platforms as LS2080, LS2088, **LX2160** [read, manual].
Tier C question answered in two halves: hardware present, but the entire
software stack above it is dead-ended (no DPC/DPL provisions DCE
anywhere in the corpus, no kernel driver, no DPDK PMD, no ls-* usage) —
so MC-level create is plausible yet the family is unusable in practice.
The intent layer still refuses it, on consumer absence rather than
hardware absence [read].

## Command surface

restool v2.4: 4 verbs (`dpdcei_commands.c:531-549`) [read]. `create`
requires **both** `--engine=DPDCEI_ENGINE_{COMPRESSION,DECOMPRESSION}`
(exact string match) and `--priority=<1..8>` (client-side range check);
optional `--container`. `info` runs open → get_attr(v2) → get_api_version →
close; `destroy` issues DESTROY on the parent dprc token. The usage text
claims "default options" exist — it lies; both options are mandatory
(copy-paste help from another family) [read].

restool's flib is a strict subset: 8 of the MC flib's 18 functions.
Missing entirely: `enable/disable/is_enabled/reset`, all irq setters, and
**`set_rx_queue/get_rx_queue/get_tx_queue`** — the only runtime-mutable
state the MC API has (queue dest = DPIO/DPCON) is unreachable from restool;
a restool-created dpdcei is created-but-never-enabled [read].

## Option inventory: used vs available

**Used by ls-main/ls-append-dpl: nothing.** The single repo-wide hit is
`ls-debug:266`, where `dpdcei.*` appears in a glob that routes any object
name to `restool dpdbg dump` — no dpdcei verb is ever called. 100% of the
surface is available-but-unused [read].

## Attribute mutability

`engine` is create-time immutable (compression XOR decompression, manual
§18). `priority` is **write-only**: present in the create config and on the
wire, absent from `dpdcei_attr` — it can never be read back, in any flib
version examined. `dce_version` is fetched over the wire and then discarded
(never printed). Runtime mutability exists only for the rx queue binding,
and only through the MC flib, not restool [read].

## MC API notes

The dpdcei flib is **byte-identical from MC 10.32.0 through 10.40.0**
(API frozen at 2.3); the only change in the window is a header-layout move
[read, mc-utils diff]. The firmware changelog contains **zero** dpdcei
entries in all of 10.x; the only DCE mentions are 2017 LX2-emulation notes
listing DCE among "modules initializations **disabled**" — emulation-model
scaffolding, not a silicon statement (`CHANGELOG.md:1022,1026`) [read, MC
changelog]. Frozen flib + silent changelog reads as **unshipped
software**, not absent hardware — Table 2-1 confirms the block exists on
this SoC [read, manual].

## Kernel-side behavior (Linux 6.6.52)

Bus plumbing only: `fsl_mc_bus_dpdcei_type` is registered
(`fsl-mc-bus.c:378-381,420`) and the open cmd id is in the generic table
(`obj-api.c:28`), so a dpdcei in a scanned dprc enumerates with a sysfs
node — and then **nothing binds**; there is no DCE driver in the tree.
Consequence: restool's `in_use()` sysfs guard is inert for this family —
`destroy` can never be blocked by a driver claim [read].

DPDK (22.11-qoriq and 26.03): zero dpdcei/DCE support; no NXP entry among
the compressdev PMDs. DCE is not a DPDK-reachable path on this stack
[read].

## Lifecycle ordering and dependencies

On paper: create (parent-dprc token) → enable + set_rx_queue to a
DPIO/DPCON (MC flib only) → traffic via an out-of-tree userspace DCE
library → destroy in the creating container with all tokens closed
(manual §18.2.4). On this stack the chain terminates at create: no in-tree
consumer exists at any layer [read].

## Intent mapping

None. No network construct derives a dpdcei; the intent compiler treats
the family as **refuse-at-compile-time** on this platform (invariant
DPDCEI-I1) rather than emitting an object nothing can drive.

## Silent-failure notes

- **DPL round-trip drops `priority`**: the generate-dpl emitter writes only
  `engine` (priority is unreadable), so regenerate-and-reapply yields a
  differently-configured object with no warning [read].
- **`destroy` exit-code bug (non-root container path)**: the MC destroy
  error is overwritten by the `dprc_close` result — a failed destroy
  returns 0 to the shell after printing the error (`dpdcei_commands.c:482-484`;
  the correct `if (error == 0)` pattern exists 200 lines up) [read].
- `assert(false)` on an unknown engine enum in both `info` and
  generate-dpl: a future engine value aborts restool — or, built with
  NDEBUG, falls through emitting no `engine` property at all [read].
- create failure prints the raw MC status; "SoC has no DCE" vs "out of DCE
  resources" is not distinguishable from the tool output [read].

## Invariant candidates

| Id | Proposition | Observables | Status |
|---|---|---|---|
| DPDCEI-I1 | Consumer-unreachable: the DCE block exists on this SoC (Table 2-1) but no software in the corpus can drive a dpdcei — the model refuses dpdcei intents on consumer absence, not hardware absence | manual Table 2-1; `dpdcei create` probe status (`0x0` expected now; `0xB`/`0x8` would re-open the hardware question) | candidate (hardware read, create board-pending) |
| DPDCEI-I2 | **Breaking:** the model must NOT assume generate-dpl round-trips config — `priority` is write-only and silently dropped by the DPL emitter | diff DPL node vs create args | candidate |
| DPDCEI-I3 | **Breaking:** the model must NOT trust restool's destroy exit code in a non-root container — MC error is overwritten by dprc_close success | `$?` after a forced-fail destroy | candidate |
| DPDCEI-I4 | Immutability: `engine` fixed at create; the only runtime-mutable state (rx-queue dest) is unreachable from restool, so restool-visible state is create-frozen | `dpdcei info` before/after any restool sequence | candidate |

## Unknown / unverified register

1. Does MC 10.39.0 on this SoC accept `DPDCEI_CREATE`? With the hardware
   confirmed present, success is now the expected outcome; a refusal
   would mean the firmware build gates the module despite the silicon.
2. Does `DPDCEI_GET_API_VERSION` (token-0, cheap probe) succeed even with
   no DCE hardware — i.e., is the module linked into this MC build?
3. The `dce_version` value (fetched then discarded by restool) — would
   identify the DCE generation if the block exists.
4. MC-side legal `priority` range (restool clamps 1..8 client-side; the
   firmware's own bounds are undocumented in the corpus).
5. The DCE generation on this SoC (Table 2-1 answers presence, not
   revision) — the LX2160A SoC reference manual would say; it is not in
   the corpus (unindexed).
