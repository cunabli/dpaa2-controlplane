# PASS 3 — Architecture and code quality: `dprc-encapsulation`

Agent: software-architect (Opus override). Spend: ~179k tokens vs 130k
budget (1.37x — under the 1.5x stop rule). Saved verbatim by the
orchestrator.

| ID | file:line | category | superseded-by | severity | disposition | verification |
|---|---|---|---|---|---|---|
| PASS3-F1 | `crates/dpaa2-mc/src/restool.rs:462-468` | leak | — | misleads-a-reader | fold: move the rule to a core `ContainerState::classify(&residents)` in `dprc.rs` beside `VfioBind::classify`; shim reports residents only | `rg 'ContainerState::' crates/dpaa2-mc/src` returns nothing but the import |
| PASS3-F2 | `crates/dpaa2-mc/src/restool.rs:449-459` | leak | — | breaks-a-claim | new ADR note + amend: report origin as unobservable (`Option<ResidentKind>`) and let the core predict conservatively, or mark the render "origin unobservable" | seed a real board child holding an assigned-in resident; `render_prune` must not print `parent gains 0 resident(s)` |
| PASS3-F3 | `crates/dpaa2-tools/src/engine.rs:305-306, 321-324` | stale | task 3.1 — `RestoolMc::observe_containers` populates `residents` with `plugged` at `restool.rs:443-460`; read-back is no longer deferred | misleads-a-reader | amend: name the real ceiling (family-less `ResidentId`, no `ObjectRef` for `--object=`) and cross-link `restool.rs:451` | `rg 'resident read-back is deferred' crates/` returns nothing |
| PASS3-F4 | `crates/dpaa2-tools/src/engine.rs:283-289` | carries-cost | — | carries-cost | follow-up bead: collect per-candidate errors, always run the `:292` re-observation, report per candidate | a two-candidate fixture where candidate 1 destroys and candidate 2 refuses must still re-observe and name which id survived |
| PASS3-F5 | `crates/dpaa2-tools/src/engine.rs:286` (vs the converge path at `:176-188`) | guard-drift | — | breaks-a-claim | follow-up bead: add `PruneOutcome::Refused{id, attribution}`, call `attribute_refusal`, and map MC `0x10` to the already-typed `Teardown::ResidentPlugged` (`dprc.rs:593`) | fake refuses `dprc_destroy` with `McStatus{0x10}`; outcome must be a discriminated `Refused`, not a fatal `Err` |
| PASS3-F6 | `crates/dpaa2-tools/src/engine.rs:364-374` | leak | — | carries-cost | fold: move `attribute_refusal` into `dpaa2_api::dprc_plan` beside `attribute_mc` (D4 puts interpretation in `dpaa2-api`); both input and output types already live there | `rg 'fn attribute_refusal' crates/dpaa2-tools` returns nothing |
| PASS3-F7 | `crates/dpaa2-mc/src/restool.rs:357-376` vs `crates/dpaa2-mc/src/runner.rs:83-94` | duplicate | — | misleads-a-reader | fold: make the classifying call the single exit of every `McControl` verb; leave `Runner::run` as raw transport. Two error paths in one shim → one. Readability cost: negative (deletes the `run` vs `run_verb` choice a new verb author must make) | `rg 'self\.runner\.run\(' crates/dpaa2-mc/src/restool.rs` returns nothing outside `run_verb` |
| PASS3-F8 | `crates/dpaa2-mc/src/restool.rs:358-367` | guard-drift | — | carries-cost | amend: `code: None` (signal death, `runner.rs:24`) is `Error::Backend`, not a refusal — today it becomes `RestoolGuard{detail:""}` → `Attribution::RestoolClientGuard` | scripted `RunOutcome{code: None}` must yield `Error::Backend` |
| PASS3-F9 | `crates/dpaa2-mc/src/restool.rs:79-97` **and** `crates/dpaa2-mc/src/parse.rs:157-165` | duplicate | — | carries-cost | fold: one `&[(&str, OptionBit)]` table in `parse.rs` driving both render and decode. Readability cost: near zero — one 4-row table replaces eight literal spellings; a fifth bit at tiles #5–8 currently needs two unlinked edits | `rg -c 'DPRC_CFG_OPT_' crates/dpaa2-mc/src/*.rs` shows one non-test site |
| PASS3-F10 | `crates/dpaa2-mc/src/restool.rs:1135-1155` **and** `:768-822` | duplicate | — | carries-cost | fold: delete `CannedRunner`; add `ScriptedRunner::canned(&[(&str,&str)])` wrapping each in `ok()`. `ScriptedRunner` strictly subsumes it, and in-scope task-3.1 tests use both with no stated principle. Readability cost: zero | `rg 'impl Runner for' crates/dpaa2-mc/src` shows one test double |
| PASS3-F11 | `crates/dpaa2-mc/src/restool.rs:1184-1193` **and** `crates/dpaa2-mc/src/parse.rs:507-516` | duplicate | — | carries-cost | fold: one `#[cfg(test)] pub(crate) fn dprc_info` in `parse.rs` (outside `mod tests`). Byte-identical body and comment. Readability cost: zero | `rg -c 'fn dprc_info' crates/dpaa2-mc/src` == 1 |
| PASS3-F12 | `crates/dpaa2-tools/tests/dprc_convergence.rs:25-52` **and** `crates/dpaa2-tools/tests/vdprc9_intents.rs:27-54` | duplicate | — | carries-cost | fold: call `dpaa2_api::testkit::ref_inventory(16)` (`lib.rs:66-114`) — already the single-sourced `REF_INVENTORY` twin, already enabled for this crate (`crates/dpaa2-tools/Cargo.toml:27`), already used by the same epic's `tests/consumer_containers.rs:16`. `vdprc9_intents.rs:23-24` admits the copy. Readability cost: −55 lines. Caveat: `ref_inventory` uses `Ceiling::Observed{18}` and seeds `labels{(Dpni,0):""}` | `rg 'fn inventory\(\)' crates/dpaa2-tools/tests` leaves only the pre-existing `render.rs` |
| PASS3-F13 | `crates/dpaa2-api/src/port.rs:104-111` | leak | — | carries-cost | follow-up bead: add `observe_container(&self, id: DprcId) -> Result<Option<ObservedContainer>, Error>` beside the enumerate verb. The doc promises "re-querying the affected container" but the verb names none and rescans all (`restool.rs:427-481` = 1+2N process spawns, called 4× per `ensure`); tiles #5–8 (pools/dpnis inside children) rewrite this signature | `rg 'observe_containers' crates/dpaa2-tools/src/engine.rs` shows only the enumerate use sites |
| PASS3-F14 | `crates/dpaa2-mc/src/restool.rs:451-452` (ponytail disposition) | leak | — | breaks-a-claim | follow-up bead + amend marker. **Not safe — the prune census makes it reachable today**: `plan_teardown` (`dprc_plan.rs:436-449`) and `predict_eviction` already consume the map, and `render_prune` prints its post-state. `dpbp.0` vs `dpmcp.0` collide, `BTreeMap::insert` keeps the last row, so a plugged resident can record unplugged → no `UnplugResident` → MC `-EBUSY`, un-attributed (F5). Fix must **not** re-type `dprc::ResidentId` (ADR-0014 twin of `dprc.qnt`); key the observation type `dprc_plan::ObservedContainer.residents` by `ObjectRef` (`model.rs:249`, already the operand of `dprc_assign`/`dprc_unassign`) and leave `dprc::Container` on `ResidentId` | child `dprc show` listing `dpbp.0 plugged` + `dpmcp.0 unplugged` must yield two residents and an `UnplugResident` step |
| PASS3-F15 | `crates/dpaa2-api/src/fake.rs:352-355` (+`:357-386`) | carries-cost | — | carries-cost | follow-up bead: make `dprc_destroy` return `McStatus{0x10}` while any resident is plugged. Today it removes unconditionally and every prune fixture seeds `plugged:false` (`dprc_convergence.rs:188-215`), so `engine.rs:321`'s `UnplugResident` arm and the `-EBUSY` precondition have zero coverage — one fixture covers F3/F4/F5/F14 | add a plugged-resident orphan; `cargo test -p dpaa2-tools prune` must fail before the F5 fix |

