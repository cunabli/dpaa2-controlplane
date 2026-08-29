#!/usr/bin/env python3
"""Emit the MC command-id ioctl policy table (verify-foundation task 6.5, kvl.37).

What: the fsl-mc uapi driver gates every MC command a userspace client sends
through /dev/dprc.N against a fixed whitelist (`fsl_mc_accepted_cmds[]`). This
script reads that whitelist from the reference kernel and, from the restool
flib sources, resolves each restool verb our adapter (crates/dpaa2-mc) drives
to the 16-bit command id it emits, then matches it against the whitelist.

Why: a row that misses the whitelist is refused with -EACCES no matter the
privilege, so those rows are the exact scope of a future kernel patch or of a
VFIO userspace transport. We want them named and kept honest against source.
Rerunning produces a byte-identical file.

The verb -> flib-call trace is a hand-reviewed constant (VERBS below); the
command-id values, the whitelist, and the provenance are all derived from the
trees, so they stay correct when the headers move.
"""

import argparse
import re
import subprocess
from pathlib import Path

SCRIPT = Path(__file__).resolve()
REPO = SCRIPT.parents[2]  # models/helpers/mc-ioctl-policy.py -> repo root
ROOT = REPO.parent.parent

# Every family restool speaks, in whitelist-module order; drives macro loading
# and the flib -> cmdid map. Matches `enum Family` in dpaa2-verify/adapter.rs.
FAMILIES = ("dprc", "dpni", "dpmac", "dpbp", "dpio", "dpcon", "dpmcp", "dpseci",
            "dpsw", "dpdmux", "dpaiop", "dpci", "dpdcei", "dpdmai", "dprtc", "dpdbg")

# Each verb: (key, [flib calls], note, claim). The key is the exact
# "<fam> <verb>" a suite renders (first cell, before any parenthetical) so
# slice B can look it up. flib calls are traced from <obj>_commands.c ->
# cmd_<obj>_<verb> -> flib in mc_v10/<obj>.c. claim: "verified" only when a
# committed suite under models/board/ has actually run the verb (grepped),
# else "read".
V = "verified"
R = "read"

# The dpaa2-mc adapter (crates/dpaa2-mc/src/restool.rs).
ADAPTER = [
    ("(every invocation)", ["mc_get_version", "dprc_open", "dprc_close"],
     "restool startup: dpmng version check, then open/close the root dprc", V),
    ("dprc assign", ["dprc_assign"],
     "plug/unplug (--plugged); the model driver's sibling move (--child) resolves here too", V),
    ("dprc connect", ["dprc_connect"], "wire two endpoints", V),
    ("dprc disconnect", ["dprc_disconnect"], "unwire an endpoint", V),
    ("dprc show", ["dprc_get_obj_count", "dprc_get_obj"],
     "enumerate the container (show_mc_objects path)", V),
    ("dprc sync", [], "no MC command; writes /sys/bus/fsl-mc/rescan", V),
    ("dpni create", ["dpni_create_v10"], "", V),
    ("dpni info", ["dpni_open_v10", "dpni_get_attributes_v10", "dpni_get_api_version_v10",
                   "dpni_get_primary_mac_addr_v10", "dpni_get_link_state_v10",
                   "dpni_get_statistics_v10", "dpni_get_max_frame_length",
                   "dprc_get_connection", "dpni_close_v10"], "", V),
    ("dpni update", ["dpni_open_v10", "dpni_get_attributes_v10",
                     "dpni_set_primary_mac_addr_v10", "dpni_close_v10"],
     "adapter set_mac and the model driver's PreplugMutate both pass only --mac-addr", V),
    ("dpni destroy", ["dpni_destroy_v10"], "", V),
    ("dpbp create", ["dpbp_create_v10"], "", V),
    ("dpbp destroy", ["dpbp_destroy_v10"], "rollback / <kind> destroy", V),
    ("dpio create", ["dpio_create_v10"], "", V),
    ("dpio destroy", ["dpio_destroy_v10"], "rollback / <kind> destroy", V),
    ("dpcon create", ["dpcon_create_v10"], "", V),
    ("dpcon destroy", ["dpcon_destroy_v10"], "rollback / <kind> destroy", V),
    ("dpmcp create", ["dpmcp_create_v10"], "", V),
    ("dpmcp destroy", ["dpmcp_destroy_v10"], "rollback / <kind> destroy", V),
    ("dpmac info", ["dpmac_open_v10", "dpmac_get_attributes_v10", "dpmac_get_mac_addr_v10",
                    "dpmac_get_api_version_v10", "dpmac_close_v10"], "", V),
]


