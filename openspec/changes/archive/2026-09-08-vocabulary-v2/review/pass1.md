# Pass 1 — Staleness and residue map (`vocabulary-v2`, 34e708b..a578470)

Agent: Explore (medium). Budget ~50k; actual ~91k (completed, over budget).

## Findings

| ID | file:line | category | superseded-by | severity | disposition | verification |
|---|---|---|---|---|---|---|
| **PASS1-F1** | `/workspace/dev/clearfog-cx-lx2/src/dpaa2-controlplane/models/intent/scenarios/vwire.qnt:32` | stale | task 1.1 (42dfe06) deleted both compile refusals | misleads-a-reader | amend (drop the clause or reword to "unrepresentable, not quiet") | `grep -n 'RestrictedWithoutPool\|PoolWithoutRestricted' models/intent/scenarios/vwire.qnt` → empty |
| **PASS1-F2** | `/workspace/dev/clearfog-cx-lx2/src/dpaa2-controlplane/docs/adr/0013-accepted-intent-vocabulary.md:462` | stale (line pin) | task 2.1 (a50f412) inserted `TenantRef`+`tenantRefOf` above it in `types.qnt` | misleads-a-reader | amend — repoint to `types.qnt:59-62`, or better delete the pin and name the `Dataplane` comment (every other cross-ref in the repo names a function, not a line) | `sed -n '462p' docs/adr/0013-*.md` no longer says `35-37`; `grep -rnE '\.(rs\|qnt):[0-9]+' crates/ models/ docs/` → empty |
| **PASS1-F3** | `/workspace/dev/clearfog-cx-lx2/src/dpaa2-controlplane/crates/dpaa2-verify/src/raw_itf.rs:219-220` (+ `raw_itf.rs:239`, `crates/dpaa2-verify/tests/raw_conformance.rs:41`, `models/intent/raw_alphabet.qnt:173`, `models/intent/intent_raw.qnt:391`, `package.json:6`, `models/COVERAGE.md:291`) | copy-drift (half-applied rename) | task 4.1 (34c332f) renamed the raw variant to `RawLinkSelfLoop` but left `Kind::LinkSelfLoop`, `isLinkSelfLoop`, `wLinkSelfLoop` | misleads-a-reader | amend, or follow-up bead — the repo's own precedent one line below is `RawMemberUnresolved` / `Kind::RawMemberUnresolved` / `wRawMemberUnresolved` | `grep -rn 'LinkSelfLoop' crates/dpaa2-verify/ models/intent/raw_alphabet.qnt models/intent/intent_raw.qnt` shows `Raw`-prefixed spellings only on the raw side; R11 (`crates/dpaa2-verify/src/ledger.rs:994`, `w{Variant}` law) parses `alphabet.qnt` only, so nothing lints this today |
| **PASS1-F4** | `/workspace/dev/clearfog-cx-lx2/src/dpaa2-controlplane/crates/dpaa2-config/src/parse.rs:381-382` | simplify | — (not a staleness claim) | carries-cost | fold: `t.pool` is already `Option<String>`; the code manufactures `""` from it just to emptiness-test. `match t.pool.as_deref() { None \| Some("") => false, _ => true }` keeps `pool = ""` behaviour and the byte-identical errors, and drops the sentinel from the last non-model site. Note the spelling `pool.as_str().is_empty()` is precisely what makes task 1.1's verify grep (`pool.is_empty`) pass here — the grep is weaker than it reads | after the fold, `grep -rn 'is_empty' crates/dpaa2-config/src/parse.rs` shows no pool-sentinel test; `cargo test -p dpaa2-verify --test raw_conformance` green |
| **PASS1-F5** | `/workspace/dev/clearfog-cx-lx2/src/dpaa2-controlplane/crates/dpaa2-api/src/refuse.rs:1643-1647` | duplicate | task 1.1 (42dfe06) — the same tombstone landed at six sites | carries-cost | fold: delete this one (a comment in a `#[cfg(test)]` module explaining a test that does not exist). The type-site notes (`intent.rs:118`, `refuse.qnt:54`), the ADR record (`0013:317`) and the decoder note (`intent_itf.rs:733`) each earn their place; this one is answerable by grepping the variant name | `grep -rn 'PoolWithoutRestricted' crates/dpaa2-api/` → empty |

## Category (b)/(c) — not findings, do not re-litigate

