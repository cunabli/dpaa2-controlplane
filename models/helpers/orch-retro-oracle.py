#!/usr/bin/env python3
"""Drive the orchestration retro-oracle (resource-aware-orchestration tasks 2.1 + 2.2).

What: runs every half of the two directed expect-red checks over
`models/orchestration/retro_oracle.qnt` (the variant module) and
`models/orchestration/pipeline.qnt` (the shipped model), and asserts each
half's outcome:

  task 2.1 (mask-a-family, the dpsw-typestate/dpseci gap class, bead
  dpaa2-controlplane-dbf.19, ADR-0023 design D3/D10):
    - the shipped model's `stateInvariants` stays green (baseline),
    - `ORCH_I1_maskNone` stays green (the unmasked restatement), and
    - `ORCH_I1_mask<Family>` goes red for EACH of the nine table rows.

  task 2.2 (handover live-read pair, ADR-0023 design D9):
    - `handoverSafe` over the shipped `step` stays green (live-read in
      place → the switch-wire-over-MacHeld state is unreachable), and
    - `handoverSafe` over `stepCensus` goes red (live-read replaced by the
      census snapshot → the state is reachable).

Why: this is the honesty plumbing the bead requires. A script that merely
inverts an exit code would also pass on a model red for an unrelated reason;
instead every expected-red run is paired with its own baseline that must be
green, and the nine masked reds differ from the green `ORCH_I1_maskNone` by
exactly the one masked family — so a red is provably caused by the mask, not
by a regression. If the shipped baseline (`stateInvariants` or `maskNone` or
the live-read `handoverSafe`) is itself red, the oracle fails loudly here
rather than silently blessing a broken model.

Wired into the ladder as `pnpm model:retro-orch` (in `model:validation`).
Rerunning is pure: it only reads the two models through `quint`.
"""

import subprocess
import sys
from pathlib import Path

SCRIPT = Path(__file__).resolve()
REPO = SCRIPT.parents[2]  # models/helpers/orch-retro-oracle.py -> repo root

PIPELINE = "models/orchestration/pipeline.qnt"
RETRO = "models/orchestration/retro_oracle.qnt"

# Same bound as the orchestration model's `model:simulate` gate.
BOUND = ["--max-steps=20", "--max-samples=200"]

# (label, model, main, invariant, step, expect_green). step=None uses the
# module's default `step`. expect_green True => must report [ok]; False => must
# report [violation].
CHECKS = [
    # task 2.1 baselines (must be green — the loud-failure guard)
    ("shipped stateInvariants (baseline)", PIPELINE, "orchestration",
     "stateInvariants", None, True),
    ("ORCH-I1 unmasked (baseline)", RETRO, "retro_oracle",
     "ORCH_I1_maskNone", None, True),
    # task 2.1 mask-a-family: each row's servicing transition forced dead =>
    # ORCH-I1 red. Every create-bearing row of the full derived vocabulary plus
    # the DPC-born dpmac (obligated by its converged status) — all nine table rows.
    ("ORCH-I1 mask Dprc", RETRO, "retro_oracle", "ORCH_I1_maskDprc", None, False),
    ("ORCH-I1 mask Dpni", RETRO, "retro_oracle", "ORCH_I1_maskDpni", None, False),
    ("ORCH-I1 mask Dpbp", RETRO, "retro_oracle", "ORCH_I1_maskDpbp", None, False),
    ("ORCH-I1 mask Dpio", RETRO, "retro_oracle", "ORCH_I1_maskDpio", None, False),
    ("ORCH-I1 mask Dpcon", RETRO, "retro_oracle", "ORCH_I1_maskDpcon", None, False),
    ("ORCH-I1 mask Dpmcp", RETRO, "retro_oracle", "ORCH_I1_maskDpmcp", None, False),
    ("ORCH-I1 mask Dpsw", RETRO, "retro_oracle", "ORCH_I1_maskDpsw", None, False),
    ("ORCH-I1 mask Dpmac", RETRO, "retro_oracle", "ORCH_I1_maskDpmac", None, False),
    ("ORCH-I1 mask Dpseci", RETRO, "retro_oracle", "ORCH_I1_maskDpseci", None, False),
    # task 2.2 handover pair
    ("ORCH-I2 handover live-read in place (baseline)", RETRO, "retro_oracle",
     "handoverSafe", "step", True),
    ("ORCH-I2 handover live-read removed (census)", RETRO, "retro_oracle",
     "handoverSafe", "stepCensus", False),
    # plan-rollback: the re-apply-without-re-admission pair (ORCH-I3 restated,
    # plan-rollback design D6). The shipped rollback resets the verdict to Unjudged,
    # so a rolled-back run must be re-admitted before it re-dispatches (green); the
    # re-apply variant leaves the verdict Admitted, so ORCH_I3's reverted-implies-
    # Unjudged conjunct goes red the instant doRollbackReapply fires.
    ("ORCH-I3 rollback re-admits (baseline)", RETRO, "retro_oracle",
     "ORCH_I3", "step", True),
    ("ORCH-I3 rollback re-applies w/o re-admission", RETRO, "retro_oracle",
     "ORCH_I3", "stepReapply", False),
]


def run_check(label, model, main, invariant, step, expect_green):
    """Run one `quint run --invariant` and judge its outcome against the
    expectation. Returns True iff the run matched expect_green."""
    cmd = ["pnpm", "exec", "quint", "run", model, f"--main={main}",
           f"--invariant={invariant}", *BOUND]
    if step is not None:
        cmd.append(f"--step={step}")
    proc = subprocess.run(cmd, cwd=REPO, capture_output=True, text=True)
    out = proc.stdout + proc.stderr
    green = "[ok]" in out
    red = "[violation]" in out
    if green == red:  # neither or both: a parse error or a quint failure
        print(f"FAIL  {label}: no clear [ok]/[violation] verdict (quint error?)")
        print(out.strip()[-800:])
        return False
    got = "green" if green else "red"
    want = "green" if expect_green else "red"
    ok = green == expect_green
    print(f"{'ok  ' if ok else 'FAIL'}  {label}: {got} (expected {want})")
    return ok


def main() -> int:
    results = [run_check(*c) for c in CHECKS]
    passed = all(results)
    print(f"\nretro-oracle: {sum(results)}/{len(results)} checks as expected")
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
