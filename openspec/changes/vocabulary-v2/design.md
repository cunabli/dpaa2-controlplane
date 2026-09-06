# vocabulary-v2 design

## Context

The intent vocabulary (ADR-0013) shipped with three defects the intent-layer
review pinned to one shared spine — `refuse.qnt`, ADR-0013 §5 (the refusal
vocabulary), `REFUSAL_VARIANTS`, the R11 witnesses, and the alphabet:

- `Tenant.pool: TenantName` uses `""` as "no pool" (PASS5-F4). The illegal
  shapes (`Restricted` without pool, pool without `Restricted`) are
  representable, refused twice (parse error + compile refusal pair), and
  `derive.rs:500` re-derives restrictedness from the sentinel rather than the
  isolation field.
- Three parse-side checks have no compile-side twin (Pass 5 runtime-check
  inventory), so a programmatic `Intent` bypasses them entirely: a self-loop
  link reaches `derive.rs:1036` unrefused; a rename double-claim reaches the
  matcher; a declared `kernel` tenant is silently tolerated via
  `effective_tenants`. Design D11's posture is defense in depth at both
  boundaries; these rows are one-sided.
- `TenantAbsent.construct` smuggles the literal tokens `"crypto"`, `"extra"`,
  `"pool"` through `ConstructName` (PASS3-F13) — colliding with a legally
  declarable port named `pool` — and an ownerless port refuses as
  `TenantAbsent{tenant:"kernel"}`, a name the operator never typed
  (PASS2-F13/Q3: parse's `effTenant` maps `""` → `KERNEL`, then compile
  resolves `KERNEL` only at link ends).

Constraints: the parse-side named errors are pinned by the review mandate and
`raw_conformance` (boundary checks stay, deliberately duplicated — the code
comments at `parse.rs:369-372` and `parse.rs:471-473` say why); **the Quint
model is the specification (ADR-0002), not a trace oracle** — every type
shape in this change is authored in the model first and the Rust surface
must be structurally isomorphic to it, sum-for-sum and
payload-for-payload, because the isomorphism is what the proofs are
proofs *of* (this binds the upcoming typestates especially). The law is
about type structure and the semantics of the relationships, not
spelling: names on both sides converge on the most readable English for
the taxonomy, and where an incumbent model spelling is lazy it is renamed
in the same lockstep rather than transliterated into Rust. The lockstep
lints (ADR-0014, R11/R14) enforce the token bijection while review
enforces the shape bijection; the alphabet and witness corpus regenerate
rather than being hand-edited.

## Goals / Non-Goals

**Goals**

- Make the illegal pool shapes unrepresentable; one source of truth for
  restrictedness.
- Close the D11 one-sided rows for programmatic Intents.
- A `TenantAbsent` payload that cannot collide with operator-declared names
  and never renders a tenant name the operator did not write.
- One coordinated pass over the shared spine; ADR-0013 §5 amended once.

**Non-Goals**

- No change to parse-side named errors or TOML surface (`raw_conformance`
  stays green unmodified).
- No change to derive semantics beyond reading isolation from the type.
- No fold of the deliberate parse/compile duplication (PASS2-F13's fold
  verdict was leave-it; this change adds twins, it does not dedup).
