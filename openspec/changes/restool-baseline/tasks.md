# Tasks

Entry point only (design D11): granular tasks, dependencies, and acceptance
criteria live as bd issues under the `restool-baseline` epic. Checkboxes here
track phase completion; one bead at a time through acceptance.

Bead anchors: epic `dpaa2-controlplane-mru`; task N.M = child issue in tasks.md
order (0.1 → `.1` … 7.3 → `.25`). `bd list --parent dpaa2-controlplane-mru`.

## 0. Tracking scaffold

- [x] 0.1 Create the `restool-baseline` beads epic and one issue per task
      below, wiring dependencies (0 → 1 → {2,3} → 4 → 5 → 6 → 7) and
      instantiating the DoD template per issue (model/conformance gates N/A
      for this docs-only change; board milestone = task 2.2)
- [x] 0.2 Create `docs/baseline/` skeleton: 16 family documents from one
      shared template + `object-model.md`, `traffic-inventory.md`,
      `reference-environment.md` stubs

## 1. Session-decision capture

- [x] 1.1 Write ADR-0002 formal-methods process (Quint primary / TLA+
      escalation, dual-mode MBT, frozen traces, model-before-code) from
      design D2–D3
- [x] 1.2 Write ADR-0003 board interaction protocol and safety envelope
      (human-launched transparency, deny-list, port classes, traffic
      classification, per-step→per-block promotion, recovery-guarantee
      assumption) from design D4–D5
- [x] 1.3 Write ADR-0004 southbound evolution (staged dual-backend,
      differential gate, MC v10 pin, single unsafe ioctl module) from
      design D6
- [x] 1.4 Write ADR-0005 intent layer (network-construct vocabulary,
      intent-compiles-to-objects b>c>a, derivation invariants, YANG revisit
      trigger) from design D7
- [x] 1.5 Write ADR-0006 single-initiating-writer assumption with the
      kernel as reactive environment, from design D9
- [x] 1.6 Write `docs/ROADMAP.md`: the full change series (tiers A/B/C,
      dependencies, board-session sync points, Mellanox-revert and DPL
      tape-out decision points, FPGA/ASIC framing) from design D1, D8

## 2. Reference environment

- [x] 2.1 Write the read-only capture script (versions: MC firmware,
      kernel, restool; DPC/DPL snapshot via firmware-exposed blobs or
      `dprc generate-dpl` fallback; asserts nothing, mutates nothing)
- [x] 2.2 Operator runs the capture on the board; record results in
      `docs/baseline/reference-environment.md` (board milestone; no
      device-identifying information)

## 3. Tier A family documents

- [x] 3.1 `dprc.md` — containment, assign/plug, pools, generate-dpl noted
      as tape-out seed
- [x] 3.2 `dpni.md` — extend from ADR-0001/C1-C2 knowledge; full option
      inventory vs ls-addni
- [x] 3.3 `dpmac.md` — link types, MAC inheritance semantics (C2), counters
- [x] 3.4 `dpbp.md` + `dpio.md` + `dpcon.md` + `dpmcp.md` — pool objects,
      including the paid-for sizing rules (DPIO ≥ 2·(1+workers), two-DPBP,
      DPCON per queue, DPMCP companions) with their vpp-dpaa2-support
      evidence anchors
- [x] 3.5 `dpseci.md` — SEC queue pairs, priorities, congestion; anchor to
      the crypto ADRs in vpp-dpaa2-support

## 4. Tier B family documents

- [x] 4.1 `dpdmux.md` — demux topologies, kernel/VPP sharing relevance
- [x] 4.2 `dpsw.md` — switch object, port model, control-interface options

## 5. Tier C family documents

- [x] 5.1 `dpaiop.md`, `dpci.md`, `dpdcei.md`, `dpdmai.md`, `dprtc.md`,
      `dpdbg.md` — same template; each MUST answer board-exercisability on
      this DPC (design Open Questions) and resolve whether dpdbg is an
      object family or a debug facade

## 6. Cross-cutting documents

- [x] 6.1 `object-model.md` — relationship map: containment, connect edges,
      create-vs-allocate, allocation pools, lifecycle ordering
- [x] 6.2 `traffic-inventory.md` — scenario classification against the port
      safety matrix (spec: no scenario names dpmac.3)
- [x] 6.3 Analyze `ls-main`/`ls-debug`/`ls-append-dpl` end-to-end and place
      each script behavior in the family documents' used-by columns
- [ ] 6.4 Seed `docs/upstream/findings.md` with divergences found during
      3–6

## 7. Close-out

- [ ] 7.1 Verify every spec requirement scenario against the produced
      documents; fill unknown/unverified registers honestly
- [ ] 7.2 Cross-link: family docs ↔ object-model ↔ ROADMAP ↔ ADRs;
      CHANGELOG flows from conventional commits
- [ ] 7.3 Close beads, promote spec delta, ready the change for archive
