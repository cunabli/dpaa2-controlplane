#!/bin/sh
# suite: V-ADMIT-1
# class: read-only
# HAND-AUTHORED — not emitted by dpaa2-verify. The fit-check emitter refuses
# mutating verbs by design, and this suite's steps are `dpaa2ctl` invocations
# (read -> compile -> admit), not model-trace restool verbs, so there is no
# generator to regenerate from. Edit this file directly.
# operator: run as root — only for the census reads restool makes (the MC portal
# /dev/dprc.1 is 0600 root:root, V-DPRC-4). The two `dpaa2ctl` legs dispatch
# NOTHING: admission refuses the over-budget intent before any verb, so the whole
# suite is READ-ONLY against the MC and rides any sitting.
#
# The live admission-refusal witness (resource-aware-orchestration task 6.1, bead
# dpaa2-controlplane-m7a.6; provisioning-cli spec "Ensure refuses before
# dispatch"). An over-budget intent (one terminated kernel port on dpmac.4 with a
# +200 dpcon surplus) against the reference board's observed pools is refused
# BEFORE any verb (ADR-0023 design D5/D7/D8 — planning-time refusal,
# check-without-reservation, gross draw of pending creates). dpcon is the family
# raised because its ceiling is Unknown to compile (ref_inventory / ADR-0011): a
# raise on dpbp (ceiling Counted(63)) would be refused by compile as Infeasible
# before admission ran, so the raise lands on dpcon to leave the over-budget draw
# for admission to judge against the observed headroom. `dry-run` renders the
# admission verdict beside the plan with the dpcon row refused, and `ensure`
# exits refused naming the budget/draw/headroom. The board is provably
# untouched — pre/post `dprc show dprc.1` and the MC-global pools are
# byte-identical and the boot residents stay on their reference drivers. The
# operand derivation and refusal are pinned offline by
# crates/dpaa2-tools/tests/vadmit1_intents.rs.
set -u
RESULTS="${1:?usage: $0 <results-dir>}"
# A relative results dir is anchored to the operator's cwd before the cd below.
case "$RESULTS" in /*) ;; *) RESULTS="$PWD/$RESULTS" ;; esac
mkdir -p "$RESULTS"

# --- repo-root cd (ADR-0003 §2) ---
# The `dpaa2ctl` steps name the operand relative to the repo root, so the sitting
# runs from there regardless of the operator's cwd. This script lives three
# levels down at models/board/<id>/<id>.sh; SELF is resolved absolute BEFORE the
# cd so the total-deny self-check still greps the right file.
SELF="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")"
cd "$(dirname "$SELF")/../../.." || { echo "refusing: cannot reach the repo root from $SELF" >&2; exit 1; }
[ -f models/board/V-ADMIT-1/intent-over.toml ] || { echo "refusing: not the repo root (models/board/V-ADMIT-1/intent-over.toml missing); run this script from its checkout" >&2; exit 1; }

# --- independent safety self-check (ADR-0003 §4) ---
# The execution side refuses total-deny references even if this script was
# hand-edited after authoring.
if grep -nE 'dpmac[.]3([^0-9]|$)|dpmac[.]17([^0-9]|$)|dpni[.]0([^0-9]|$)' "$SELF" | grep -v safety-self-check; then
  echo "refusing: total-deny object referenced in this script" >&2  # safety-self-check
  exit 1
fi

# --- dpaa2ctl from this checkout's build output (ADR-0003 §2) ---
# Evidence is only valid if the sitting exercises THIS checkout's compiler, not a
# stale dpaa2ctl on PATH (sudo's secure_path defeats a PATH prefix). Honor a
# pre-set $DPAA2CTL, else take the first binary this checkout built; refuse with
# the exact build command if neither exists.
if [ -z "${DPAA2CTL:-}" ]; then
  if [ -x target/release/dpaa2ctl ]; then
    DPAA2CTL=target/release/dpaa2ctl
  elif [ -x target/debug/dpaa2ctl ]; then
    DPAA2CTL=target/debug/dpaa2ctl
  else
    echo "refusing: no dpaa2ctl in target/release or target/debug — run: cargo build -p dpaa2-tools" >&2
    exit 1
  fi
fi
{ echo "$DPAA2CTL"; sha256sum "$DPAA2CTL" 2>/dev/null || ls -l "$DPAA2CTL"; } > "$RESULTS/dpaa2ctl-provenance.txt"

# --- kernel-log window (ADR-0008) ---
# A marker stamps the sitting's start; the close saves everything after it to
# dmesg.txt so rescan markers are files, not operator memory.
KMSG="dpaa2-verify V-ADMIT-1 pid $$"
echo "$KMSG start" > /dev/kmsg 2>/dev/null || true
save_dmesg() {
  dmesg 2>/dev/null | awk -v m="$KMSG start" 'w || index($0, m) { w = 1 } w' > "$RESULTS/dmesg.txt"
  [ -s "$RESULTS/dmesg.txt" ] || dmesg > "$RESULTS/dmesg.txt" 2>&1 || true
}
# Read-only suite: the only teardown is to save the kernel-log window.
trap save_dmesg EXIT

# --- reference pair assertion (ADR-0003 §2) ---
# Evidence is only valid against the stamped pair; refuse anything else.
mc="$(restool -m 2>/dev/null || true)"
case "$mc" in *10.39.0*) ;; *) echo "refusing: MC firmware is not 10.39.0: $mc" >&2; exit 1 ;; esac
kernel="$(uname -r)"
case "$kernel" in 6.6.52*) ;; *) echo "refusing: kernel is not 6.6.52: $kernel" >&2; exit 1 ;; esac

# --- residents helper for the census (ADR-0008 §4–§5, §8) ---
. models/board/residents.sh

# --- helpers ---
R="$RESULTS/admit.txt"            # the PASS/FAIL face log, counted at close
log() { echo "$1" | tee -a "$R"; }
# run N cmd...: echo, execute, capture stdout, stderr and the exit code.
run() { n="$1"; shift; echo "+ $*"; "$@" >"$RESULTS/step-$n-out.txt" 2>"$RESULTS/step-$n-err.txt"; echo $? > "$RESULTS/step-$n-exit.txt"; }
# probe FILE cmd...: capture a read-back to a named file; never fail the script.
probe() { f="$1"; shift; echo "+ (probe) $*"; "$@" > "$RESULTS/$f" 2>/dev/null || true; }
# pool_capture FILE: snapshot MC-global resource pools (ADR-0011), best-effort.
pool_capture() { restool dprc show mc.global --resources > "$RESULTS/$1" 2>/dev/null || true; }

# step 0: pre census — the read-only baseline diffed against step 3. The root
# container show and the MC-global pools are captured whole; residents_pre checks
# every boot resident against its clean-boot reference driver (ADR-0008 §8).
probe step-0-show-dprc1.txt restool dprc show dprc.1
pool_capture pool-baseline.txt
if residents_pre; then log "PASS step 0: residents baseline on their reference drivers"; else log "FAIL step 0: a boot resident is off its reference driver"; fi

# step 1: dry-run renders the admission verdict beside the plan. The DryRun arm
# always exits 0 (crates/dpaa2-tools/src/main.rs) — a dry-run dispatches nothing —
# so the refusal lives in the rendered table (render_admission), NOT in the
# `refused: pool` voice (that is the `ensure` leg, step 2): the overall verdict is
# [refused] and the dpcon budget row carries draw/headroom and the per-row
# [refused] (resource-aware-orchestration task 5.1; ADR-0023 design D5).
run 1 "$DPAA2CTL" --config models/board/V-ADMIT-1/intent-over.toml dry-run
rc1="$(cat "$RESULTS/step-1-exit.txt")"
if [ "$rc1" = 0 ] \
  && grep -q 'admission (.*) \[refused\]:' "$RESULTS/step-1-out.txt" \
  && grep -qE 'dpcon draw=[0-9]+ headroom=[0-9]+ \[refused\]' "$RESULTS/step-1-out.txt"; then
  log "PASS step 1: dry-run exits 0 and renders the dpcon budget row refused"
else
  log "FAIL step 1: dry-run did not render the refused dpcon admission row (exit $rc1)"
fi

# step 2: ensure refuses BEFORE any verb (ADR-0023 design D5/D7). Admission is
# judged after the reify guard and before the port loop, so a refusal returns
# ExitCode::FAILURE (nonzero) having dispatched nothing — --no-link is
# unnecessary because no link work is reached. render_admission_refusal prints the
# `refused: pool <family> draw <n> exceeds free headroom <m>` voice to stdout.
run 2 "$DPAA2CTL" --config models/board/V-ADMIT-1/intent-over.toml ensure --allow disruptive
rc2="$(cat "$RESULTS/step-2-exit.txt")"
if [ "$rc2" != 0 ] && grep -q 'refused: pool dpcon draw [0-9]* exceeds free headroom [0-9]*' "$RESULTS/step-2-out.txt"; then
  log "PASS step 2: ensure exits refused ($rc2) naming the dpcon budget, draw and headroom"
else
  log "FAIL step 2: ensure did not refuse naming the dpcon budget (exit $rc2)"
fi

# step 3: post census — the board must be byte-identical to step 0 (admission
# changed nothing). The root show and the MC-global pools are re-captured and
# compared; residents_post re-checks every boot resident.
probe step-3-show-dprc1.txt restool dprc show dprc.1
pool_capture pool-post.txt
if cmp -s "$RESULTS/step-0-show-dprc1.txt" "$RESULTS/step-3-show-dprc1.txt"; then
  log "PASS step 3: dprc.1 show byte-identical pre vs post (board untouched)"
else
  log "FAIL step 3: dprc.1 show differs pre vs post (the board was mutated)"
fi
if cmp -s "$RESULTS/pool-baseline.txt" "$RESULTS/pool-post.txt"; then
  log "PASS step 3: MC-global pools byte-identical pre vs post"
else
  log "FAIL step 3: MC-global pools differ pre vs post"
fi
if residents_post; then log "PASS step 3: residents still on their reference drivers"; else log "FAIL step 3: a boot resident is off its reference driver"; fi

# --- close: count the face log and exit nonzero on any FAIL (ADR-0003 §4) ---
passes="$(grep -c '^PASS ' "$R")"
fails="$(grep -c '^FAIL ' "$R")"
echo "suite V-ADMIT-1 complete: $passes PASS, $fails FAIL"
[ "$fails" = 0 ] || exit 1
