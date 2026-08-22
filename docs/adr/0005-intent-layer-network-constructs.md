# ADR-0005: Intent layer — network constructs compile to MC objects

- **Status:** Accepted — scoping session 2026-08-22
- **Date:** 2026-08-22
- **Supersedes / relates to:** OpenSpec change `restool-baseline` (design D7);
  ADR-0001 §§2–3 (level-triggered reconciliation and DPMAC-anchored identity
  the compiled plan feeds into); ADR-0002 (derivation rules as invariants)

## Context

Provisioning DPAA2 correctly means knowing rules no operator should have to
carry: a 4-worker consumer needs DPIOs sized `≥ 2·(1+workers)`, two DPBPs,
one DPCON per queue, DPMCP companions — knowledge that today lives in
`ls-addni`'s shell and in lessons this workspace paid for on the board
(ADR-0001 C1). Asking operators to declare raw MC objects reproduces the
problem restool has; asking them to pick from canned profiles hides the
model without removing it. The operator's real vocabulary is networking:
a runtime that consumes ports, a port on a physical MAC, a link between
consumers, a crypto engine.

## Decision

### 1. Operators declare network constructs, anchored in hardware

The northbound schema (`dpaa2-config`) speaks network constructs —
**consumer/runtime** (e.g. kernel, VPP, with its worker count), **port**
(anchored on a DPMAC, per ADR-0001 §3), **link** (consumer↔consumer
pseudo-wire), **crypto engine** — each backed by something the hardware
actually implements (MC objects, netlink-visible artifacts). Constructs are
not free-floating abstractions: if it cannot be anchored, it is not in the
vocabulary.

### 2. A pure derivation compiler turns intent into the object plan

A pure function in `dpaa2-api` compiles the declared constructs into the
full MC object plan, codifying the sizing rules as code: DPIO ≥
2·(1+workers), the two-DPBP rule, DPCON per queue, DPMCP companions, and
successors as the baseline documents establish them. The compiler is total
and deterministic — same intent, same plan — and sits upstream of the
existing `reconcile(desired, observed)`.

### 3. Derivation rules are model invariants

Each sizing rule is also a Quint invariant (ADR-0002): a derived plan that
violates one cannot pass the model gate. The rules carry their evidence
anchors (the baseline sections and board findings that justify them), so a
rule change is a documented amendment, not a tweak.

### 4. Dry-run shows provenance; overrides exist per object

`--dry-run` prints the derived plan with per-object provenance — which rule,
from which declared construct, produced each object and each size. A
per-object override is the escape hatch for the case the rules do not cover;
an override is visible in the same provenance output, never silent.

### 5. Priority: intent layer > profiles > raw objects

Build order and survivability follow b > c > a: the construct vocabulary and
compiler come first (change #3, `intent-layer`); named profiles are later
sugar over it; raw object-level configuration remains only as the escape
hatch. If intent expression outgrows this tool, adopting an established
intent modeling language (YANG or similar) is the recorded revisit trigger —
the construct vocabulary is designed to survive that translation.

## Consequences

**Positive**

- The paid-for provisioning knowledge lives in one pure, testable, formally
  checked place instead of shell scripts and human memory.
- Operators state what they mean; the derivation explains itself, which
  makes review of a topology change a review of intent, not arithmetic.
- The compiler is board-free by construction — pure function, model-checked.

**Negative / to watch**

- A wrong sizing rule now fails systematically instead of anecdotally; the
  provenance output and evidence anchors exist so a bad rule is traceable
  and correctable in one place.
- The construct vocabulary is a commitment; extending it casually would
  erode the "anchored in hardware" rule. New constructs come through changes
  with baseline backing.

## References

- OpenSpec change `restool-baseline`, `design.md` D7.
- ADR-0001 C1 — the driver-dependency recipe that motivates rule codification.
- `docs/baseline/` — per-family intent-mapping sections; sizing-rule evidence
  anchors in the pool-object documents.
- `docs/ROADMAP.md` — change #3 `intent-layer`; YANG revisit trigger.
