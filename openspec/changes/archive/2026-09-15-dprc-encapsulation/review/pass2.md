# PASS 2 — Isomorphism and the Typestate Law: `dprc-encapsulation`

Agent: software-architect (Fable 5, pinned). Spend: ~128k tokens vs 150k
budget. Saved verbatim by the orchestrator.

## Findings

| ID | file:line | category | superseded-by | severity | disposition | verification |
|---|---|---|---|---|---|---|
| PASS2-F1 | crates/dpaa2-verify/tests/dprc_replay.rs:239-252 | dead | — | misleads-a-reader | amend | The `match attr { <every arm> => r.mc_status() }` binds ALL six `Attribution` arms to `r.mc_status()`, then asserts `attr_status == r.mc_status()` — a tautology. The header (line 20) advertises "the conformance is the refusal vocabulary, its MC status" but no status is derived from the attribution. Fix: map each `Attribution` arm to its *expected* status (`LockGate`/`PluggedMove`/`PermissionGap{TopologyChanges}` ⇒ 4, `PermissionGap{Spawn}` ⇒ 6, `PermissionGap{Alloc}`/`PoolExhaustion` ⇒ 8) and assert against `r.mc_status()`. Verify: mutate `attribute_mc` to return `PoolExhaustion` unconditionally — today `cargo test -p dpaa2-verify --test dprc_replay` stays green; after the fix it must fail |
| PASS2-F2 | crates/dpaa2-api/src/dprc_plan.rs:208-216 | guard-drift | — | misleads-a-reader | follow-up bead | `attribute_mc` discriminates 0x4 by mask alone: on the child DEFAULT mask (`topology_changes: false`, DPRC-I4) every 0x4 attributes `PermissionGap{TopologyChanges}` — including a lock-strip refusal of a *create/destroy/assign* verb, where the model says 0x4 can only be the lock (`dprc.qnt:326-340`: `createResidentAt` has no topology branch). The verb context the model's per-action guards carry is absent from the signature. Latent (no production caller yet — only tests/replay call it), but the operator-facing attribution will name the wrong cause the day the engine wires it. Fix: take the refused verb/step as a parameter. Verify: `attribute_mc(TopologyLockGate, Options::DEFAULT, Verb::CreateResident) == LockGate` unit test |
| PASS2-F3 | crates/dpaa2-api/src/dprc_plan.rs:436-449 | guard-drift | — | carries-cost | amend | `plan_teardown` never inspects `observed.state`: for a `ContainerState::Locked` orphan it emits `UnplugResident*`+`Destroy`, both of which the model refuses 0x4 (`dprc.qnt:486-495` destroy lock branch; `dprc.qnt:394-404` unplug lock branch). A locked foreign-labelled container classifies `PruneCandidateFull` (`classify_container` ignores state too, line 770-804), so `plan_prune` attaches a doomed plan — contradicting this module's own doctrine "the doomed step is never emitted" (line 279). Fix: `state == Locked` ⇒ record `Attribution::LockGate` gap, emit no steps. Verify: unit test `plan_teardown` on a Locked `ObservedContainer` yields empty steps + LockGate gap |
| PASS2-F4 | crates/dpaa2-api/src/dprc_plan.rs:324-332 | guard-drift | — | misleads-a-reader | fold (into F3 amend) | `plan_move_out` checks only `Locked` and `plugged`; the model's `moveResidentOutAt` guard also requires `isActive` (`dprc.qnt:362-366`). A `Declared`/`Emptied`/`Destroyed` state with a resident argument emits a `MoveResidentOut` step where `plan_create_resident`/`plan_assign_in` correctly return `FaceNotAssignable` for the same faces. Unreachable in practice (those faces observe no residents) but the three sibling planners disagree on the same predicate. Verify: unit test `plan_move_out(Destroyed, …) == Refused(FaceNotAssignable)` |
| PASS2-F5 | crates/dpaa2-api/src/dprc.rs:937-980 | iso-violation | — | misleads-a-reader | new ADR note | Guard-order inversion on the Locked face: the model conjoins the enabling precondition BEFORE the lock branch — `createResidentAt` under lock with a duplicate id is *disabled* (`dprc.qnt:326-328`), `plugResidentAt`/`unplugResidentAt`/`moveResidentOutAt` under lock require the resident to *exist* to be enabled at all (`dprc.qnt:381, 394, 363-366`) — while `Container<Locked>` refuses 0x4 unconditionally on any id, and the doc claims the opposite ordering ("the lock strip dominates, so no allocation or duplicate check is reached", dprc.rs:939). Either amend the model to lock-first (likely the board truth) or note the refinement; today the two sides answer the duplicate-under-lock question differently. Verify: quint directed run `createResidentAt(1)` twice under lock vs the Rust arm, or an ADR-0002 note recording the deliberate ordering |
| PASS2-F6 | crates/dpaa2-api/src/dprc.rs:910-923 | twin-drift | — | misleads-a-reader | amend | `bind_vfio`/`unbind_vfio` accept as a no-op when already in the target state ("a no-op if already bound. Always accepted"); the model's `bindVfio` is *disabled* unless `Plugged(Unbound)` (`dprc.qnt:426-431`) — an accepted transition exists in Rust that has no model counterpart. Harmless (idempotency-friendly) but it is a guard the ADR-0002 "same guard semantics" law does not cover; one doc sentence declaring it a deliberate idempotent refinement closes it. Verify: `grep -n "no-op if already" crates/dpaa2-api/src/dprc.rs` shows the amended rationale citing the model's disabled guard |
| PASS2-F7 | crates/dpaa2-verify/tests/dprc_replay.rs:510-522 | dead | — | carries-cost | follow-up bead | The `single_removed` (accepted move-out) inference branch is unexercised by all 14 traces — no frozen trace contains an accepted `moveResidentOutAt` (the only move-out trace, `pluggedMoveRefused0x4Test`, is the refusal). If the inference is wrong, nothing fails. Same for `single_plug_flip` with `now_plugged == false` (accepted `unplugResidentAt`, lines 523-545) and the `Unlocked::Empty` arm (line 454). Freeze one directed run covering accepted move-out + resident unplug. Verify: new trace listed in `TRACES`, `every_committed_trace_is_listed` and `dprc_traces_replay_green` pass |
| PASS2-F8 | crates/dpaa2-api/src/dprc.rs:984-990 | dead | — | carries-cost | amend | `Unlocked::Empty` (unlock of a residentless container → `Created`) has no unit test (`unlock_restores_test` covers only `Occupied`), no model directed run, and no trace — the branch is entirely unwitnessed on both sides of the twin. One-line unit test: `created().lock().unlock()` matches `Unlocked::Empty`. Verify: `cargo test -p dpaa2-api unlock` shows both arms asserted |
| PASS2-F9 | crates/dpaa2-verify/tests/dprc_replay.rs:40-91 | twin-drift | — | misleads-a-reader | follow-up bead | The freeze set stops at the task-1.2 faces: the three task-1.4 runs (`DPRC_I12Test`, `labelVoidEscapeTest`, `labelVoidUnderLockEscapeTest`, dprc.qnt:780-811) are not frozen, so DPRC-I12 bucket parity rides on unit tests only (`scenario_empty_label_is_report_only_the_label_void_escape`) and the replay never sees a voiding `setLabelAt("")`. The ghost fields are ignored by `world_view` (dprc_itf.rs:148-159), so freezing them is mechanical — replay the states, assert `classify_container` bucket per step. This proposes freezing *existing* runs only; it does not touch the recorded one-escape/no-fingerprint-widening decision. Verify: 17 traces on disk, `every_committed_trace_is_listed` green |

