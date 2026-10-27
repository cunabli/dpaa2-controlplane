#!/bin/sh
# suite: V-DPCON-2
# class: object-lifecycle-only
# rev: 1 (plan-rollback design D2 — the dpcon channel-pool disambiguation sitting).
# HAND-AUTHORED — not emitted by dpaa2-verify. The witness READS the
# `dprc show mc.global --resources` listing BETWEEN restool verbs (a snapshot,
# then again after each create, then after the destroys) and diffs the
# dpcon-relevant rows across the create/destroy boundary. No model trace expresses
# reading the global ledger between its own steps, so there is no generator to
# regenerate from. Edit this file directly.
# operator: run as root (the MC portal /dev/dprc.1 is 0600 root:root, V-DPRC-4);
# the sitting is OPERATOR-LAUNCHED. Object-lifecycle-only: every object is created
# and destroyed inside a scratch child (bus-invisible, DPRC-I6), nothing in the
# root is ever touched, so it rides the V-CEIL-1 scratch-children slot.
# V-DPCON-1 is taken by the offline intents pin (crates/dpaa2-tools/tests/
# vdpcon1_intents.rs), so this board row is -2.
#
# WHAT THIS WITNESSES (plan-rollback design D2 — "the dpcon channel pools
# (cqch/swpch) are a scoped disambiguation sitting"; plan-rollback spec scenario
# "A hardware-side drain is bounded too", which admits a family to the MC-global
# budget face "only with [its] own proving observation"). The MC-global hardware
# ledger lists dpcon-adjacent pool rows whose semantics are UNPROVEN: cqch.*
# (e.g. cqch.ctm0.ins0, cqch.ctm0.ins1) and swpch.* (e.g. swpch.2wq, swpch.8wq) —
# see the listing shape in crates/dpaa2-mc/tests/fixtures/mc_global_resources.txt.
# We do not know which row(s) a `dpcon create` actually draws from, by how much,
# or whether `dpcon destroy` credits them back symmetrically. This sitting creates
# dpcon until-refused in a scratch child, reads the dpcon-relevant global rows
# after each create as per-create deltas, captures the refusal text, then destroys
# every created dpcon and re-reads the rows. So the per-create row deltas, the
# refusal text, and the symmetric credit-back are the disambiguation witness.
#
# THE RULE THIS SITTING GATES. NO dpcon global row joins the capacity face's
# MC-global budget until THIS sitting's verdict is on the record (plan-rollback
# design D2; the ADR-0023 Consequences monotonic-growth rule). Only bp joined so
# far, proven by V-CEIL-1 and V-DPRC-7 rev 2; dpcon waits on this observation.
#
# SAFETY (ADR-0003 §4). The total-deny trio (macs 3 and 17, the management dpni)
# is NEVER referenced in object syntax anywhere in this file — the self-check
# below greps this header too, so the trio stays prose. Every create and destroy
# is scoped to the suite's own scratch child, reclaimed by the single EXIT-trap
# teardown; no boot resident is touched.
set -u
RESULTS="${1:?usage: $0 <results-dir>}"
mkdir -p "$RESULTS"

# --- independent safety self-check (ADR-0003 §4) ---
# The execution side refuses total-deny references even if this script was
# hand-edited after authoring.
if grep -nE 'dpmac[.]3([^0-9]|$)|dpmac[.]17([^0-9]|$)|dpni[.]0([^0-9]|$)' "$0" | grep -v safety-self-check; then
  echo "refusing: total-deny object referenced in this script" >&2  # safety-self-check
  exit 1
fi

# --- precondition gate, part 1 (exit 2 skip) + the offline witness ---
# On a non-board host restool is absent, so the sitting SKIPS (exit 2) having
# touched nothing — no trap armed, no object created. This same gate is the
# offline witness: on this dev host the script exits 2 with no side effect.
if ! command -v restool >/dev/null 2>&1; then
  echo "PRECONDITION NOT MET: restool is not on PATH — this is not the board host."
  echo "suite V-DPCON-2 not run: precondition unmet"
  exit 2
fi

# --- reference pair assertion (ADR-0003 §2) ---
# Evidence is only valid against the stamped pair; refuse anything else.
mc="$(restool -m 2>/dev/null || true)"
case "$mc" in *10.39.0*) ;; *) echo "refusing: MC firmware is not 10.39.0: $mc" >&2; exit 1 ;; esac
kernel="$(uname -r)"
case "$kernel" in 6.6.52*) ;; *) echo "refusing: kernel is not 6.6.52: $kernel" >&2; exit 1 ;; esac