## Mandate verdicts

- **(a) Sans-io discipline (D2): HOLDS-WITH-FINDINGS** — `PASS3-F1`,
  `PASS3-F2` (two lifecycle judgments made in the shim, where the sibling
  VFIO face deliberately reports raw and lets `VfioBind::classify` judge),
  `PASS3-F6` (a pure `Error`→`Attribution` map over two `dpaa2-api` types
  stranded in the imperative shell). No *planning* logic leaked into
  `engine.rs`: gate → dispatch → re-observe → judge is the documented
  boundary and `dprc_plan.rs:822-823, 834-837` explicitly assigns the
  double gate to the engine.
- **(b) Refusal typing end-to-end (D4): HOLDS-WITH-FINDINGS** — no
  string-matching anywhere; `parse_mc_status` extracts the raw byte,
  `Error::McStatus`/`RestoolGuard` stay distinct, and the converge path
  reaches a discriminated `Attribution::PermissionGap` (proved by
  `dprc_convergence.rs:149-169`). The collapse is by omission, not
  scraping: `PASS3-F5` (the prune path never attributes, and MC `0x10` has
  no wire to the already-typed `Teardown::ResidentPlugged`), `PASS3-F7`
  (every non-`dprc` verb in the same shim still string-collapses through
  `Runner::run` — the default a tile #5–8 verb author will inherit),
  `PASS3-F8`.
- **(d) Seam quality for #5–8: HOLDS-WITH-FINDINGS** —
  `ObjectRef`/`DprcId`/`dprc::Options` generalize cleanly, and
  create-into-child is already reachable via
  `dprc_assign(--child, --plugged)` without a new verb. Two dprc-only
  assumptions the next family must rewrite: `PASS3-F13` (root-only,
  all-at-once, argument-less observation) and `PASS3-F14` (family-less
  resident key, already reachable). One ordering note, not filed as a row:
  `main.rs:214-248` converges ports *before* containers; at tile #5
  (dpnis into a tenant's child) that order inverts.

## Footer

**Read:** `crates/dpaa2-mc/src/{restool,kernel,parse,runner}.rs` (full,
incl. test modules); `crates/dpaa2-hal/src/sysfs.rs` (full);
`crates/dpaa2-api/src/{port,fake,error}.rs` (full), `model.rs:235-295`,
`dprc.rs:20-80, 180-400, 580-620, 770-820, 1110-1140`,
`dprc_plan.rs:215-330, 380-500, 538-862`, `matcher.rs:1-130`,
`lib.rs:25-149`; `crates/dpaa2-tools/src/{engine,render,main}.rs` (full);
`crates/dpaa2-tools/tests/{dprc_convergence,vdprc9_intents}.rs` (full);
`openspec/changes/dprc-encapsulation/{review/pass1.md,
specs/reconciler/spec.md}`, `design.md` (grepped D4/D5/D8);
`crates/*/Cargo.toml` feature blocks.

**Deliberately not read:** `models/` and all Quint/board artifacts (Pass
2/out of scope); `docs/adr/`, `docs/ROADMAP.md`, `CHANGELOG.md` (Pass 4);
ADR-0016 process files; `crates/dpaa2-api/src/{compile,refuse,reconcile,
intent}.rs` beyond the call sites above; `crates/dpaa2-mc/tests/shim.rs`
(grepped for `Runner` doubles only); `crates/dpaa2-api/tests/
consumer_containers.rs` (grepped for the fixture import only); the three
`.snap` files; `crates/dpaa2-tools/src/{link,status}.rs`.

**Open questions:**
1. `PASS3-F2`: is the hardcoded `ResidentKind::CreatedIn` a recorded
   deliberate simplification somewhere outside `design.md` D4/D5? It
   carries no `ponytail:` marker and its stated rationale ("this tool
   creates everything in a child") is false for the one surface that
   consumes it — an *undeclared* container. If it was ratified, it needs a
   marker or an ADR line; if not, it is the sharpest finding here.
2. `PASS3-F12`: the fold to `ref_inventory(16)` changes
   `Ceiling::Counted(18)` → `Observed{18}` and adds a `labels` entry. I
   could not run `cargo test` (read-only, no shell) to confirm the two
   suites still pass under the fold.
3. `PASS3-F13`: whether each `restool` spawn draws from the per-boot dpmcp
   budget that `restool.rs:58-60` documents as never returned by destroy.
   If it does, the 1+2N-spawns-×4-per-`ensure` scan is a resource cost,
   not just latency — one measurement settles it.
