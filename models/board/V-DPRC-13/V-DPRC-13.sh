#!/bin/sh
# suite: V-DPRC-13
# class: hand-authored, operator-run — read-only census, no generator, no plan.
# purpose: OI-3 / PASS3-F13-OQ (dprc-hardening task 2.1). The prune path's
# observation seam spawns restool 1+2N times and 4x per `ensure`
# (crates/dpaa2-tools observe_containers; review synthesis M12/OI-3). The open
# question: does each restool spawn draw a portal from the never-returned
# per-boot dpmcp budget — turning that scan from latency into a resource leak?
# This sitting censuses the MC-global `mcp` pool free count across a run of
# trivial, read-only restool spawns and records the trend for offline reading.
# The script OBSERVES; it does not judge (mirrors V-DPDBG-2's record ethos).
#
# OI-1 (the duplicate-id-under-lock ordering, PASS2-F5) is NOT probed here: a
# duplicate id is not constructible through restool — no create verb pins an
# object id, and ids mint lowest-free in one global namespace per family
# (ADR-0010; docs/baseline/object-model.md sec on id reuse), so two live
# objects can never share an id and the MC's duplicate check is never reached.
# It was adjudicated off-board (quint directed evidence + a note on
# ADR-0002); see models/board/README.md's V-DPRC-13 ledger row.
#
# SAFETY: this sitting creates and destroys NOTHING. It issues only read-only
# restool queries (`restool -m`, `dprc show`) and reads sysfs. There is no
# scratch child, no lock, and therefore no teardown of objects — so the
# ADR-0011 one-portal-per-container-create/destroy confound (mcp 203 -> 202,
# V-DPRC-9/10/12) cannot arise: the baseline is the clean boot pool and any
# per-spawn `mcp` movement is attributable to the restool spawn alone.
#   usage: sudo sh models/board/V-DPRC-13/V-DPRC-13.sh <results-dir>
# No repo build artifact is needed: plain restool + sysfs, POSIX sh, no
# project binary. restool queries are unprivileged once /dev/dprc.1 opens,
# but the reference envelope (ADR-0003) runs the sitting as root regardless.
set -u
RESULTS="${1:?usage: $0 <results-dir>}"
mkdir -p "$RESULTS"

# Repo root is derived from this script's own path so the sitting never
# depends on the caller's cwd or on PATH-under-sudo (ADR-0003 §4). The script
# lives at models/board/V-DPRC-13/, three levels below the repo root.
CDPATH=''
REPO="$(cd -- "$(dirname -- "$0")/../../.." && pwd)"

# --- kernel-log window (ADR-0008) ---
# A marker stamps the sitting's start in the kernel log; the teardown saves
# everything after it to dmesg.txt, so any portal/allocator activity is a
# file, not operator memory.
KMSG="dpaa2-verify V-DPRC-13 pid $$"
echo "$KMSG start" > /dev/kmsg 2>/dev/null || true
save_dmesg() {
  dmesg 2>/dev/null | awk -v m="$KMSG start" 'w || index($0, m) { w = 1 } w' > "$RESULTS/dmesg.txt"
  [ -s "$RESULTS/dmesg.txt" ] || dmesg > "$RESULTS/dmesg.txt" 2>&1 || true
}

# --- independent safety self-check (ADR-0003 §4) ---
# The execution side refuses total-deny references even if a script was
# hand-edited after authoring. (This sitting names no object at all, but the
# guard is the standing envelope every board script carries.)
if grep -nE 'dpmac[.]3([^0-9]|$)|dpmac[.]17([^0-9]|$)|dpni[.]0([^0-9]|$)' "$0" | grep -v safety-self-check; then
  echo "refusing: total-deny object referenced in this script" >&2  # safety-self-check
  exit 1
fi

# --- reference pair assertion (ADR-0003 §2) ---
# Evidence is only valid against the stamped pair; refuse anything else. This
# `restool -m` is the sitting's first restool spawn — a setup spawn whose
# effect (if any) is already folded into the baseline census below, so the
# discriminator stays the marginal delta over the trivial spawns.
mc="$(restool -m 2>/dev/null || true)"
case "$mc" in *10.39.0*) ;; *) echo "refusing: MC firmware is not 10.39.0: $mc" >&2; exit 1 ;; esac
kernel="$(uname -r)"
case "$kernel" in 6.6.52*) ;; *) echo "refusing: kernel is not 6.6.52: $kernel" >&2; exit 1 ;; esac

