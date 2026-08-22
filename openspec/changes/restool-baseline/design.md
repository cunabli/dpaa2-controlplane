## Context

The repo ships a board-validated control plane for one object pattern
(DPNI+DPMAC with driver-dependency provisioning, ADR-0001). The project goal
is the full configuration-management surface of DPAA2: all 16 restool object
families ported to typestate-correct Rust, driven by a network-construct
intent layer, converged by the existing level-triggered reconciler, and —
new to this series — verified with formal models (Quint) that are validated
against the board itself via model-based testing.

This change is the series opener. It produces no code: it produces the
documented ground truth (what restool/`ls-main` actually do), the master
roadmap, and the ADRs that fix the engineering process for every later
change. The decisions below were settled in the 2026-08-22 scoping session
and are recorded here so the ADR tasks transcribe, not re-litigate, them.

Ground-truth corpus (all read-only, in the workspace): `src/restool` C source
and its `ls-main`/`ls-debug`/`ls-append-dpl` scripts; `src/mc-utils/api`
per-MC-release API deltas; Linux 6.6.52 dpaa2 drivers; the DPAA2 reference
manual via the MCP index; validated learnings in `vpp-dpaa2-support`
(scripts, ADRs 0001–0012, archived openspec changes).

## Goals / Non-Goals

**Goals:**

- One baseline document per object family, uniform template, exhaustive on
  the command/option surface and honest about what is unknown vs verified.
- A cross-object relationship map precise enough to seed typestate design
  and Quint lifecycle models without re-reading C.
- A pinned reference environment: all later board evidence is stamped
  against MC 10.39.0 + Linux 6.6.52 + restool v2.4.
- A master roadmap naming every change in the series, its tier, its
  dependencies, its board-session sync points, and its decision points.
- ADRs 0002–0006 codifying the process decisions (D2–D9 below).

**Non-Goals:**

- No Rust, no Quint models, no harness (change `verify-foundation` opens
  those). No spec changes to existing capabilities.
- No multi-MC-version analysis beyond documenting deltas: the port pins to
  MC v10 on this board's firmware.
- No board mutation: the single board task is a read-only capture script.

## Decisions

### D1. Tiered coverage, all families ported

All 16 families are in scope for the port; tiers order the work, they do not
cut it. Tier A (datapath, battle-tested here): dprc, dpni, dpmac, dpbp,
dpio, dpcon, dpmcp, dpseci. Tier B (switching topology): dpdmux, dpsw.
Tier C (accelerators/misc): dpaiop, dpci, dpdcei, dpdmai, dprtc, dpdbg.
The baseline documents all 16 with the same template; tier only affects
roadmap ordering. Alternative (Tier C as inventory-only) was rejected: the
big picture requires every tile carved, with depth added when a tile is
reached.

### D2. Quint primary, TLA+ escalation (→ ADR-0002)

Formal models are written in Quint (TLA+ semantics, Apalache-checkable,
executable via simulator, pnpm-installable — fits the repo toolchain and the
frequent-rebaseline workflow). A single model escalates to hand-translated
TLA+ only when Apalache cannot check a needed temporal property; the
escalation is recorded when taken. Models are living artifacts: amended and
rebaselined as understanding grows, never frozen documents.

### D3. Dual-mode MBT over one shared adapter (→ ADR-0002)

Model↔board conformance runs in two modes over one adapter (model action ↔
restool command ↔ observed state): **batch** (default) — traces generated
offline from the Quint simulator, replayed linearly on the board, diffed
offline; **online** (discovery) — a driver couples model and board
step-by-step, steering into unexplored states; used for opaque MC semantics
(dprc containment/pools, dpsw, dpdmux), exhaustion/error paths, and whenever
a batch divergence shows the model's branching structure is wrong. Every
online finding is frozen into a batch trace; frozen traces are committed and
later replay against the Rust core (ITF) as regression tests. Model-before-
code ordering: a change's Quint model lands green before its Rust does.

### D4. Human-launched board interaction, total transparency (→ ADR-0003)