def info_calls(fam, extra=()):
    """The regular <fam> info flib set: open, attributes, api-version, [family
    gets], close — all _v10 on this v10 board."""
    return ([f"{fam}_open_v10", f"{fam}_get_attributes_v10", f"{fam}_get_api_version_v10"]
            + list(extra) + [f"{fam}_close_v10"])


# The model driver and suite harness (crates/dpaa2-verify: adapter.rs drive_with
# /readback, generate.rs preamble/postamble). Rows §2's adapter block lacks.
HARNESS = [
    # model driver (drive_with) verbs beyond the adapter's
    ("dprc create", ["dprc_create_container"], "CreateContainer", V),
    ("dprc unassign", ["dprc_unassign"], "AssignChild pulled one hop up", V),
    ("dprc set-locked", ["dprc_set_locked"], "SetLocked", V),
    ("dprc destroy", ["dprc_destroy_container"], "Destroy of a dprc", V),
    ("dpdbg create", ["dpdbg_create_v10"], "no args; pins container/id itself", R),
    ("dpdbg destroy", ["dpdbg_destroy_v10"], "no args", R),
    ("dpci create", ["dpci_create_v10"], "", V),
    ("dpci destroy", ["dpci_destroy_v10"], "", V),
    ("dpdmux create", ["dpdmux_create_v10"], "", V),
    ("dpdmux destroy", ["dpdmux_destroy_v10"], "", V),
    ("dpdmai create", ["dpdmai_create_v10"], "", V),
    ("dpdmai destroy", ["dpdmai_destroy_v10"], "", V),
    ("dpdcei create", ["dpdcei_create_v10"], "", V),
    ("dpdcei destroy", ["dpdcei_destroy_v10"], "", V),
    ("dpsw create", ["dpsw_create_v10"], "", V),
    ("dpsw destroy", ["dpsw_destroy_v10"], "", V),
    ("dpseci create", ["dpseci_create_v10"], "", V),
    ("dpseci destroy", ["dpseci_destroy_v10"], "", V),
    ("dpmac create", ["dpmac_create_v10"], "restool supports it; models never create dpmacs", R),
    ("dpmac destroy", ["dpmac_destroy_v10"], "", R),
    ("dpaiop create", ["dpaiop_create_v10"], "", R),
    ("dpaiop destroy", ["dpaiop_destroy_v10"], "", R),
    ("dprtc create", ["dprtc_create_v10"], "", R),
    ("dprtc destroy", ["dprtc_destroy_v10"], "", R),
    # suite harness (generate.rs preamble/postamble, readback) reads
    ("restool -m", ["mc_get_version"],
     "reference-pair assert; reuses the startup version query, opens no dprc", V),
    ("dprc list", ["dprc_get_obj_count", "dprc_get_obj",
                   "dprc_get_res_count", "dprc_get_res_ids"], "preamble tree walk", V),
    ("dprc generate-dpl", ["dprc_get_obj_count", "dprc_get_connection", "dprc_get_attributes",
                           "dpni_open_v10", "dpni_get_attributes_v10",
                           "dpni_get_primary_mac_addr_v10", "dpni_close_v10",
                           "dpmac_open_v10", "dpmac_get_attributes_v10", "dpmac_close_v10",
                           "dpbp_open_v10", "dpbp_get_attributes_v10", "dpbp_close_v10",
                           "dpio_open_v10", "dpio_get_attributes_v10", "dpio_close_v10",
                           "dpcon_open_v10", "dpcon_get_attributes_v10", "dpcon_close_v10",
                           "dpmcp_open_v10", "dpmcp_get_attributes_v10", "dpmcp_close_v10",
                           "dpci_open_v10", "dpci_get_attributes_v10",
                           "dpci_get_peer_attributes_v10", "dpci_close_v10",
                           "dpseci_open_v10", "dpseci_get_attributes_v10",
                           "dpseci_get_tx_queue_v10", "dpseci_close_v10",
                           "dpsw_open_v10", "dpsw_get_attributes_v10", "dpsw_close_v10",
                           "dpdmux_open_v10", "dpdmux_get_attributes_v10", "dpdmux_close_v10",
                           "dpdcei_open_v10", "dpdcei_get_attributes_v10", "dpdcei_close_v10",
                           "dpdmai_open_v10", "dpdmai_get_attributes_v10", "dpdmai_close_v10"],
     "read-only DPL walk over every object family; distinct whitelist entries shown", V),
    ("dprc info", ["dprc_get_attributes"],
     "hand-authored hooks, not drive_with/readback (which use dprc show); a run verb", V),
    ("dpbp info", info_calls("dpbp"), "", V),
    ("dpci info", info_calls("dpci", ["dpci_get_link_state_v10", "dpci_get_peer_attributes_v10"]),
     "renders link status", V),
    ("dpdcei info", info_calls("dpdcei"), "", V),
    ("dpdmai info", info_calls("dpdmai"), "", V),
    ("dpdmux info", info_calls("dpdmux"), "per-interface blocks", V),
    ("dpsw info", info_calls("dpsw"), "per-interface blocks", V),
    ("dpio info", info_calls("dpio"), "postboot absence check only", R),
    ("dpcon info", info_calls("dpcon"), "postboot absence check only", R),
    ("dpmcp info", info_calls("dpmcp"), "postboot absence check only", R),
    ("dpseci info", info_calls("dpseci", ["dpseci_get_tx_queue_v10"]),
     "postboot absence check only", R),
    ("dpaiop info", info_calls("dpaiop", ["dpaiop_get_sl_version_v10", "dpaiop_get_state_v10"]),
     "not created by committed suites", R),
    ("dprtc info", info_calls("dprtc"), "not created by committed suites", R),
]

