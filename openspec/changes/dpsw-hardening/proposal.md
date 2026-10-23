# Proposal

## Why

The dpsw-typestate epic review (synthesis banked in
`openspec/changes/dpsw-typestate/review/synthesis.md`, epic
dpaa2-controlplane-dbf) judged the core promise and ADR-0002/ADR-0022
compliance YES but left ranked findings the seal did not cover: the
task-3.2 typed switch plan is production-dead while the sealed 6.4 task
text claims it is dispatched (so the DPSW-I10 port-overflow refusal
guards no production path), one operator-reachable fabric shape escapes
compile refusal and dies at dispatch as an "internal error", one
ADR-0022 table row asserts the inverse of the shipped witness law, and
a sweep of staleness hits spans ADR-0013, COVERAGE, the baseline, two
dpsw-typestate spec deltas, and in-code doc comments. The dpsw-typestate
archive is gated on the pre-archive sweep (group 7): archiving first
would copy known-wrong requirement text into the main specs.

This change carries the review's seven bead-shaped follow-ups (synthesis
§4, beads A–G) plus the two rule outcomes of §5. Epic and child beads
are created at spec-init; each bead is self-contained (sites, decided
fold direction, acceptance, verification) from its tasks.md group.

## What Changes

- **The engine dispatches the typed switch plan (A / M1)**:
  `dispatch_switch_create` consumes `plan_switched_domain`'s
  `SwitchStep`s instead of re-stating create → connects → plug from
  `plan.edges`; the interposition dispatch matches every
  `InterposeStep` arm with the severed proof bound; the hardcoded
  `Class::Disruptive` folds onto `DriftRepair::class()`. DPSW-I10's
  `PortOverflow` typed refusal joins the live path.
- **Member-count bound as compile law (B / M2)**: a typed fabric
  refusal (the `MacTableOverflow` shape) bounds membership at the
  `max_fdbs` field envelope on both sides of the seam
  (`refuse.qnt` + `fabric.rs`; `dpswDerivationLawful` and
  `dpsw_attrs_well_formed` extended).
- **ADR-0022 row truth + edge-kind negative witnesses (C / M3+M15)**:
  the dpsw.N.M↔dpmac row attributes the interposition witness to the
  displaced dpni↔dpmac edge and states both dpsw kinds carry none; the
  `severed_witness_is_demanded_only_by_dpni_dpmac` test gains the two
  missing negative assertions.
- **DPSW-I1 evidence trail sync (D / M6, regen-bearing)**: the
  "read-not-verified" provenance anchor becomes the board-anchored
  wording (V-DPSW-1 positive, V-DPSW-4 flooding refusal, remaining
  refusal faces source-read), Rust+Quint twins in lockstep;
  `fabricAcceptedTrace` re-frozen, the dry-run snapshot re-accepted,
  the `docs/baseline/dpsw.md` I1/I2 cells flipped off bare "candidate".
- **Dry-run switch preview (E / M4)**: `converge_switches`'
  mutation-free verdict loop factors into a `plan_switches` /
  `render_switches` pair (the plan_links idiom) so a switched intent
  no longer previews as "hitless" while ensure does Disruptive work.
- **Capture-tap diagnostic attribution (F / M16)**: the preamble
  refusal names the real cause (restool spawn failure vs unreadable
  capture file vs genuinely dead tap); the 1s flush sleep gets its
  `ponytail:` marker.
- **Pre-archive doc/spec staleness sweep (G / M5, M7–M14)**: the nine
  wording-only hits across ADR-0013, COVERAGE, dpsw-typestate's
  design.md and two spec deltas, proposal.md, compiled.rs/render.rs
  comments, mc.rs doc, and the V-DPSW-5.sh header — one sweep, one
  commit under the dpsw-typestate trailer, before `/opsx:archive`.
- **Rule outcomes (synthesis §5)**: the board-suite protocol gains the
  named offline pre-check step with a kill-the-sitting rule on any
  typed refusal (R1, accepted reshaped); close-out doc-bank steps gain
  the annotate-at-bank sweep for sealed task text naming overturned
  design decisions (R2, annotate never rewrite).
- **Not in this change**: the `bd note` on dpaa2-controlplane-94k
  naming the banked corpora (M17) lands directly on that bead.

## Capabilities

### New Capabilities

None — this change hardens surfaces the dpsw-typestate change
introduced.

### Modified Capabilities

- `reconciler`: the ensure switch pass executes the api-stated
  `SwitchPlan` (ordering and the port-overflow refusal api-owned on the
  live path); disruption classing reads `DriftRepair::class()`.
- `intent-compiler`: fabric membership beyond the `max_fdbs` envelope
  refuses typed at compile; the DPSW-I1 provenance anchor cites the
  banked board evidence.
- `formal-models`: `dpswDerivationLawful` carries the membership bound;
  `fabricAcceptedTrace` re-frozen with the synced anchor.
- `provisioning-cli`: dry-run renders the switch pass preview with its
  Disruptive headline.
- `mbt-harness`: the capture preamble refusal attributes its cause.

## Impact

- Crates: `dpaa2-api` (plan/dpsw consumption seam, refuse/fabric bound,
  connect.rs test, compiled.rs/mc.rs doc wording), `dpaa2-tools`
  (engine dispatch fold, dry-run preview, render wording + snapshots),
  `dpaa2-verify` (capture.rs diagnostics).
- Models: `models/intent/{refuse,invariants,derive}.qnt` bound + anchor,
  `fabricAcceptedTrace.itf.json` re-freeze. The 8 dpsw family traces
  and the dpmac family traces stay byte-identical.
- Docs: ADR-0022 row, ADR-0013 consequences, COVERAGE DPSW-I6 cell,
  `docs/baseline/dpsw.md` I1/I2 cells, dpsw-typestate change docs
  (design.md annotation, proposal.md word fix, mc-backend and
  mbt-harness spec deltas), `models/board/V-DPSW-5/V-DPSW-5.sh` header,
  board-suite protocol record (R1).
- No new dependencies; no McControl trait change; no board sitting —
  everything verifies offline.
