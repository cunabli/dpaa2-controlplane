# PASS 1 — Residue, Deferrals, and Verify Re-run: `dprc-encapsulation`

Agent: Explore (Sonnet). Spend: ~92k tokens vs 60k budget (within the 1.5x
stop rule). Saved verbatim by the orchestrator.

## Findings

No stale, dead, duplicate, iso-violation, guard-drift, twin-drift, leak,
doc-drift, or simplify findings survive verification against the grounding
facts. Every candidate hit traced during this sweep resolved to one of:
(b) deliberate/protected, (c) historical/accurate prose, or out-of-scope
(file/line not touched by an in-scope epic commit). No PASS1-F rows are
reported.

Candidates investigated and dispositioned (not reported as findings, listed
for audit trail):

- `crates/dpaa2-api/src/dprc_plan.rs:643` "create-only" wording — checked
  against D8 prune addition (task 2.4). Confirmed
  `plan_consumer_container`/`plan_consumer_convergence` remains genuinely
  create-only by construction (declared-consumer path); prune lives in a
  clearly delimited separate section
  (`// ---- undeclared-consumer prune (reconciler spec) ----` at line 675)
  with its own `plan_prune`/`prune_containers` functions. Classification:
  (c) historical/accurate, not stale. No superseding event applies.
- `crates/dpaa2-tools/tests/dprc_convergence.rs:91` "create only its child
  DPRC" — describes a first-run declared-consumer test unrelated to the
  prune surface. (c) accurate.
- `crates/dpaa2-api/src/model.rs:472`, `crates/dpaa2-verify/src/fitcheck.rs:112`,
  `crates/dpaa2-api/tests/compile_props.rs:164`, `models/intent/observed.qnt:30`,
  `models/intent/invariants.qnt:104` ponytail markers — all blamed to
  commits (620feece, 93f6a6ae, ce38382c/549928ce) outside the reviewed span
  0aac741..1051e71 and in files/lines never touched by an in-scope commit.
  Out of scope, not findings.
- `crates/dpaa2-mc/src/restool.rs:451` ponytail marker — blamed to
  `0b07112` (task 5.3, in-scope). Matches the grounding fact's single
  expected marker exactly. (b) deliberate, expected.

## tasks.md verify obligations re-checked

| Task | Obligation | Result |
|---|---|---|
| 1.1 | typecheck + simulate green | UNCHECKABLE-OFFLINE (requires quint) |
| 1.2 | Apalache marks on I1/I5/I7/I9/I10/I11-remainder; `models/COVERAGE.md` dispositions updated; deferral rows for I8/I11-unlock/OBJ_CREATE→#10; label-under-lock open question resolved in model | PASS — marks present (`dprc.qnt:106` `stateInvariants`), COVERAGE.md rows updated (I8/I11/I4 all point to `#10`), design.md open question struck through and resolved to `setLabelAt`/`labelUnderLockTest` |
| 1.3 | Spec/design delta for undeclared-consumer prune (bead cd3.15) | PASS — `reconciler/spec.md` "Undeclared consumer containers are pruned under the double gate" requirement present with scenarios; `design.md` D8 section present |
| 1.4 | DPRC-I12 `createdByUs` ghost, bucket invariant, voiding-verb enumeration (bead cd3.16) | PASS — `dprc.qnt`: `createdByUs` ghost field, `DPRC_I12` invariant, `labelVoidEscapeTest`/`labelVoidUnderLockEscapeTest` all present |
| 2.1 | Parity test binding Rust to Quint sum (ADR-0002) | PASS — `dprc.rs:1026` `container_states_match_the_enum_and_the_model`, `dprc.rs:1050` `typestate_markers_match_the_container_states` |
| 2.2 | Plan semantics (containment guards, eviction teardown, re-observation verdicts, typed refusal discrimination) | PASS (by inspection) — `plan_teardown`, `ContainerVerdict`, `attribute_mc` all present in `dprc_plan.rs` |
| 2.3 | ITF conformance: frozen traces replay green in `cargo test` | PASS structurally / UNCHECKABLE-OFFLINE for the actual run — `dprc_traces_replay_green` + self-checking `every_committed_trace_is_listed` test exist; 14 `TRACES` entries match exactly 14 files on disk under `models/traces/families/`, no orphans, no missing |
| 2.4 | Prune buckets/planning, tests pinned to 1.4 enumeration (bead cd3.17) | PASS — `dprc_plan.rs` `scenario_empty_label_is_report_only_the_label_void_escape` (labelVoidEscapeTest twin), `scenario_relabel_remedy_re_enters_the_candidate_buckets`, `prune_buckets_match_the_enum_and_the_model` all present |
| 3.1 | Restool shim verbs; unit tests against recorded restool transcripts | PASS — `restool.rs` `mod tests` with 19 `#[test]` fns; `crates/dpaa2-mc/tests/fixtures/dprc_show.txt` referenced via `include_str!` in `shim.rs:13` |
| 3.2 | KernelControl VFIO face; sysfs plumbing testable via Runner seam | PASS (by inspection) — `kernel.rs`/`sysfs.rs` have `mod tests` with multiple `#[test]` fns |
| 4.1 | Consumer→container derivation; container-only (no residents) asserted by test | PASS — `crates/dpaa2-api/tests/consumer_containers.rs:153` `the_derivation_is_container_only_dropping_companions_and_dpnis` |
| 4.2 | E2E convergence; dry-run per-object provenance | PASS (by inspection) — `engine::plan_containers`/`converge_containers` wired in `dprc_convergence.rs` tests |
| 4.3 | `ensure --prune` dispatch; FakeBackend + snapshot tests (bead cd3.18) | PASS — exactly 3 `dprc_convergence__*.snap` snapshots present, `FakeBackend` used throughout `dprc_convergence.rs` |
| 5.1–5.4 | Board milestone suites | OUT OF SCOPE (models/board/ operator-sealed, per mandate) |
| 6.1 | Deferral rows verified present, pointing at #10; ADR for solidified/died decisions; roadmap row #4; CHANGELOG via cliff; quality floor green | Deferral rows: PASS (see below). ADRs: PASS — ADR-0007 has an in-scope "Amendment 2026-09-14" section, ADR-0011 has in-scope board-outcome additions, ADR-0017 is new, all landed by commit `1051e71`. Roadmap row #4: PASS, `docs/ROADMAP.md:24` shows `delivered 2026-09-14 (ADR-0017; ADR-0007/0011 amendments)`. CHANGELOG: UNCHECKABLE-OFFLINE — `CHANGELOG.md` has zero epic-attributable entries and zero commits touching it in the reviewed span, but this matches the repo's existing convention (the file is a bare `## [Unreleased]` stub with no entries for any prior delivered epic either, e.g. no `intent-layer` entries), so `git cliff` generation appears release-time, not per-epic; cannot be confirmed without running `git cliff`. Quality floor (`cargo build\|fmt\|clippy\|clippy --tests\|doc\|test`): UNCHECKABLE-OFFLINE (no cargo) |

