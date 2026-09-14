## 1. Archive gate (docs on the reviewed change)

- [ ] 1.1 Amend dprc-encapsulation in place before its archive: reconciler delta gains a `## MODIFIED Requirements` block restating the ownership fence with the container carve-out (M3/PASS4-F1); formal-models delta states the Apalache-vs-simulate split (M4/PASS4-F3, tasks.md 1.2 wording included); all DPRC-I8 pointers repointed to `pool-objects` (#6) per design D2 (M5/PASS4-F4: proposal, design, tasks, specs/mbt-harness, specs/mc-backend); V-DPDBG-2 task row added to §6 per design D3 (PASS4-F6); reviewed-change proposal Impact corrected `dpaa2-config` → `dpaa2-api`, tools surface honest (PASS4-F7). Then `/opsx:archive dprc-encapsulation` runs clean, review directory included (bead dpaa2-controlplane-am0.1)

## 2. Board sitting (operator-launched, blocks nothing)

- [ ] 2.1 OI-1 + OI-3 probe pair on a scratch child, serial, self-cleaning: duplicate-id create under `set-locked 1` (0x4 vs config error settles the lock-strip ordering; loser side — model or `dprc.rs:937-980` doc — amended as an ADR-0002 note, PASS2-F5); dpmcp census across restool spawns (budget draw or not; COVERAGE/baseline row recorded, PASS3-F13-OQ). Outcomes land in this task, not task 6 (bead dpaa2-controlplane-am0.2)

## 3. Parcel A — prune soundness (dpaa2-api, dpaa2-tools)

- [ ] 3.1 The fixture that fails first (TDD, M1): `fake.rs` `dprc_destroy` returns `McStatus{0x10}` while any resident is plugged; a plugged-resident orphan fixture and a Locked-orphan fixture exist; `cargo test -p dpaa2-tools prune` fails on pre-3.3 code (PASS3-F15) (bead dpaa2-controlplane-am0.3)
- [ ] 3.2 Attribution carries the verb (M6/M7): `attribute_mc` takes the refused verb — `attribute_mc(TopologyLockGate, DEFAULT, CreateResident) == LockGate` (PASS2-F2); `attribute_refusal` relocates to `dpaa2_api::dprc_plan` (PASS3-F6) (bead dpaa2-controlplane-am0.4)
- [ ] 3.3 Plan and engine honor the states prune will meet (M1): `dprc_plan::ObservedContainer.residents` keyed by `ObjectRef` — `dprc::Container` keeps `ResidentId`, ADR-0014 twin untouched (PASS3-F14); `plan_teardown` on Locked yields empty steps + `Attribution::LockGate` gap (PASS2-F3); `plan_move_out(Destroyed,…) == Refused(FaceNotAssignable)` (PASS2-F4); `PruneOutcome::Refused{id, attribution}`, MC 0x10 → `Teardown::ResidentPlugged`, a refusing candidate never aborts the pass, re-observation always runs naming survivors (PASS3-F4/F5); 3.1's fixtures go green (bead dpaa2-controlplane-am0.5)

## 4. Parcel B — southbound shim (dpaa2-mc)

- [ ] 4.1 The shim stops judging (M2): observation producer emits `ObjectRef`-keyed residents (a child listing `dpbp.0 plugged` + `dpmcp.0 unplugged` yields two residents and an `UnplugResident` step, PASS3-F14); state classification moves to core `ContainerState::classify` beside `VfioBind::classify` (PASS3-F1); resident origin reported `Option<ResidentKind>` with conservative core prediction + ADR note (PASS3-F2); the stale "read-back is deferred" comment in `engine.rs` replaced by the real ceiling reference (PASS3-F3) (bead dpaa2-controlplane-am0.6)
- [ ] 4.2 Exit hygiene and folds (M10/M11): every `McControl` verb exits through the classifying path, `Runner::run` stays raw transport (PASS3-F7); `code: None` ⇒ `Error::Backend` (PASS3-F8); option-bit table single-sourced (PASS3-F9); `CannedRunner` folded into `ScriptedRunner::canned` (PASS3-F10); `dprc_info` helper single-sourced (PASS3-F11); test inventory folded to `testkit::ref_inventory(16)` only if both suites stay green under the `Observed{18}`/labels caveat (PASS3-F12) (bead dpaa2-controlplane-am0.7)

## 5. Parcel C — model witnesses (models, dpaa2-verify)

- [ ] 5.1 Freeze the missing witnesses (M9) and fix the tautology (M6): one directed run frozen for accepted move-out + resident unplug (PASS2-F7), one lock-strip sweep covering the five untested `Container<Locked>` refusals, the three task-1.4 runs frozen states-only (PASS2-F9, fingerprint untouched); `created().lock().unlock()` matches `Unlocked::Empty` (PASS2-F8); `dprc_replay.rs` maps each `Attribution` arm to its expected MC status — a mutated `attribute_mc` fails the suite — and the silent `if let` gets a loud else (PASS2-F1); `every_committed_trace_is_listed` green at 17+ traces (bead dpaa2-controlplane-am0.8)

## 6. Doc polish (desk-only, independent)

- [ ] 6.1 M8 + M13: `--prune` help names container teardown behind `--allow disruptive` with a D8 pointer (PASS4-F5); ADR-0001 §4 amendment section — ownership explicit and label-anchored for containers, fence survives for empty-label/zero-overlap (PASS4-F2); ADR-0017 Decision-3 consequence corrected + one crate citation anchor at the propagation observable (PASS4-F8); baseline/COVERAGE "rev 2" antecedent named (PASS4-F9); bind/unbind idempotent-refinement sentence citing the disabled model guard (PASS2-F6); the nominated rule amendment recorded with the process checks — a delta overriding a base-capability SHALL must carry a MODIFIED block (bead dpaa2-controlplane-am0.9)
