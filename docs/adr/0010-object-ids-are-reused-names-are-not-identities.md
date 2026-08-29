# ADR-0010: Object ids are reused, so a name is not an identity across a destroy

- **Status:** Accepted — board sitting 2026-08-29 (task 5.10); the law
  is written into object-model.md §6.7 and the hazard is filed here for
  the changes that build the reconciler (`#3`) and the container
  typestate (`#4`)
- **Date:** 2026-08-29
- **Supersedes / relates to:** OpenSpec change `verify-foundation` task
  5.10; ADR-0001 (stateless, level-triggered convergence); ADR-0006
  (single initiating writer); ADR-0007 (creator-bound destroy);
  `models/core` `nextNum`

## Context

The MC names objects `family.N` and the control plane addresses them by
that name in every command, every read-back and every DPL. The Quint
models allocate `N` from a monotone per-family counter (`nextNum`), so
inside one trace a name is unique for the life of the trace and a stale
reference cannot alias a newer object.

The firmware does not do that. Every sitting since 5.4 has shown the MC
reissuing the lowest free id of a family, in one namespace across all
containers: each scratch child comes back as `dprc.2`; a dpbp created in
a child while `dpbp.0` stands in the root is `dpbp.1`; V-DPRC-5 released
`dpci.0` and `dpci.1` and the next suite, V-DPCI-2, was handed `dpci.0`
as its first object; `dpmcp.53` was reissued six times across the
earlier sittings. Object ids are reused immediately, not retired.

That is harmless for a scripted suite whose teardown holds the names it
created. It is a hazard for what this project is building: a stateless,
level-triggered reconciler (ADR-0001) that reads the board, decides, and
acts — with the read and the act separated by time, and with other
writers (a provisioning unit, an operator with restool, a kernel driver)
outside its control (ADR-0006 makes single-writer an assumption, not a
guarantee).

## Decision

1. **A name is an address, not an identity.** `family.N` identifies an
   object only while the holder knows that object to be alive. Across a
   destroy — its own or anyone else's — the same name may denote a
   different object. Nothing in the crates may cache a name across an
   action that can destroy, or compare two names read at different
   times as if equality meant sameness.
2. **The models keep the monotone counter, declared as an abstraction.**
   `nextNum` stays monotone so traces remain replayable and readable; it
   is recorded in object-model.md §6.7 as a modeling choice that the
   firmware does not honour. Suites that need the reuse (a
   create→destroy→create cycle asserting which id comes back) express it
   as a hook read-back, not as a model law.
3. **Identity is re-established by read-back before every destructive
   act.** A reconciler that decides "destroy `dpni.3`" from an earlier
   read must, immediately before the destroy, re-read enough of the
   object to know it is still the object it decided about: its
   container, its creator-bound destroy portal (ADR-0007 §2), its
   configuration block, and — where a family has one — a label the
   reconciler itself wrote. A mismatch is refuse-and-report, never
   "destroy whatever is at that name now".
4. **Labels are the only identity the MC carries, and they survive a
   lock.** `dprc set-label` is accepted even on a locked container's
   objects (V-DPRC-3 rev 1) and reads back through `dprc show`. The
   reconciler tags every object it creates with a label it can
   recognise; an object at an expected name without that label is
   somebody else's and is left alone. This is the cheapest ABA guard
   the hardware offers and the first thing `#3` should do.
5. **Numeric ranges are never reserved.** No plan may assume "our
   objects are `dpni.4` and up" or that a freed id stays free; the DPL
   tape-out (`#14`) must emit whatever names the board reports at
   emission time, not names remembered from creation.

## Consequences

- The `dpaa2-api` object references gain a "known-alive" scope: a
  reference is valid for one reconcile pass and is re-resolved from a
  fresh read at the start of the next. The typestate makes holding a
  name across a destroy unrepresentable.
- Every generated suite already behaves this way (it holds the ids it
  created and destroys in reverse order inside one run); nothing changes
  for `dpaa2-verify`.
- Labels become part of the convergence intent: the intent layer (`#5`)
  reserves a label namespace, and the read-back parser must surface the
  label column of `dprc show`.
- The reference snapshot's object list is a set of names at one moment,
  not a set of identities; snapshot diffs report "a `dpni.3` is present"
  and never "the same `dpni.3` is still present".

## Revisit triggers

- A firmware release whose id allocator is shown *not* to reuse (a
  create→destroy→create cycle that returns a higher id): decision 2
  can then be strengthened to a law rather than an abstraction.
- A family whose objects carry no label (the `dprc show` label column
  empty by construction): decision 4 needs a substitute identity for
  that family — record which and why.
- The first reconciler bug traced to a stale name: file it against this
  ADR and add the cycle suite decision 2 describes.
