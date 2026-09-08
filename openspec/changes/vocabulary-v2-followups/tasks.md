# vocabulary-v2-followups tasks

Each task lands as one lockstep commit (model + Rust + tests where shapes
move, per ADR-0002's structural-isomorphism law), model authored first;
`cargo build | fmt | clippy | clippy --tests | doc` and the model ladder
green at every checkbox; `raw_conformance` green unmodified throughout.
Granular tracking lives in beads (epic `dpaa2-controlplane-06l`, serial
chain B1 → B2 → B3 → B4, six-point DoD per issue); each checkbox names its
bead. Evidence for every item: the review ledger in
`openspec/changes/archive/2026-09-08-vocabulary-v2/review/synthesis.md`.

## 1. Kernel resolves at TenantName referents (design D1)

- [x] 1.1 (bead dpaa2-controlplane-27w) Compile rule 1 treats the reserved
  kernel as declared at `Fabric.forwarded_by`, `Crypto.tenant`, and
  `Extra.tenant`, mirroring parse's `resolves()`; `refuse.qnt` rule 1
  moves in the same lockstep; the `refuse.rs:480` claim is scoped
  truthfully; ADR-0013 §5 records the decision once; unit test pins the
  kernel-forwarded-fabric and kernel-crypto scenarios (spec delta).
  Verify: no `TenantAbsent` path can carry the reserved kernel name —
  grep the rule's referent set construction, not one spelling.

## 2. The empty-string case-encoding leaves the twins (design D2)

- [x] 2.1 (bead dpaa2-controlplane-5vz) `types.qnt` rename field becomes
  the `NotRenamed | RenamedFrom` sum (model-first; Rust `Option` is its
  image, field doc names the twin); rule 15 and every `from != ""` test
  become match arms; traces regenerate. Same commit: `holderPool` becomes
  a bool-returning match; `from_name`/`tenantRefOf` domain-split notes at
  both twins plus the `from_name("")` doctest; `parse.rs:381` reads
  `as_deref()` with byte-identical errors; typestate hazard note at
  `Isolation`. Verify: no sentinel emptiness test on any rule path
  (concept grep), `raw_conformance` green.

## 3. The raw self-loop rename finishes (design D3)

- [x] 3.1 (bead dpaa2-controlplane-xgu) `Kind::LinkSelfLoop`,
  `isLinkSelfLoop`, `wLinkSelfLoop` take the `Raw*` spelling across
  `raw_itf.rs`, `raw_conformance.rs`, `raw_alphabet.qnt`,
  `intent_raw.qnt`, `package.json`, `COVERAGE.md`; raw witnesses
  regenerate. Verify: `grep -rn 'LinkSelfLoop'` on the raw side shows
  only `Raw`-prefixed spellings.

## 4. Docs, pointers, and tests say what shipped (design D3)

- [ ] 4.1 (bead dpaa2-controlplane-568) The ten amendments from synthesis
  §5 B4: matcher.rs 093.7 note repoints to `observed.qnt` and the
  archived tasks.md 5.2 is corrected; COVERAGE.md records all 18
  requested raw witnesses or states the omission; ADR-0002 names the
  projection exemption; `RenameDoubleClaim`/`KernelDeclared` docs name
  `RenamedFromDeclared`/`ReservedKernel`; `vwire.qnt` reworded;
  ADR-0013:462 repointed by name; the three parity tests use complete-set
  `assert_eq!`; the `refuse.rs:1643` tombstone is deleted; the archived
  proposal path is fixed; the duplicate-name D11 posture row lands in
  ADR-0013 §5. Verify: one grep per item, per the bead's acceptance.