## Deferral-row check

PASS. All three promised deferral rows exist in both files and point at
tile `#10` (`mc-portal-backend`):
- `models/COVERAGE.md:79` (DPRC-I8) → `#10`
- `models/COVERAGE.md:82` / `docs/baseline/dprc.md:347` (DPRC-I11 unlock face) → `#10`
- `models/COVERAGE.md:75` / `docs/baseline/dprc.md:344,385` (OBJ_CREATE_ALLOWED gate) → `#10`

`docs/baseline/dprc.md:344` and `:347` independently carry the same two
deferral rows with matching `#10` pointers; `models/COVERAGE.md:68` tally
(`55 modeled, 47 deferred, 7 board-settled, 0 board-pending — 109
candidates`) matches the grounding facts exactly.

## Ponytail sweep

Exactly one `ponytail:` marker traces to an in-scope commit:
`crates/dpaa2-mc/src/restool.rs:451-452` (commit `0b07112`, task 5.3).
Matches the grounding fact precisely — no additional or missing markers
found in epic-touched code.

## Footer

**Read:** `openspec/changes/dprc-encapsulation/tasks.md`, `design.md`,
`specs/reconciler/spec.md`; `models/COVERAGE.md`,
`models/families/dprc.qnt`, `models/traces/families/` (directory listing);
`docs/baseline/dprc.md`, `docs/ROADMAP.md`, `docs/adr/0001*`,
`docs/adr/0007*`, `docs/adr/0011*`, `docs/adr/0017*`; `CHANGELOG.md`,
`cliff.toml` (existence only); `crates/dpaa2-api/src/dprc.rs`,
`dprc_plan.rs`; `crates/dpaa2-api/tests/consumer_containers.rs`,
`compile_props.rs`, `plan_by_hand.rs` (grep/partial reads);
`crates/dpaa2-mc/src/restool.rs`, `kernel.rs`;
`crates/dpaa2-hal/src/sysfs.rs`; `crates/dpaa2-tools/src/engine.rs`,
`tests/dprc_convergence.rs`; `crates/dpaa2-verify/tests/dprc_replay.rs`;
fixture/snapshot directory listings; git log/blame across the full commit
range for scope attribution.

**Deliberately not read:** `models/board/` and any board-evidence
artifacts; ADR-0016 and the process files (`scripts/checks/`, `.githooks/`,
`CONTRIBUTING.md`, `CLAUDE.md`); portal/ioctl code; any code untouched by
this epic; full body text of `crates/dpaa2-mc/tests/shim.rs` and other
non-listed test files beyond confirming references; full `cargo`/`quint`
execution (no tool access).

**Open questions:**
1. Grounding facts state "ADR-0001 §4 + ADR-0007 §3 amended," but `git log`
   shows zero in-scope commits touching `docs/adr/0001-*.md` — ADR-0001 §4
   is *cited* (as prior-decision grounding for the report-only fence) but
   not edited by this epic. The file actually amended alongside ADR-0007 is
   ADR-0011 (board-outcome additions at task 5.1/5.3-5.4), which matches
   `docs/ROADMAP.md:24`'s own text ("ADR-0007/0011 amendments"). This looks
   like a possible transposition (0001↔0011) in the review brief rather
   than a repo defect. [Orchestrator note: confirmed, brief corrected
   before Pass 4 dispatch.]
2. `git cliff`/CHANGELOG generation could not be verified to actually run
   cleanly against the epic's commit messages (no tool access); flagged
   UNCHECKABLE-OFFLINE rather than PASS or FAIL.
