#!/bin/sh
# suite: V-ROLLBACK-1
# class: mutating
# rev: 4 (plan-rollback design D12). The step-1 precondition now also gates on the
# MC-global bp row (G > D) with every count runtime-derived from the dry-run global
# row; the drain writer drains the runtime-derived post-grow remainder (NEEDED =
# G - D) to zero within the ~5s kernel-bind window, so the run's next (child-
# population) dpbp create refuses at the empty pool (closing the rev-3 to-exhaustion
# race); and step 4 greps the global-budget planning-time refusal voice.
# HAND-AUTHORED — not emitted by dpaa2-verify. The suite drives `dpaa2ctl`
# intents (read -> compile -> admit -> dispatch), not a model-trace of restool
# verbs, and it owns a background drain writer the trace vocabulary cannot
# express, so there is no generator to regenerate from. Edit this file directly.
# operator: run as root (the MC portal /dev/dprc.1 is 0600 root:root, V-DPRC-4).
# RUN THIS SUITE LAST IN A SITTING AND REBOOT AFTER IT. It binds a kernel dpni on
# dpmac.4 (evicting that port's standalone driver for the run) and leaves
# grow-only root-pool residue the rollback cannot reverse (ADR-0020, ADR-0003
# §7); only a reboot reclaims those. The closing reboot is the operator's.
#
# WHAT THIS WITNESSES (plan-rollback spec "The board program witnesses rollback
# under kernel probing"; plan-rollback design D1/D6/D7; the offline twin is
# crates/dpaa2-verify/tests/rollback_overcommit.rs). Admission is lockless and
# check-without-reservation (ADR-0023 design D5/D7/D8): it reads the observed
# budgets ONCE at run start and never reserves. This suite makes a LATER create
# of the SAME `ensure` run refuse MC status 0x8 (No resources) because a
# concurrent writer drained the budget after admission passed — the overcommit
# (plan-rollback design D1), discovered at dispatch, which the default rollback
# policy unwinds.
#
# THE STAGING (the one design point this suite owns). Two candidate shapes were
# weighed against the board registers:
#   (a) a suite-owned background writer that drains the raced family after the
#       run's first actuation, and
#   (b) the kernel probe itself as the concurrent consumer (DPSW-I9's draw).
# [SUPERSEDED IN PART by bead dpaa2-controlplane-f59: V-CONC-1's single-opener law means the writer and engine collide on the root portal, so the dpaa2-mc shim now retries portal-open EINVAL and a lost race is a benign skip, not a false failure; decisiveness of shape (a) remains open on bead dpaa2-controlplane-3qv.]
# [SETTLED by ADR-0024: the shape question is decided — the mid-run witness is not reachable on the restool transport and is deferred to the ioctl-portal trigger; this suite keeps its machinery verdict and its re-sit requirement is retired (bead dpaa2-controlplane-275).]
# [RETRACTED by ADR-0024: the paragraph below is the pre-ADR-0024 staging rationale. The writer's draw cannot be placed deterministically between two engine verbs on the restool transport, so the drain is NOT decisive; the machinery is board-proven (rev 3), the live mid-run witness is deferred per ADR-0024, and the aged-board ceiling conflation (bead dpaa2-controlplane-b4c) is resolved by the plan-rollback capacity-census delta (task 12.2).]
# This suite takes (a) for the DRAIN because it is decisive — the writer drains
# a runtime-derived handful of the root dpbp pool on a deterministic signal, so the run's next
# dpbp create provably refuses — where the kernel probe's draw is asynchronous
# (DPSW-I9: defer-not-fail, automatic retry races, explicit kick needed;
# docs/baseline/dpsw.md DPSW-I9) and so not decisively orderable. Shape (b) is
# still LIVE: the intent binds a kernel dpni on dpmac.4, so the kernel side is
# actively probing (the DPSW-I9 race class, the V-TRAF-1 census-competitor
# class) while the overcommit fires. The two together give a decisive drain
# under genuine live kernel probing.
#
# WHY dpbp IS THE RACED FAMILY, AND THE OPERATOR PRECONDITION. Admission reads
# the ROOT pool's free counts (crates/dpaa2-tools/src/engine.rs plan_admission,
# observe_pool(None, ...)). The reference board is pool-tight: V-ADMIT-1 (ledger)
# recorded dpbp free headroom 1 and the root dpcon pool locally dry (V-DPRC-10:
# all 16 dpcons plugged/held). dpcon free 0 means admission refuses ANY dpcon
# draw at planning time, so dpcon cannot host an overcommit (admission never
# passes). That leaves dpbp — the only tight-but-nonzero family — as the raced
# family. But an overcommit needs admission to PASS FIRST (gross dpbp draw <=
# observed free), and the reference board's free dpbp (1) is below a kernel
# port's dpbp draw (V-ADMIT-1 saw draw 2 refused at free 1), so on the board as
# recorded admission would REFUSE at planning time — the V-ADMIT-1 headroom
# verdict, not an overcommit.
#
# OPERATOR PREREQUISITE (declared, because the recorded board state cannot stage
# this witness unaided): before the sitting, the operator must ensure (i)
# ADMISSION ADMITS EVERY DRAWN BUDGET — dpcon was recorded dry at the reference
# boot (V-DPRC-10), and any dry drawn family refuses the whole intent at
# planning time, the V-ADMIT-1 verdict, never an overcommit — and (ii) the ROOT
# container's free dpbp headroom sits at least one above the intent's gross dpbp
# draw D, so the writer has a handful to drain after the grow pass. Step 1 reads
# the overall admission verdict, D, the observed census free F, and the MC-global
# bp free G from the dry-run and REFUSES TO PROCEED (a PRECONDITION skip, exit 2,
# distinct from a witness FAIL) unless admission admits and both F > D and G > D.
# Nothing in this script frees a dpbp
# itself — that would mean tearing down a boot resident (ADR-0003 §4,
# ADR-0008 §7), forbidden.
#
# THE RUN ORDER THAT PUTS THE OVERCOMMIT UNDER KERNEL PROBING. `ensure` phases
# are, in order (crates/dpaa2-tools/src/main.rs): grow root pool -> port loop
# (bind the kernel dpni) -> converge child containers -> populate child
# (its dpni + dpbp companion) -> links -> switches -> prune. The grow pass
# pre-creates the kernel interface's dpbp (the admitted draw D). The port loop
# then binds the kernel dpni on dpmac.4 — the kernel is now probing. The writer,
# having waited for that bind, drains the runtime-derived handful (NEEDED) of root dpbp, so
# the LATER child-population dpbp create (not counted in D) refuses 0x8 while the
# kernel dpni is live. This mirrors the offline twin, whose overcommit also fires
# on a create that follows a bound dpni prefix.
#
# RESIDUE IS REACHABLE AND GRADED. The grow pass creates and plugs the kernel
# interface's root-pool dpbp before the overcommit, and a plugged root pool
# object is grow-only reboot-named residue (ADR-0020; the render snapshot
# crates/dpaa2-tools/tests/snapshots/rollback_cli__overcommit_rollback_report_is_actionable.snap
# carries a `residue:` line for exactly this). So the rollback report's
# `  residue: ` line is expected PRESENT here and is graded.
#
# SAFETY (ADR-0003 §4, ADR-0008 §7). The total-deny trio (macs 3 and 17, the
# management dpni) is NEVER referenced in object syntax anywhere in this file —
# the self-check below greps this header too, so the trio stays prose. Only unwired
# lifecycle-safe ports are named (dpmac.4 kernel, dpmac.5 remote-VFIO). The suite
# creates and destroys objects ONLY in its own scope: the writer's scratch child
# (and the dpbp drawn into it) is reclaimed by the single EXIT-trap teardown; the
# root container's standing residents are never unbound or destroyed. The run's
# own objects are unwound by the rollback policy itself; its grow-only residue is
# left standing for the closing reboot, never torn down here.
#
# Operand derivation is pinned offline by the vrollback1_intents pin test
# (crates/dpaa2-tools/tests/vrollback1_intents.rs); this script never validates
# the operand against /sys itself.
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
OPERAND=models/board/V-ROLLBACK-1/intent-overcommit.toml
[ -f "$OPERAND" ] || { echo "refusing: not the repo root ($OPERAND missing); run this script from its checkout" >&2; exit 1; }

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

