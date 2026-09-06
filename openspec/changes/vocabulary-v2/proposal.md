# vocabulary-v2: programmatic-surface hardening

## Why

The intent-layer review (synthesis row B1, findings PASS5-F4, PASS3-F13,
PASS2-F13/Q3, and the Pass 5 runtime-check inventory's one-sided rows) found
three defects that all edit the same linted spine — `refuse.qnt`, ADR-0013 §5,
`REFUSAL_VARIANTS`, the R11 witnesses, and the refusal alphabet. Landing them
as one change touches that spine once instead of three times:

1. **Option-as-empty-string** (PASS5-F4): `Tenant.pool` uses a `""` sentinel,
   so `Isolation::Restricted` with no pool — and a pool on a non-restricted
   tenant — are representable, forcing a compile-side refusal pair and letting
   `derive.rs` re-derive restrictedness from the sentinel instead of the
   isolation field: two sources of one truth.
2. **One-sided validation** (Pass 5 inventory): three parse-side checks have no
   compile-side twin, so a programmatic `Intent` (built in Rust, never parsed
   from TOML) can carry a self-loop link, a rename double-claim, or a declared
   `kernel` tenant straight into `derive` unrefused — an open hole in the
   design D11 defense-in-depth posture.
3. **Sentinel tokens in a newtype** (PASS3-F13, PASS2-F13/Q3): `TenantAbsent`
   smuggles the literal tokens `"crypto"`/`"extra"`/`"pool"` through
   `ConstructName` — an operator can legally declare a port named `pool` and
   collide in refusal rendering — and an ownerless port refuses as
   `TenantAbsent{tenant:"kernel"}`, a tenant name the operator never typed.

## What Changes

- **BREAKING** (programmatic API): `Isolation` becomes
  `Public | Restricted { pool: TenantName } | Isolated`; the `Tenant.pool`
  field and its `""` sentinel are deleted. The refusal variants
  `PoolWithoutRestricted` and `RestrictedWithoutPool` are deleted — those
  shapes become unrepresentable. The remaining pool-shape refusals
  (absent holder, holder not public, holder pooled, dataplane mismatch)
  stay, keyed off the `Restricted` payload.
- Parse-side named errors are **unchanged**: the TOML boundary still names
  "restricted without pool" and "pool without restricted" as parse errors
  (per the review mandate, boundary checks stay); parse now constructs the
  new `Isolation` shape.
- New compile-side refusals close the D11 one-sided rows for programmatic
  Intents: a link whose two ends are the same tenant (self-loop), a rename
  double-claim, and a declared `kernel` tenant each gain a refusal variant
  mirroring the existing parse-side check.
- `TenantAbsent`'s `construct` payload becomes an enum of referencing sites
  (port, link end, fabric forwarder, crypto, extra, pool) instead of a
  `ConstructName` carrying reserved tokens.
- **BREAKING** (programmatic API): `Port.tenant` becomes a two-case sum —
  `Kernel` (the reserved default when the operator names none) or
  `Named(TenantName)` — deleting the `""`→`kernel` eff-mapping at parse, so
  the ownerless-port wrinkle stops rendering a `kernel` name the operator
  never typed and the kernel path is a match arm, not a string test
  (design D2 records the alternatives considered).
- The Quint mirror moves in lockstep: `types.qnt` `Isolation`/`Tenant`,
  `refuse.qnt` rules and alphabet, R11 witnesses, and the raw↔compile
  conformance stay bijective with the Rust surface (ADR-0014 lints R11/R14
  keep passing).
- ADR-0013 §5 is amended once for the whole vocabulary revision.

## Capabilities

### New Capabilities

None — this hardens existing vocabulary and refusal surfaces.

### Modified Capabilities

- `intent-compiler`: the `Isolation`/`pool` shape in the Intent vocabulary
  requirement; the "pool is refused when its shape is illegal" scenario loses
  its two now-unrepresentable arms; the refusal-alphabet requirement gains the
  three parity variants and the `TenantAbsent` payload enum.
- `formal-models`: the intent-model corpus requirement follows the vocabulary
  (types.qnt/refuse.qnt lockstep, witness regeneration for the changed
  alphabet).

## Impact

- `crates/dpaa2-api`: `intent.rs` (Isolation/Tenant), `refuse.rs` (variants,
  `REFUSAL_VARIANTS`, rules 12 and the new parity rules), `derive.rs`
  (restrictedness from the isolation field, `derive.rs:500`), `compiled.rs`.
- `crates/dpaa2-config`: `parse.rs` constructs `Restricted { pool }`; named
  errors untouched.
- `crates/dpaa2-tools`: test helpers building tenants (`tests/render.rs`),
  `main.rs` sites naming pool.
- `crates/dpaa2-verify`: ledger R11/R14 legs re-checked against the new
  alphabet.
- `models/intent/`: `types.qnt`, `refuse.qnt`, `intent_raw.qnt` (DEVIATION
  note), `alphabet.qnt`, regenerated witnesses under `models/traces/`.
- `docs/adr/0013-*`: §5 amended once.
- Acceptance (synthesis B1 verbatim): `grep -rn 'pool.is_empty\|pool: "".into'
  crates/` returns nothing; parse-side named errors unchanged
  (`raw_conformance` green); R11/R14 green; ADR-0013 §5 amended once.