# --- kernel-log window (ADR-0008) + single EXIT-trap teardown ---
# A marker stamps the sitting's start; the close saves everything after it. The
# one trap cascade-destroys the suite's scratch child (idempotent) — a cascade
# destroy returns every dpcon drawn into it (V-DPRC-7 rev 2), so it is the safety
# net behind step 4's explicit destroys. It NEVER touches a boot resident.
KMSG="dpaa2-verify V-DPCON-2 pid $$"
echo "$KMSG start" > /dev/kmsg 2>/dev/null || true
CHILD_FILE="$RESULTS/scratch-child.txt"
save_dmesg() {
  dmesg 2>/dev/null | awk -v m="$KMSG start" 'w || index($0, m) { w = 1 } w' > "$RESULTS/dmesg.txt"
  [ -s "$RESULTS/dmesg.txt" ] || dmesg > "$RESULTS/dmesg.txt" 2>&1 || true
}
teardown() {
  if [ -f "$CHILD_FILE" ]; then
    c="$(cat "$CHILD_FILE" 2>/dev/null)"
    if [ -n "$c" ] && restool dprc show "$c" >/dev/null 2>&1; then
      echo "+ (teardown) restool dprc destroy $c"
      restool dprc destroy "$c" >>"$RESULTS/teardown.log" 2>&1 || true
    fi
  fi
  save_dmesg
}
trap teardown EXIT

# --- helpers ---
R="$RESULTS/dpcon.txt"             # the PASS/FAIL/RECORD/VERDICT face log, counted at close
log() { echo "$1" | tee -a "$R"; }
# probe FILE cmd...: capture a read-back to a named file; never fail the script.
probe() { f="$1"; shift; echo "+ (probe) $*"; "$@" > "$RESULTS/$f" 2>/dev/null || true; }
# status_of FILE: the MC/restool status text of a refusal (the refusal witness).
status_of() { grep -oE '\(status 0x[0-9a-fA-F]+\)|No resources|No memory available|No privilege|does not exist' "$1" 2>/dev/null | head -1; }
# dpcon_pool FILE: the dpcon-relevant rows (cqch.* and swpch.*) only, one
# "name value" per line, sorted — the projection every delta is taken over.
dpcon_pool() { grep -E '^(cqch|swpch)\.' "$1" 2>/dev/null | sed 's/:[[:space:]]*/ /' | sort; }
# row_delta PREV CUR: name and signed change for every dpcon-pool row that moved
# (PREV/CUR are dpcon_pool projections). Empty output means nothing moved.
row_delta() { awk 'NR==FNR { a[$1]=$2; next } { if (($1 in a) && (a[$1] != $2)) printf "%s %+d\n", $1, $2 - a[$1] }' "$1" "$2"; }

# --- precondition gate, part 2 (exit 2 skip) ---
# The root portal must render the resource listing AND it must name at least one
# cqch.* and one swpch.* row — the rows this disambiguation reads. A board that
# names none cannot stage the witness; that is a prerequisite skip, not a FAIL.
# Every count below is runtime-derived from this listing, never hardcoded.
PRE="$RESULTS/pool-snapshot.txt"
if ! restool dprc show mc.global --resources > "$PRE" 2>"$RESULTS/pre.err"; then
  cat "$RESULTS/pre.err" >&2
  echo "PRECONDITION NOT MET: 'restool dprc show mc.global --resources' did not render (root portal unreachable?)."
  echo "suite V-DPCON-2 not run: precondition unmet"
  exit 2
fi
cqch_rows="$(grep -cE '^cqch\.' "$PRE" || true)"
swpch_rows="$(grep -cE '^swpch\.' "$PRE" || true)"
if [ "$cqch_rows" -lt 1 ] || [ "$swpch_rows" -lt 1 ]; then
  echo "PRECONDITION NOT MET: the listing names no cqch.* (found $cqch_rows) and/or swpch.* (found $swpch_rows) row."
  echo "suite V-DPCON-2 not run: precondition unmet"
  exit 2
fi

# step 1: snapshot the dpcon-relevant global rows and the root dpcon census — the
# baseline every per-create delta and the credit-back check are diffed against.
probe step1-root-show.txt restool dprc show dprc.1
root_dpcon="$(grep -cE '(^| )dpcon\.[0-9]' "$RESULTS/step1-root-show.txt" || true)"
dpcon_pool "$PRE" > "$RESULTS/pool-baseline.rows"
log "RECORD step 1: baseline cqch rows=$cqch_rows swpch rows=$swpch_rows; root dpcon census=$root_dpcon"

# step 2: create the scratch child (the V-CEIL-1 idiom — it has the ALLOC bit and
# so draws from the same MC pools as the boot residents without starving anyone).
CHILD="$(restool --script dprc create dprc.1 2>"$RESULTS/child-create.err" | grep -oE 'dprc\.[0-9]+' | head -1)"
if [ -z "$CHILD" ]; then
  cat "$RESULTS/child-create.err" >&2
  log "FAIL step 2: could not create the scratch child dprc — nothing to witness"