# --- kernel-log window (ADR-0008) + single EXIT-trap teardown ---
# A marker stamps the sitting's start; the close saves everything after it. The
# one trap also reclaims the writer's scratch child (idempotent): a cascade
# destroy returns its drawn dpbp to the pool (V-DPRC-7 rev 2). It NEVER touches a
# boot resident or any object the suite did not create, and it leaves the run's
# own grow-only residue standing for the closing reboot.
KMSG="dpaa2-verify V-ROLLBACK-1 pid $$"
echo "$KMSG start" > /dev/kmsg 2>/dev/null || true
WRITER_PID=""
WRITER_CHILD_FILE="$RESULTS/writer-child.txt"
save_dmesg() {
  dmesg 2>/dev/null | awk -v m="$KMSG start" 'w || index($0, m) { w = 1 } w' > "$RESULTS/dmesg.txt"
  [ -s "$RESULTS/dmesg.txt" ] || dmesg > "$RESULTS/dmesg.txt" 2>&1 || true
}
reclaim_writer() {
  [ -f "$WRITER_CHILD_FILE" ] || return 0
  wc="$(cat "$WRITER_CHILD_FILE" 2>/dev/null)"
  [ -n "$wc" ] || return 0
  if restool dprc show "$wc" >/dev/null 2>&1; then
    echo "+ (teardown) restool dprc destroy $wc"
    restool dprc destroy "$wc" >>"$RESULTS/teardown.log" 2>&1 || true
  fi
}
cleanup() {
  if [ -n "$WRITER_PID" ]; then kill "$WRITER_PID" 2>/dev/null || true; fi
  reclaim_writer
  save_dmesg
}
trap cleanup EXIT

