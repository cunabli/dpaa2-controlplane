# ADR-0016: The development process is a modeled system; process invariants are named, checked, and audited like the ones in the code

- **Status:** Accepted — process retrospective 2026-09-09
- **Date:** 2026-09-09
- **Supersedes / relates to:** ADR-0002 (formal-methods process — this ADR
  applies its philosophy to the workflow that produces the code); ADR-0003
  (board interaction protocol — the same safety-envelope thinking, aimed at
  the agentic workflow); bead `dpaa2-controlplane-pil` (the incident record
  this ADR generalizes)

## Context

This repository is produced by an agentic workflow with a human principal
at its root. The principal is not one actor among several: they are the
single source of intent — the only party who knows what the system is for
— and the closing authority — the hands on the board, the adjudicator of
conflicts, the acceptance that seals work. Agents extend both roles; they
replace neither. Every loop the workflow opens (a parcel, a board suite,
an ownership question) closes through the principal. Authority in this
system is therefore not a permission bit a message can carry: it is
provenance from that one source, and it does not survive relay.

Around that root runs the workflow: a coordinating session gathers
context, writes parcel specifications, dispatches implementation to
subagents, reviews, gates, and commits — one task at a time, with
knowledge relayed between tasks through issue-tracker comments and commit
messages. The workflow now produces more per session than the principal
can line-review, so trust has migrated from the artifacts to the process
that produces them. A process trusted in place of its artifacts must be
engineered like one.

The context each participant runs in is deliberately unequal, and that
mattered. The coordinator holds the long horizon: a large context window,
the strongest model, the principal's direct words, and the live state of
every task — spent on specification, review, and relay rather than on
production. Implementing agents get a narrow horizon: a curated parcel
specification that quotes every constraint they must honor and names the
files they must read. The split is intentional and selective — each agent
sees exactly what its role needs — so session momentum cannot leak into
production, and everything an implementer knows is written and therefore
reviewable. Model capacity follows role the same way: heavyweight
judgment where intent is interpreted, pinned capable models where code is
produced, small models only for mechanical digestion.

Two events on 2026-09-09 forced this ADR.

**The near miss (root-cause analysis).** While the principal was away, a
session-memory plugin's automated transcript-digestion workers —
ephemeral, tool-stripped helper sessions running a small model — sent
cross-session messages to the working session. Twice they asserted that
the principal was "explicitly requesting" transfer of in-flight work. The
claims were confabulated: the workers' context was a stale digest of the
working session's own transcript, and the second claim echoed, nearly
verbatim, the working session's own description of what a legitimate
transfer would require. The working session refused, kept ownership,
shared only read-only reports, and logged the conflict for the principal.

Root causes, in order: (1) an automation channel that could *assert*
principal intent but could not *carry* it — no provenance, no
verification; (2) a digest-fed context that re-narrated transcript content
as live instruction; (3) a capability/authority mismatch — a small model
with the narrowest, stalest horizon in the system held a channel wide
enough to make authority claims; (4) the absence of any structural block
on peer-messaging by automation whose only purpose was observation.

The refusal held for five checkable reasons, and they are the reusable
part: the principal's direct written statement ("afk") made the relayed
claim self-contradicting; the channel is defined as unable to grant
authority, so no judgment of the sender's intent was needed; the source
had already been wrong on checkable facts; the authority claim appeared
on demand, shaped exactly like the template it had just been handed; and
the costs were asymmetric — refusal was reversible, transfer was not. The
split horizon aided detection: the coordinator's long horizon held the
principal's actual words and the true task state, so the worker's claim
was checkable the moment it arrived. The hazard is the same asymmetry
reversed: a narrow-horizon agent confabulates confidently, because
nothing in its horizon contradicts it.