## (a) State-by-state isomorphism map

| Quint (dprc.qnt) | Rust (dprc.rs) | Match | Enforcement |
|---|---|---|---|
| `ContainerState` 7-case sum, `Plugged(VfioBind)` payload | `ContainerState` enum + 7 `Phase` markers, `Plugged { bind }` | yes (parity tests :1026, :1050) | both |
| `VfioBind` 2-case | `VfioBind` enum | yes | runtime enum, phase-carried at type level |
| `ResidentKind` / `Refusal` / `Outcome` / `Options` / `Identity` / `mcStatus` | same-name types; `mc_status`+`from_status` round-trip | yes | runtime (tested) |
| `Container` record, `Set[Resident]` w/ id | `Container<S>` private fields, `BTreeMap<ResidentId, Resident>` | yes (documented re-keying) | — |
| `createContainerWith`: `Declared→Created`, identity pooled once | `Container<Declared>::create` | yes | compile-time (only method on Declared; no identity/options setter — DPRC-I10 by construction) |
| `createManagedWith` (label + `createdByUs` ghost) | no Rust counterpart; ghosts declared model-only (dprc.qnt:222-227) | yes — deliberate | n/a |
| `createResidentAt` / `assignResidentInAt`: `unpluggedFace ∨ Locked` | `UnpluggedFace` bound + `Locked` refusing methods; absent on `Plugged` | yes | compile-time absence on Plugged (`compile_fail` witness dprc.rs:884-894); alloc/duplicate runtime; **guard order inverted under lock (F5)** |
| `spawnGrandchild` 0x6/0x4 | `spawn_grandchild` on UnpluggedFace + Locked | yes | runtime |
| `moveResidentOutAt`: `isActive`, lock∨plugged ⇒ 0x4 | `move_out` on `UnlockedHolder` + `Locked::move_out` | yes (Emptied/Created hold no residents ⇒ vacuous) | runtime refusal, membership-unchanged tested |
| `plugResidentAt`/`unplugResidentAt` | `plug_resident`/`unplug_resident` + Locked twins | yes (existence check inverted under lock — F5) | runtime |
| `connectEndpoints`: unplugged∨Plugged∨Locked | `connect_endpoints` on `LiveFace` + `Locked` | yes | runtime |
| `plugContainer`: `Populated→Plugged(Unbound)` | `Container<Populated>::plug` only | yes | compile-time (no plug on Created/others) |
| `bindVfio`/`unbindVfio` guards | `bind_vfio`/`unbind_vfio` accept-as-no-op | **no — F6** | runtime, Plugged-only at compile time |
| `lockHierarchy`: unpluggedFace→Locked | `lock()` on UnpluggedFace | yes | compile-time |
| `unlock` → Created/Populated by residents | `Unlocked` sum | yes (Empty arm unwitnessed — F8) | runtime-determined typestate |
| `setLabelAt`: `isActive` | `set_label` on `Active` | yes | compile-time face set |
| `emptyContainer`/`destroyContainer` + plugged-resident disabled guard | `empty`/`destroy` on LiveFace, `Teardown::ResidentPlugged`; `Emptied::destroy`; `Locked::destroy` ⇒ 0x4 | yes | runtime; Emptied unconditional at compile time |
| `busRemoveEvent`: isActive no-op | `bus_remove_event` on Active | yes | runtime no-op |
| `pruneBucket` 4 arms, arm order | `classify_container` same precedence; adds `root_ok` to the Full arm — the field the model documents as constant-true and deliberately says so (dprc.qnt:203-205) | yes | runtime, parity test `prune_buckets_match_the_enum_and_the_model` |
| `evictInto` | `evict_into` (typestate) + `predict_eviction` (plan), bound by `predict_eviction_matches_the_task_2_1_typestate` | yes | runtime |

