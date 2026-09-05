# ADR-0015: Intent identity is the name, the runtime handle is N, and the label is the seam between them

- **Status:** Accepted — 2026-09-05; amended 2026-09-05 with the
  identity-across-time contract.
- **Date:** 2026-09-05
- **Supersedes / relates to:** ADR-0010 (object ids are reused, names
  are not identities — this record carries its taxonomy forward);
  ADR-0013 §2 (the constructs), §4 (derived quantities), §5 (the
  refusal posture decision 11 holds), §6 INTENT_I5
  (`keysAreIdentities`); ADR-0005 (the stateless posture the identity
  ladder preserves); ADR-0001 §3 (the dpmac anchor the ladder leans
  on); OpenSpec change `intent-layer`;
  `crates/dpaa2-api/src/compiled.rs` (the label seam)

## Context

Design review found identity living in three layers that no single
record names as one contract. The intent carries identity in the
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

Identity across time rests on an impossibility, stated here so it is
not re-litigated. For a stateless control plane (ADR-0005: no
persisted state), cross-time identity cannot be any pure function of the
intent document alone, because every part of the document is editable —
so whatever the function reads, some innocent edit changes it.
Name-keying breaks on a rename, position-keying on a reorder, and
structure-keying on the very config edit that *is* the operator's
intent. Given `(intent', board)` and nothing else, "rename
`wan0`→`eth0`" and "delete `wan0`, create `eth0`" are indistinguishable.
Every system that survives a rename does one of three things: it stores
the binding, it is told the rename, or it guesses from structure. This
record chooses a blend — anchors where the hardware vouches for
identity, hardware-held labels where it cannot, and told-renames for the
rest — and never the fourth non-option of a pure document function.

Amended 2026-09-05: decisions 7–13 record the identity-across-time
contract — what the reconciler faces when the operator edits the intent
and the board already carries objects from the last converge. Decisions
3 and 5 are reframed by it and carry supersession notes in place.

## Decision

1. **Intent identity is the name (the key).** Identity is structural, in
   the TOML table key and the plan key, never in a data field. Keyed
   tables carry it — `[tenant.<name>]`, `[fabric.<name>]`,
   `[extra.<tenant>]`, `[port.<name>]`, and `[link.<name>]` — and the
   plan key is `(tenant, family, ordinal)`. The
   format and the type enforce it: a duplicate name is a TOML
   key-redefinition parse error, unrepresentable rather than a validation
   clause someone must remember to write (ADR-0013 §2).
2. **The runtime handle is N.** `family.N` is MC-owned, assigned from
   allocator state, and reused across a destroy (ADR-0010). No config
   field and no plan key ever names an N; nothing the operator writes or
   the compiler derives depends on which integer the MC hands back.
3. *Superseded in part by decisions 9 and 13: the label carries the bare
   construct name — lossless under the decision-13 name rule — not the
   ordinal-bearing rendering below.* **The label is the re-association
   seam.** The MC label rendered from the plan key —
   `<tenant>/<family>/<ordinal>`, `compiled.rs` — is the one place intent
   identity binds to a runtime object: the reconciler writes it on create
   and reads it back to know an object at some `family.N` is the one its
   plan means. The projection is lossy — restool caps `set-label` at 15
   characters, so a long tenant name overflows — so the label is a
   re-association hint, and the plan key stays the identity.
4. **Nameable families carry a name; the anonymous one does not.** Most
   constructs are nameable and take a key. `[[crypto]]` stays an array by
   the provenance decision (ADR-0013 §2): it is genuinely
   anonymous — declaration order *is* the dpseci ordinal — and that is
   acceptable precisely because nothing external binds to a crypto block
   by name.
5. *Refined by decision 8: the law's precondition — same name set, same
   definitions — is now explicit, and the ordinal is not identity across
   time.* **A position-independence law replaces N-invariance.** The
   enforceable guarantee is not that N is stable but that no plan key is
   ever derived from a construct's position in the document: reordering
   cosmetic blocks never rewires hardware or renumbers a dpni. It is
   enforced as a Quint invariant (INTENT_I10, `positionIndependence`);
   this record cites it and does not restate Quint. It is the honest,
   checkable replacement for the rejected N-invariance.
