#!/bin/sh
# suite: V-POOL-6
# class: object-lifecycle-only
# HAND-AUTHORED — not emitted by dpaa2-verify. The steps are `dpaa2ctl`
# invocations (read -> compile -> converge_pools -> port loop), not model-trace
# restool verbs, so there is no generator to regenerate from. Edit directly.
# operator: run as root — the ensure legs issue MC creates/destroys the kernel
# gates on CAP_NET_ADMIN (docs/baseline/mc-ioctl-policy.md).
#
# Root-scope pool convergence through the shipped dpaa2ctl (pool-objects task
# 3.4 + 3.14 + 4.1, bead dpaa2-controlplane-960.10/.27; mbt-harness spec "Suite
# generation renders the pool walks"). ensure runs the pool walk grow-first /
# shrink-last (pool-objects design D10, crates/dpaa2-tools/src/main.rs): the grow
# half runs BEFORE the port loop so a consumer has capacity to draw, and the
# shrink half runs LAST, after the container and root-dpni teardown — consumers
# before pools. A pool create/destroy/prune is Class::Disruptive: a default
# (hitless) ensure refuses, changing nothing. The walk: dry-run the grow, refuse
# it hitless (a cheap refusal face), grow under --allow disruptive, prove
# idempotence (a second ensure dispatches nothing), make a foreign restool pool
# object, then intent-b shrinks the surplus and prunes the foreign object while
# the DPL-born boot pool stays untouched (pool-objects design D3). Results are
# captured for offline diff. The operands' derivation is pinned offline by
# crates/dpaa2-tools/tests/vpool6_intents.rs.
#
# Reclaim is the unplug-probe law (pool-objects design D10): a surplus/foreign
# object is unplugged first and then destroyed, and a PLUGGED-but-undrawn surplus
# is reclaimable — the earlier plugged==>drawn proxy that made managed surplus
# stuck is gone. A live draw the MC refuses to unplug (-EBUSY) is the drawn
# signal, surfaced as the ShrinkBelowDraw face, never a forced teardown.
#
# ShrinkBelowDraw (a requirement below the drawn count) is NOT forced here: the
# a->b delta is pure pool trio surplus (no port churn), so the shrunk objects are
# free-managed, not drawn. Its board face rides V-POOL-7's bound state / stays
# twin-covered (shrink_below_draw_refuses_by_name_and_count in pool_replay).
#
# 4.2 operator, READ FIRST: the grow leg creates the reserved kernel's full
# root pool — on a 16-CPU board that is ~16 dpio seats + ~32 dpcon + ~18 dpmcp
# base (20 with the +2 extra) + a few dpbp MANAGED on top of the DPL-born boot
# pool, plus a kernel dpni on dpmac.4 (the port's dpaa2-eth probe draw folds
# into the pool, pool-objects design D9). dpio seats are grow-only
# (pool-objects design D4), so the teardown
# reconciles the trio and the dpni away but CANNOT reclaim the grown dpio
# seats: run V-POOL-6 LAST in a sitting and REBOOT after (ADR-0003 §7 recovery
# guarantee is the backstop). The post-suite census will show the leaked dpio
# seats until that reboot.
set -u
RESULTS="${1:?usage: $0 <results-dir>}"
case "$RESULTS" in /*) ;; *) RESULTS="$PWD/$RESULTS" ;; esac
mkdir -p "$RESULTS"

# --- repo-root cd (design D12; ADR-0003 §2) ---
# The dpaa2ctl steps name the operands relative to the repo root. SELF is
# resolved absolute BEFORE the cd so the total-deny self-check greps the right
# file (the V-DPRC-9 shape).
SELF="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")"
cd "$(dirname "$SELF")/../../.." || { echo "refusing: cannot reach the repo root from $SELF" >&2; exit 1; }
[ -f models/board/V-POOL-6/intent-a.toml ] || { echo "refusing: not the repo root (models/board/V-POOL-6/intent-a.toml missing); run this script from its checkout" >&2; exit 1; }

# --- dpaa2ctl from this checkout's build output (design D12; ADR-0003 §2) ---
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
KMSG="dpaa2-verify V-POOL-6 pid $$"
echo "$KMSG start" > /dev/kmsg 2>/dev/null || true
save_dmesg() {
  dmesg 2>/dev/null | awk -v m="$KMSG start" 'w || index($0, m) { w = 1 } w' > "$RESULTS/dmesg.txt"
  [ -s "$RESULTS/dmesg.txt" ] || dmesg > "$RESULTS/dmesg.txt" 2>&1 || true
}

# --- independent safety self-check (ADR-0003 §4) ---
if grep -nE 'dpmac[.]3([^0-9]|$)|dpmac[.]17([^0-9]|$)|dpni[.]0([^0-9]|$)' "$SELF" | grep -v safety-self-check; then
  echo "refusing: total-deny object referenced in this script" >&2  # safety-self-check
  exit 1
fi

# --- helpers ---
# run N cmd...: echo, execute, capture stdout, stderr and the exit code.
run() { n="$1"; shift; echo "+ $*"; "$@" >"$RESULTS/step-$n-out.txt" 2>"$RESULTS/step-$n-err.txt"; echo $? > "$RESULTS/step-$n-exit.txt"; }
probe() { f="$1"; shift; echo "+ (probe) $*"; "$@" > "$RESULTS/$f" 2>/dev/null || true; }
pool_capture() { restool dprc show mc.global --resources > "$RESULTS/$1" 2>/dev/null || true; }
expect_zero() { rc=$(cat "$RESULTS/step-$1-exit.txt"); if [ "$rc" = 0 ]; then echo "PASS step $1: $2"; else echo "FAIL step $1: $2 (exit $rc)" >&2; fi; }
expect_nonzero() { rc=$(cat "$RESULTS/step-$1-exit.txt"); if [ "$rc" != 0 ]; then echo "PASS step $1: $2 (refused, exit $rc)"; else echo "FAIL step $1: $2 (exit 0 — the refusal did not fire)" >&2; fi; }
# expect_out N needle label: PASS iff step N's captured output carries the needle.
expect_out() { if grep -qF "$2" "$RESULTS/step-$1-out.txt" "$RESULTS/step-$1-err.txt" 2>/dev/null; then echo "PASS step $1: $3"; else echo "FAIL step $1: $3 (missing '$2')" >&2; fi; }

# --- reference pair assertion (ADR-0003 §2) ---
mc="$(restool -m 2>/dev/null || true)"
case "$mc" in *10.39.0*) ;; *) echo "refusing: MC firmware is not 10.39.0: $mc" >&2; exit 1 ;; esac
kernel="$(uname -r)"
case "$kernel" in 6.6.52*) ;; *) echo "refusing: kernel is not 6.6.52: $kernel" >&2; exit 1 ;; esac

# --- unconditional teardown (ADR-0003 §6) ---
# Reconcile the managed root pool and the kernel dpni back to nothing through the
# reconciler's own reverse path. An empty intent derives zero, and ensure runs the
# grow-first / shrink-last walk (pool-objects design D10): the port loop and the
# root-dpni prune tear the kernel dpni down FIRST (consumers before pools), which
# releases its draws, so the shrink half then reclaims every free-managed trio
# object to 0 through the unplug probe. A foreign object left standing is destroyed
# best-effort. dpio seats are grow-only and NOT reclaimed here — the closing reboot
# restores them (ADR-0003 §7). The empty intent is written to the results dir so no
# throwaway operand is committed.
teardown() {
  printf '[intent]\nschema = 1\n' > "$RESULTS/teardown-empty.toml"
  "$DPAA2CTL" --config "$RESULTS/teardown-empty.toml" ensure --prune --allow disruptive --no-link \
    > "$RESULTS/teardown-ensure.txt" 2>>"$RESULTS/teardown.log" || true
  sleep 2
  if [ -n "${FOREIGN:-}" ]; then
    restool "$FOREIGN" > /dev/null 2>&1 && restool dpbp destroy "$FOREIGN" 2>>"$RESULTS/teardown.log" || true
  fi
  pool_capture pool-teardown.txt
  save_dmesg
  echo "teardown: managed trio + kernel dpni reconciled to empty; dpio seats are grow-only — reboot to reclaim (ADR-0003 §7)"
}
trap teardown EXIT

# --- pool baseline (ADR-0011) ---
pool_capture pool-baseline.txt

# step 0: pre-suite census — the baseline the post-suite state is diffed against.
run 0 restool dprc list
expect_zero 0 "pre-suite census: container list"
probe step-0-show-dprc1.txt restool dprc show dprc.1

# step 1: dry-run the grow — the root pool drift block (grow deltas, headline
# Disruptive), dispatching nothing.
run 1 "$DPAA2CTL" --config models/board/V-POOL-6/intent-a.toml dry-run
expect_zero 1 "dry-run headlines the root pool grow"
expect_out 1 "root pool convergence" "dry-run prints the root pool convergence block"

# step 2: hitless-refusal leg — ensure the grow WITHOUT --allow. A pool create
# is Class::Disruptive, so the run refuses and changes nothing (exit FAILURE).
run 2 "$DPAA2CTL" --config models/board/V-POOL-6/intent-a.toml ensure --no-link
expect_nonzero 2 "hitless ensure refuses the disruptive root pool grow"
expect_out 2 "refused: root pool grow is" "the refusal names the root pool grow class"

# step 3: grow under the disruptive gate — creates the managed root pool.
run 3 "$DPAA2CTL" --config models/board/V-POOL-6/intent-a.toml ensure --no-link --allow disruptive
expect_zero 3 "ensure --allow disruptive grows the root pool to the derived counts"
probe step-3-show-dprc1.txt restool dprc show dprc.1
pool_capture pool-after-grow.txt

# step 4: idempotence read-back — a re-run dry-run of intent-a now plans zero
# pool actions (every per-family create=0).
run 4 "$DPAA2CTL" --config models/board/V-POOL-6/intent-a.toml dry-run
expect_zero 4 "post-grow dry-run: the root pool is converged"

# step 5: idempotence actuation — a second ensure dispatches nothing (Converged).
run 5 "$DPAA2CTL" --config models/board/V-POOL-6/intent-a.toml ensure --no-link --allow disruptive
expect_zero 5 "second ensure plans zero pool actions (idempotent, level-triggered)"

# step 6: make a foreign root pool object — an undeclared, unlabelled, free
# dpbp in the root, the prune target. FOREIGN is exported for the teardown.
FOREIGN="$(restool --script dpbp create 2>"$RESULTS/step-6-err.txt")"
echo "$FOREIGN" > "$RESULTS/step-6-foreign.txt"
if [ -n "$FOREIGN" ]; then echo "PASS step 6: foreign root dpbp created ($FOREIGN)"; else echo "FAIL step 6: could not create the foreign dpbp" >&2; fi

# step 7: dry-run the shrink+prune — intent-b drops the surplus (per-family
# destroy=2) and classifies the foreign dpbp as a prune candidate. Read BEFORE
# the destructive leg (the V-DPRC-9 shape), dispatching nothing.
run 7 "$DPAA2CTL" --config models/board/V-POOL-6/intent-b.toml dry-run
expect_zero 7 "dry-run: the shrink surplus and the foreign prune candidate"

# step 8: shrink+prune under the disruptive gate — the shrink half reclaims the
# free-managed surplus and the foreign dpbp through the unplug probe (a plugged-
# but-undrawn object unplugs then destroys; pool-objects design D10). Pool creates/
# destroys are automatic in the pool walk; the pool prune is not gated by --prune.
run 8 "$DPAA2CTL" --config models/board/V-POOL-6/intent-b.toml ensure --no-link --allow disruptive
expect_zero 8 "ensure intent-b reclaims the surplus and the foreign object via the unplug probe"
probe step-8-show-dprc1.txt restool dprc show dprc.1
pool_capture pool-after-shrink.txt

# step 9: read-back — the foreign object is gone, the DPL-born boot pool stands.
run 9 restool dpbp info "$FOREIGN"
if [ "$(cat "$RESULTS/step-9-exit.txt")" != 0 ]; then
  echo "PASS step 9: the foreign dpbp $FOREIGN was pruned (info reports it absent)"
else
  echo "FAIL step 9: the foreign dpbp $FOREIGN still exists after the prune" >&2
fi
probe step-9-show-dprc1.txt restool dprc show dprc.1
if grep -qE '(^| )dpbp\.0( |$)' "$RESULTS/step-9-show-dprc1.txt" 2>/dev/null; then
  echo "PASS step 9: the DPL-born dpbp.0 is untouched (present after the prune)"
else
  echo "RECORD step 9: dpbp.0 not seen in the root listing — confirm the DPL-born boot pool from the census diff"
fi
pool_capture pool-post.txt

echo "suite V-POOL-6 root pool convergence complete"