SUMMARIZE = {"dprc generate-dpl"}  # show distinct matched entries, not every call

# Two raw MC commands no restool verb emits, resolved from the kernel dpaa2
# driver headers (restool has no verb, so its own headers omit them).
PROBES = [
    ("DPNI set-tx-confirmation-mode (probe V-DPNI-4)",
     "DPNI_CMDID_SET_TX_CONFIRMATION_MODE", "dpni-cmd.h"),
    ("DPMAC set-link-state (probe V-LINK-3)",
     "DPMAC_CMDID_SET_LINK_STATE", "dpmac-cmd.h"),
]

FLAG_BITS = {1: "CHECK_MODULE_ID", 2: "CAP_NET_ADMIN"}

HEADER = """<!-- generated by models/helpers/mc-ioctl-policy.py (writes this file and models/core/ioctl_policy.qnt); do not edit by hand. -->
# MC command-id ioctl policy

The fsl-mc uapi driver checks every MC command a client sends through
`/dev/dprc.N` against a fixed whitelist before forwarding it to the MC
firmware; a command not on the list is refused with `-EACCES`
(`fsl_mc_command_check`), regardless of privilege. This table reads that
whitelist and resolves each restool verb the adapter drives to the 16-bit
command id it emits, so the rows outside the whitelist are the exact scope
of a kernel patch or of a VFIO userspace transport (task 6.5).

Claim markers as in every baseline document: **[read]** = derived from
source, **[verified]** = the command has crossed this whitelist on the
board (restool goes through the same ioctl path the adapter will).

## Provenance

- kernel: `{kdesc}`, HEAD `{khead}`, `fsl-mc-uapi.c` at `{kfile}`
- restool: tag `{tag}`, commit `{rcommit}`
- match rule: `cmdid & mask == value`, first entry wins;
  `CHECK_MODULE_ID` also needs `(cmdid >> 4) & 0x3f` in `1..=0x10`.

## 1. Whitelist (`fsl_mc_accepted_cmds[]`)

| # | name | value | mask | size | token | flags |
|---|---|---|---|---|---|---|"""

SEC2 = """
## 2. Verb resolution

The key (first cell, before any parenthetical) is the exact `<fam> <verb>` a
suite renders. Every invocation also pays the `(every invocation)` row first.
`dprc sync` issues no MC command at all: it writes the sysfs bus rescan node.
`claim`: **[verified]** = a committed suite under `models/board/` has run the
verb; **[read]** = derived from source only."""

TABLE_HEAD = ("\n| verb | flib calls | cmdid symbols = values | "
              "matched whitelist entry | verdict | claim |\n|---|---|---|---|---|---|")

SEC3 = """
## 3. Outside the whitelist (kernel patch or VFIO transport scope)

| command | cmdid symbol = value | whitelist entry | verdict | claim |
|---|---|---|---|---|"""

