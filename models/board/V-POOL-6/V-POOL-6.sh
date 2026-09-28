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
# idempotence (a second ensure dispatches nothing), make two foreign restool pool
# objects, then intent-b NARROWS the requirement — but root capacity is grow-only
# (ADR-0020 decisions 1-2), so the surplus renders as reboot-required residue, NOT
# reclaimed, while the labeled foreign is pruned (born unplugged, ADR-0020 decision 4)
# and the bare unlabeled foreign + the DPL-born boot pool stay untouched (one-label
# law; pool-objects design D3). Results are captured for offline diff. The operands'
# derivation is pinned offline by crates/dpaa2-tools/tests/vpool6_intents.rs.
#
# Root is grow-only (ADR-0020): every plugged root object is fsl_mc_allocator-bound
# with no safe drawn-ness signal, so a runtime shrink/reclaim of managed root capacity
# is not attempted — the surplus is reported and the reboot named (ADR-0003 §7). The
# unplug-probe reclaim law and the ShrinkBelowDraw refusal are CHILD-scoped (ADR-0020
# decision 3); at root prune keeps only the reach the hardware allows: a never-plugged
# undeclared object (ADR-0020 decision 4). This suite exercises the ROOT leg, so no
# managed destroy fires here; the child reclaim / ShrinkBelowDraw face stays twin-covered
# (shrink_below_draw_refuses_by_name_and_count in pool_replay).
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
# expect_line N regex label: PASS iff a captured line of step N matches the regex.
# The census regexes anchor the ROOT block by its 2-space indent (render.rs:261, capitalized
# family), so a converged child family (4-space indent, render.rs:365) cannot false-PASS.
expect_line() { if grep -qE "$2" "$RESULTS/step-$1-out.txt" "$RESULTS/step-$1-err.txt" 2>/dev/null; then echo "PASS step $1: $3"; else echo "FAIL step $1: $3 (no line matching /$2/)" >&2; fi; }

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
  # Clear BOTH step-6 foreigns on every abort path (info-guarded: a stray already pruned is
  # skipped; the bare survivor is destroyed here — born unplugged, unlabeled).
  for f in "${FOREIGN:-}" "${FOREIGN_STRAY:-}"; do
    [ -n "$f" ] || continue
    restool dpbp info "$f" > /dev/null 2>&1 && restool dpbp destroy "$f" 2>>"$RESULTS/teardown.log" || true
  done
  pool_capture pool-teardown.txt
  save_dmesg
  echo "teardown: kernel dpni reconciled to empty; the managed trio capacity plus the grow-only dpio seats remain as reboot-required residue (ADR-0020 decision 2) — the closing reboot restores the DPL baseline (ADR-0003 §7)"
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

# step 6: TWO foreign root dpbps exercise the one-label law under ADR-0020 decision 4.
# (1) FOREIGN_STRAY carries an undeclared label 'stray' ⇒ Foreign, and a fresh create is
#     born unplugged, so root prune REACHES it (ADR-0020 decision 4). (2) FOREIGN is
#     bare/unlabeled ⇒ the empty label is the DPL sentinel, prune-EXEMPT (one-label law;
#     pool_lifecycle.rs:767) ⇒ it SURVIVES the shrink and only the EXIT trap clears it.
#     Both are exported for the trap (it destroys each on every abort path, info-guarded).
FOREIGN_STRAY="$(restool --script dpbp create 2>"$RESULTS/step-6-stray-err.txt")"
echo "$FOREIGN_STRAY" > "$RESULTS/step-6-stray.txt"
[ -n "$FOREIGN_STRAY" ] && restool dprc set-label "$FOREIGN_STRAY" --label=stray 2>>"$RESULTS/step-6-stray-err.txt" || true
if [ -n "$FOREIGN_STRAY" ]; then echo "PASS step 6: labeled foreign root dpbp created ($FOREIGN_STRAY, label=stray)"; else echo "FAIL step 6: could not create the labeled foreign dpbp" >&2; fi
FOREIGN="$(restool --script dpbp create 2>"$RESULTS/step-6-err.txt")"
echo "$FOREIGN" > "$RESULTS/step-6-foreign.txt"
if [ -n "$FOREIGN" ]; then echo "PASS step 6: bare unlabeled foreign root dpbp created ($FOREIGN)"; else echo "FAIL step 6: could not create the bare foreign dpbp" >&2; fi

