---
name: epic-review
description: Run a multi-pass quality review of a completed epic/change from a per-review brief. Use after every large chunk of work lands (epic close, phase close), before the change is archived. Invoke as /epic-review <change-slug>, e.g. /epic-review intent-layer.
---

# Epic quality review

Executes the review described in a brief file at
`openspec/changes/<slug>/review/brief.md`. The review lives inside the
openspec change it judges, so brief, pass reports, and synthesis archive
together with the change. Run it after the epic closes and BEFORE
`/opsx:archive`. The skill is the mechanism; the brief is the content. Never
start reviewing without a brief — if the slug has none, say so and stop.

## Procedure

1. **Read the brief in full.** It defines the passes, their scope, mandates,
   ordering, report schema, and budgets. Do not re-derive or re-scope.
2. **Workspace is the brief's own directory** `openspec/changes/<slug>/review/`.
   Pass reports and the synthesis land there as files so the review survives
   context compaction and becomes part of the change record.
3. **Dispatch passes in the brief's dependency order.** Independent passes run
   concurrently (one message, multiple Agent calls). Per pass:
   - Use the agent type the brief names. Review agents are read-only
     (Read/Grep/Glob); they must not edit anything.
   - The pass prompt = the brief's pass section verbatim + the report schema
     section + the paths of prerequisite pass reports.
   - Review agents cannot write files: save each returned report to
     `openspec/changes/<slug>/review/pass<N>.md` yourself before dispatching
     dependents.
4. **Synthesis.** When all passes are saved, dispatch the judge agent
   (`change-judge`) with all report paths and the brief's synthesis mandate.
   Save its output as `openspec/changes/<slug>/review/synthesis.md`.
5. **Close-out (main loop, after user reviews the synthesis):**
   - Findings that warrant fixes become beads under a follow-up openspec
     change proposed just-in-time — never fixed inline during the review.
   - A guidelines deliverable (if the brief has one) becomes a parcel that
     edits the named target files and applies the retire list.
   - Report per-pass token spend vs the brief's budget.
   - The review/ directory is committed as its own commit on the reviewed
     change (subject to the session's git profile — propose, don't push),
     then the change archives normally, review included.

## Hard rules

- The review changes no code, docs, or specs. Read-only until the user
  accepts the synthesis.
- A pass that exceeds ~1.5x its budget stops and reports partial rather than
  grinding on.
- Findings without the schema's mandatory fields (especially `superseded-by`
  on staleness claims) are dropped at synthesis, not argued about.

## Writing a new brief

When a change's implementation completes, write
`openspec/changes/<slug>/review/brief.md` before archival. Copy the newest
existing brief (search `openspec/changes/**/review/brief.md`, archived ones
included) as the template. Keep its section
skeleton: Slug & scope → Grounding facts → Passes (scope/mandates/agent/budget
each) → Ordering → Report schema → Synthesis mandate → Budgets. Ground the
new brief's facts (commit range, line counts, amendment trail) before writing
passes; a brief with stale grounding produces false positives.