FOOTER = """
No adapter, model-driver, or harness verb resolves outside the whitelist:
every command they emit is accepted (some need `CAP_NET_ADMIN`, which restool
holds as root). The two rows above are raw probes that no restool verb emits;
reaching them needs a kernel patch to `fsl_mc_accepted_cmds[]` or a VFIO
userspace transport that bypasses the uapi whitelist.

## 4. Unresolved
"""

# The same policy as a Quint module, so the model (simulator + apalache) owns
# the whitelist and the Rust harness is traced against it through ITF rather
# than owning the mapping. Quint has no bitwise ops; both kernel masks are
# nibble-aligned, so each is expressed as a divisor (0xfff0 -> 16,
# 0xfc00 -> 1024) and `(cmdid & mask) == value` becomes `cmdid / div ==
# value / div`. Provenance mirrors the markdown so both files move together.
QNT_HEADER = """// generated by models/helpers/mc-ioctl-policy.py; do not edit by hand.
// MC command-id ioctl policy as a Quint module (verify-foundation task 6.5).
//
// The fsl-mc uapi driver checks every MC command a client sends through
// /dev/dprc.N against this fixed whitelist before forwarding it to the MC
// firmware; a command not on the list is refused with -EACCES, regardless of
// privilege. Quint has no bitwise ops and both kernel masks are nibble
// aligned, so each is a divisor (0xfff0 -> 16, 0xfc00 -> 1024): the kernel's
// (cmdid & mask) == value is written here as cmdid / div == value / div.
//
// Provenance:
// - kernel: @@KDESC@@, HEAD @@KHEAD@@, fsl-mc-uapi.c at @@KFILE@@
// - restool: tag @@TAG@@, commit @@RCOMMIT@@
// - match rule: cmdid / div == value / div, first entry wins; CHECK_MODULE_ID
//   also needs (cmdid / 16) % 64 in 1..=16.
//
// Named invariants: none here — VERB_OK is consumed by machine.qnt
// (lastVerbs) and main.qnt (IOCTL_OK / the DPNI_I11 id). TLA+ escalation: none.
module ioctl_policy {
  // Section 1 whitelist (fsl_mc_accepted_cmds[]), in kernel order; the first
  // row that admits a command id wins, exactly as fsl_mc_command_check scans.
  pure val WHITELIST: List[{ name: str, value: int, div: int, checkModule: bool, cap: bool }] = [
@@WHITELIST@@
  ]

  // fsl_mc_command_check for one row: the masked equality as a division, plus
  // the CHECK_MODULE_ID module-field range test where the row asks for it.
  pure def rowMatches(r: { name: str, value: int, div: int, checkModule: bool, cap: bool },
                      cmdid: int): bool = and {
    cmdid / r.div == r.value / r.div,
    r.checkModule implies (1 <= (cmdid / 16) % 64 and (cmdid / 16) % 64 <= 16),
  }

  // Index of the first whitelist row that admits cmdid, or -1 if none does.
  pure def matchOf(cmdid: int): int =
    WHITELIST.foldl({ idx: 0, found: -1 }, (acc, r) =>
      { idx: acc.idx + 1,
        found: if (acc.found >= 0) acc.found
               else if (rowMatches(r, cmdid)) acc.idx
               else -1 }).found

  // Whether the kernel forwards cmdid (a whitelist row admits it) rather than
  // refusing it with -EACCES.
  pure def accepted(cmdid: int): bool = matchOf(cmdid) >= 0

  // Whether the admitting row additionally gates cmdid on CAP_NET_ADMIN
  // (restool holds it as root); false for refused or ungated commands.
  pure def needsCap(cmdid: int): bool =
    val i = matchOf(cmdid)
    and { i >= 0, WHITELIST.nth(i).cap }

  // Section 2 verb resolution: every restool verb key the adapter, the model
  // driver and the suite harness render, mapped to the command ids its row
  // emits (empty set = no MC command, e.g. `dprc sync`).
  pure val VERB_CMDIDS: str -> Set[int] = Map(
@@VERBS@@
  )

  // Whether every command a verb emits is accepted — the string channel the
  // machine's `lastVerbs` reads (verbOkAgreesWithAcceptedTest ties it to the
  // proven `accepted` rule, so Apalache never has to fold the whitelist).
  pure val VERB_OK: str -> bool = Map(
@@VERBOK@@
  )

  // Section 3 raw probes no restool verb emits — outside the whitelist by
  // construction, so the ioctl path cannot carry them (kernel patch or VFIO).
  pure val PROBES: str -> int = Map(
@@PROBES@@
  )

  // Directed checks (quint test): every command a verb emits is accepted,
  // VERB_OK agrees with that proven rule for every key, and every raw probe
  // is refused — the model's own statement of the transport.
  run everyVerbAcceptedTest =
    assert(VERB_CMDIDS.keys().forall(k => VERB_CMDIDS.get(k).forall(c => accepted(c))))

  run verbOkAgreesWithAcceptedTest =
    assert(VERB_CMDIDS.keys().forall(k =>
      VERB_OK.get(k) == VERB_CMDIDS.get(k).forall(c => accepted(c))))

  run everyProbeRefusedTest =
    assert(PROBES.keys().forall(k => not(accepted(PROBES.get(k)))))
}
"""