6. **The N-boundary contract.** External consumers hold N as literal
   text outside the label mechanism — a VPP fsl-mc `dpni.N` allowlist, a
   udev path match — and cannot read a label. To them the control plane
   owes three things: N is deterministic from a clean boot for a fixed
   intent and inventory; N does not churn without an intent change; and
   the name→N→netdev bindings are exportable by read-back so a consumer
   can pin its text to what the board actually assigned. The export
   itself (a `dpaa2ctl bindings`-style read) is recorded as a follow-up
   feature below, not built here.
7. **Canonical ordering is an internal detail, binding is by read-back.**
   The order that mints ordinals — name order, a lexicographic sort today
   (ADR-0013 §4, decision 5) — is the control plane's to change: no
   operator input influences it and no outside consumer may encode it.
   The anticipated question "how do I force this port to be `dpni.0`?"
   has one honest answer — there is no way, by design. A consumer that
   must pin `dpni.N` text binds it to what the board assigned through the
   decision-6 read-back, never by predicting the sort. Leaving the order
   unpromised is exactly what keeps the control plane free to change
   canonicalization later.
8. **Decision 5 reframed: the ordinal is not identity across time.**
   Name-sorted ordinals stay the law for clean-boot determinism and
   creation order, and INTENT_I10 stands unchanged. But an ordinal is not
   a construct's identity across an edit: a rename or an additive edit
   shifts a sibling's sort position, so plan keys and any ordinal-bearing
   label are unstable across a name-set edit by construction. Decision
   5's position-independence law carries a hidden precondition, now
   stated explicitly — it holds for the *same name set and the same
   definitions*: reordering cosmetic blocks is inert, but adding or
   renaming a construct legitimately renumbers its siblings. Identity
   across time is not the ordinal; it is the matching relation of
   decision 9.
9. **The identity ladder — anchor first, hardware label second.** The
   reconciler re-associates an edited intent to standing objects by a
   two-rung matching relation, both rungs stateless.
   - **Anchor-first.** A construct anchored in hardware takes its
     cross-time identity from that anchor: a port's identity is its dpmac
     (ADR-0001 §3), a hardware fabric's is its wired dpmac set, and a
     construct attached to an anchored construct is transitively
     anchored. Both realms derive the anchor independently — the intent
     says "the port on `dpmac.7`", the board shows "the dpni connected to
     `dpmac.7`" — so no stored binding is needed. Moving a port to
     another dpmac *is* a rewire, not a rename: the physical attachment
     is the intent, so this is the one case where name-stability and
     anchor-stability disagree, and the anchor wins — recorded so it is
     not read later as a bug.
   - **Label-second.** Only unanchored constructs need the label —
     tenants, and parallel links between the same tenant pair whose
     configs differ. For these the MC label carries the intent-side name
     and the hardware is the state store, so the control plane stays
     stateless. The label carries the construct *name* only (the
     intent→object direction); the object already knows its own
     `family.N`, so a label embedding the ordinal is both redundant and
     unstable (decision 8). Label drift — after an edit that shifts a
     sibling's ordinal but nothing semantic — is repaired by `set-label`,
     never by destroy/create.

   This reframes decision 3: the label written on create is the construct
   name, not the `<tenant>/<family>/<ordinal>` projection decision 3
   first described. The ordinal drops out as redundant, and with names
   bounded to the `set-label` width (decision 13) the label is no longer
   lossy — the plan key stays the identity, but the seam it renders is
   the name alone.
