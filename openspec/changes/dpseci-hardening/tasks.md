# dpseci-hardening — tasks

One bead per group, worked one at a time through acceptance (beads
dpaa2-controlplane-dsx, -ne4, -vje carry the full per-item detail from the
dpseci-typestate review synthesis; this list sequences them).

## 1. Comment and fixture sync (bead dsx — S4, S5, S12, S13, S16)

- [x] 1.1 Reword `plan/dpseci.rs:6-8,26-36` to the desired-vs-desired role,
  rename the `observed` parameter, cite D9; sync `contract/mc.rs:111-117` +
  `restool.rs:1054` container doc (keep the parameter, T3); hal
  `lib.rs:6`/`portal.rs:121-122` + ADR-0018:112-121 attribution per
  ADR-0021; `dpseci_detail.rs` fixture `dprc.5` → `dprc.1`. Zero behavior,
  trait signatures byte-for-byte; synthesis ledger-row greps return no
  hits; quality-floor green. CLAUDE.md:28 excluded (user-gated).

## 2. Attribution and the complete_kernel fold (bead ne4 — S1, S14)

- [ ] 2.1 Mint the raw-escape carrier through `decode_dpseci_options`
  (restool.rs:260-274) per design D1/D2: unnamed bit keeps its identity on
  the decoded mask, census projection unchanged; decode test attributes
  raw bits; `families/dpseci.rs:92-94` true as written.
- [ ] 2.2 Render the escaped bit by value in the detail row (render.rs:621
  face), closing the :621 vs :170 asymmetry; repeated status still plans
  zero actions.
- [ ] 2.3 Lift `complete_kernel` into the dpaa2-tools testkit — one
  definition site; `grep -rn 'port_names_kernel' crates/dpaa2-tools` →
  one definition; all four operand pins green; quality-floor green.

## 3. Model parity and trace coverage (bead vje — S7, S15)

- [ ] 3.1 State the census-poisoning law in `models/families/dpseci.qnt`
  with `censusUnobservableMemberJudgesNothingTest`, twinning
  `populate.rs:150-157`; one-line corpus check whether dpni/pool share the
  idiom gap (bead if yes, note if no); quint test green.
- [ ] 3.2 Extend `model:freeze-dpseci` with census/destroy runs plus a
  dpaa2-verify replay arm, or land the recorded sufficiency note in
  COVERAGE (design D5 either/or); trace count grows with replay green, or
  the note lands; synthesis B5/B7 greps pass.