Compile-time claims audit: the module doc (dprc.rs:13-43) honestly scopes
compile-time enforcement to the phase orderings (plug-then-assign,
plug-from-Populated-only, Declared/Emptied/Destroyed method absence) and
declares plugged-move/permission-matrix/EBUSY runtime. No runtime refusal
is advertised as a typestate. Clean on that spot-check.

## (c) Trace-coverage map (model arm ↔ replaying trace)

| Model transition / arm | Trace |
|---|---|
| createContainerWith accepted | all 14 |
| createManagedWith accepted | NONE (deliberate: ghost-bearing; unit-twinned in dprc_plan) |
| spawnGrandchild accepted | **NONE** |
| spawnGrandchild ⇒ 0x6 | spawnRefused0x6Test |
| spawnGrandchild under lock ⇒ 0x4 | **NONE** |
| createResidentAt accepted | lifecycleTest + 8 others |
| createResidentAt ⇒ 0x8 | allocRefused0x8Test |
| createResidentAt under lock ⇒ 0x4 | lockStripsCreateTest |
| assignResidentInAt accepted | evictionBothKindsTest, DPRC_I1Test, teardownReachableTest |
| assignResidentInAt under lock ⇒ 0x4 | **NONE** |
| moveResidentOutAt accepted | **NONE (F7)** |
| moveResidentOutAt plugged ⇒ 0x4 | pluggedMoveRefused0x4Test |
| moveResidentOutAt under lock ⇒ 0x4 | **NONE** |
| plugResidentAt accepted | pluggedMoveRefused0x4Test |
| plugResidentAt under lock ⇒ 0x4 | lockRefusesPlugTest |
| unplugResidentAt accepted | **NONE (F7)** |
| unplugResidentAt under lock ⇒ 0x4 | **NONE** |
| connectEndpoints accepted | connectWithTopologyTest |
| connectEndpoints no-topo ⇒ 0x4 | connectNoTopologyRefused0x4Test |
| connectEndpoints under lock ⇒ 0x4 | **NONE** |
| plugContainer / bindVfio / unbindVfio | lifecycleTest (+DPRC_I7Test for plug/bind) |
| lockHierarchy | labelUnderLockTest, lockStripsCreateTest, lockRefusesPlugTest, unlockRestoresTest |
| unlock → Populated | unlockRestoresTest |
| unlock → Created (empty) | **NONE (F8)** |
| setLabelAt accepted (non-void) | labelUnderLockTest (Locked only) |
| setLabelAt("") void escape | **NONE (F9)** |
| emptyContainer accepted | teardownReachableTest |
| emptyContainer plugged-resident guard | NONE — unrepresentable in a trace (disabled guard); unit-tested `destroy_with_plugged_resident_is_ebusy…` |
| destroyContainer non-empty accepted | evictionBothKindsTest, DPRC_I1Test |
| destroyContainer from Emptied | teardownReachableTest |
| destroyContainer under lock ⇒ 0x4 | **NONE** |
| destroyContainer plugged-resident guard | NONE — unrepresentable (disabled); unit-tested |
| busRemoveEvent | DPRC_I7Test |
| refusalStatusesDistinctTest | NONE — stateless value check, unit-twinned `refusal_statuses_distinct_test` |
| DPRC_I12Test / labelVoidEscapeTest / labelVoidUnderLockEscapeTest | **NONE (F9)** — unit-twinned in dprc_plan prune tests |