# --- reference pair assertion (ADR-0003 §2) ---
# Evidence is only valid against the stamped pair; refuse anything else.
mc="$(restool -m 2>/dev/null || true)"
case "$mc" in *10.39.0*) ;; *) echo "refusing: MC firmware is not 10.39.0: $mc" >&2; exit 1 ;; esac
kernel="$(uname -r)"
case "$kernel" in 6.6.52*) ;; *) echo "refusing: kernel is not 6.6.52: $kernel" >&2; exit 1 ;; esac

# --- residents helper for the census (ADR-0008 §4–§5, §8) ---
# At the PRE face dpmac.4 must hold its reference fsl_dpaa2_mac like every
# resident (an eviction already present means a dirty board). The run's kernel
# dpni evicts it mid-run, so the POST face tolerates exactly dpmac.4 reading
# unbound (or rebound to reference after the unwind) and still fails any other
# mismatch — graded inline at step 3, not via RESIDENTS_EXPECT_UNBOUND, whose
# contract expects the object unbound at every face. The closing reboot
# restores the driver either way.
. models/board/residents.sh

# --- helpers ---
R="$RESULTS/rollback.txt"         # the PASS/FAIL face log, counted at close
log() { echo "$1" | tee -a "$R"; }
# run N cmd...: echo, execute, capture stdout, stderr and the exit code.
run() { n="$1"; shift; echo "+ $*"; "$@" >"$RESULTS/step-$n-out.txt" 2>"$RESULTS/step-$n-err.txt"; echo $? > "$RESULTS/step-$n-exit.txt"; }
# probe FILE cmd...: capture a read-back to a named file; never fail the script.
probe() { f="$1"; shift; echo "+ (probe) $*"; "$@" > "$RESULTS/$f" 2>/dev/null || true; }
# pool_capture FILE: snapshot MC-global resource pools (ADR-0011), best-effort.
pool_capture() { restool dprc show mc.global --resources > "$RESULTS/$1" 2>/dev/null || true; }
# eth_count: how many dpni objects are bound to the kernel eth driver right now
# (the kernel interfaces). The baseline is captured at step 0; the drain writer
# fires when this rises, i.e. when the run's kernel dpni has bound and is
# probing. The driver's sysfs symlink basename is fsl_dpaa2_eth on the board
# (dmesg: "fsl_dpaa2_eth dpni.1: Probed interface eth1"); the old dpaa2-eth
# spelling is kept as tolerance. The underscore basename is the real cause of
# the rev4e TIMEOUT drain (results/V-ROLLBACK-1/rev4e/writer.log).
eth_count() {
  c=0
  for d in /sys/bus/fsl-mc/devices/dpni.*; do
    [ -e "$d" ] || continue
    drv="$(readlink "$d/driver" 2>/dev/null)"
    case "${drv##*/}" in fsl_dpaa2_eth|dpaa2-eth) c=$((c + 1)) ;; esac
  done
  echo "$c"
}

