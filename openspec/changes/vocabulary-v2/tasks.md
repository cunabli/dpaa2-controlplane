# vocabulary-v2 tasks

Each task lands as one lockstep commit (model + Rust + tests, per design
D5), model authored first and the Rust surface written as its structurally
isomorphic image — sum-for-sum, payload-for-payload (ADR-0002: the model
is the specification, never a trace maker); `cargo build | fmt | clippy |
clippy --tests | doc` and the model ladder green at every checkbox. Granular tracking lives in beads (epic
`dpaa2-controlplane-093`, serial chain .1 → .6, six-point DoD per issue);
each checkbox names its bead.

## 1. Restricted pool typestate (design D1)

- [x] 1.1 (bead dpaa2-controlplane-093.1) Move `pool` into `Isolation::Restricted { pool: TenantName }` in
  `intent.rs` and `types.qnt` lockstep; delete the `Tenant.pool` field, the
  `""` sentinel, and the kernel pseudo-tenant's empty pool; `derive.rs`
  reads restrictedness from the variant (kills `derive.rs:500`'s
  re-derivation); `parse.rs` constructs the new shape with its named errors
  byte-identical; delete `PoolWithoutRestricted` and `RestrictedWithoutPool`
  from `refuse.rs`, `refuse.qnt`, and `REFUSAL_VARIANTS`; update every
  tenant literal in tests/tools. Verify: `grep -rn 'pool.is_empty\|pool:
  "".into' crates/` empty; `raw_conformance` green.

## 2. Shared tenant-reference sum (design D2)

- [x] 2.1 (bead dpaa2-controlplane-093.2) `Port.tenant` and both link ends
  become the shared `TenantRef` sum (`Kernel | Named(TenantName)`, no
  `Default` impl), authored in `types.qnt` first, `intent.rs` as its image;
  delete `effTenant` from `intent_raw.qnt` and the parse eff-mapping —
  parse supplies `Kernel` for an omitted port tenant (the TOML-boundary
  default) and normalises explicit `kernel` at ports and link ends;
  compile/derive match on the case, replacing the link-end `is_kernel()`
  exemptions (`refuse.rs:436,441`) and derive's kernel-materialisation
  string test, so no refusal can ever name `kernel` as absent at any site;
  drop the PASS2-F13 DEVIATION sentence that documented the wrinkle;
  regenerate traces whose ITF carries `tenant:""` ports or string link
  ends. Verify: the "port without a tenant belongs to the kernel" and
  "kernel is nameable at a link end" scenarios green; `rawDuplicateNameTrace`
  replays; no `is_kernel()` call remains in `tenant_absent_refusals` (the
  pool-holder check keeps its own — a pool names a `TenantName`, not a
  `TenantRef`).

## 3. TenantAbsent referrer enum (design D3)

- [x] 3.1 (bead dpaa2-controlplane-093.3) Replace `TenantAbsent.construct: ConstructName` with
  `referrer: Referrer` (Port/LinkEnd/Fabric/Crypto/Extra/Pool) — the sum
  authored in `refuse.qnt` first, `refuse.rs` as its isomorphic image
  (ADR-0002: the model is the spec); rendering derives the
  human string; the `"crypto"`/`"extra"`/`"pool"` tokens leave
  `ConstructName` space. Add the port-named-`pool` collision test (spec
  scenario). Verify: no reserved-token `ConstructName` literals remain in
  `refuse.rs`.

## 4. Parity refusals close D11 (design D4)

- [x] 4.1 (bead dpaa2-controlplane-093.4) Add `KernelDeclared`, `LinkSelfLoop`, and `RenameDoubleClaim` to
  `refuse.rs` and `refuse.qnt` as verbatim twins of `parse.rs:172-179`,
  `parse.rs:472-476`, and `check_renames`; each site carries the F3-form
  doc note naming its twin; unit tests build the three programmatic Intents
  and assert the refusals (spec scenarios). Verify: parse-side behavior
  unchanged; `REFUSAL_VARIANTS` count 25 and the variant-name uniqueness
  test green.

## 5. Alphabet, witnesses, and docs (designs D5, D6)

- [ ] 5.1 (bead dpaa2-controlplane-093.5) Regenerate `alphabet.qnt` and the R11 witness corpus for the
  revised refusal set; update the witness counts in COVERAGE.md (single
  authoritative source). Verify: R11 and R14 ledger lints green; full model
  ladder green.
- [ ] 5.2 (bead dpaa2-controlplane-093.6) Amend ADR-0013 §5 once for the whole revision (deleted pair,
  parity trio, `Referrer` payload, port-tenant sum) and touch §2
  spellings only where pool already appears; amend ADR-0002 with the
  structural-isomorphism law (design D6: Rust is the model's isomorphic
  image, sum-for-sum — trace equivalence alone violates the spec); update
  ROADMAP.md. Land the two discovered-decision doc notes (beads
  dpaa2-controlplane-093.7 and .8, closed by this task): the matcher's
  `ConfigFacet::Tenant.pool` stays a deliberate second reading of the
  `Restricted` payload (noted at matcher.rs and `match.qnt`, decision 11),
  and the tools shell's `complete_kernel` owns port-only kernel
  materialisation while the pure core's `effective_tenants` owns the link
  trigger (noted at both twins). Verify: synthesis B1 acceptance greps all
  pass; one amendment per ADR.
