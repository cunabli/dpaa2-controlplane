#!/usr/bin/env python3
"""Fail the ladder when a scenario is unpaired (intent-layer task 1.5, gqf.8).

Enforces the formal-models spec scenario "An unpaired scenario fails the
ladder": every `models/intent/scenarios/<name>.qnt` must sit beside its
`<name>.toml` (the operator's file) and vice versa, so a scenario module and
the intent it claims to express can never drift. Each missing counterpart is
printed as `unpaired scenario: expected <path>` and the script exits 1.

A missing `scenarios/` directory passes silently (task 2.1 creates it); a
fully-paired directory exits 0 silently. The `model:typecheck` rung runs it
after the Quint typechecks so a stray `.qnt` stops the ladder by name.
"""

import sys
from pathlib import Path

SCRIPT = Path(__file__).resolve()
REPO = SCRIPT.parents[2]  # models/helpers/intent-pairing.py -> repo root
DEFAULT_ROOT = REPO / "models/intent/scenarios"


def unpaired(root: Path) -> list[Path]:
    """Every counterpart file a scenario expects but does not have."""
    missing = []
    for qnt in sorted(root.glob("*.qnt")):
        toml = qnt.with_suffix(".toml")
        if not toml.exists():
            missing.append(toml)
    for toml in sorted(root.glob("*.toml")):
        qnt = toml.with_suffix(".qnt")
        if not qnt.exists():
            missing.append(qnt)
    return missing


def main() -> int:
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_ROOT
    if not root.is_dir():  # task 2.1 has not created scenarios/ yet
        return 0
    missing = unpaired(root)
    for path in missing:
        print(f"unpaired scenario: expected {path}")
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
