# Invariant coverage ledger

One row per invariant candidate from the baseline family documents
(`docs/baseline/*.md`). The ledger is the honesty mechanism (design D9
of `openspec/changes/verify-foundation`): a candidate absent from the
model corpus is a decision on record here, never an omission.

Dispositions:

- **modeled** — encoded under its baseline id; the row names the model
  file and the highest CI rung it runs at (`typecheck` / `simulate` /
  `itf-replay` / `apalache`).
- **deferred** — not encoded in this change; the row names the roadmap
  change that owns it.
- **board-pending** — encoded but only the board can settle it; the row
  names the traffic-inventory scenario that settles it. Board results
  fold back into the row as suites complete.

| Candidate | Disposition | Location / owning change / settling scenario | CI rung | Board status |
|-----------|-------------|----------------------------------------------|---------|--------------|

<!-- Filled by task 2.4; board columns settled by phase 5 (tasks 5.1-5.6). -->