| site(s) | class | why |
|---|---|---|
| `crates/dpaa2-verify/src/raw_itf.rs` ×16 (`RawRefusal::PoolWithoutRestricted/RestrictedWithoutPool`, `Kind::*`, decoder arms, error matchers) | (b) parse-side raw alphabet | D5: the raw layer keeps its own encoding; these are the ITF twins of `intent_raw.qnt`'s raw-native refusals |
| `models/intent/intent_raw.qnt:148,149,243,245,397-400,582,652,653` | (b) | same — parse-side rules deliberately untouched |
| `models/intent/raw_alphabet.qnt:178-179`, `models/intent/raw_replay.qnt:87-88`, `models/intent/traces/rawPoolContradictionTrace.itf.json`, `package.json:6` raw witness list | (b) | raw witnesses/traces for parse-side refusals that still exist |
| `crates/dpaa2-verify/tests/raw_conformance.rs:45-46` | (b) | required-kind list for the parse boundary |
| `models/intent/traces/raw{DuplicateName,RenamedFromDeclared,RenameSwap}Trace.itf.json` carrying `tenant:""` | (b) | raw-record ITF, not neutral `Intent`. The neutral traces are correctly tagged (`"tenant":{"tag":"Kernel"\|"Named"}`, `"interfaceA":{"tag":"Named"}`) |
| `crates/dpaa2-config/src/parse.rs:373-395`, `docs/adr/0013:68` (`pool = ""` in the TOML example) | (b) | the TOML surface keeps two keys by design (Non-Goals: no TOML change). `pool = ""` is the only place the `""` spelling survives as *documented operator surface*; see PASS1-F4 for the code side |
| `docs/adr/0013:317-321`, `models/COVERAGE.md:234`, `models/intent/alphabet.qnt:95`, `models/intent/refuse.qnt:54`, `crates/dpaa2-verify/src/intent_itf.rs:733`, `crates/dpaa2-api/src/intent.rs:118` | (c) historical/tombstone prose | records what was deleted and why; all read as current, none claim the variants still fire (see PASS1-F5 on their number) |
| `crates/dpaa2-api/src/refuse.rs:43,1699,1701,1710` (`"crypto"`/`"extra"`/`"pool"`) | (c)/(b) | doc prose about the retired tokens + the port literally named `pool` in the D3 collision test — the test task 3.1 asked for |
| `refuse.rs:686,845,936`, `derive.rs:*`, `compiled.rs:505,508,660` `is_kernel()` | (b) | outside `tenant_absent_refusals`; `refuse.rs:845` is the pool-holder check task 2.1 explicitly exempts (a pool names a `TenantName`) |
| `ponytail:` markers in diff-touched files: `crates/dpaa2-api/tests/compile_props.rs:164`, `models/intent/observed.qnt:30` | live ceilings, not retired | F1's `containmentByTenant` hole is *created* by D1 (a compile-legal `Restricted` kernel-netlink drawer) and mirrors the model marker; `observed.qnt`'s int-facet ceiling was extended by 093.7 with the `ConfigFacet::Tenant.pool` decision note. Neither ceiling was retired by this change |

## Verify-grep re-run (from tasks.md, verbatim)

| task | command | result |
|---|---|---|
| 1.1 | `grep -rn 'pool.is_empty\|pool: "".into' crates/` | **passes** (empty) — but vacuously at `parse.rs:381`, see PASS1-F4 |
| 2.1 | `effTenant` anywhere outside `openspec/`/`.beads/` | **passes** (0; sole survivor is one closed-bead reason string in `.beads/interactions.jsonl:167`) |
| 2.1 | no `is_kernel()` in `tenant_absent_refusals` | **passes** — `refuse.rs:475-529` is six `Referrer::*` arms, no string test |
| 3.1 | no reserved-token `ConstructName` literals in `refuse.rs` | **passes** — remaining `"pool"` hits are doc prose and the collision-test port name |
| 4.1 | `REFUSAL_VARIANTS` count 25 + uniqueness test | **passes** — `refuse.rs:309` is `[&str; 25]`; test at `refuse.rs:1086-1091` |
| 5.2 | synthesis B1 acceptance greps; one amendment per ADR | **passes** — one `*vocabulary-v2 revision.*` block at `docs/adr/0013:317`; one `*Structural isomorphism (amended 2026-09-07, vocabulary-v2 D6).*` at `docs/adr/0002:76`; `docs/ROADMAP.md:23` updated |
| 2.1 / 4.1 / 5.1 | scenario replay, `raw_conformance`, R11/R14 ledger lints, model ladder | **not re-run** — require `cargo`/`quint` execution (writes `target/`), outside read-only scope |

## Footer

**Read:** `openspec/changes/vocabulary-v2/{design,tasks,proposal}.md`; `crates/dpaa2-api/src/{refuse.rs (rules 12-15, variants, tests), intent.rs (TenantRef/Isolation), derive.rs (effective_tenants)}`; `crates/dpaa2-config/src/parse.rs` (`convert`, `convert_tenant`, `convert_link`, `check_renames`); `crates/dpaa2-verify/src/{raw_itf.rs, intent_itf.rs, ledger.rs (R11 witness parser)}`; `crates/dpaa2-verify/tests/raw_conformance.rs`; `models/intent/{types,refuse,alphabet,intent_raw,raw_alphabet,observed}.qnt`, `models/intent/scenarios/vwire.qnt`, `models/COVERAGE.md`; `docs/adr/{0002,0013}`, `docs/ROADMAP.md`, `package.json`; full-tree greps for every retired token plus `ponytail:` in all 60 diff-touched files.

**Deliberately not read:** everything under `openspec/` except the change's own design/tasks/proposal (the live base specs still describe pre-change behaviour by workflow design, and the archived intent-layer review is history); `models/intent/main.qnt` and `derive.qnt` bodies (heavy diffs, but their vocabulary greps are clean — left to the pass that owns model/Rust isomorphism); `.beads/` except as a grep sink; anything requiring a build or a quint run.

**Open questions:**
1. `Isolation` (`intent.rs:123`) and `Intent` (`intent.rs:325`) still `#[derive(Default)]`. D2 argued no `Default` for `TenantRef` because a zero-initialised reference is a footgun; `Tenant::default()` yielding a `Public` tenant is the same shape of hazard. Pre-existing, no superseding event, so not a finding — worth a decision line if a later typestate pass revisits it.
2. `link_self_loop_refusals` compares `TenantRef` (post-normalisation) while `parse.rs:504` compares raw strings (pre-normalisation, deliberately, to keep the error byte-identical). They agree only because a link end can never be `""`. The doc note admits the asymmetry ("two `Kernel` ends compare equal too"); is that stated anywhere as a law, or only in that comment?
3. Should the raw alphabet get an R11-equivalent lint? PASS1-F3 exists only because nothing checks `w<X>` ↔ raw variant name (`ledger.rs:994` parses `alphabet.qnt` alone).
