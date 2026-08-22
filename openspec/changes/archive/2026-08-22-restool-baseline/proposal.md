## Why

The control plane today covers one slice of the MC surface (DPNI+DPMAC
provisioning with driver dependencies); the goal is to replace all of
`ls-main` and eventually `restool` itself with typestate-correct Rust across
every MC object family, verified with formal models validated against the
board. No such port can be planned, sequenced, or verified without an
authoritative baseline of what restool actually offers, how `ls-main` uses
it, how objects relate, and which behaviors live in the MC firmware or the
kernel driver rather than in restool's C — this change produces that
baseline, the master roadmap for the series, and the ADRs that codify the
process the series runs under.

## What Changes

- Add `docs/baseline/` with one document per restool object family (16
  total: dprc, dpni, dpmac, dpbp, dpio, dpcon, dpmcp, dpseci, dpdmux, dpsw,
  dpaiop, dpci, dpdcei, dpdmai, dprtc, dpdbg), each following a fixed
  template: command surface, options used by `ls-main` vs
  available-but-unused, immutable vs mutable attributes, MC 10.x API notes,
  kernel-side behavior (Linux 6.6.52), lifecycle ordering and dependencies,
  intent mapping, silent-failure notes.
- Add `docs/baseline/object-model.md` — the cross-object relationship map
  (containment, connect edges, create-vs-allocate semantics, allocation
  pools) that seeds the typestate design and the Quint models.
- Add `docs/baseline/traffic-inventory.md` — every planned validation
  scenario classified as object-lifecycle-only, link-signaling, or
  traffic-bearing against the board port safety matrix (dpmac.3 total-deny).
- Add `docs/baseline/reference-environment.md` — the pinned validation pair
  (MC 10.39.0 + Linux 6.6.52 + restool v2.4) plus DPC/DPL snapshot, captured
  via a user-run, read-only board script.
- Add `docs/ROADMAP.md` — the full change series with tiers, dependencies,
  decision points (Mellanox DT revert, DPL tape-out) and board-session sync
  points.
- Add ADRs 0002–0006: formal-methods process (Quint primary, TLA+
  escalation, dual-mode MBT), board interaction protocol and safety
  envelope, southbound evolution (staged dual-backend, MC v10 pin, single
  unsafe module), intent layer (network-construct vocabulary), and the
  single-initiating-writer assumption.
- Seed `docs/upstream/findings.md` for undocumented MC/restool semantics
  worth sharing upstream.
- Move granular task tracking to beads: `tasks.md` becomes the entry point;
  tasks, dependencies, and acceptance criteria live as bd issues.

## Capabilities

### New Capabilities

- `object-baseline`: the documented ground truth of the restool/MC object
  surface — per-family baseline documents, the cross-object relationship
  map, the traffic-pattern inventory, the pinned reference environment, and
  the roadmap/ADR process artifacts every later change in the series
  anchors to and amends in place.

### Modified Capabilities

- (none — documentation and decision records only; no requirement of an
  existing capability changes)

## Impact

- Documentation only: no crate code, no packaging, no CI changes.
- One user-run board script (reference-environment capture) that reads
  versions and snapshots DPC/DPL; it mutates nothing on the board.
- Source corpus consumed read-only from the workspace: `src/restool` (C
  source + `ls-main`/`ls-debug`/`ls-append-dpl`), `src/mc-utils/api`
  (per-release MC API deltas), Linux 6.6.52 dpaa2 drivers, the DPAA2
  reference manual (MCP index), and the validated learnings in
  `vpp-dpaa2-support` (scripts, ADRs, openspec archives).
- Every subsequent change in the series cites `docs/baseline/` sections as
  its anchor; divergences discovered on the board amend the baseline in the
  same change that finds them.