def hexset(ids):
    """Render a list of cmdid ints as a Quint `Set(...)` of hex literals."""
    if not ids:
        return "Set()"
    return "Set(" + ", ".join(f"0x{v:04x}" for v in ids) + ")"


def write_qnt(out, prov, entries, verb_cmdids, verb_ok, probe_map):
    """Emit the policy as the Quint module `ioctl_policy`, byte-identical on
    rerun. `div` renders each nibble-aligned kernel mask as a divisor."""
    rows = []
    for e in entries:
        div = (0xFFFF ^ e["mask"]) + 1  # 0xfff0 -> 16, 0xfc00 -> 1024
        rows.append(
            f'    {{ name: "{e["name"]}", value: 0x{e["value"]:04x}, div: {div}, '
            f'checkModule: {str(bool(e["flags"] & 1)).lower()}, '
            f'cap: {str(bool(e["flags"] & 2)).lower()} }}')
    verbs = ",\n".join(f'    "{verb}" -> {hexset(ids)}'
                       for verb, ids in verb_cmdids.items())
    verbok = ",\n".join(f'    "{verb}" -> {str(ok).lower()}'
                        for verb, ok in verb_ok.items())
    probes = ",\n".join(f'    "{vid}" -> 0x{val:04x}'
                        for vid, val in probe_map.items())
    text = (QNT_HEADER
            .replace("@@WHITELIST@@", ",\n".join(rows))
            .replace("@@VERBS@@", verbs)
            .replace("@@VERBOK@@", verbok)
            .replace("@@PROBES@@", probes)
            .replace("@@KDESC@@", prov["kdesc"])
            .replace("@@KHEAD@@", prov["khead"])
            .replace("@@KFILE@@", prov["kfile"])
            .replace("@@TAG@@", prov["tag"])
            .replace("@@RCOMMIT@@", prov["rcommit"]))
    out.write_text(text)
    print(f"wrote {out}")


def git(tree, *args):
    return subprocess.run(["git", "-C", str(tree), *args],
                          capture_output=True, text=True, check=True).stdout.strip()


def parse_whitelist(src):
    order = re.search(r"enum fsl_mc_cmd_index\s*\{(.*?)\}", src, re.S).group(1)
    names = [t.split("=")[0].strip() for t in order.split(",") if t.strip()]
    flagmap = {"FSL_MC_CHECK_MODULE_ID": 1, "FSL_MC_CAP_NET_ADMIN_NEEDED": 2}
    entries = []
    for name in names:
        body = re.search(r"\[" + name + r"\]\s*=\s*\{(.*?)\}", src, re.S).group(1)
        flags = 0
        fm = re.search(r"\.flags\s*=\s*([^,]+)", body)
        if fm:
            flags = sum(b for w, b in flagmap.items() if w in fm.group(1))
        entries.append(dict(
            name=name,
            value=int(re.search(r"\.cmdid_value\s*=\s*(0x[0-9A-Fa-f]+)", body).group(1), 16),
            mask=int(re.search(r"\.cmdid_mask\s*=\s*(0x[0-9A-Fa-f]+)", body).group(1), 16),
            size=int(re.search(r"\.size\s*=\s*(\d+)", body).group(1)),
            token="true" in re.search(r"\.token\s*=\s*(\w+)", body).group(1),
            flags=flags))
    return entries


def match(entries, cmdid):
    """Replicate fsl_mc_command_check: first entry whose (cmdid & mask) == value,
    then the CHECK_MODULE_ID range test. Returns (entry_or_None, refused_bool)."""
    for e in entries:
        if (cmdid & e["mask"]) == e["value"]:
            if e["flags"] & 1 and not 1 <= ((cmdid >> 4) & 0x3F) <= 0x10:
                return e, True
            return e, False
    return None, True