else
  echo "$CHILD" > "$CHILD_FILE"
  log "RECORD step 2: scratch child $CHILD created"

  # step 3: create dpcons one at a time with --num-priorities=2 (a bare dpcon
  # create refuses — V-ROLLBACK-1's header records it) inside the child, re-reading
  # the global rows after each create as a per-create delta, until the create
  # refuses (capture the refusal text) or the V-CEIL-1 cap of 64. The cap bounds a
  # board that never refuses (dpcon reached 64 in V-CEIL-1's scratch child).
  CAP=64
  created=""
  n=0
  refusal=""
  prev="$RESULTS/pool-baseline.rows"
  CEIL="$RESULTS/pool-baseline.rows"
  while [ "$n" -lt "$CAP" ]; do
    err="$RESULTS/create-$((n + 1)).err"
    id="$(restool --script dpcon create --num-priorities=2 --container="$CHILD" 2>"$err")"
    if [ -z "$id" ] || grep -qiE 'error|does not exist|status 0x' "$err"; then
      refusal="$(status_of "$err")"
      cp "$err" "$RESULTS/refusal.txt" 2>/dev/null || true
      break
    fi
    created="$created $id"
    n=$((n + 1))
    rows="$RESULTS/pool-after-$n.rows"
    restool dprc show mc.global --resources 2>/dev/null | grep -E '^(cqch|swpch)\.' | sed 's/:[[:space:]]*/ /' | sort > "$rows" || true
    moved="$(row_delta "$prev" "$rows")"
    if [ -n "$moved" ]; then
      log "RECORD create #$n ($id): dpcon-pool rows moved [$(echo "$moved" | tr '\n' ' ')]"
    else
      log "RECORD create #$n ($id): no cqch.*/swpch.* row moved"
    fi
    prev="$rows"
    CEIL="$rows"
  done
  log "RECORD step 3: created $n dpcon(s) before refusal '${refusal:-<cap reached>}'"
  # An MC-short status (No memory available 0x9, timeout 0x7) says the MC itself,
  # not a pool, is short — record it loud so the verdict is not read as a pool law.
  case "$refusal" in
    *0x9* | *0x7* | *"No memory available"*)
      log "FAIL step 3: the refusal is MC-short ($refusal), not a pool boundary — the draw is not disambiguated"
      ;;
  esac

  # step 4: destroy every created dpcon (in the child, never the root) and re-read
  # the rows — the symmetric credit-back check. The trap is the safety net behind
  # this explicit destroy loop.
  for id in $created; do
    restool dpcon destroy "$id" 2>>"$RESULTS/destroy.err" || true
  done
  restool dprc show mc.global --resources 2>/dev/null | grep -E '^(cqch|swpch)\.' | sed 's/:[[:space:]]*/ /' | sort > "$RESULTS/pool-restored.rows" || true
  back="$(row_delta "$RESULTS/pool-baseline.rows" "$RESULTS/pool-restored.rows")"
  if [ "$n" = 0 ]; then
    log "RECORD step 4: no dpcon was created, so credit-back is vacuous"
  elif [ -z "$back" ]; then
    log "PASS step 4: every dpcon-pool row credited back symmetrically after $n destroy(s)"
  else
    log "FAIL step 4: dpcon-pool rows did not return to their baseline [$(echo "$back" | tr '\n' ' ')]"
  fi

  # step 5: the disambiguation verdict. The NET movement across all N creates
  # names the row(s) a dpcon create draws from and the per-create rate (the draw
  # divided by the accepted count). An empty net says the dpcon draw is NOT on
  # the cqch.*/swpch.* rows — itself a disambiguating answer.
  if [ "$n" -gt 0 ]; then
    net="$(row_delta "$RESULTS/pool-baseline.rows" "$CEIL")"
    if [ -n "$net" ]; then
      echo "$net" | while read -r name chg; do
        log "VERDICT: $n dpcon create(s) moved $name by $chg (per-create $((chg / n)))"
      done
    else
      log "VERDICT: $n dpcon create(s) moved NO cqch.*/swpch. row — the dpcon draw is not on these pools"
    fi
  fi
fi

# --- close: count the face log and exit nonzero on any FAIL (ADR-0003 §4) ---
save_dmesg
passes="$(grep -c '^PASS ' "$R")"
fails="$(grep -c '^FAIL ' "$R")"
records="$(grep -c '^RECORD ' "$R")"
verdicts="$(grep -c '^VERDICT' "$R")"
echo "suite V-DPCON-2 complete: $passes PASS, $fails FAIL, $records RECORD, $verdicts VERDICT"
[ "$fails" = 0 ] || exit 1
