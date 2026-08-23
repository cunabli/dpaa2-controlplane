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

## Open questions and revisit triggers

- **Where does owned-resident release live?** Whether `restool dprc
  destroy` walks its owned residents in userspace or the MC's
  `DPRC_DESTROY_CONTAINER` releases them itself is not decidable from
  this evidence. Revisit if restool or MC firmware is upgraded, or when
  an online-driver session can afford a strace/verbose run.
- **Release/eviction over wider shapes.** A probe destroying a container
  with a kernel-bound or nested-dprc resident would extend or refute the
  observed-shape guard. Do this only on a scratch subtree, never on a
  container holding live traffic objects.
- **Eviction destination.** The evicted dpni landed in the destroyed
  container's parent, which was also the restool caller's root — the two
  candidate destinations coincide on a depth-2 scratch tree. A deeper
  scratch nesting would separate them.
