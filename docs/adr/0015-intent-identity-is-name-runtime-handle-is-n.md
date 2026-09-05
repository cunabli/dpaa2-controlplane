# ADR-0015: Intent identity is the name, the runtime handle is N, and the label is the seam between them

- **Status:** Accepted — gqf.40 design review 2026-09-05 (task 5.2a,
  bead gqf.47)
- **Date:** 2026-09-05
- **Supersedes / relates to:** ADR-0010 (object ids are reused, names
  are not identities — this record carries its taxonomy forward);
  ADR-0013 §2 (the constructs), §4 (derived quantities), §6 INTENT_I5
  (`keysAreIdentities`); OpenSpec change `intent-layer` design D5/D7,
  tasks 3.3b–3.3d (the keyed-table migration) and 5.2a;
  `crates/dpaa2-api/src/compiled.rs` (the label seam)

## Context

The gqf.40 design review found identity living in three layers that no
single record names as one contract. The intent carries identity in the
TOML the operator types and in the plan keys `compile` derives; the MC
carries a different identity, `family.N`, that it owns and reuses; and a
label rendered from the plan key is the only thread tying the two
together. Each layer is documented where it arose — ADR-0010 for the
reused id, ADR-0013 §2/§6 for the keyed tables and `keysAreIdentities`,
`compiled.rs` for the label — and nowhere as the single taxonomy an
operator or a reconciler author must hold in their head.

The review also weighed an operator proposal and rejected it on hardware
grounds worth recording. The proposal kept the constructs as positional
arrays, added a `numerable`/`nameable` trait to mark which families take
a name, and promised an **N-invariance** invariant: the same intent
would always converge the same object to the same `family.N`. That
promise is unpromisable. ADR-0010 is board-verified: the MC reissues the
lowest free id of a family from allocator state, in one namespace across
all containers, so N depends on what else exists at emission time and on
what was destroyed before it. An invariant the firmware can violate on
the next reconcile is worse than none — it invites callers to cache N.
The taxonomy below replaces it with a guarantee the tool can keep.

## Decision

1. **Intent identity is the name (the key).** Identity is structural, in
   the TOML table key and the plan key, never in a data field. Keyed
   tables carry it — `[tenant.<name>]`, `[fabric.<name>]`,
   `[extra.<tenant>]`, and, pending task 3.3d, `[port.<name>]` /
   `[link.<name>]` — and the plan key is `(tenant, family, ordinal)`. The
   format and the type enforce it: a duplicate name is a TOML
   key-redefinition parse error, unrepresentable rather than a validation
   clause someone must remember to write (ADR-0013 §2).
2. **The runtime handle is N.** `family.N` is MC-owned, assigned from
   allocator state, and reused across a destroy (ADR-0010). No config
   field and no plan key ever names an N; nothing the operator writes or
   the compiler derives depends on which integer the MC hands back.
3. **The label is the re-association seam.** The MC label rendered from
   the plan key — `<tenant>/<family>/<ordinal>`, `compiled.rs` — is the
   one place intent identity binds to a runtime object: the reconciler
   writes it on create and reads it back to know an object at some
   `family.N` is the one its plan means. The projection is lossy —
   restool caps `set-label` at 15 characters, so a long tenant name
   overflows — so the label is a re-association hint, and the plan key
   stays the identity.
4. **Nameable families carry a name; the anonymous one does not.** Most
   constructs are nameable and take a key. `[[crypto]]` stays an array by
   the 2.6e provenance decision (ADR-0013 §2): it is genuinely
   anonymous — declaration order *is* the dpseci ordinal — and that is
   acceptable precisely because nothing external binds to a crypto block
   by name.
5. **A position-independence law replaces N-invariance.** The
   enforceable guarantee is not that N is stable but that no plan key is
   ever derived from a construct's position in the document: reordering
   cosmetic blocks never rewires hardware or renumbers a dpni. It lands
   as a Quint invariant with the port/link keyed-table change (task
   3.3d, bead gqf.46); this record cites it as forthcoming and does not
   restate Quint. It is the honest, checkable replacement for the
   rejected N-invariance.
6. **The N-boundary contract.** External consumers hold N as literal
   text outside the label mechanism — a VPP fsl-mc `dpni.N` allowlist, a
   udev path match — and cannot read a label. To them the control plane
   owes three things: N is deterministic from a clean boot for a fixed
   intent and inventory; N does not churn without an intent change; and
   the name→N→netdev bindings are exportable by read-back so a consumer
   can pin its text to what the board actually assigned. The export
   itself (a `dpaa2ctl bindings`-style read) is recorded as a follow-up
   feature below, not built here.

## Consequences

- The three layers are now one contract to cite: intent authors reason
  in names, the reconciler re-associates through the label, and only the
  MC speaks N. A parcel that lets N leak into config or plan keys
  contradicts decisions 1–2 and is a regression, not a choice.
- Duplicate-name safety is a property of the format, not of the refusal
  vocabulary: ADR-0013 §5 gains no clause for it because the parse fails
  first (design D5, tasks 3.3b–3.3d).
- External consumers get a documented obligation to hold and, once the
  export lands, to reconcile their N text against read-back — rather than
  an N-invariance promise the firmware would break.

## Revisit triggers

- **The bindings export.** A `dpaa2ctl bindings`-style read-back
  emitting name→N→netdev is the follow-up decision 6 owes the boundary
  consumers; file it as a feature when a consumer needs to pin its text
  programmatically rather than by convention.
- **A firmware release that stops reusing ids** (ADR-0010's own revisit
  trigger): decision 2 could then expose N as a weaker stability hint,
  but decision 5's position-independence law stands regardless.
- **A second genuinely anonymous construct.** Decision 4 names
  `[[crypto]]` as the sole array; a new construct that wants to stay
  positional must show, as 2.6e did, that nothing external binds to it by
  name — record which and why.