# step 7 (drift narrows to residue, ADR-0020 decisions 1-2): intent-b drops the +2 extra,
# but root capacity is grow-only — the managed surplus is NOT destroyed, it renders as
# reboot-required residue (4-space marker, PoolResidue Display). The labeled stray is still a
# prune candidate (born unplugged; ADR-0020 decision 4). Read BEFORE the ensure (the V-DPRC-9
# shape), dispatching nothing.
run 7 "$DPAA2CTL" --config models/board/V-POOL-6/intent-b.toml dry-run
expect_zero 7 "dry-run: the root surplus residue and the labeled foreign prune candidate"
expect_out 7 "    reboot-required: Dpbp pool:" "dry-run reports the dpbp surplus as reboot-required residue (ADR-0020)"
expect_out 7 "    reboot-required: Dpmcp pool:" "dry-run reports the dpmcp surplus as reboot-required residue (ADR-0020)"
expect_out 7 "    reboot-required: Dpcon pool:" "dry-run reports the dpcon surplus as reboot-required residue (ADR-0020)"
expect_line 7 '^  Dpcon observed managed=[0-9]+/[0-9]+ required=32 \[hitless\] create=0 destroy=0 prune=0' "root dpcon surplus is NOT reclaimed (destroy=0, grow-only)"

# step 8 (ensure intent-b, ADR-0020 decisions 1-2): the root managed surplus is REPORTED as
# reboot-required residue (no-indent `residue:` marker, main.rs), NOT reclaimed (root
# grow-only, no runtime destroy); the labeled stray foreign is pruned (born unplugged,
# ADR-0020 decision 4). ensure exits zero — residue is a report, not a failure. The surplus
# persists to the closing reboot (ADR-0003 §7).
run 8 "$DPAA2CTL" --config models/board/V-POOL-6/intent-b.toml ensure --no-link --allow disruptive
expect_zero 8 "ensure intent-b exits zero over the root surplus (residue is a report, not a failure)"
expect_out 8 "residue: Dpbp pool:" "ensure reports the dpbp surplus as reboot-required residue (ADR-0020 decision 2)"
expect_out 8 "residue: Dpmcp pool:" "ensure reports the dpmcp surplus as reboot-required residue (ADR-0020 decision 2)"
expect_out 8 "residue: Dpcon pool:" "ensure reports the dpcon surplus as reboot-required residue (ADR-0020 decision 2)"
probe step-8-show-dprc1.txt restool dprc show dprc.1
pool_capture pool-after-shrink.txt

# step 9: read-back — the labeled stray is pruned, the bare unlabeled foreign STANDS
# (empty-label DPL sentinel, prune-exempt; one-label law), the DPL-born boot pool stands,
# and the managed census still equals INTENT-A's counts (grow-only: no reclaim, ADR-0020).
run 9 restool dpbp info "$FOREIGN_STRAY"
if [ "$(cat "$RESULTS/step-9-exit.txt")" != 0 ]; then
  echo "PASS step 9: the labeled foreign dpbp $FOREIGN_STRAY was pruned (info reports it absent, ADR-0020 decision 4)"
else
  echo "FAIL step 9: the labeled foreign dpbp $FOREIGN_STRAY still exists after the prune" >&2
fi
run 10 restool dpbp info "$FOREIGN"
expect_zero 10 "the bare unlabeled foreign $FOREIGN survives the shrink (empty-label DPL sentinel is prune-exempt, one-label law)"
probe step-9-show-dprc1.txt restool dprc show dprc.1
if grep -qE '(^| )dpbp\.0( |$)' "$RESULTS/step-9-show-dprc1.txt" 2>/dev/null; then
  echo "PASS step 9: the DPL-born dpbp.0 is untouched (present after the shrink)"
else
  echo "RECORD step 9: dpbp.0 not seen in the root listing — confirm the DPL-born boot pool from the census diff"
fi
# Managed census UNCHANGED — a dry-run of intent-a reads the grown counts as converged
# (dpmcp still 20), proving the a->b narrowing reclaimed nothing at root (ADR-0020 decision 1).
run 11 "$DPAA2CTL" --config models/board/V-POOL-6/intent-a.toml dry-run
expect_zero 11 "post-shrink dry-run of intent-a reads the grown pool"
expect_line 11 '^  Dpmcp observed managed=20/[0-9]+ required=20 \[hitless\] create=0 destroy=0 prune=0' "root dpmcp still at the grown 20 (grow-only: intent-b reclaimed nothing)"
pool_capture pool-post.txt

echo "suite V-POOL-6 root pool convergence complete"