# --- the drain writer (the suite's concurrent consumer) ---
# Fires on the kernel-bind signal, then drains exactly NEEDED dpbp (the step-1
# runtime-derived handful) from the ROOT pool into its own scratch child so the
# run's next dpbp create refuses 0x8 on the MC-global ledger. NEEDED is a handful
# that fits the ~5s kernel-bind window (rev 4, closing the rev-3 to-exhaustion
# race). Every object it makes is reclaimed by the EXIT trap. The scratch child's
# dpbp are off the fsl-mc bus (DPRC-I6), so no allocator unbind is needed before
# the cascade destroy. The loop is still hard-capped at the dpbp ceiling (63,
# V-CEIL-1) so a mis-staged NEEDED cannot spin forever.
drain_writer() {
  child="$(restool dprc create dprc.1 2>>"$RESULTS/writer.log" | grep -oE 'dprc\.[0-9]+' | head -1)"
  if [ -z "$child" ]; then
    echo "writer: could not create scratch child" >> "$RESULTS/writer.log"
    return 0
  fi
  echo "$child" > "$WRITER_CHILD_FILE"
  # Wait for the run's kernel dpni to bind (eth_count rises above the baseline),
  # bounded by a timeout; record whether the signal arrived.
  waited=0
  while [ "$waited" -lt 120 ]; do
    now="$(eth_count)"
    if [ "$now" -gt "$ETH_BASE" ]; then
      echo "writer: kernel bind observed (eth $ETH_BASE -> $now) after ${waited}s" >> "$RESULTS/writer.log"
      break
    fi
    sleep 1
    waited=$((waited + 1))
  done
  [ "$waited" -lt 120 ] || echo "writer: TIMEOUT waiting for kernel bind; draining anyway" >> "$RESULTS/writer.log"
  # Drain exactly NEEDED dpbp into the scratch child — the post-grow remainder that
  # drains the MC-global bp free to zero inside the kernel-bind window, so the run's
  # next (child-population) dpbp create refuses 0x8 at the empty pool.
  # Still hard-capped at the dpbp ceiling (63, V-CEIL-1) so a mis-staged NEEDED
  # cannot spin forever.
  cap=63
  target="$NEEDED"
  [ "$target" -le "$cap" ] || target="$cap"
  drained=0
  while [ "$drained" -lt "$target" ]; do
    if restool dpbp create --container="$child" >>"$RESULTS/writer.log" 2>&1; then
      drained=$((drained + 1))
    else
      break
    fi
  done
  echo "$drained" > "$RESULTS/writer-drained.txt"
  echo "writer: drained $drained dpbp into $child" >> "$RESULTS/writer.log"
}

# step 0: pre census — the baseline the post-run and post-teardown censuses are
# diffed against. The root show and MC-global pools are captured whole;
# residents_pre checks every boot resident against its clean-boot reference.
probe step-0-show-dprc1.txt restool dprc show dprc.1
pool_capture pool-baseline.txt
ETH_BASE="$(eth_count)"
echo "$ETH_BASE" > "$RESULTS/eth-baseline.txt"
if residents_pre; then log "PASS step 0: residents baseline on their reference drivers"; else log "FAIL step 0: a boot resident is off its reference driver"; fi