# --- provenance (ADR-0003 §2) ---
# Pin the sitting to its inputs: the repo revision, the restool/MC version,
# the kernel and machine, and the wall clock. git is best-effort — the board
# copy may be a checkout or an export.
{
  echo "suite: V-DPRC-13"
  echo "date: $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
  echo "git-rev: $(git -C "$REPO" rev-parse HEAD 2>/dev/null || echo unknown)"
  echo "restool -m: $mc"
  echo "uname -a: $(uname -a)"
} > "$RESULTS/provenance.txt"

# --- unconditional teardown (ADR-0003 §6) ---
# Nothing was created, so there is nothing to destroy: the trap only closes
# the kernel-log window and prints the census tally. This is the whole reason
# the OI-3 probe is safe to leave standing between operator windows.
teardown() {
  save_dmesg
  echo
  echo "V-DPRC-13 OI-3 dpmcp census complete: $(ls "$RESULTS"/census-*.txt 2>/dev/null | wc -l) censuses recorded"
  echo "mcp free-count trend (raw files in the results dir):"
  grep -H . "$RESULTS"/census-*.mcp 2>/dev/null || echo "  (no mcp line captured — inspect census-*.txt raw)"
}
trap teardown EXIT

# --- census helper ---
# The dpmcp *budget* is the root portal pool ("mcp"), visible only through
# `restool dprc show mc.global --resources` (the same face V-DPDBG-2's
# pool_capture and V-DPRC-12's pool census read; V-DPRC-4 recorded mcp 203
# there). A /dev/dprc.N opener draws a DPMCP from this pool
# (docs/baseline/dprc.md, "Every additional concurrent opener consumes a
# DPMCP from the root pool"; fsl-mc-uapi.c:477-522) — so the pool free count,
# not the sysfs dpmcp *device* count, is the instrument OI-3 needs.
# census N LABEL: capture the full --resources output raw, and extract the
# mcp line to a sidecar for the trend tally. Never judged in-script.
census() {
  n="$1"; label="$2"
  echo "+ (census $n: $label) restool dprc show mc.global --resources"
  restool dprc show mc.global --resources > "$RESULTS/census-$n.txt" 2>&1 || true
  grep -iE '(^|[^a-z])mcp([^a-z]|$)' "$RESULTS/census-$n.txt" > "$RESULTS/census-$n.mcp" 2>/dev/null || true
  echo "census $n ($label): $(cat "$RESULTS/census-$n.mcp" 2>/dev/null)"
}
# trivial N: one trivial, read-only restool spawn — the exact shape of a
# prune-path observation spawn (a `dprc show`), the thing OI-3 asks about.
trivial() {
  n="$1"
  echo "+ (trivial spawn $n) restool dprc show dprc.1"
  restool dprc show dprc.1 > "$RESULTS/trivial-$n.txt" 2>&1 || true
}

# --- OI-3 probe: does a trivial restool spawn draw a dpmcp portal? ---
# Baseline first (census 0), then three trivial read-only spawns each followed
# by a census (censuses 1-3), then a settle sleep and a final census (census
# 4). THREE samples are the point: one sample cannot tell a per-spawn draw
# from a one-off, two cannot tell linear from a single step, but three spaced
# deltas separate a flat pool (spawn returns its portal — no leak), a linear
# draw (mcp falls by a fixed step per spawn — a leak), and a one-off drop.
# Note the censuses are themselves restool spawns, so the observation is
# conservative: more spawns occur than the three trivial ones, and a flat mcp
# across all of them is the stronger no-leak reading.
census 0 baseline
trivial 1; census 1 "after trivial spawn 1"
trivial 2; census 2 "after trivial spawn 2"
trivial 3; census 3 "after trivial spawn 3"

# Settle before the closing census: a drawn-then-returned portal may lag the
# spawn's exit (the rescan/allocator race window, ADR-0008). A pool that
# recovers only after the settle is a transient draw, not a leak.
sleep 2
census 4 "after settle"

echo "suite V-DPRC-13 body complete; teardown records dmesg and the census tally (no objects to destroy)"