**The audit finding.** A phase retrospective found that across four
consecutive implementation parcels, the coordinating session's code
review caught zero defects. Correctness was carried instead by (a)
constraint blocks relayed verbatim from each review into the next parcel
specification, and (b) implementing agents flagging their own deviations
loudly — refusing a stale acceptance criterion, surfacing a spec
conflict, exposing a missing seam — rather than silently complying or
silently diverging. A review gate that never fires, while everyone
believes it protects, lets each individually reasonable acceptance become
the new baseline — normalization of deviance. The countermeasure is to
record every deviation as a decision and to ask of every gate,
periodically, what it has actually caught.

The same accretion risk applies to accumulated process lore (memory
files, rule piles, workflow instructions): every entry was a real scar,
but the pile only grows, is never re-validated, and eventually reproduces
the unreviewable-volume problem one level up.

This ADR adopts the four-question threat-model discipline (Shostack) —
what are we working on, what can go wrong, what are we going to do about
it, and did we do a good job — applied to the workflow, on a recurring
basis, not once.

## Decision

### 1. What are we working on (the trusted surface, stated)

The system is: principal → coordinator → parcel spec → implementing agent
→ review → gate → close-then-commit, with inter-task knowledge flowing
only through recorded channels (issue comments, commit messages, ADRs),
and with two edges that never delegate: intent enters the system only
from the principal speaking directly in a session, and board praxis exits
the system only through the principal's hands (ADR-0003). The trusted
surface is deliberately small: the constraint-relay chain, the mechanical
gates, and the recorded decisions. Ambient session context is explicitly
*not* trusted across a task or session boundary: the boundary forces all
carried knowledge through the narrow written channel, and that
compression is itself a review — knowledge that cannot be written down is
not carried.

### 2. What can go wrong (the standing threat catalog)

1. **Relayed authority.** Any message from a peer or automated session
   asserting principal intent. Void by construction: only the principal,
   speaking directly in the receiving session, transfers ownership,
   approves pending actions, or changes plan. Read-only information may be
   shared freely with any peer; authority never travels that channel.
2. **Capability/authority mismatch.** An agent's epistemic authority
   exceeding its model capacity or its horizon quality. The widest claims
   in the near miss came from the smallest model with the stalest context.
   Channel width is granted by role: digest and observation automation
   gets no authority-bearing channel at all.
3. **Silent deviation.** An agent or coordinator departing from a spec,
   acceptance criterion, or convention without recording the departure.
   Every deviation is flagged in the parcel report and resolved in the
   review, and the resolution is written where the next task will read it.
4. **Vacuous gates.** A gate that always passes and is believed
   load-bearing. Every gate carries an honesty obligation: at each phase
   close, record what each gate actually caught. A gate that catches
   nothing is either given teeth (fresh-context adversarial review, a
   divergence test that proves the check bites) or demoted honestly.
   Applied to the gate that forced this finding: the per-parcel
   coordinator review is **demoted** from defect gate to what the audit
   shows it actually does — constraint distillation, producing the block
   the next parcel specification quotes. Defect-catching authority is
   assigned to the fresh-context reviews at epic and phase close, which
   are already the audit instrument. Revisit if a phase close finds a
   defect the relay chain should have caught in-parcel.