# step 1: dry-run reads the admission verdict, the dpbp census budget row, the
# MC-global bp row, and previews the default (rollback) failure policy. The rows are
# `dpbp draw=D headroom=F [...]` (census) and `mc-global dpbp draw=D headroom=G [...]`
# (the MC-global ledger, plan-rollback design D7). The PRECONDITION gate: proceed
# only if admission admits AND the census free F > D AND the MC-global free G > D
# (drain room on both ledgers). Every count is runtime-derived from the dry-run,
# never hardcoded. NEEDED = G - D is the writer's drain count — the post-grow
# remainder it drains to zero, so the run's next (child-population) dpbp create
# refuses at the empty pool. A missing mc-global row, or an
# unmet gate, is an operator-prerequisite skip (exit 2), NOT a witness FAIL.
run 1 "$DPAA2CTL" --config "$OPERAND" dry-run
rc1="$(cat "$RESULTS/step-1-exit.txt")"
# The census row renders before the mc-global row (render_admission emits the pool
# row then the global row), so head -1 on the bare `dpbp draw=` pattern keys on the
# census row; G is read from the mc-global row explicitly.
D="$(sed -n 's/.*dpbp draw=\([0-9]*\) headroom=\([0-9]*\).*/\1/p' "$RESULTS/step-1-out.txt" | head -1)"
F="$(sed -n 's/.*dpbp draw=\([0-9]*\) headroom=\([0-9]*\).*/\2/p' "$RESULTS/step-1-out.txt" | head -1)"
G="$(sed -n 's/.*mc-global dpbp draw=[0-9]* headroom=\([0-9]*\).*/\1/p' "$RESULTS/step-1-out.txt" | head -1)"
# A missing MC-global bp row cannot stage the global-ledger witness: the engine proves
# that row from the live `dprc show mc.global --resources` listing, so its absence is an
# operator prerequisite, not a witness FAIL.
if [ -z "$G" ]; then
  echo "PRECONDITION NOT MET: the dry-run rendered no mc-global dpbp row; the engine proves the global"
  echo "bp ledger from 'dprc show mc.global --resources' — stage a board whose MC-global bp pool is proven before this sitting (see the script header)."
  echo "suite V-ROLLBACK-1 not run: operator prerequisite unmet"
  exit 2
fi
NEEDED=$(( G - ${D:-0} ))
[ "$NEEDED" -ge 1 ] || NEEDED=1
if [ "$rc1" = 0 ] && grep -q 'on failure: rollback' "$RESULTS/step-1-out.txt" && [ -n "$D" ] && [ -n "$F" ]; then
  log "PASS step 1: dry-run exits 0, previews the rollback policy, dpbp draw=$D headroom=$F, mc-global headroom=$G (writer drains NEEDED=$NEEDED)"
else
  log "FAIL step 1: dry-run did not render the rollback preview and the dpbp census+global budget rows (exit $rc1)"
fi
ADMITTED=0
grep -q 'admission (.*) \[admitted\]:' "$RESULTS/step-1-out.txt" && ADMITTED=1
if [ "$ADMITTED" != 1 ] || [ -z "$D" ] || [ -z "$F" ] || [ "$F" -le "$D" ] || [ "$G" -le "$D" ]; then
  echo "PRECONDITION NOT MET: admission must admit every drawn budget (admitted=$ADMITTED); root free dpbp (F=${F:-?}) and MC-global bp free (G=${G:-?}) must both sit strictly above the gross dpbp draw (D=${D:-?})."
  echo "The reference board is pool-tight (dpbp near-dry, dpcon recorded dry at the reference boot); stage free headroom for every drawn family before this sitting (see the script header)."
  echo "suite V-ROLLBACK-1 not run: operator prerequisite unmet"
  exit 2
fi

# step 2: the overcommit run. Launch the writer, then `ensure` under the DEFAULT
# rollback policy (no --on-failure flag). The grow pass pre-creates the kernel
# dpbp; the port loop binds the kernel dpni (writer's signal); the writer drains
# the rest of the root dpbp pool; the child-population dpbp create then refuses
# 0x8 mid-run and the default policy unwinds. The run exits nonzero and its
# stdout carries the task-6.1 overcommit report.
drain_writer &
WRITER_PID=$!
run 2 "$DPAA2CTL" --config "$OPERAND" ensure --allow disruptive
rc2="$(cat "$RESULTS/step-2-exit.txt")"
wait "$WRITER_PID" 2>/dev/null || true
WRITER_PID=""
DRAINED="$(cat "$RESULTS/writer-drained.txt" 2>/dev/null || echo 0)"
if [ "$rc2" != 0 ] \
  && grep -qF 'refused: overcommit — admission passed, but a concurrent writer drained the resource budget mid-run' "$RESULTS/step-2-out.txt" \
  && grep -q '^  unwound: ' "$RESULTS/step-2-out.txt" \
  && grep -qF 're-apply refuses until headroom returns.' "$RESULTS/step-2-out.txt"; then
  log "PASS step 2: ensure overcommits mid-run ($rc2), unwinds the prefix, and names re-apply refusal"
else
  log "FAIL step 2: ensure did not report the mid-run overcommit rollback (exit $rc2)"
fi
# The writer's drain is what starved the run; record its strength. Fewer than NEEDED
# means the writer lost the kernel-bind window (or the pool shifted under it) — the
# overcommit then did not come decisively from the concurrent writer, so the witness
# is weaker, not a pass.
if [ "$DRAINED" -ge "$NEEDED" ]; then
  log "PASS step 2: the concurrent writer drained $DRAINED dpbp (>= NEEDED=$NEEDED, the genuine mid-run drain)"