- The `DpmacId::new(0)` sentinel noted in passing by PASS3-F13 is out of
  scope (not part of B1's acceptance).

## Decisions

Decisions are numbered in pipeline flow order: vocabulary types first, then
the parse boundary, then compile refusals, then the lockstep and docs.

### D1. `Isolation::Restricted { pool: TenantName }`

The `pool` field moves off `Tenant` into the `Restricted` variant's payload;
the `""` sentinel and the `PoolWithoutRestricted` / `RestrictedWithoutPool`
refusal variants are deleted. `derive.rs` matches on the variant instead of
`!pool.is_empty()`. The remaining pool-shape refusals (holder absent, holder
not public, holder itself pooled, dataplane mismatch) stay as compile
refusals keyed off the payload — they depend on cross-tenant lookups a type
cannot carry.

*Alternative considered*: `pool: Option<TenantName>` on `Tenant`. Rejected —
it deletes the sentinel but keeps both illegal shapes representable, so the
refusal pair and the double-derivation survive; the review's typestate
verdict (PASS5-F4) names the variant payload explicitly.

### D2. A port's tenant is a two-case sum defaulting to the kernel

`Port.tenant` becomes a `PortTenant` sum — `Kernel` (the reserved default,
`#[derive(Default)]`) or `Named(TenantName)`. A tenant is never optional:
when the operator names none, the port belongs to the reserved kernel, and
the type says so as a case, not as an absence. Parse stops eff-mapping
`""` → `kernel` and constructs the sum (an explicit `tenant = "kernel"`
normalises to `Kernel` at parse, the one place normalisation lives);
compile and derive match on the sum, so the kernel path is a variant arm,
never an `is_kernel()` string test. `TenantAbsent` for a port can then only
fire on `Named` with an undeclared name — the `tenant:"kernel"` wrinkle is
structurally unrepresentable, and the same sentinel disease D1 cures for
pool is cured here.

*Alternatives considered*: `Option<TenantName>` — rejected: same
cardinality, wrong semantics; "no tenant" is not the fact, "the default
tenant" is, and every consumer would re-encode the rule as an `if let`
branch instead of a match arm. Keeping `effTenant` with a referrer arm
marking the synthesized kernel — rejected: compile cannot distinguish a
typed `kernel` from a synthesized one without parse smuggling that bit
anyway. Link ends keep `TenantName` with the D6a kernel allowance: a link
end has no default (both ends are always stated), so the sum's motivating
structure is absent there; adopting `PortTenant` at link ends is a possible
later unification, not part of this change. The sum is new on both sides
(today the model carries the same `""`/`effTenant` encoding as the Rust):
it is authored in `types.qnt` first with the same case names the Rust enum
takes — `Kernel | Named` — and `effTenant` dies in both.

### D3. `TenantAbsent` referrer enum

`TenantAbsent.construct: ConstructName` becomes `referrer: Referrer` — a sum
authored in `refuse.qnt` naming the referencing site with its identifying
payload, with the Rust enum as its isomorphic image:

```rust
pub enum Referrer {
    Port(ConstructName),
    LinkEnd(ConstructName),
    Fabric(ConstructName),
    Crypto(TenantName),   // crypto constructs carry no name of their own
    Extra(TenantName),    // likewise; identified by the referencing tenant
    Pool(TenantName),     // the restricted drawer naming the absent holder
}
```

The reserved tokens `"crypto"` / `"extra"` / `"pool"` disappear from
`ConstructName` space, so a port named `pool` can no longer collide in
refusal rendering. Rendering derives the human string from the enum; the
variant *name* (`TenantAbsent`) and the alphabet token are unchanged.

*Alternative considered*: reserve the three tokens in ADR-0013 §5 prose
(PASS3-F13's minimal fix). Rejected here — B1 is exactly the "follow-up
vocabulary change" that note deferred to, and prose reservation leaves the
collision representable.

### D4. Three parity refusal variants close the D11 rows

New compile-side variants, each mirroring an existing parse check verbatim
(same predicate, refusal instead of `Err`):

- `LinkSelfLoop { link: ConstructName }` — a link whose two ends resolve to
  the same tenant (`parse.rs:472-476` twin).
- `RenameDoubleClaim { construct: ConstructName, from: ConstructName }` — a
  `from` naming a construct currently declared and not itself renamed
  (`check_renames` twin, both tenant and port/link/fabric namespaces).
- `KernelDeclared` — the intent declares a tenant named `kernel`
  (`parse.rs:172-179` twin); the payload carries nothing because the name is
  the fact.

Each site gains the same deliberate-duplication doc note the existing twins
carry (F3 exemplar form: name the twin file:line and the raw_itf pin).
`REFUSAL_VARIANTS` goes 24 → 25 (−2 pool shapes, +3 parity).

### D5. Model-first lockstep and regenerated witnesses

Within each task the model is authored first — `types.qnt` (`Isolation` sum
with payload, `Tenant` loses `pool`, the `PortTenant` sum), `refuse.qnt`
(rule 12 shrinks, rules for the three parity variants, `Referrer` sum),
`alphabet.qnt` (regenerated token list) — and the Rust surface is then
written as its isomorphic image; both land with the regenerated R11
witnesses in one lockstep commit per the L1 precedent, never split across
commits in either direction. `intent_raw.qnt` keeps
its parse-side rules untouched and its DEVIATION block updated (the
PASS2-F13 divergence sentence dies with `effTenant`).

### D6. ADR-0013 §5 amended once; ADR-0002 gains the isomorphism sentence

ADR-0002 already states the model is the design artifact; task 5.2 amends
it with the sharper standing law this change works under: the Rust surface
is structurally isomorphic to the model's types — sum-for-sum,
payload-for-payload, typestates included — because the conformance proofs
quantify over that shared shape; an encoding that is merely
trace-equivalent (option-for-sum, sentinel-for-case) violates the spec
even when every trace matches. The law governs structure and relationship
semantics, not spelling: names converge on the most readable English on
both sides, renaming a lazy incumbent model spelling in lockstep rather
than importing it.

For ADR-0013 §5, one amendment recording: the two deleted variants and why (typestate), the
three parity variants and the D11 posture they complete, the `Referrer`
payload, and the ownerless-port option. No other ADR-0013 section changes;
the vocabulary examples in §2 gain the `Restricted { pool }` spelling only
where they already show pool.

## Risks / Trade-offs

- [Breaking programmatic API: every `Tenant`/`Port` literal in tests and
  tools changes] → mechanical; the compiler finds every site; test helpers in
  `dpaa2-tools/tests/render.rs` and `main.rs` are already on B1's named-site
  list.
- [D2 touches derive's kernel materialisation path] → behavior is pinned by
  existing scenarios ("a port without a tenant belongs to the kernel"); the
  conformance suite (`raw_conformance`) and frozen traces replay must stay
  green, and traces whose ITF carries `tenant:""` ports are regenerated, not
  hand-patched.
- [Alphabet regeneration churns witness counts] → COVERAGE.md is the single
  authoritative count source since 9yy.11; regenerate and update it there
  only.
- [Three new refusals could drift from their parse twins later] → same
  exposure every F3-noted pair already has; the doc-note convention plus
  raw-conformance divergence traces are the accepted mitigation, no new
  machinery.

## Migration Plan

Single-repo, no persisted state, no external consumers yet (pre-release
public repo): land as one change, semver-major note in the changelog via the
conventional-commit `!`. Rollback is git revert of the change's commits.

## Open Questions

- None blocking. `KernelDeclared`'s empty payload is the only zero-payload
  refusal in the alphabet; if the ledger lint requires a payload, carry the
  offending declared name (`tenant: TenantName`) even though it is always
  `kernel`.