The untraced Locked-refusal arms (spawn/assign/unplug/move/destroy under
lock, connect under lock) share one mitigation today: `check_refusal`'s
Locked context asserts only `connect_endpoints` + `create_resident`
refusals (dprc_replay.rs:262-273); the other five `Container<Locked>`
refusing methods have neither trace nor unit test. Folding one lock-strip
sweep run into the F7 freeze bead covers them cheaply.

## (b) Containment-law verdicts (summary)

- **Permission matrix 0x6/0x8/0x4**: model ↔ typestate ↔ plan predicates
  match arm-for-arm; the only fidelity gaps are F2 (verb-blind 0x4
  attribution) and F1 (vacuous replay assertion).
- **Eviction law**: exact match, triple-bound (model `evictInto` ↔
  `evict_into` ↔ `predict_eviction`, parity test dprc_plan.rs:1087).
- **Visibility law**: honored by construction on both sides — no sync
  transition in the model, no sync-trust path in
  `verdict`/`plan_consumer_convergence`; `Missing` verdict from a `None`
  observation only.
- **Plugged-move**: same predicate model/typestate/plan (`plugged ⇒ 0x4,
  membership unchanged`); only the inactive-face edge diverges (F4).
- **Plan-vs-model residual**: F3 (`plan_teardown` blind to Locked) is the
  one place a plan emits a step the model refuses.

## (d) DPRC-I12 bucket parity

Parity holds. `pruneBucket` arm order (declared → empty-label fence → full
→ partial) is reproduced exactly in `classify_container` with the
precedence comment pinned to the model (dprc_plan.rs:794-804); the added
`root_ok` conjunct in the Full arm is the model's own documented
constant-true elision made real, and matches the model's enumeration note
(c) (placement move ⇒ partial, never report-only). The empty-label
stranding escape lands `ReportOnly` on both sides:
`labelVoidEscapeTest`/`labelVoidUnderLockEscapeTest` (model) ↔
`scenario_empty_label_is_report_only_the_label_void_escape` +
`scenario_empty_label_beats_a_matching_fingerprint` (Rust), with
`plan: None` enforcing never-touched. The `createdByUs`/`labelVoided`
ghosts stay model-side exactly as declared (dprc.qnt:222-227); no Rust
ghost was smuggled in. No finding; the only I12 gap is trace-freeze
coverage (F9).

---

**Read:** openspec/changes/dprc-encapsulation/review/pass1.md;
models/families/dprc.qnt (full); crates/dpaa2-api/src/dprc.rs (full);
crates/dpaa2-api/src/dprc_plan.rs (full);
crates/dpaa2-verify/src/dprc_itf.rs (full);
crates/dpaa2-verify/tests/dprc_replay.rs (full);
crates/dpaa2-api/src/port.rs (targeted grep of the dprc/container/VFIO
trait surface only); cross-crate grep for `attribute_mc` call-sites.

**Deliberately not read:** shim/tools crates and restool/kernel adapters
(Pass 3); specs/design/ADR prose beyond what the code cites (Pass 4);
models/board/; traces' JSON bodies (inventory taken from Pass 1's verified
no-orphan listing); no cargo/quint execution (offline).

**Open questions:** (1) F5 hinges on which side is board-truth for
duplicate-id-under-lock — does the real MC permission check precede the
duplicate check? A one-line V-DPRC verify item would settle whether the
model or the Rust ordering is amended. (2) `check_refusal`'s Locked arm
silently skips assertion when the `AnyContainer` variant mismatches the
phase (`if let` with no else, dprc_replay.rs:263) — unreachable given the
replay keeps them in lockstep, but a `finding(...)` else-arm would match
the file's own loud-failure doctrine; folded under F1's amend rather than
a separate row.
