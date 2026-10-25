# Tasks

Groups are sittings: each group is one bead, one parcel lane, sealed
close-then-commit before the next dependent group starts. Group 2
depends on group 1 (it re-freezes on the amended law). Group 3 is
independent of 1–2. Group 4 closes the change.

## 1. The model's budget law aligns to the board (bead: group-1)

- [ ] 1.1 Amend the model's budget arithmetic to the board-proven law:
      remove the wire-FDB pricing from `grossFdbDraw`, `wireEnabled`,
      and `applyWire` (the WANT_PORT constant may stay for intent
      shape); exempt dpio from the refusing pool fold (spent headroom,
      never refuses); fold `retro_oracle.qnt`'s hand-copied wire effect
      onto `applyWire` so no effect copy remains. Verify: model
      typecheck green; retro-orch 13/13; `grep WANT_PORT` shows no
      draw-pricing use; `grep "fdbFree' = fdbFree - 1"` empty in the
      retro oracle.
- [ ] 1.2 Add the replay verdict re-derivation and the FDB trip trace:
      `check_pair` rebuilds the capacity face and gross draw from the
      prior state's pending/poolFree/fdbFree and asserts the Rust
      `admit` equals the frozen verdict on every verdict transition;
      freeze `envEatFdb → admit` (the FDB-side refusal lane); re-freeze
      the full orchestration trace set against the amended law; update
      the COVERAGE orchestration rows (unexercised-action gaps close or
      carry recorded cost). Verify: `orchestration_replay` green with
      the new assertion and the trace count grown; `model:confluence`
      [ok]; a deliberate budget-law mutation on either side fails the
      replay (demonstrated once, then reverted).

## 2. The model reaches what the engine does (bead: group-2, after 1)

- [ ] 2.1 Make `Status::Orphan` reachable: a D7-shaped orphaning
      environment action enables `doPrune`; freeze one teardown trace
      through the prune lane; ORCH-I7's teardown half and the PPrune
      confluence pairs stop being vacuous. Verify: the
      Orphan-unreachability probe (quint run with the forall-no-Orphan
      invariant) flips [ok] → [violation]; the teardown trace replays;
      confluence [ok] with the cost figure updated in COVERAGE.
- [ ] 2.2 Make the Unimplemented refusal whole-intent: creates guarded
      on no unimplemented family pending; the directed converge run
      drops dpseci from its intent (the dedicated refusal trace keeps
      the lane); "whole-intent" reads true throughout the model.
      Verify: replay + retro-orch green after re-freeze of touched
      traces; `grep "whole-intent"` consistent with the semantics.

## 3. One snapshot per admission judgment (bead: group-3, independent)

- [ ] 3.1 Fold `plan_admission` onto one snapshot: one
      `read_inventory` feeds face, pending, and switch work; the
      function returns the wide carrier (`Admission`, pool drift,
      switch work) and the dry-run arm renders its pool and switch
      sections from it; the engine doc's "one read" claim becomes
      true. Verify: exactly one `read_inventory()` reachable from
      `plan_admission`; dry-run arm calls `plan_pools`/`plan_switches`
      at most once each; `cargo test -p dpaa2-tools` green; quality
      floor green.
- [ ] 3.2 Pin the dry-run table to the board suite's contract:
      `vadmit1_intents.rs` asserts `render_admission(&admission)`
      carries the `[refused]:` header and the `dpcon draw=… [refused]`
      row — the exact strings the V-ADMIT-1 step-1 greps demand.
      Verify: `cargo test -p dpaa2-tools --test vadmit1_intents`
      green; the asserted strings match the suite script's greps.

## 4. Close-out (bead: group-4, after 1–3)

- [ ] 4.1 Adopt the self-application rule as process: a process commit
      records "every rendered judgment ships with a fixpoint test and
      a replay verdict re-derivation" (design D6) where the repo keeps
      its process rules, citing both shipped instances; COVERAGE and
      ADR-0023 cross-references updated if the wording moved; ROADMAP
      follow-on row updated. Verify: quality floor; openspec validate;
      the rule text names its two instances.
