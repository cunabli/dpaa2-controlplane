#!/usr/bin/env python3
"""Emit the reference board's inventory as a Quint value (intent-layer task 1.1, gqf.4).

What: the intent compiler takes two inputs, the operator's intent and the
hardware's offer (design D2 of openspec/changes/intent-layer). This script
reads the change-#2 board snapshot (models/board/baselines/reference.json —
every dpmac's `dpmac info` attributes and the container's pool listing) and
writes models/intent/inventory.qnt: one `Inventory` value the model and the
Rust tests compile against, with no board attached.

Why: the inventory is observed, never operator-written — a hand-typed
inventory is a second source of truth the board would contradict. The
attributes and pool counts come from the snapshot; the three policy inputs
(the ADR-0003 safety matrix, the DPL-owned objects of ADR-0001 §4, and the
ADR-0011 ceiling dispositions) are tables below, each with its anchor, so a
reader can see which line is a reading and which is a decision. Rerunning
produces a byte-identical file; task 4.1 points the same reader at a live
census.
"""

import argparse
import json
import re
from pathlib import Path

SCRIPT = Path(__file__).resolve()
REPO = SCRIPT.parents[2]  # models/helpers/intent-inventory.py -> repo root

# --- policy inputs (decisions, not readings) --------------------------------

# Online CPUs of the kernel container: the kernel regime draws one dpio per
# CPU (ADR-0012, "the board runs 16 in the kernel container for 16 cores").
CPUS = 16

# ADR-0003 §3 safety matrix: which dpmacs a plan may never anchor a port on.
RESERVED = {
    3: "ADR-0003 §3: wired to a peer that must never see traffic (total-deny)",
    17: "ADR-0003 §3: management plane (dpni.0), never touched",
}

# DPL-owned objects a plan must never claim (ADR-0001 §4, ADR-0003 §3).
FOREIGN = [("Dpni", 0, "dpl")]

# ADR-0011 ceiling dispositions, one per family the derivation emits
# (intent/types.qnt DERIVED_FAMILIES, in that order). A lambda receives the
# snapshot's `resources` map and returns the Quint expression.
CEILINGS = [
    ("Dprc", lambda r: "Unknown", "ADR-0011: never measured"),
    ("Dpni", lambda r: 'Observed({ n: 18, provenance: "ADR-0011 decision 2 (V-CEIL-1, '
     '2026-08-29): the 18th dpni is refused with every listed pool showing room; '
     'an unlisted resource" })', "ADR-0011 decision 2: an unlisted resource, measured"),
    ("Dpbp", lambda r: f"Counted({r['bp']})",
     "ADR-0011 decision 1: the listed buffer-pool count (bp) is the ceiling"),
    ("Dpio", lambda r: "Unknown",
     "ADR-0011: the swp pool is listed but its gating was never measured — a recorded unknown"),
    ("Dpcon", lambda r: "Unknown", "ADR-0011: 64 created without a refusal (cap ended)"),
    ("Dpmcp", lambda r: f'Observed({{ n: {r["mcp"]}, provenance: "ADR-0011 decision 3 '
     '(V-CEIL-1 rev 2, 2026-08-29): MC portals are a fixed per-boot budget the '
     'listing reports, never returned by destroy" })', "ADR-0011 decision 3: a per-boot budget, listed but never returned"),
    ("Dpseci", lambda r: "Unknown", "ADR-0011: never measured"),
    ("Dpsw", lambda r: "Unknown", "ADR-0011: never measured"),
]

# --- readings -----------------------------------------------------------------

ETH_IF = {
    "DPMAC_ETH_IF_XFI": "XFI",
    "DPMAC_ETH_IF_CAUI": "CAUI",
    "DPMAC_ETH_IF_RGMII": "RGMII",
}
LINK_TYPE = {
    "DPMAC_LINK_TYPE_NONE": "LinkNone",
    "DPMAC_LINK_TYPE_FIXED": "LinkFixed",
    "DPMAC_LINK_TYPE_PHY": "LinkPhy",
    "DPMAC_LINK_TYPE_BACKPLANE": "LinkBackplane",
}
RATE_KEY = re.compile(r"maximum supported rate (\d+) Mbps")


def dpmac_offers(objects: dict) -> list[str]:
    lines = []
    for name in sorted((n for n in objects if n.startswith("dpmac.")),
                       key=lambda n: int(n.split(".")[1])):
        num = int(name.split(".")[1])
        info = objects[name]["info"]
        rates = [m.group(1) for k in info if (m := RATE_KEY.fullmatch(k))]
        if len(rates) != 1:
            raise SystemExit(f"{name}: expected one 'maximum supported rate' key, got {rates}")
        eth_if = ETH_IF.get(info["DPMAC ethernet interface"])
        link_type = LINK_TYPE.get(info["DPMAC link type"])
        if eth_if is None or link_type is None:
            raise SystemExit(f"{name}: unmapped attribute {info['DPMAC ethernet interface']!r} / "
                             f"{info['DPMAC link type']!r} — the alphabet grows only with a board "
                             "that has it (intent/types.qnt)")
        avail = f'Reserved("{RESERVED[num]}")' if num in RESERVED else "Free"
        lines.append(f"      {num} -> {{ id: {num}, maxRate: {rates[0]}, ethIf: {eth_if}, "
                     f"linkType: {link_type}, avail: {avail} }},")
    return lines


def render(snapshot: dict) -> str:
    dpmacs = "\n".join(dpmac_offers(snapshot["objects"]))
    foreign = ", ".join(f'{{ fam: {fam}, num: {num} }} -> "{owner}"' for fam, num, owner in FOREIGN)
    ceilings = "\n".join(f"      {fam} -> {expr(snapshot['resources'])},  // {why}"
                         for fam, expr, why in CEILINGS)
    return f"""// GENERATED by models/helpers/intent-inventory.py from
// models/board/baselines/reference.json — never hand-edited; a rerun is
// byte-identical. The reference board's inventory (intent-layer design D2):
// dpmac attributes as `dpmac info` reports them, availability from the
// ADR-0003 §3 safety matrix, DPL-owned objects per ADR-0001 §4, ceilings per
// ADR-0011 (the pool counts are the snapshot's `--resources` listing).
//
// Named invariants: none.
// Apalache-marked subset: none.
module intent_inventory {{
  import types.* from "../core/types"
  import intent_types.* from "./types"

  pure val REF_INVENTORY: Inventory = {{
    cpus: {CPUS},  // ADR-0012: 16 online CPUs in the kernel container
    dpmacs: Map(
{dpmacs}
    ),
    foreign: Map({foreign}),  // ADR-0001 §4
    ceilings: Map(
{ceilings}
    ),
  }}
}}
"""


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--snapshot", type=Path,
                    default=REPO / "models/board/baselines/reference.json")
    ap.add_argument("--out", type=Path, default=REPO / "models/intent/inventory.qnt")
    args = ap.parse_args()
    snapshot = json.loads(args.snapshot.read_text())
    args.out.write_text(render(snapshot))
    print(f"wrote {args.out.relative_to(REPO)}")


if __name__ == "__main__":
    main()