The assistant never executes anything on the board and never crosses an SSH
boundary. Batch suites are emitted as reviewable scripts (every restool
command visible, the model's expected outcome as a comment beside each step)
that the operator runs; results come back as files. The online driver is a
program the operator launches and supervises with step/pause/abort and a
full transcript. Online granularity: per-step confirm while a family is
being learned; per-block with abort after its model survives a full batch
suite; promotion is per-family and recorded.

### D5. Port safety envelope, coded not conventional (→ ADR-0003)

Hard deny-list enforced in the harness: dpmac.3 (wired to a peer that must
never see traffic — no connect, no bind, no link-up, nothing that could
emit), dpmac.17/dpni.0 (management). Lifecycle/connect-edge churn uses the
unwired MACs (dpmac.4–6 at 25G, dpmac.8/10 at 10G — link-up can never be
asserted there). Link-up and traffic-bearing scenarios use dpmac.7/9 (cn10k
production peer) only, each explicitly flagged. Every validation scenario
declares its traffic class — object-lifecycle-only, link-signaling, or
traffic-bearing — and the harness refuses a trace whose class does not match
the ports it touches. All work runs in scratch child DPRCs, purely additive,
unconditional teardown. Decision point recorded in the roadmap: before the
first traffic-bearing phase, choose flagged use of dpmac.7/9 vs reverting
the device tree to land dpmac.3 on the on-board Mellanox (kills the
forbidden wire, gains a hammerable local peer).

### D6. Staged dual-backend southbound, MC v10 pinned (→ ADR-0004)

The restool shim remains the board-side executor (human-readable, auditable
under supervision) and the oracle. The Rust MC-portal (ioctl) transport
lands as its own mid-series change after Tier-A models stabilize; object
families then migrate behind the unchanged `McControl` trait with a
differential gate (same plan through both backends → identical observed
state). The portal targets the MC v10 command format only, single-version,
with a startup firmware assertion; multi-version marshalling is a recorded
non-goal (revisit trigger: a second board on different firmware). Unsafe
code is confined to the ioctl call-site module in `dpaa2-mc`; the workspace
`forbid` stands everywhere else; marshalling is safe explicit-LE
serialization.

### D7. Intent compiles to objects; network-construct vocabulary (→ ADR-0005)

Operators declare network constructs (consumer/runtime, port, link, crypto
engine) anchored in hardware; a pure derivation function in `dpaa2-api`
compiles intent to the full object plan (codifying the paid-for sizing
rules: DPIO ≥ 2·(1+workers), the two-DPBP rule, DPCON per queue, DPMCP
companions), with dry-run showing per-object rule provenance and a
per-object override escape hatch. Priority b > c > a: intent layer first,
named profiles as later sugar, raw object-level config only as the escape
hatch. YANG (or similar) is a recorded revisit trigger if intent expression
outgrows the tool. Derivation rules are themselves Quint invariants.

### D8. Runtime convergence is the spine; DPL tape-out is a late option (→ roadmap)

FPGA/ASIC framing: level-triggered runtime reconciliation stays primary
(topology re-derived from intent every boot); compiling intent to a DPL blob
("tape-out" via the build DTI) is a late, self-contained change whose spec
must solve the ownership inversion (DPL objects are foreign under today's
rules) and the return of persisted state. Nothing earlier depends on it.

### D9. Single initiating writer; kernel as reactive environment (→ ADR-0006)

During a convergence pass the tool is the only actor that originates MC-bus
mutations. The kernel is not a second actor: its reactions (dpaa2-eth pool
allocation on plug, udev rename) fold into the transition function of the
tool's own actions. Atomicity within a pass is an assumption; healing across
passes is guaranteed by level-triggered design. Violation mode ("someone
else mutated the bus mid-pass") is recorded, not modeled.

### D10. Versioned evidence and the loud-failure lens

Every baseline document carries a kernel-side section (6.6.52) beside the
MC/restool sections — the kernel is the second opaque state machine and both
C1/C2 lessons came from it. Every emitted board script asserts the reference
pair (MC 10.39.0 + Linux 6.6.52) before running; evidence is only valid
against its stamped pair, and the kernel stays pinned until the port
finishes. Every family document records known silent-failure modes; the
models that follow must state loudness invariants ("ill-formed plans fail
detectably at a named step") because every hard lesson in this project
failed quietly.

### D11. tasks.md is an entry point to beads

Granular tasks, dependencies, claims, and the DoD gates live as bd issues
(epic per change); `tasks.md` lists phases and their bead anchors. One bead
at a time through acceptance, per repo rules. The six-point DoD template
(baseline anchor, model gate, conformance gate, board milestone, docs,
quality floor) is instantiated per issue; for this docs-only change the
model and conformance gates are N/A and the board milestone is the capture
script.

## Risks / Trade-offs

- [Baseline drifts from reality as later changes discover divergences] →
  amend-in-place rule: the change that finds the divergence amends the
  baseline document in the same change; documents carry a verified-vs-read
  marker per claim class.
- [16 uniform documents invite copy-paste filler] → template demands an
  explicit "unknown/unverified" register per document; an empty section is a
  finding, not a failure.
- [Reference capture misses DPC/DPL provenance (blobs may not be readable
  from the running system)] → capture script reads what the firmware
  exposes (`dprc generate-dpl` snapshot as fallback); provenance gaps are
  recorded in reference-environment.md as open items.
- [Recovery guarantee ("reboot restores DPL baseline") stays assumed during
  this change] → deliberately verified early in `verify-foundation`, before
  any mutating board suite runs; documented as an explicit assumption until
  then.
- [Roadmap over-specifies a 15-change future] → the roadmap is a living
  document; order and cut-points amend as tiles are reached ("we will
  discover what we miss along the way").

## Open Questions

- Whether `dpdbg` is a real MC object family or a debug facade over others —
  affects whether it gets a typestate port or a diagnostics-only wrapper;
  resolved by its baseline document.
- Which Tier-C families are board-exercisable at all on this DPC (dpaiop
  needs AIOP firmware; dpdcei/dpdmai need DCE/DMA hardware paths) — each
  family document must answer "can a scratch-DPRC suite drive this?".
- Exact mechanism for DPC/DPL snapshot on this board (filesystem blob vs
  `dprc generate-dpl` reconstruction) — settled by the capture task.
