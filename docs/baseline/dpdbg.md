# dpdbg baseline

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

Tier C question answered up front: dpdbg is a **real MC object family, not
a facade** — it owns module id 0xf in the MC opcode space with the full
canonical verb set, is created/destroyed against a dprc token, opened by
object id through a normal dprc-scoped OPEN, appears in `dprc show`,
enumerates on the fsl-mc bus with its own device type, is DPL-declarable
(`fsl,dpdbg`), and the firmware blob carries a complete object driver
(`dpdbg_drv_*`) [read]. The facade intuition is half right, though: it is
a **singleton, root-container-only control object** whose sole payload is
a command surface over MC-*global* debug state (console, DDR log, log
level, timestamps, UART routing) — one attribute (`id`), no endpoints, no
resources, no connections. Added at MC 10.20.0 together with ls-debug;
absent from every reference DPC in the corpus, so on this board it exists
only if runtime-created (**board-exercisable: yes** — the only Tier C
family besides dprtc that is) [read, MC changelog].

## Command surface

restool v2.4: 6 verbs (`dpdbg_commands.c:745-771`) [read]. `create` issues
CREATE against the **root** dprc handle only (firmware enforces:
"object creation forbidden - DPDBG must be created in a root container");
`destroy` likewise, object id hardcoded 0 — every restool call site pins
`dpdbg_id = 0`. `dump --memory` and `dump --object=dpXX.N` issue
`DPDBG_DUMP` (0x130); `set --console/--log/--timestamp/--level/--uart`
issue `DPDBG_SET` (0x140) with a module string. Every verb does its own
open/close pair — no persistent handle [read].

Not in restool's vendored flib: the CTLU profiling counter commands
(0x152/0x153) present in the mc-utils flib — unreachable from this tool
[read].

## Option inventory: used vs available

**ls-debug is the consumer** (the one Tier C family a ls-* script
actually drives): it lazily auto-creates dpdbg before *every* operation
(existence probed by string-matching `info` output), then uses all five
`set` modules and both `dump` forms. Its `destroy_dpdbg()` helper is
**defined and never called** — the object, once created, is never removed
[read]. Side effect beyond the family: for its whole run it disables
`/sys/bus/fsl-mc/autorescan` bus-wide ("we manually handle this"),
restoring the prior value via an EXIT trap — suppressing child-DPRC
hot-plug visibility for every concurrent actor (see dprc.md,
Kernel-side) [read]. Available-unused: `destroy`, `info` as an actual display (only
substring-scanned), the get-api-version flib call (implemented, no verb).
No shell completion, no restool.md entry — the family is undocumented in
restool's own manual [read].

## Attribute mutability

Degenerate object surface: `dpdbg_attr { int id; }` — nothing else
readable. All mutability lives in `DPDBG_SET` and mutates **MC-global
firmware state**, not object state: console on/off, DDR log on/off,
timestamp on/off, log level 0–5, uart id 0–4. **Write-only**: there is no
`dpdbg_get` counterpart — current debug state cannot be read back through
the API at all [read].

## MC API notes

Present and **byte-identical from MC 10.32.0 through 10.39.0** (first
appeared at 10.20.0; only the header-layout move in the window) [read,
mc-utils diff]. Changelog: exactly two entries ever — 10.20.0 "Added
DPDBG object and integrated it with restool; a single DPDBG object can be
created and can only be added in a root container", and 10.30.0 lifting
the 1204-byte log output cap. Nothing in the pinned window [read, MC
changelog]. Strings in the shipped firmware itb confirm a live object
driver and pool, and the root-container refusal message verbatim [read].

## Kernel-side behavior (Linux 6.6.52)

