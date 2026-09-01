# ADR-0005: Intent layer — network constructs compile to MC objects

- **Status:** Accepted — scoping session 2026-08-22; elaborated in place by
  ADR-0013 (2026-09-02)
- **Date:** 2026-08-22
- **Supersedes / relates to:** OpenSpec change `restool-baseline` (design D7);
  ADR-0001 §§2–3 (level-triggered reconciliation and DPMAC-anchored identity
  the compiled plan feeds into); ADR-0002 (derivation rules as invariants);
  **elaborated by ADR-0013** (the accepted intent vocabulary)

## Decision, elaborated in ADR-0013

This record fixed the shape of the answer on 2026-08-22: operators declare
**network constructs anchored in hardware** — a tenant with its dataplane, a
port on a DPMAC, a link between tenants, a crypto engine — and never raw MC
objects; a **pure, total derivation compiler** in `dpaa2-api` turns those
constructs into the full MC object plan, codifying the sizing rules
(DPIO ≥ 2·(1+workers), the two-DPBP rule, DPCON per queue, DPMCP companions);
each sizing rule is also a **model invariant** (ADR-0002); `--dry-run` shows
**per-object provenance** and the escape hatch is **visible, never silent**;
and the build order is **intent layer > profiles > raw objects**, with
YANG/gNMI recorded as the revisit trigger if intent expression outgrows this
tool.

Those five decisions (§§1–5 of the original) are carried forward and made
concrete by **[ADR-0013](0013-accepted-intent-vocabulary.md)**, the phase-1
gate artefact of OpenSpec change `intent-layer`: the accepted `[intent]`
schema and its constructs, the two compiler inputs (intent + observed
inventory), the derived quantities and container-tree placement, the complete
refusal vocabulary, the INTENT_I1–I9 invariants, the scenarios as worked
witnesses, the open questions, the honest relaxations, and the revisit
triggers (CEL for extras, the worker-table measurement trigger, YANG/gNMI).
The section references other records make to "ADR-0005 §N" resolve through
ADR-0013.

## References

- ADR-0013 — the accepted intent vocabulary and its consequences.
- OpenSpec change `restool-baseline`, `design.md` D7; ADR-0001 C1 (the
  driver-dependency recipe that motivates rule codification).
- `docs/ROADMAP.md` — change #3 `intent-layer`; the YANG revisit trigger.