else
  log "FAIL step 2: the writer drained $DRAINED dpbp, short of NEEDED=$NEEDED — no decisive mid-run drain staged (precondition/timing)"
fi
# Residue is reachable here (a plugged grow-pass dpbp is grow-only residue); grade it.
if grep -q '^  residue: ' "$RESULTS/step-2-out.txt"; then
  log "PASS step 2: the rollback report names typed reboot residue"
else
  log "FAIL step 2: no residue line — expected the grow-pass pool object as grow-only residue"
fi

# step 3: post-run census — the unwind completed without wedging. residents_post
# records dpmac.4 unbound (expected for the run) and fails any OTHER resident off
# its reference driver; the kernel log window is saved. The root/pool census is
# captured for the operator diff (it is NOT byte-identical here: the drain
# writer's child stands and the run left grow-only residue — both accounted for,
# the writer's by teardown and the residue by the reboot).
probe step-3-show-dprc1.txt restool dprc show dprc.1
pool_capture pool-post.txt
save_dmesg
if residents_post; then
  log "PASS step 3: no boot resident off its reference driver (dpmac.4 rebound after the unwind)"
else
  # Tolerate exactly dpmac.4 reading unbound — the run's kernel dpni evicted
  # it; the reboot restores it. Any other mismatch is a real residents FAIL.
  others="$(awk '$2 != $3 && !($1 == "dpmac.4" && $3 == "unbound")' "$RESULTS/residents-post.txt")"
  if [ -z "$others" ]; then
    log "PASS step 3: dpmac.4 unbound (evicted by this run's kernel dpni, reboot restores it); every other resident on reference"
  else
    log "FAIL step 3: a boot resident is off its reference driver"
  fi
fi

# step 4: the follow-on refusal while the drain still holds (plan-rollback spec
# "The re-run refuses in the global-budget voice" — one rollback per drain, no
# oscillation). The writer's drained dpbp are still held OFF the MC-global bp
# ledger, so a re-run refuses at ADMISSION on the MC-global row and dispatches
# nothing (the V-ADMIT-1 step-2 proof that an admission refusal returns before any
# verb).
run 4 "$DPAA2CTL" --config "$OPERAND" ensure --allow disruptive
rc4="$(cat "$RESULTS/step-4-exit.txt")"
if [ "$rc4" != 0 ] && grep -qE 'refused: mc-global pool dpbp draw [0-9]+ exceeds global free headroom [0-9]+' "$RESULTS/step-4-out.txt"; then
  log "PASS step 4: the re-run refuses at admission on the MC-global dpbp row, dispatching nothing"
else
  log "FAIL step 4: the re-run did not refuse at admission in the MC-global dpbp voice (exit $rc4)"
fi

# step 5: teardown + post-teardown census. Reclaim the writer's scratch child
# (cascade destroy returns its dpbp to the pool), then read the census back.
# The root show should now match the baseline UP TO the run's grow-only residue
# (reclaimed only by the closing reboot); the drain writer's objects are gone.
WRITER_CHILD="$(cat "$WRITER_CHILD_FILE" 2>/dev/null || echo __none__)"
reclaim_writer
rm -f "$WRITER_CHILD_FILE"
probe step-5-show-dprc1.txt restool dprc show dprc.1
pool_capture pool-teardown.txt
if [ "$WRITER_CHILD" != __none__ ] && restool dprc show "$WRITER_CHILD" >/dev/null 2>&1; then
  log "FAIL step 5: the writer's scratch child was not reclaimed by teardown"
else
  log "PASS step 5: teardown reclaimed the writer's scratch child and its drained dpbp"
fi

# --- close: count the face log and exit nonzero on any FAIL (ADR-0003 §4) ---
# REMINDER TO THE OPERATOR: reboot the board now — grow-only residue stands.
echo "reminder: reboot the board after this suite; grow-only root-pool residue stands until then" | tee -a "$R"
passes="$(grep -c '^PASS ' "$R")"
fails="$(grep -c '^FAIL ' "$R")"
echo "suite V-ROLLBACK-1 complete: $passes PASS, $fails FAIL"
[ "$fails" = 0 ] || exit 1
