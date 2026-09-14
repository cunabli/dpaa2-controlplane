#!/bin/sh
# suite: V-DPRC-9
# class: object-lifecycle-only
# HAND-AUTHORED — not emitted by dpaa2-verify. The fit-check emitter refuses
# mutating verbs by design, and this suite's steps are `dpaa2ctl` invocations
# (read -> compile -> reconcile -> dispatch), not model-trace restool verbs, so
# there is no generator to regenerate from. Edit this file directly.
# operator: run as root — the ensure legs issue MC creates/destroys the kernel
# gates on CAP_NET_ADMIN (docs/baseline/mc-ioctl-policy.md).
#
# End-to-end consumer-container convergence (dprc-encapsulation task 5.3;
# system-integration spec "End-to-end convergence diffs clean"): with one
# declared consumer, the first `ensure` creates the child DPRC, a second run
# plans zero actions, and the read-back is captured for offline diff against
# the derived model. A prune leg tears the container down through the
# reconciler's own path (DPRC-I9 under plans), and the post-suite census must
# match the pre-suite baseline. Results are captured under the results
# directory for offline diffing; the model side is the two operands' derivation
# (crates/dpaa2-tools/tests/vdprc9_intents.rs).
set -u
RESULTS="${1:?usage: $0 <results-dir>}"
# A relative results dir is anchored to the operator's cwd before the cd below.
case "$RESULTS" in /*) ;; *) RESULTS="$PWD/$RESULTS" ;; esac
mkdir -p "$RESULTS"

# --- repo-root cd (design D12; ADR-0003 §2) ---
# The `dpaa2ctl` steps name the operands relative to the repo root, so the
# sitting runs from there regardless of the operator's cwd. This script lives
# three levels down at models/board/<id>/<id>.sh; SELF is resolved absolute
# BEFORE the cd so the total-deny self-check still greps the right file.
SELF="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")"
cd "$(dirname "$SELF")/../../.." || { echo "refusing: cannot reach the repo root from $SELF" >&2; exit 1; }
[ -f models/board/V-DPRC-9/intent-a.toml ] || { echo "refusing: not the repo root (models/board/V-DPRC-9/intent-a.toml missing); run this script from its checkout" >&2; exit 1; }

# --- dpaa2ctl from this checkout's build output (design D12; ADR-0003 §2) ---
# Evidence is only valid if the sitting exercises THIS checkout's compiler, not
# a stale dpaa2ctl on PATH (sudo's secure_path defeats a PATH prefix). Honor a
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

# --- kernel-log window ---
# A marker stamps the sitting's start; the teardown saves everything after it
# to dmesg.txt so rescan markers (ADR-0008) are files, not operator memory.
KMSG="dpaa2-verify V-DPRC-9 pid $$"
echo "$KMSG start" > /dev/kmsg 2>/dev/null || true
save_dmesg() {
  dmesg 2>/dev/null | awk -v m="$KMSG start" 'w || index($0, m) { w = 1 } w' > "$RESULTS/dmesg.txt"
  [ -s "$RESULTS/dmesg.txt" ] || dmesg > "$RESULTS/dmesg.txt" 2>&1 || true
}

# --- independent safety self-check (ADR-0003 §4) ---
# The execution side refuses total-deny references even if this script was
# hand-edited after authoring.
if grep -nE 'dpmac[.]3([^0-9]|$)|dpmac[.]17([^0-9]|$)|dpni[.]0([^0-9]|$)' "$SELF" | grep -v safety-self-check; then
  echo "refusing: total-deny object referenced in this script" >&2  # safety-self-check
  exit 1
fi

# --- helpers ---
# run N cmd...: echo, execute, capture stdout, stderr and the exit code.
run() { n="$1"; shift; echo "+ $*"; "$@" >"$RESULTS/step-$n-out.txt" 2>"$RESULTS/step-$n-err.txt"; echo $? > "$RESULTS/step-$n-exit.txt"; }
# probe FILE cmd...: capture a read-back to a named file; never fail the script.
probe() { f="$1"; shift; echo "+ (probe) $*"; "$@" > "$RESULTS/$f" 2>/dev/null || true; }
# pool_capture FILE: snapshot MC-global resource pools (ADR-0011), best-effort.
pool_capture() { restool dprc show mc.global --resources > "$RESULTS/$1" 2>/dev/null || true; }
# expect_zero N label: PASS iff the step exited zero.
expect_zero() { rc=$(cat "$RESULTS/step-$1-exit.txt"); if [ "$rc" = 0 ]; then echo "PASS step $1: $2"; else echo "FAIL step $1: $2 (exit $rc)" >&2; fi; }
# expect_any N label: informational — the capture is the evidence, never the exit.
expect_any() { rc=$(cat "$RESULTS/step-$1-exit.txt"); echo "PASS step $1: $2 (exit $rc — informational)"; }
# vdprc9_child: the root child DPRC labelled exactly `vdprc9`, empty when absent
# (step 7 already tore it down). Judged by re-observation, never a sync (DPRC-I6).
vdprc9_child() { restool dprc show dprc.1 2>/dev/null | awk '$2 == "vdprc9" { print $1; exit }'; }

# --- reference pair assertion (ADR-0003 §2) ---
# Evidence is only valid against the stamped pair; refuse anything else.
mc="$(restool -m 2>/dev/null || true)"
case "$mc" in *10.39.0*) ;; *) echo "refusing: MC firmware is not 10.39.0: $mc" >&2; exit 1 ;; esac
kernel="$(uname -r)"
case "$kernel" in 6.6.52*) ;; *) echo "refusing: kernel is not 6.6.52: $kernel" >&2; exit 1 ;; esac

# --- unconditional teardown (ADR-0003 §6) ---
# The single sanctioned destroyer: best-effort and label-scoped ONLY. It greps
# the root listing for a child labelled exactly `vdprc9` and destroys that one
# container — nothing else. A no-op when step 7 already cleaned up.
teardown() {
  child="$(vdprc9_child)"
  if [ -n "$child" ]; then
    restool dprc destroy "$child" 2>>"$RESULTS/teardown.log" || true
    sleep 2
  fi
  save_dmesg
}
trap teardown EXIT

# --- pool baseline (ADR-0011) ---
pool_capture pool-baseline.txt

# step 0: pre-suite census — the census-clean baseline diffed against step 8.
# expect: zero exit; the root container tree and its labelled listing, both
# captured whole for the offline census diff (ADR-0003 §5 queries are
# object-lifecycle-only).
run 0 restool dprc list
expect_zero 0 "pre-suite census: container list"
probe step-0-show-dprc1.txt restool dprc show dprc.1

# step 1: the shipped dry-run headlines the container create with provenance.
# expect: zero exit; the plan headlines CreateContainer for `vdprc9` with
# per-object provenance (dprc-encapsulation design D6). Captured, dispatches nothing.
run 1 "$DPAA2CTL" --config models/board/V-DPRC-9/intent-a.toml dry-run
expect_zero 1 "dry-run headlines CreateContainer with provenance"

# step 2: first ensure — creates the child DPRC.
# expect: zero exit; CreateContainer headlines `disruptive`, so the gate must be
# opened with --allow disruptive (never implied). --no-link: no port, no dpni,
# no .link file. First run creates the container (DPRC-I4 default option mask).
run 2 "$DPAA2CTL" --config models/board/V-DPRC-9/intent-a.toml ensure --no-link --allow disruptive
expect_zero 2 "first ensure creates the container"

# step 3: read-back probes for the offline diff against derivation. Judged by
# re-observation, never a sync (DPRC-I6). Resolve the created child by its
# `vdprc9` label, then read its options mask and its (empty) resident set.
run 3 restool dprc list
expect_zero 3 "read-back: container list after create"
probe step-3-show-dprc1.txt restool dprc show dprc.1
child="$(vdprc9_child)"
echo "vdprc9 child: ${child:-<none>}" > "$RESULTS/step-3-child.txt"
if [ -n "$child" ]; then
  # expect: options mask decoded as the restool default SPAWN|ALLOC|OBJ_CREATE|
  # IRQ_CFG (DPRC-I4), and `dprc show <child>` lists zero residents (the
  # container-only charter — no companion/dpni populated).
  probe step-3-info-child.txt restool dprc info "$child"
  probe step-3-show-child.txt restool dprc show "$child"
else
  echo "FAIL step 3: no child labelled vdprc9 after create" >&2
fi

# step 4: repeat the exact ensure of step 2 — the plans-zero-actions face.
# expect: zero exit; the container already converged, so the reconciler plans
# and dispatches nothing (idempotent, level-triggered). The captured output is
# the acceptance evidence.
run 4 "$DPAA2CTL" --config models/board/V-DPRC-9/intent-a.toml ensure --no-link --allow disruptive
expect_zero 4 "second ensure plans zero actions"

# step 5: status on the declared intent.
# Container verdicts do NOT feed the status exit: the Status arm of
# crates/dpaa2-tools/src/main.rs derives its exit solely from the port
# StatusReport, and this portless intent has zero ports, so a converged board
# reports zero divergence and exits 0. expect_zero — but the container's
# converged state is evidenced by step 3's read-back and step 4's zero-action
# ensure, not by this exit.
run 5 "$DPAA2CTL" --config models/board/V-DPRC-9/intent-a.toml status
expect_zero 5 "status: portless intent reports zero port divergence"

# step 6: prune dry-run — the buckets read BEFORE the destructive leg.
# expect: any exit; the prune report classifies `vdprc9` as a candidate by its
# label fingerprint and the board's DPL-born containers as report-only
# (DPRC-I12 buckets). The operator reads this before step 7 runs; dispatches
# nothing.
run 6 "$DPAA2CTL" --config models/board/V-DPRC-9/intent-b.toml dry-run --prune
expect_any 6 "prune dry-run: candidate vs report-only buckets"

# step 7: prune ensure under the double gate — the reconciler-path teardown.
# expect: zero exit; --prune --allow disruptive tears the `vdprc9` container
# down through the reconciler's own plans (DPRC-I9 exercised under plans, not
# hand restool verbs); unlabelled DPL-born containers stay untouched.
run 7 "$DPAA2CTL" --config models/board/V-DPRC-9/intent-b.toml ensure --prune --allow disruptive --no-link
expect_zero 7 "prune ensure tears the container down through the reconciler"

# step 8: post census — diffed offline against step 0's baseline.
# expect: zero exit; the container list, the root listing and the pools must
# match the pre-suite baseline object-for-object (the census-clean guarantee).
run 8 restool dprc list
expect_zero 8 "post-suite census: container list"
probe step-8-show-dprc1.txt restool dprc show dprc.1
pool_capture pool-post.txt

echo "suite V-DPRC-9 end-to-end convergence complete"
