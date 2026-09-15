# ADR-0007: Moves are single-hop; destroy authority is creator-bound

- **Status:** Accepted — board sittings 2026-08-23 (V-DPRC-1 rev 1 and
  rev 2)
- **Date:** 2026-08-23
- **Supersedes / relates to:** OpenSpec change `verify-foundation` (task
  5.2, suite V-DPRC-1); ADR-0002 §4 (dual-mode MBT); ADR-0003 §2
  (evidence is read-back against a stamped reference pair)

## Context

Two V-DPRC-1 board sittings contradicted the core machine model, on the
stamped reference pair (MC firmware 10.39.0, kernel 6.6.52):

1. **A sibling-to-sibling object move was refused** (rev 1). The model's
   `canMove` allowed assigning an unplugged, unbound, undrawn object to
   *any* container. The board refused
   `dprc assign <src> --object=<o> --child=<dst>` where `dst` was the
   source's sibling (nonzero exit, object unmoved in read-back).
2. **Destroying an object away from its creator was refused** (rev 2).
   After the dpni had been moved out of the container that created it —
   and that container destroyed — `dpni destroy` failed with MC
   "No privilege (status 0x4)". restool nonetheless **exited 0**; only
   the read-back caught it, vindicating the exit-codes-are-untrusted law
   (DPMAC-I8) at the harness level.
3. **A container destroy treats residents by ownership.** Rev 1
   destroyed a container still holding an unplugged dpni *it had
   created* in a single `dprc destroy` — container and resident both
   gone in read-back. Rev 2 destroyed a container holding a *moved-in*
   dpni it had not created — the container died but the dpni survived,
   reappearing unplugged in the container's parent.

The reference manual documents the destroy law directly: every family's
`*_DESTROY` section (DPNI §7.3.2, DPBP, DPCON, DPCI, DPSW, DPRTC —
identical wording) says destroy "must be invoked in the software context
that created the object", with "the authentication token of the parent
container that created the object", and that presenting the token of a
container the object was merely assigned to "will return an error". The
first sitting's model amendment read the rev-1 evidence as a general
cascade; the manual's ownership framing plus the rev-2 eviction is the
correct account.

Prior art bounds the move surprise the same way. NXP's own provisioning
tooling (`dynamic_dpl.sh` in the qoriq DPDK tree) moves objects only
along one tree edge — the command is always issued on the object's
current container with `--child=` a *direct* child (or the self-assign
plug idiom). The manual frames the whole DPRC command family as a parent
operating on its own direct children. NXP's teardown script
(`destroy_dynamic_dpl.sh`) empties a container object-by-object before
`dprc destroy` — their tooling never leans on release-or-evict behavior.

## Decision

### 1. `canMove` admits exactly one tree edge per move

A move is legal only when the destination is a direct child of the
object's container (rendered `dprc assign <container> --object
--child=<child>`) or that container's own parent (rendered
`dprc unassign <parent> --object --child=<container>`). mc.global is
never a destination. A sibling move is two legal hops: up, then down.
**Board-anchored by rev 2**: both renderings passed read-back, including
the `dprc unassign` up-hop that no NXP script exercises.

### 2. Destroy authority is creator-bound; container destroy releases owned residents and evicts foreign ones

Each object carries its creating container as `owner`, fixed for life.
`destroyAt` requires `owner == parent` — a moved object cannot be
destroyed where it stands (rev 2, manual-documented), and repatriating
it restores authority (the rev-3 suite is the positive anchor).
Destroying a container releases the residents it created (rev 1) and
evicts foreign residents one hop up into its own parent, not destroying
them (rev 2). Only the observed resident shape is admitted either way —
unplugged, unbound, undrawn, not itself a dprc: NXP's empty-first
teardown tooling is evidence that neither release nor eviction is a
documented contract to lean on.

### 3. The orphan hazard

The MC happily destroys a container whose creations live elsewhere
(rev 2 destroyed the creator at step 6 with its dpni two containers
away). The stranded object then has **no destroy authority left
anywhere** — its owner token can never be presented again — and only a
reboot (DPL re-apply) removes it. Reconciliation must therefore never
destroy a container while objects it created reside elsewhere:
repatriate or destroy the creations first. This ordering constraint is a
topology-intent input for the controlplane design.

## Amendment 2026-09-14 — the owned shape widens; the release law holds under plans

Board sittings of change `dprc-encapsulation` (tasks 5.1 and 5.4) extend
Decision 2's observed-shape guard on the release side:

- **An owned resident may be plugged.** `dprc destroy` of a scratch
  child holding a *plugged* owned dpbp exited 0 and cascade-destroyed
  container and resident, with no MC or restool refusal (V-DPRC-7 rev 1,
  2026-09-13; rev 2 re-anchored the face in a clean 9-step trace). The
  destroy fence in `models/core/machine.qnt` admits a plugged *owned*
  resident; a *foreign* (assigned-in) resident stays unplugged-only
  until the wider evict shape is observed.
- **Ownership is the creating container, not the creating actor.** A
  dpbp created *by restool* inside the reconciler's own labelled
  container was classified and released with the container — one
  cascade, parent gained 0 residents — under the reconciler's own
  generated plan (V-DPRC-11 rev 1, 2026-09-14). The release law needs no
  explicit empty-first leg, consistent with V-DPRC-7's cascade.

## Amendment 2026-09-15 — resident origin is unobservable through restool

Change `dprc-hardening` review finding PASS3-F2 caught the southbound shim
hardcoding `ResidentKind::CreatedIn` for every observed resident. A `dprc
show` row carries the plugged bit but never who created the object, so
origin is **unobservable through restool**; the earlier default was a guess
that the one consumer — the undeclared-container prune — would turn into a
post-state lie ("parent gains 0 residents") for a foreign, assigned-in
resident, which Decision 2 evicts one hop up. V-DPRC-11's board evidence
only ever exercised the created-in case, so the guess was unproven, not
proven.

Resolution: the shim reports origin as `Option<ResidentKind>` and never
invents a kind (`None` when unobserved); the core predicts the eviction
post-state conservatively — only a *known* created-in resident is released,
any unknown origin is kept in the parent's gained set and the render marks
it origin-unobservable rather than claiming a release. Revisit when an
origin-bearing observation face arrives (a DPL-defined child, or the MC
portal path that can read creation provenance), at which point the
prediction can narrow from conservative to exact.

## Open questions and revisit triggers

- **Where does owned-resident release live?** Whether `restool dprc
  destroy` walks its owned residents in userspace or the MC's
  `DPRC_DESTROY_CONTAINER` releases them itself is not decidable from
  this evidence. Revisit if restool or MC firmware is upgraded, or when
  an online-driver session can afford a strace/verbose run.
- **Release/eviction over wider shapes.** The plugged *owned* shape is
  now observed (amendment above); a probe destroying a container with a
  kernel-bound or nested-dprc resident, or evicting a plugged *foreign*
  one, would extend or refute the remaining guard. Do this only on a
  scratch subtree, never on a container holding live traffic objects.
- **Eviction destination.** The evicted dpni landed in the destroyed
  container's parent, which was also the restool caller's root — the two
  candidate destinations coincide on a depth-2 scratch tree. A deeper
  scratch nesting would separate them.