def load_macros(*texts):
    """Split #defines into object-like and function-like tables."""
    obj, fn = {}, {}
    for text in texts:
        text = text.replace("\\\n", " ")  # join line-continued #defines
        for line in text.splitlines():
            m = re.match(r"\s*#define\s+(\w+)\((\w+)\)\s+(.+)", line)
            if m:
                fn[m.group(1)] = (m.group(2), m.group(3).strip())
                continue
            m = re.match(r"\s*#define\s+(\w+)\s+(.+)", line)
            if m:
                obj[m.group(1)] = m.group(2).strip()
    return obj, fn


def evaluate(symbol, obj, fn):
    """Expand a cmdid symbol to its 16-bit value the way the DPXX_CMD*() macros do."""
    expr = obj.get(symbol, symbol)
    for _ in range(30):
        m = re.search(r"\b(\w+)\s*\(\s*(0x[0-9A-Fa-f]+|\d+)\s*\)", expr)
        if m and m.group(1) in fn:
            param, body = fn[m.group(1)]
            sub = re.sub(r"\b" + param + r"\b", m.group(2), body)
            expr = expr[:m.start()] + "(" + sub + ")" + expr[m.end():]
            continue
        m = re.search(r"\b([A-Za-z_]\w*)\b", expr)
        if m and m.group(1) in obj:
            expr = expr[:m.start()] + str(obj[m.group(1)]) + expr[m.end():]
            continue
        break
    return eval(expr, {"__builtins__": {}}) & 0xFFFF  # noqa: S307 - arithmetic only


def fn_to_cmdid(texts):
    """flib function name -> the cmdid symbol its first mc_encode_cmd_header uses."""
    out = {}
    for src in texts:
        for m in re.finditer(r"\n[A-Za-z_][\w ]*?\b(\w+)\([^;{]*\)\s*\{", src):
            name, start = m.group(1), m.end()
            end = src.find("\n}", start)
            enc = re.search(r"mc_encode_cmd_header\(\s*([A-Z0-9_]+)", src[start:end])
            if enc and name not in out:
                out[name] = enc.group(1)
    return out


def verdict(entry, refused):
    if refused or entry is None:
        return "refused EACCES"
    return "accepted, CAP_NET_ADMIN" if entry["flags"] & 2 else "accepted"


