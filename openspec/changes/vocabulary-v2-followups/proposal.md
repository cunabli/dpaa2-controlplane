# vocabulary-v2-followups proposal

## Why

The vocabulary-v2 epic review (synthesis at
`openspec/changes/archive/2026-09-08-vocabulary-v2/review/synthesis.md`)
closed with a 16-row ledger and four bead-shaped follow-ups. Two carry real
defects the review verified: a comment in `refuse.rs` claims "no site can
ever name `kernel` as absent" while three `TenantName` reference sites still
can, TOML-reachably; and the change that legislated the sentinel-for-case
ban (ADR-0002 D6) built its newest rule on the one surviving sentinel
encoding. The other two are a half-applied rename and a batch of misleading
pointers at exactly the sites a reader consults.

## What Changes

- **B1 — kernel agreement at `TenantName` referents**: compile and parse
  give one answer for `kernel` at `Fabric.forwarded_by`, `Crypto.tenant`,
  and `Extra.tenant`; the decision is recorded in ADR-0013 and pinned by a
  unit test; the `refuse.rs:480` claim becomes true.
- **B2 — the `""` case-encoding leaves the twins**: the model's rename
  field (`from: str`, `""` = absent) converges with Rust's `Option` or is
  DEVIATION-marked; `holderPool` becomes a bool-returning match;
  `TenantRef::from_name("")` behavior is decided and doctest-pinned;
  `parse.rs` stops manufacturing `""` from an `Option`; a typestate note
  records the `Default`/empty-`TenantName` constructibility hazards.
- **B3 — the raw self-loop rename finishes**: `Kind::LinkSelfLoop`,
  `isLinkSelfLoop`, `wLinkSelfLoop` take the `Raw*` spelling their variant
  already has, across all six files.
- **B4 — docs, pointers, and tests say what shipped**: the ten mechanical
  amendments from ledger rows 4, 7, 8, 9, 11, 12, 13, 15, 16 plus the
  duplicate-name D11 posture row (synthesis §5 B4 acceptance list).

No breaking API changes; B1's decided behavior may change which intents
compile (a currently-refused kernel-forwarded fabric may become legal, or a
currently-parsed one refused — the design decides which side moves).

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `intent-compiler`: the `TenantAbsent` rule's treatment of the reserved
  kernel at the three `TenantName` reference sites becomes a stated
  requirement (B1), and the deliberately raw-only duplicate-name refusal
  posture is recorded against design D11 (B4).

## Impact

- Crates: `dpaa2-api` (refuse.rs, intent.rs, matcher.rs), `dpaa2-config`
  (parse.rs), `dpaa2-verify` (raw_itf.rs, raw_conformance.rs).
- Model: `types.qnt`, `refuse.qnt`, `intent_raw.qnt`, `raw_alphabet.qnt`,
  `scenarios/vwire.qnt`; regenerated raw witnesses if spellings change.
- Docs: ADR-0002, ADR-0013, `models/COVERAGE.md`, the archived change's
  `proposal.md` path fix.
- Gates: cargo suite, model ladder, `raw_conformance`, R11/R14 unchanged
  green throughout.
