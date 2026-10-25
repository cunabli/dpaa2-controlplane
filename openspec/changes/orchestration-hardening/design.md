# Design

## Context

The resource-aware-orchestration review (its record archives with that
change under `review/`) ruled on every open question this change
executes. The design here records those rulings as decisions so the
implementing parcels inherit them without re-reading the review, and
each group's bead carries its full execution context (the review's
ledger rows are restated in the beads; nothing below requires the
archived reports).

## Decisions

- **D1 — Rust wins every twin divergence.** The budget law is the one
  the board proved: V-DPSW-2 fixed the FDB bracket as
  dpsw-object-granular (a port connect allocates no FDB), and dpio
  seats are spent headroom that never refuses (ADR-0003 §7). The model
  amends in its three coupled wire-FDB sites
  (`grossFdbDraw`/`wireEnabled`/`applyWire`) and exempts dpio from the
  refusing pool fold; the Rust admission does not move. The weaker
  alternative — documenting the model's pricing as "conservative
  over-approximation" — is rejected: it would permanently falsify the
  twin claim in `plan/admission.rs` and force the replay's verdict
  re-derivation to carry a model-only correction term.
- **D2 — the replay re-derives what it replays.** Structural row
  agreement is necessary, never sufficient: on every verdict
  transition the replay rebuilds the capacity face and gross draw from
  the prior state and asserts the Rust `admit` returns the frozen
  verdict. One frozen `envEatFdb → admit` trace is the standing trip
  wire — it fails while either budget-law divergence exists and stays
  green after, which is why the model amendment, the re-derivation,
  and the new trace land as one group.
- **D3 — reachability over vacuity.** `Status::Orphan` becomes
  reachable through an orphaning environment action (the D7 adversary
  shape: the environment withdraws an intent's claim on a converged
  construct), so `doPrune` fires, ORCH-I7's teardown half carries
  weight, and the prune lane of the ORCH-I6 diamond stops being
  vacuously confluent. The acceptance probe is executable: the
  Orphan-unreachability invariant run flips from `[ok]` to
  `[violation]`.
- **D4 — whole-intent refusal in the model.** Creates are guarded on
  no `Unimplemented` family pending, matching the engine
  (`reify_guard` refuses the whole intent before admission) and the
  model's own row documentation. The directed converge run drops the
  refused family from its intent; the dedicated refusal trace keeps
  that lane covered. Documenting the old construct-scoped semantics as
  a deliberate relaxation was rejected: the model would keep depicting
  a run the CLI can never exhibit.
- **D5 — the wide carrier, one fold.** `plan_admission` derives
  everything from one `read_inventory` snapshot and returns
  `(Admission, PoolDrift, Vec<SwitchWork>)`; the dry-run arm renders
  its pool and switch sections from that carrier instead of re-running
  the passes. The narrow alternative (only threading the inventory
  into `plan_pools`) fixes the double-read but leaves dry-run's
  admission section judging a different snapshot than the tables beside
  it. The wide carrier matches the established `plan_*` seam-carrier
  idiom, so the seam widening is idiomatic, not new structure.
  `converge_pools`' own re-read at dispatch time stays: that is D7
  re-observation, a different judgment.
- **D6 — judgment self-application is acceptance law.** Every judgment
  the engine renders ships with (a) a fixpoint test — the judgment
  applied to its own converged output yields the identity verdict —
  and (b) a twin-side verdict re-derivation in replay. The shipped
  converged-intent-re-admits tests are (a)'s template; this change's
  D2 assertion is (b)'s first instance. The rule exists because the
  same blind-spot class shipped sealed twice (the admission-basis
  defect; the replay's structural-only check) and both times only an
  out-of-band read caught it.
- **D7 — re-freeze discipline.** Group 1 re-freezes the full
  orchestration trace set against the amended law; group 2 adds its
  teardown trace and re-freezes only what its guards touch. COVERAGE
  rows update in the same commit as the behavior they describe; the
  confluence rung and retro-orch run green at every seal.

## Risks / Trade-offs

- [Re-frozen traces mask a semantics change] → the D2 re-derivation is
  the countermeasure: a re-freeze that smuggles a verdict change fails
  the Rust-side assertion.
- [The wide carrier drifts into a second decision layer] → the carrier
  returns what the judgment already computed; render consumes, never
  re-judges. Spec-align checks the seam at the group 3 seal.
- [Orphaning action distorts confluence cost] → the diamond's pair
  count is unchanged (the alphabet does not grow; one environment
  action joins the env set); COVERAGE records the new cost figure.

## Open Questions

None — every choice above was ruled by the review synthesis and acked;
deviations found during implementation stop the task and return to the
user (repo rule 1).