def flags_words(f):
    return ", ".join(FLAG_BITS[b] for b in (1, 2) if f & b) or "-"


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--kernel", type=Path, default=ROOT / ".build/src/linux")
    ap.add_argument("--restool", type=Path, default=ROOT / "src/restool")
    ap.add_argument("--tag", default="lf-6.6.52-2.2.0")
    ap.add_argument("--out", type=Path, default=REPO / "docs/baseline/mc-ioctl-policy.md")
    a = ap.parse_args()

    def show(path):
        return subprocess.run(["git", "-C", str(a.restool), "show", f"{a.tag}:{path}"],
                              capture_output=True, text=True, check=True).stdout

    entries = parse_whitelist((a.kernel / "drivers/bus/fsl-mc/fsl-mc-uapi.c").read_text())
    tag_commit = git(a.restool, "rev-list", "-n1", a.tag)

    objs = (*FAMILIES, "dpmng")
    obj, fn = load_macros(*[show(f"mc_v10/fsl_{o}_cmd.h") for o in objs],
                          show("mc_v10/fsl_mc_cmd.h"))
    fmap = fn_to_cmdid([show(f"mc_v10/{o}.c") for o in objs])
    unresolved = []

    kdir = a.kernel / "drivers/net/ethernet/freescale/dpaa2"
    kobj, kfn = load_macros((kdir / "dpni-cmd.h").read_text(),
                            (kdir / "dpmac-cmd.h").read_text())

    prov = dict(
        kdesc=git(a.kernel, "describe", "--tags"),
        khead=git(a.kernel, "rev-parse", "HEAD"),
        kfile=git(a.kernel, "log", "-1", "--format=%H", "--",
                  "drivers/bus/fsl-mc/fsl-mc-uapi.c"),
        tag=a.tag, rcommit=tag_commit)
    L = [HEADER.format(**prov)]
    for i, e in enumerate(entries):
        L.append(f"| {i} | {e['name']} | {e['value']:#06x} | {e['mask']:#06x} | "
                 f"{e['size']} | {str(e['token']).lower()} | {flags_words(e['flags'])} |")
    def row(verb, calls, note, claim):
        if not calls:
            L.append(f"| {verb} ({note}) | (none) | (sysfs `rescan`, no ioctl) | - | "
                     f"not an MC command | **[{claim}]** |")
            return
        syms, matched, verds = [], [], []
        for c in calls:
            sym = fmap.get(c)
            if sym is None:
                unresolved.append(f"{verb}: {c}")
                continue
            val = evaluate(sym, obj, fn)
            e, refused = match(entries, val)
            syms.append(f"{c} = {sym} = {val:#06x}")
            matched.append(e["name"] if e else "(none)")
            verds.append(verdict(e, refused))
        agg = ("refused EACCES" if "refused EACCES" in verds
               else "accepted, CAP_NET_ADMIN" if "accepted, CAP_NET_ADMIN" in verds
               else "accepted")
        tail = f" ({note})" if note else ""
        if verb in SUMMARIZE:
            # keep the cell readable: distinct matched entries, not every call
            seen = list(dict.fromkeys(matched))
            calls_cell = f"{len(calls)} read calls (open/get_attr/get/close per family)"
            L.append(f"| {verb}{tail} | {calls_cell} | (per-family OPEN/GET_ATTR/"
                     f"GET_API_VERSION/CLOSE + family gets) | {'<br>'.join(seen)} | "
                     f"{agg} | **[{claim}]** |")
            return
        L.append(f"| {verb}{tail} | {'<br>'.join(calls)} | {'<br>'.join(syms)} | "
                 f"{'<br>'.join(matched)} | {agg} | **[{claim}]** |")

    L.append(SEC2)
    L.append("\n### 2a. Adapter (`dpaa2-mc`)\n")
    L.append("The southbound restool shim's own verbs.")
    L.append(TABLE_HEAD)
    for verb, calls, note, claim in ADAPTER:
        row(verb, calls, note, claim)
    L.append("\n### 2b. Model driver and suite harness (`dpaa2-verify`)\n")
    L.append("Verbs the model driver (`adapter.rs` `drive_with`/`readback`) and the "
             "suite preamble/postamble (`generate.rs`) render beyond the adapter's.")
    L.append(TABLE_HEAD)
    for verb, calls, note, claim in HARNESS:
        row(verb, calls, note, claim)
    L.append(SEC3)
    for label, sym, hdr in PROBES:
        val = evaluate(sym, kobj, kfn)
        e, refused = match(entries, val)
        L.append(f"| {label} | {sym} = {val:#06x} ({hdr}) | (none) | "
                 f"{verdict(e, refused)} | **[read]** |")
    L.append(FOOTER)
    if unresolved:
        L.append("These traced calls did not resolve to a cmdid; investigate:\n")
        L.extend(f"- {u}" for u in unresolved)
        L.append("")
    else:
        L.append("None. Every traced flib call resolved to a cmdid and a whitelist verdict.")
        L.append("")

    a.out.write_text("\n".join(L))
    print(f"wrote {a.out}")

    # The Quint mirror: the same whitelist and the same verb -> cmdid resolution
    # as data, so the model owns the policy. Keys are the §2 verb keys verbatim.
    def resolve_ids(calls):
        ids = []
        for c in calls:
            sym = fmap.get(c)
            if sym is None:
                continue
            val = evaluate(sym, obj, fn)
            if val not in ids:
                ids.append(val)
        return ids

    verb_cmdids = {verb: resolve_ids(calls) for verb, calls, _, _ in ADAPTER + HARNESS}
    # VERB_OK: a verb is ok unless any command it emits is refused (no ids ->
    # ok). This is the string channel the machine reads, kept honest against
    # the whitelist rule by verbOkAgreesWithAcceptedTest in the module.
    verb_ok = {verb: all(not match(entries, v)[1] for v in ids)
               for verb, ids in verb_cmdids.items()}
    probe_map = {}
    for label, sym, _ in PROBES:
        vid = re.search(r"probe (V-[\w-]+)", label).group(1)
        probe_map[vid] = evaluate(sym, kobj, kfn)
    write_qnt(REPO / "models/core/ioctl_policy.qnt", prov, entries,
              verb_cmdids, verb_ok, probe_map)


if __name__ == "__main__":
    main()