Recognized bus type, no driver: `fsl_mc_bus_dpdbg_type` registered, open
cmd id in the generic table — a created dpdbg enumerates driver-less.
The `/dev/dprc.N` uapi allowlist carries `DPDBG_DUMP` and `DPDBG_SET` as
its **first two entries, with no capability flag** — dump/set need no
CAP_NET_ADMIN, while create/destroy go through the generic CREATE/DESTROY
entries which do. The kernel putting dpdbg's two opcodes first in the
allowlist is the strongest kernel-side signal that this surface is meant
to be driven from userspace on production systems [read].

## Lifecycle ordering and dependencies

Singleton lifecycle with no dependencies: CREATE (root dprc only) →
open/op/close per command → DESTROY. No endpoints, no connect, no pool
membership; creatable/destroyable at any point after MC boot.
Alternative entry: DPL-declared `dpdbg@0 { compatible = "fsl,dpdbg"; }` —
reference DPCs don't, so runtime-create is the only path here. ls-debug's
pattern is create-lazily-never-destroy [read].

## Intent mapping

Not a network construct — an **operational facility**. The intent layer
never derives a dpdbg from topology; the Rust ls-debug replacement owns
it as a singleton companion (create-if-absent in the root container,
DPDBG-I1), the same lazy pattern ls-debug uses today.

## Silent-failure notes

- **Dump output is observable only in the MC log at level ≥ INFO**: with
  console off or level above INFO (a common default), `dump` succeeds,
  prints nothing anywhere, and ls-debug still reports "dumped information
  available in MC log/console". Console/level must be set *before* any
  dump — an ordering trap the script does not enforce [read].
- ls-debug's dump glob accepts `dpaiop.*` and `dprtc.*`, but the firmware
  has **no dump handler for either** (only the 13 families + mem); whether
  that returns a status or just logs "Unknown Object type" is unresolved
  (unknown register) [read].
- `restool dpdbg destroy` prints "is destroyed" **unconditionally** — the
  error branch prints the MC status but falls through to the success
  message with no early return [read].
- `SET` is fire-and-forget and restool does no range validation — the
  0–5/0–4 bounds live only in ls-debug's shell checks; `set --level=99`
  reaches firmware unvalidated [read].
- restool's create version gate returns `1` (not a negative errno) on
  too-old MC — a `< 0` caller would treat refusal as success (dead weight
  at 10.39, wrong-shaped anyway) [read].

## Invariant candidates

| Id | Proposition | Observables | Status |
|---|---|---|---|
| DPDBG-I1 | Singleton, root-only: at most one dpdbg exists, only in the root container; create elsewhere or a second create is refused by firmware | MC status of second create / non-root create; firmware refusal string | candidate (corpus-attested, board-pending for status codes) |
| DPDBG-I2 | **Breaking:** the model must NOT treat debug state as readable — SET has no GET counterpart; the model can only track what it has itself written (and must tolerate unknown initial state) | absence of any get verb in flib 10.20–10.40 | candidate |
| DPDBG-I3 | **Breaking:** the model must NOT infer dump success from restool/ls-debug exit status — output goes to the MC log gated by console+level state set by a *different* command | dump with console off: exit 0, no output anywhere | candidate |
| DPDBG-I4 | Object-family membership: a created dpdbg is visible in `dprc show` and enumerates on the fsl-mc bus driver-less — indistinguishable in listing mechanics from any other family | `dprc show`; /sys/bus/fsl-mc/devices | board-pending |

## Unknown / unverified register

1. `DPDBG_DUMP` with an unhandled type (dpaiop/dprtc): nonzero MC status,
   or 0 with only an MC-log line? Decides how loud the Rust wrapper must
   be.
2. `DPDBG_SET` with out-of-range level/uart — rejected or silently
   clamped?
3. Whether `--uart` rerouting is recoverable without reboot on a board
   where the MC UART is shared with the boot console (safety-envelope
   question before ls-debug parity testing, ADR-0003).
4. Persistence across fsl-mc rescan and warm reset (nothing in the DPC
   recreates it).
5. Confirm dump/set truly work unprivileged through the uapi (allowlist
   says yes; ls-debug needs root anyway for autorescan writes).
