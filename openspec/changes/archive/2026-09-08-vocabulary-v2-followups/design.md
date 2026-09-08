# vocabulary-v2-followups design

## Context

Every item here descends from a verified row in the vocabulary-v2 review
synthesis (`openspec/changes/archive/2026-09-08-vocabulary-v2/review/
synthesis.md`); the ledger and the pass reports beside it carry the
file:line evidence, so this design does not restate it. Two constraints
carry over from vocabulary-v2 unchanged: parse-side named errors and the
TOML surface stay pinned by `raw_conformance`, and the Quint model is the
specification (ADR-0002) — every shape edit is authored model-first with
the Rust surface as its structurally isomorphic image. The review reports
cite pre-reword commit hashes; the mapping lives in the review commit
message (c799de0).

## Goals / Non-Goals

**Goals:**

- One answer, both boundaries, for `kernel` at the three `TenantName`
  reference sites; the `refuse.rs` claim made true (ledger #1).
- No sentinel-for-case encoding left on any path a rule reads, or a
  DEVIATION marker where one deliberately stays (ledger #3, #5, #10, #14).
- The raw self-loop family fully `Raw`-prefixed (ledger #6).
- Every pointer the review found misleading now says what shipped
  (ledger #4, #7, #8, #9, #11, #12, #13, #15, #16).

**Non-Goals:**

- No typestate work beyond recording the hazards (the typestate roadmap is
  its own future change; P1-Q1/P2-Q2 land as a note, not code).
- No new lint machinery (P1-Q3 resolved drop-the-lint, fix-the-drift).
- No fold of any protected duplication (ADR-0014 lockstep copies, the
  parse/compile twins, the matcher's second pool reading, the split kernel
  materialisation).
- No TOML surface change; `raw_conformance` stays green unmodified.

## Decisions

### D1. Compile mirrors parse: `kernel` always resolves at `TenantName` referents (B1)

At `Fabric.forwarded_by`, `Crypto.tenant`, and `Extra.tenant`, compile's
rule 1 treats the reserved kernel name as always-resolving, exactly as
parse's `resolves()` already does — the referent set becomes "declared
tenants plus the reserved kernel" at these three sites. Rationale: D2 of
vocabulary-v2 already established that the kernel is a legitimate referent
everywhere a tenant can be named; parse shipped that answer first and its
named errors are pinned; moving compile preserves every currently-parsing
intent and only stops a refusal that named a tenant the operator never
declared as *absent* when it is in fact reserved. Whether the kernel
object materialises for such a reference stays the business of the
existing split materialisation (shell port-trigger, core link-trigger) —
if neither trigger fires, derivation's existing rules own the outcome;
this rule only stops the lie that `kernel` is undeclared.

*Alternative considered*: parse refuses `kernel` at these sites, compile
keeps rule 1 narrow. Rejected — it changes pinned parse behavior (a
vocabulary-v2 non-goal inherited here), breaks currently-legal intents,
and contradicts D2's own reserved-referent posture.

The model twin (`refuse.qnt` rule 1) moves in the same lockstep commit,
and ADR-0013 §5 records the decision in the same one-amendment style as
the vocabulary-v2 revision block.

### D2. The rename encoding converges on a sum; the last `""` tests die (B2)

`types.qnt`'s `from: str` with `""` = absent becomes the two-case sum the
Rust `Option<ConstructName>` already is — authored model-first as
`Rename = NotRenamed | RenamedFrom(str)` (names converge on readable
English per the D6 law; the Rust side keeps `Option`, which is that sum,
with the field doc naming the twin). Rule 15 and every `t.from != ""` test
become match arms. Rationale: the change that legislated the ban is the
wrong place to leave its largest surviving instance; a DEVIATION marker
was the fallback and loses to a two-file mechanical edit that regenerates
traces anyway.

Under the same bead, three smaller `""` hygiene folds from the ledger:
`holderPool` becomes a bool-returning match (hoisted-helper shape kept);
`TenantRef::from_name` keeps its current `Named("")` behavior but both
twins gain the domain-split note (Rust raw carries `Option`, so `""` is a
name, not an omission — folding `""`→`Kernel` would silently bless an
empty string the API should never see) plus the pinning doctest;
`parse.rs:381` reads `t.pool.as_deref()` with errors byte-identical. The
typestate note (`Default` on `Isolation`/`Intent`, empty-`TenantName`
constructibility) lands as one comment block at the `Isolation` type,
flagged for the typestate change.

### D3. Mechanical spellings and pointers finish (B3, B4)

No decisions to make — the synthesis acceptance lists are the work order:
B3 renames the raw self-loop kind/predicate/witness to `Raw*` across the
six cited files and regenerates the raw witness corpus; B4 executes the
ten amendments verbatim from synthesis §5. The duplicate-name posture row
(P2-Q1) records in ADR-0013 §5 that compile deliberately carries no
`DuplicateName` refusal — the raw layer owns that check — closing the
last open D11 row on record.

## Risks / Trade-offs

- [D1 changes compile output for kernel-referencing intents with no
  kernel port] → that is the point; the pinning unit test states the new
  truth, and scenario/trace regeneration is part of the task, not a side
  effect.
- [D2's model sum churns ITF traces carrying `from: ""`] → regenerate,
  never hand-patch (vocabulary-v2 precedent); `raw_conformance` and the
  ladder gate it.
- [B3 renames identifiers tests match on] → the repo's own
  `RawMemberUnresolved` precedent shows the finished shape; grep-based
  acceptance makes partial application impossible to miss.

## Migration Plan

Same posture as vocabulary-v2: single repo, no persisted state, local
unpushed main; land as lockstep commits per task, rollback is revert.
D1 and D2 are semver-noted only if their surface change is visible to a
programmatic consumer (D2's Rust surface is unchanged — `Option` stays;
D1 changes refusal outcomes, not types: no `!` needed).

## Open Questions

- None blocking. D1's "always resolves" deliberately does not decide
  materialisation; if review at apply time finds a derive path that
  panics on an unmaterialised kernel referent, that discovery amends this
  design in place (never a new change).