10. **Rename is told, and it is a single commit.** An operator declares a
    rename with `renamed = { from = "<old>" }` on the new construct — a
    temporary widening of the matcher's acceptance set, not a two-phase
    commit. There is no prepare state: the `set-label` relabel is the
    single commit and the label on the hardware is the commit record. The
    clause is self-neutralizing — after one successful converge the `from`
    matches nothing and is inert, harmless if left, garbage-collectable
    at leisure, and never consulted on a clean boot. Rollback is the
    inverse declaration through the same machinery. Two safety rules bound
    it: (i) a `from` naming a currently-declared construct refuses at
    parse; and (ii) matching runs two passes, and pass 1 excludes every
    construct named by any `from`-clause. Rule (ii) is required by the
    swap counterexample — `wan0` and `eth0` renamed to each other in one
    edit: naive pass-1 label matching would bind new-`eth0` to
    old-`eth0`'s object and "repair" its attributes disruptively instead
    of emitting the two relabels the operator meant.
11. **Indiscernible candidates match arbitrarily; ambiguity refuses.**
    When two unanchored candidates are byte-identical in config, either
    assignment is observably equivalent, so an arbitrary match is sound —
    it is not a guess. When their configs differ and no discriminator
    decides between them, the converge refuses by name, holding the
    ADR-0013 §5 refusal posture: never guess when a guess can rewire
    hardware.
12. **Every plan transition carries a disruption class.** Each transition
    is one of three classes:
    - **hitless** — a `set-label` relabel or an attribute assert, no
      traffic effect;
    - **boundary** — no traffic-path change, but an externally-held name
      changes (the netdev name systemd-networkd matches, or a VPP
      allowlist that holds it as text — the decision-6 boundary wearing a
      new face);
    - **disruptive** — a destroy/create, a link flap, or a rewired path.

    Dry-run's headline is the plan's maximum class; converge gates on an
    explicit allow of that class, and disruptive is never implied. This
    bounds decision 6's blast radius into a checkable law: an intent edit
    touching only construct *c* yields no transition above the allowed
    class outside *c*'s cone — the frame law the two-intent Quint property
    owes (revisit trigger below).
13. **A construct name is a Linux interface name and never a handle
    token.** A name must satisfy `IFNAMSIZ` — ≤ 15 characters (16
    including the NUL), which is exactly restool's `set-label` cap — with
    no `/` and no whitespace, and it must never match the reserved
    `family.N` lexical pattern (any MC family token dotted with digits:
    `dpni.4`, `dprc.15`). This settles three things at the parse boundary:
    the label is never lossy, because the name is the label byte-for-byte
    and debuggable with bare restool (reframing decision 3's lossy
    caveat, now that the label is the bare name); the name serves
    directly as the netdev name with no second mapping; and id-confusion
    is unrepresentable — an N cannot creep in through the naming side
    door, re-sealing decisions 1–2 at the point the operator types.

## Consequences

- The three layers are now one contract to cite: intent authors reason
  in names, the reconciler re-associates through the label, and only the
  MC speaks N. A change that lets N leak into config or plan keys
  contradicts decisions 1–2 and is a regression, not a choice.
- Duplicate-name safety is a property of the format, not of the refusal
  vocabulary: ADR-0013 §5 gains no clause for it because the parse fails
  first.
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
  positional must show, as the crypto record did, that nothing external
  binds to it by name — record which and why.
- **The two-intent frame property.** Decision 12's frame law — an edit to
  construct *c* perturbs nothing above the allowed class outside *c*'s
  cone — is owed as a Quint property over two intents and a bounded edit
  alphabet. Until it lands, the class of a converge is asserted by the
  reconciler, not model-checked.
- **Bounded model checking is the chosen hole-poker, not a totality
  proof.** The Quint guarantees for the matching relation (decisions
  9–11) hold over the closed construct alphabet and a bounded edit
  alphabet — bounded deliberately, because identity bugs live at sizes
  2–3 and the rename-swap counterexample (decision 10) is the proof the
  method bites. The Rust pairing/property twin runs the same relation on
  real strings as the lint — the same division of labor as `NAME_RANK`
  versus `str::cmp`. Revisit if a field identity bug escapes both.
- **MC firmware behavior the model assumes but cannot prove** —
  `set-label` atomicity, and id reuse across a destroy/create (ADR-0010)
  — has no model that checks it; its only check is board evidence through
  the read-only sitting pattern. Revisit when a firmware release changes
  either.
