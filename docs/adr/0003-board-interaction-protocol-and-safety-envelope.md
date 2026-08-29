# ADR-0003: Board interaction protocol and port safety envelope

- **Status:** Accepted — scoping session 2026-08-22; §8 amended
  2026-08-22 by `verify-foundation` (change #2)
- **Date:** 2026-08-22
- **Supersedes / relates to:** OpenSpec change `restool-baseline` (design
  D4–D5); ADR-0002 (the MBT modes that run under this protocol)

## Context

Model-based testing (ADR-0002) wants to drive the board through many MC
mutations, including states no human would type by hand. The board is not a
lab throwaway: one of its ports is wired to a peer that must never see
traffic, another carries the management plane, and the 10G pair faces a
production peer. An autonomous agent issuing command hierarchies across an
SSH boundary was considered and rejected: it removes the operator's
awareness of what ran, can disrupt beyond what the agent can observe, and
teaches the operator nothing. Safety here must be *coded*, not conventional,
and the operator must stay in the loop by construction.

## Decision

### 1. The assistant never executes on the board

No command generated in a working session crosses an SSH boundary by the
assistant's hand. Everything that reaches the board is a file the operator
reviews and runs.

### 2. Batch suites are reviewable scripts with expectations inline

A batch suite (ADR-0002) is emitted as a script in which every restool
command is visible and the model's expected outcome sits as a comment beside
each step. Results return as files and are diffed offline. Every emitted
script asserts the pinned reference pair (MC 10.39.0 + Linux 6.6.52, see
`docs/baseline/reference-environment.md`) before doing anything else;
evidence is only valid against its stamped pair.

### 3. The online driver is operator-launched and supervised

The online-MBT driver is a program the operator starts, with step / pause /
abort controls and a full transcript of every action and observation.
Granularity is promoted per family: **per-step confirmation** while a family
is being learned; **per-block with abort** once that family's model has
survived a complete batch suite. Each promotion is recorded in the owning
change.

### 4. Port safety envelope — enforced by the harness, not by care

The harness refuses to emit or execute a step that violates this matrix:

| Ports | Class | Rule |
|---|---|---|
| dpmac.3 | total-deny | Wired to a peer that must never see traffic. No connect, no bind, no link-up — nothing that could emit, in any scenario class. |
| dpmac.17 / dpni.0 | total-deny | Management plane; foreign objects, never enumerated or touched (ADR-0001 §4). |
| dpmac.4–6 (25G), dpmac.8/10 (10G) | lifecycle-only | Unwired: link-up can never be asserted, so they safely absorb all object-lifecycle and connect-edge churn. |
| dpmac.7 / dpmac.9 | flagged use only | Wired to a production peer. Link-signaling and traffic-bearing scenarios run here only, each explicitly flagged. |

### 5. Every scenario declares its traffic class

Each validation scenario is classified as exactly one of
**object-lifecycle-only**, **link-signaling**, or **traffic-bearing**
(inventory: `docs/baseline/traffic-inventory.md`). The harness rejects a
trace whose declared class does not match the ports it touches.

### 6. Scratch containers, purely additive, unconditional teardown

All MBT work runs inside scratch child DPRCs created for the run: nothing
outside the scratch container is mutated, and teardown is unconditional —
it runs whether the suite passed, failed, or aborted. A hook may connect,
disconnect, plug, and read while the created objects still stand; it never
destroys or creates an object in the root container. The teardown alone
destroys, once per run and with its destroys spaced, because a hook's own
destroys re-open the bus-rescan window the spacing exists to close
(ADR-0008 §7).

### 7. Recovery guarantee is an assumption until verified

"A reboot restores the DPL baseline" is the backstop under everything above.
It is treated as an explicit *assumption* until `verify-foundation`
(change #2) verifies it — before any mutating suite runs.

### 8. Decision point: Mellanox device-tree revert

Before the first traffic-bearing phase, choose between (a) careful
flagged use of dpmac.7/9 against the production peer, or (b) reverting
the device tree so dpmac.3 lands on the on-board Mellanox — removing the
forbidden external wire and gaining a local, non-production peer that
suites can hammer freely. (b) is preferred on paper but changes board
configuration; it is decided when reached.

**Amended 2026-08-22 — the decision point fired at change #2, not #9.**
`verify-foundation` pulled traffic-bearing scenarios forward, making it
the first traffic-bearing phase. Outcome: option (a) — per-run-flagged
use of dpmac.7/9 — is exercised there, limited to reachability-level
traffic (minimal frames through configured object groups, no rate
targets, nothing restarts or reconfigures the peer). Option (b) is not
foreclosed: it is re-decided at change #9 (`cross-dprc-links`) when
sustained traffic suites arrive. Until then dpmac.3 remains total-deny.

## Consequences

**Positive**

- The dangerous failure mode ("harness runs something no one watched") is
  structurally impossible: files are the only interface to the board.
- Port safety survives model bugs and prompt mistakes alike — it is enforced
  where scripts are generated *and* where they execute.
- The operator learns every family alongside the tool; per-step mode is a
  training loop, not just a safety gate.

**Negative / to watch**

- The operator is in the critical path of every board milestone; the
  roadmap batches suites into shared sittings to contain the cost.
- Per-step confirmation is slow by design while a family is being learned;
  the promotion rule exists so the cost falls as evidence accumulates.

## References

- OpenSpec change `restool-baseline`, `design.md` D4–D5.
- ADR-0002 — batch/online MBT definitions.
- `docs/baseline/traffic-inventory.md` — the scenario inventory this ADR's
  matrix governs.
- `docs/ROADMAP.md` — Mellanox decision point, re-decided at change #9.
- OpenSpec change `verify-foundation`, `design.md` D8 — the early firing
  of the §8 decision point.