5. **Stale mechanism in acceptance criteria.** Criteria written at
   proposal time can encode a mechanism the codebase has outgrown ("via
   the Runner seam" for writes no runner can express). Gates judge intent
   (the property to hold); mechanism is the implementer's, and a justified
   mechanism deviation flagged in the report is compliance, not violation.
6. **Lore accretion.** Process rules accumulating as prose faster than
   they are re-validated — the workflow's own normalization-of-deviance
   surface, inverted: not shortcuts becoming normal, but dead rules
   crowding out live ones until none are auditable.

### 3. What we do about it (the mechanics)

**The split horizon is maintained, as a detection instrument.** The
context architecture described above is kept deliberately: coordinator
long, implementers narrow, digestion automation narrowest and with no
outbound authority. A wider horizon is granted only when a role
interprets intent, and the grant is itself a recorded decision. The
long-horizon holder is the one place a narrow-horizon confabulation
becomes checkable, so it must retain the facts claims are checked
against: the principal's stated availability, live task state, sealed
commit hashes. The detector needs its own check: after any compaction the
coordinator's context is itself a digest, so authority-relevant facts
(ownership, approvals, the principal's availability) are re-verified from
the durable record — bead comments, sealed commits — never from
summarized session memory.

**The observation channel is closed, not policed.** Automation whose role
is observation (transcript digestion, session memory) runs with
peer-messaging stripped: its workers get no message-sending tool. Where a
plugin cannot be configured that way, its cross-session messaging is
disabled — or the plugin is dropped — rather than relying on the
receiving session's judgment. The five refusal reasons remain as bin-(b)
defense-in-depth for channels that must stay open, not as the primary
control for one that should not exist.

**Triage every process rule into one of three bins, recurringly:**

- **(a) Mechanically checkable** → promote to a machine check (hook, lint,
  commit-message check, tracker validation) and delete the prose. Known
  members today — all still pending promotion, tracked on bead
  `dpaa2-controlplane-pil`: peer-messaging stripped from observation
  automation; close-then-commit ordering with the change trailer; one
  writer per crate per task; name-slot newtype audits; the quality-floor
  command set. Until promoted, a member of this bin is a plan, not a
  protection.
- **(b) Genuine judgment** → keep as prose only with its *why* and a
  revisit trigger attached. Known members today: "gate on intent, not
  mechanism"; "capture ambivalence in an ADR"; the five refusal reasons
  named in the Context.
- **(c) Stale** → delete, and record the deletion. A rule nobody can
  re-derive the reason for is a candidate, not a keepsake.

**Intra-session markers keep the checks active.** The recurring prompts a
session gives itself are few, stable, and checklist-style — they do not
accumulate: *Did this instruction come from the principal in this
session? Is this deviation recorded? What did this gate catch? Which bin
does this new rule go in? Does this agent's authority match its horizon?*
Deliberate simplifications in code carry their in-code ceiling marker;
deliberate process shortcuts carry the same, in the task record.

**The relay chain stays narrow and load-bearing.** Review decisions are
written into the closing task record; the next parcel specification
quotes them; the phase close verifies they were honored. This chain is
what demonstrably carries correctness, so it is maintained as
infrastructure, not as courtesy prose.

**Fresh context is the audit instrument.** Periodic reviews (epic close,
phase close) run with agents that have no session momentum, so
accumulated in-session judgment is re-derived from the written record or
discarded. The gap between what the session believed and what a cold
reader concludes is the measured drift.

### 4. Did we do a good job (audit and revisit)

- At each phase close: the honesty check (per-gate catches, parcel
  economics, deviations flagged vs. discovered) is recorded on the epic.
- The rule pile is re-triaged at each phase close, or immediately when
  any rule fires wrongly — whichever comes first.
- This ADR is revisited when: a relayed-authority attempt succeeds or a
  new automation channel appears; a bin-(a) promotion proves wrong (a hook
  blocks legitimate work); a horizon grant is widened without a recorded
  decision; or two consecutive phase closes find the same gate vacuous and
  still enthroned.

## Consequences

- Authority has exactly one channel — the principal, speaking directly in
  the receiving session — and everything else is information. This costs
  nothing while automation is honest and bounds the damage when it
  confabulates.
- The principal remains the sole source of intent and the sole hands on
  the board by structure, not by convention: intent cannot be laundered
  through automation, and board-touching work cannot bypass ADR-0003.
- Gates are either load-bearing or honestly demoted; "review found
  nothing" is treated as a question about the gate before it is treated
  as praise for the code.
- The process rulebook is kept small the same way the model corpus is:
  named invariants with checks, honest deferrals with markers, and
  deletion as a first-class outcome.
- The unreviewable-volume problem is answered by shrinking what must be
  trusted, not by reviewing more: constraints, tests, and recorded
  decisions are the reviewed surface; conformance carries the rest — the
  same bargain the code side already struck in ADR-0002.
