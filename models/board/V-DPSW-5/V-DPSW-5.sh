#!/bin/sh
# suite: V-DPSW-5
# class: traffic-bearing  (explicitly flagged run — the uplink takes wired dpmac.7)
# HAND-AUTHORED — not emitted by dpaa2-verify. The steps are `dpaa2ctl`
# invocations (dry-run -> ensure -> read-back hooks -> teardown), not model-trace
# restool verbs, so there is no generator to regenerate from. Edit directly.
#
# The end-to-end switched-domain witness of dpsw-typestate (task 6.2, bead
# dpaa2-controlplane-dbf.16; the system-integration delta "The switched domain is
# witnessed end to end on the board"; dpsw-typestate design D9/D10). One switched
# domain converges the board to a kernel-bound, traffic-capable switch, over the
# three system-integration scenarios:
#   (1) CONVERGE the switched-domain intent via dpaa2ctl from the committed
#       intent-a.toml: companions exactly per DPSW-I3 (1 dpmcp + 1 dpbp from the
#       dpsw's own container, 0 dpcon, 0 dpio), the dpsw created bindable (DPSW-I1),
#       ports connected per the planned edges, the dpaa2-switch bind judged SETTLED
#       via bind_settle, and the read-back census equal to the compiled plan.
#   (2) INTERPOSITION: a flat edge-dpni <-> dpmac.7 stands first (its own created
#       dpni, intent-flat.toml), then the switched convergence SEVERS that edge
#       through the typed severed-witness path (the dpmac-typestate edge law,
#       ADR-0008 §8) before wiring both ends into the switch — no driverless
#       interval observed (dpsw-typestate design D10; plan/dpsw.rs plan_interposition).
#   (3) TEARDOWN through the typed destroy path converges to the pre-suite baseline
#       census, unconditional via the teardown trap.
# Reachability-level traffic only (the V-TRAF-0 pattern): a small counted broadcast
# burst across the switch, the member dpni's statistics page as the frame oracle; no
# rate targets, nothing touches the peer's config. The operand derivation is pinned
# offline by crates/dpaa2-tools/tests/vdpsw5_intents.rs.
# The traffic leg is deferred (change #15 trigger, bead dpaa2-controlplane-dbf.19); verdict structural 8/8.
#
# SAFETY LAWS (non-negotiable, ADR-0003 §4; dpsw-typestate design D9):
#   - The uplink is dpmac.7 ONLY — the flagged wired pair, permitted here under the
#     traffic-bearing class. The ADR-0003 §4 total-deny objects are NEVER named in
#     this script (the self-check below greps for them, so prose may not name them either).
#   - The member dpni is this change's OWN created dpni (the `edge` tenant). No
#     standing topology object is reconciled; the teardown prunes only what the
#     reconciler owns (the one-label law, V-MVP-1).
#
# Banked verdicts are CITED, never re-run:
#   - V-DPSW-1 rev 2 (models/board/README.md): the positive create+connect+bind face
#     — the switch driver takes a runtime dpsw built ctrl-if on, flooding+broadcast
#     PER_FDB, companions drawn from its own container. This suite relies on that
#     bind face; it does not re-probe the bare lifecycle.
#   - V-DPSW-4 rev 1 (models/board/README.md): the bindability refusal face — the
#     MC creates+connects an ls-addsw-default (PER_VLAN/PER_OBJECT) switch and the
#     kernel refuses it at probe (-EOPNOTSUPP). The intent here derives only the
#     bindable shape (DPSW-I1), so that refusal surface is never re-exercised.
#
# OPERATOR, READ FIRST:
#   - Run as root — the ensure legs issue MC creates/destroys and a VFIO bind that
#     gate on CAP_NET_ADMIN (docs/baseline/mc-ioctl-policy.md).
#   - dpmac.7 is a FLAGGED production-peer port (ADR-0003 §4): the peer facing it
#     MUST be admin-up before the traffic leg, and the only peer action is a counted
#     broadcast burst — no peer config is touched (dpsw-typestate design D9).
#   - KNOWN HAZARD, V-MVP-1 rev 5 finding-49 / V-DPMAC-3: a phylink link-up Oops can
#     fire at an ordered teardown on a wired port (docs/upstream/phylink-dpni-churn-crash.md).
#     If it fires, POWER-CYCLE after the sitting — the ADR-0003 §7 recovery guarantee
#     is the backstop.
#   - The ensure grows the kernel's root pool; dpio seats are grow-only (ADR-0003 §7)
#     and this suite CANNOT reclaim them at teardown — run V-DPSW-5 LAST in a sitting
#     and REBOOT after. The post-suite census shows the grow-only residue until then.
set -u
RESULTS="${1:?usage: $0 <results-dir>}"
case "$RESULTS" in /*) ;; *) RESULTS="$PWD/$RESULTS" ;; esac
mkdir -p "$RESULTS"

# --- repo-root cd (dpsw-typestate design D9; ADR-0003 §2) ---
# The dpaa2ctl steps name the operand relative to the repo root. SELF is resolved
# absolute BEFORE the cd so the total-deny self-check greps the right file.
SELF="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")"
cd "$(dirname "$SELF")/../../.." || { echo "refusing: cannot reach the repo root from $SELF" >&2; exit 1; }
INTENT="models/board/V-DPSW-5/intent-a.toml"
INTENT_FLAT="models/board/V-DPSW-5/intent-flat.toml"
[ -f "$INTENT" ] || { echo "refusing: not the repo root ($INTENT missing); run this script from its checkout" >&2; exit 1; }
[ -f "$INTENT_FLAT" ] || { echo "refusing: $INTENT_FLAT missing" >&2; exit 1; }

# --- dpaa2ctl from this checkout's build output (ADR-0003 §2) ---
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
echo "git rev: $(git rev-parse HEAD 2>/dev/null || echo unknown)" > "$RESULTS/git-rev.txt"

# --- kernel-log window ---
KMSG="dpaa2-verify V-DPSW-5 pid $$"
echo "$KMSG start" > /dev/kmsg 2>/dev/null || true
save_dmesg() {
  dmesg 2>/dev/null | awk -v m="$KMSG start" 'w || index($0, m) { w = 1 } w' > "$RESULTS/dmesg.txt"
  [ -s "$RESULTS/dmesg.txt" ] || dmesg > "$RESULTS/dmesg.txt" 2>&1 || true
}

# --- independent safety self-check (ADR-0003 §4) ---
# dpmac.7 is permitted here under the flagged traffic-bearing class; the ADR-0003 §4
# total-deny set is unchanged and must never appear by name anywhere in this file.
if grep -nE 'dpmac[.]3([^0-9]|$)|dpmac[.]17([^0-9]|$)|dpni[.]0([^0-9]|$)' "$SELF" | grep -v safety-self-check; then
  echo "refusing: total-deny object referenced in this script" >&2  # safety-self-check
  exit 1
fi

# --- passive ioctl-payload capture tap (dpsw-typestate design D7; ADR-0004 §6/§8) ---
# A bpftrace program on the ioctl syscall tracepoints, filtered to comm==restool and
# restool's SEND_MC_COMMAND request, records the 64-byte MC command+response of every
# MC ioctl. It is SYSTEM-WIDE and comm-filtered, so dpaa2ctl's restool children are
# captured with no per-spawn plumbing. Observation-only: it never alters a step. The
# payloads parse offline into the #10 encode-fixture corpus (crates/dpaa2-verify
# src/board/capture.rs). A dead tap fails the sitting loudly before any mutating step.
BPFTRACE="${BPFTRACE:-bpftrace}"
# dpaa2-hal portal::SEND_MC_COMMAND = _IOWR('R', 0xE0, 64); the single pinned request.
CAP_CMD="0xc04052e0"
CAP_PROG='tracepoint:syscalls:sys_enter_ioctl /comm == "restool" && args->cmd == '"$CAP_CMD"'/ { @p[tid] = args->arg; printf("CMD fd=%d %r\n", args->fd, buf(args->arg, 64)); } tracepoint:syscalls:sys_exit_ioctl /@p[tid] != 0/ { printf("RSP ret=%d %r\n", args->ret, buf(@p[tid], 64)); delete(@p[tid]); }'
CAP_PID=""
# cap_wait_attach FILE...: bounded wait for bpftrace's "Attaching" banner on ANY of the
# recorder's streams — the banner moved between stdout and stderr across bpftrace
# versions (board sitting 2026-10-18, v0.23.2 prints it on stdout; the Recorder in
# crates/dpaa2-verify src/board/capture.rs checks both the same way). The attach race
# is real; a timeout is a loud failure, never a shorter capture.
cap_wait_attach() { i=0; while [ "$i" -lt 10 ]; do grep -q Attaching "$@" 2>/dev/null && return 0; sleep 1; i=$((i+1)); done; return 1; }
# cap_start LABEL: spawn the recorder writing capture-LABEL.mc, wait for the banner.
cap_start() {
  st="$RESULTS/capture-$1.mc"; se="$RESULTS/capture-$1.err"
  "$BPFTRACE" -e "$CAP_PROG" > "$st" 2>"$se" &
  CAP_PID=$!
  cap_wait_attach "$st" "$se" || { kill "$CAP_PID" 2>/dev/null || true; CAP_PID=""; echo "refusing: capture recorder did not attach for $1 (dpsw-typestate design D7)" >&2; return 1; }
}
# cap_stop: SIGINT the recorder so bpftrace flushes, then reap it.
cap_stop() { [ -n "$CAP_PID" ] || return 0; kill -INT "$CAP_PID" 2>/dev/null || true; wait "$CAP_PID" 2>/dev/null || true; CAP_PID=""; }
# capture_preamble: prove the whole chain before any mutating step — bpftrace present,
# probes attach, and a read-only self-query yields >=1 CMD/RSP pair — else refuse.
capture_preamble() {
  command -v "$BPFTRACE" >/dev/null 2>&1 || { echo "refusing: capture tap requested but bpftrace ($BPFTRACE) not found (dpsw-typestate design D7)" >&2; return 1; }
  cap_start selftest || return 1
  restool dprc list >/dev/null 2>&1 || true
  sleep 1
  cap_stop
  { grep -q '^CMD ' "$RESULTS/capture-selftest.mc" && grep -q '^RSP ' "$RESULTS/capture-selftest.mc"; } || { echo "refusing: capture self-test yielded no CMD/RSP pair — a dead tap (dpsw-typestate design D7)" >&2; return 1; }
}

# --- helpers ---
# run N cmd...: echo, execute bracketed by the capture recorder (one trace file per
# step), capture stdout, stderr and the exit code.
run() { n="$1"; shift; echo "+ $*"; cap_start "step-$n" || exit 1; "$@" >"$RESULTS/step-$n-out.txt" 2>"$RESULTS/step-$n-err.txt"; rc=$?; cap_stop; echo "$rc" > "$RESULTS/step-$n-exit.txt"; }
probe() { f="$1"; shift; echo "+ (probe) $*"; "$@" > "$RESULTS/$f" 2>/dev/null || true; }
pool_capture() { restool dprc show mc.global --resources > "$RESULTS/$1" 2>/dev/null || true; }
expect_zero() { rc=$(cat "$RESULTS/step-$1-exit.txt"); if [ "$rc" = 0 ]; then echo "PASS step $1: $2"; else echo "FAIL step $1: $2 (exit $rc)" >&2; fi; }
# expect_out N needle label: PASS iff step N's captured output carries the needle.
expect_out() { if grep -qF "$2" "$RESULTS/step-$1-out.txt" "$RESULTS/step-$1-err.txt" 2>/dev/null; then echo "PASS step $1: $3"; else echo "FAIL step $1: $3 (missing '$2')" >&2; fi; }
# dpni_frames DPNI KEY: the restool dpni-statistics counter (V-TRAF-0 oracle), or empty.
dpni_frames() { restool dpni info "$1" 2>/dev/null | awk -v k="$2:" '$1 == k { print $2; exit }'; }

# bind_settle path name: a kernel bind lands only on a bus event. Wait up
# to 10s, then ask the bus to attach the device (drivers_probe, which an
# admin-unbound device needs since dprc sync re-fires only deferred probes,
# wait 5s), then the softer dprc sync (wait 10s). Settling, not evidence.
# Pasted verbatim from crates/dpaa2-verify/src/board/generate.rs; used for EVERY bind judgment.
bind_settle() { p="$1"; d="$2"; i=0; while [ "$i" -lt 10 ]; do readlink "$p" >/dev/null 2>&1 && return 0; sleep 1; i=$((i+1)); done; echo "+ (settle) bind deferred — asking the bus to probe: echo $d > drivers_probe" >&2; echo "$d" > /sys/bus/fsl-mc/drivers_probe 2>/dev/null || true; i=0; while [ "$i" -lt 5 ]; do readlink "$p" >/dev/null 2>&1 && return 0; sleep 1; i=$((i+1)); done; echo "+ (settle) still deferred — kicking the bus: restool dprc sync" >&2; restool dprc sync 1>&2 || true; i=0; while [ "$i" -lt 10 ]; do readlink "$p" >/dev/null 2>&1 && return 0; sleep 1; i=$((i+1)); done; echo "+ (settle) bind still deferred after kick" >&2; return 1; }

# --- reference pair assertion (ADR-0003 §2) ---
mc="$(restool -m 2>/dev/null || true)"
case "$mc" in *10.39.0*) ;; *) echo "refusing: MC firmware is not 10.39.0: $mc" >&2; exit 1 ;; esac
kernel="$(uname -r)"
case "$kernel" in 6.6.52*) ;; *) echo "refusing: kernel is not 6.6.52: $kernel" >&2; exit 1 ;; esac

# --- capture-tap preamble gate (dpsw-typestate design D7) ---
# Prove the whole chain before any mutating step and before the teardown trap is
# armed (nothing is created yet, so a refusal leaves nothing to tear down).
capture_preamble || exit 1

# --- unconditional teardown (ADR-0003 §6/§7) — the backstop, idempotent with the teardown leg ---
# Reconcile the switched domain back to baseline through the reconciler's own reverse
# path: an empty intent derives zero, and `ensure --prune` destroys the dpsw, its
# companions and the member dpni's child DPRC (the dpsw-typestate design D9 signature-
# multiset census delta); the dpsw edge kinds carry no teardown witness (dpsw-typestate design D5).
# dpio seats are grow-only and NOT reclaimed here — the closing reboot restores them.
teardown() {
  # Drop the traffic bridge first so the ensure reconciles clean netdevs (bead dbf.19).
  ip link del vdpsw5br 2>/dev/null || true
  printf '[intent]\nschema = 1\n' > "$RESULTS/teardown-empty.toml"
  "$DPAA2CTL" --config "$RESULTS/teardown-empty.toml" ensure --prune --allow disruptive --no-link \
    > "$RESULTS/teardown-ensure.txt" 2>>"$RESULTS/teardown.log" || true
  sleep 2
  pool_capture pool-teardown.txt
  save_dmesg
  echo "teardown: switched domain destroyed through the typed path; dpio seats are grow-only — reboot to reclaim (ADR-0003 §7)"
}
trap teardown EXIT

# --- pool baseline (ADR-0011) ---
pool_capture pool-baseline.txt

# step 0: pre-suite census — the baseline the teardown census is diffed against.
run 0 restool dprc list
expect_zero 0 "pre-suite census: container list"
probe step-0-show-dprc1.txt restool dprc show dprc.1

# ---- scenario (2) setup: the flat edge-dpni <-> dpmac.7 stands FIRST ----

# step 1: converge the flat intent — the edge tenant's dpni faces dpmac.7 directly
# (dpni <-> dpmac.7 port-edge), its OWN created dpni, no switch yet. This is the
# pre-interposition state the switched convergence will sever.
run 1 "$DPAA2CTL" --config "$INTENT_FLAT" ensure --no-link --allow disruptive
expect_zero 1 "flat convergence: the edge dpni faces dpmac.7 directly (pre-interposition)"
probe step-1-list.txt restool dprc list
# Resolve the flat edge dpni from MC truth, not sysfs: an MC-created dpni sits in a
# child container the kernel may not have scanned yet, so enumerate dpni ids from the
# containers restool reports (dprc.1 and its child) and ask each for the dpmac.7 endpoint.
FLAT_DPNI=""
flat_child="$(restool dprc list 2>/dev/null | grep -oE 'dprc\.[0-9]+' | grep -vxE 'dprc\.1' | head -1)"
for n in $( { restool dprc show dprc.1 2>/dev/null; [ -n "$flat_child" ] && restool dprc show "$flat_child" 2>/dev/null; } | awk '$1 ~ /^dpni\./ {print $1}'); do
  if restool dpni info "$n" 2>/dev/null | grep -qE 'dpmac\.7($|[^0-9])'; then FLAT_DPNI="$n"; break; fi
done
echo "flat edge dpni on dpmac.7: ${FLAT_DPNI:-<none>}" | tee "$RESULTS/flat-dpni.txt"
if [ -n "$FLAT_DPNI" ]; then
  echo "PASS step 1: the flat edge dpni ($FLAT_DPNI) faces dpmac.7 (the edge to be severed)"
else
  echo "FAIL step 1: no dpni faces dpmac.7 after the flat convergence" >&2
fi

# ---- scenario (1) + (2) sever: converge the switched domain over the same dpmac.7 ----

# step 2: dry-run the switched intent — headlines the plan, dispatches nothing. The
# plan severs the flat dpni<->dpmac.7 edge (the typed severed-witness, ADR-0008 §8)
# before wiring the dpsw uplink and member (dpsw-typestate design D10).
run 2 "$DPAA2CTL" --config "$INTENT" dry-run
expect_zero 2 "dry-run headlines the switched-domain plan"

# step 3: converge the switched domain. CreateContainer / pool grow / dpsw create are
# Class::Disruptive. The reconciler severs the flat edge, creates the dpsw with its own
# companions (DPSW-I3), connects the uplink dpmac.7 and the member dpni, and plugs it.
run 3 "$DPAA2CTL" --config "$INTENT" ensure --no-link --allow disruptive
expect_zero 3 "ensure --allow disruptive converges the switched domain"
probe step-3-list.txt restool dprc list
probe step-3-show-dprc1.txt restool dprc show dprc.1
pool_capture pool-after-converge.txt

# Resolve the dpsw (MC label = the fabric construct name "lan", ADR-0015 decision 9/13).
DPSW="$(restool dprc show dprc.1 | awk '$2=="lan"{print $1; exit}')"
[ -n "$DPSW" ] || DPSW="$(restool dprc show dprc.1 | awk '$1 ~ /^dpsw\./{print $1; exit}')"
echo "switched-domain dpsw: ${DPSW:-<none>}" | tee "$RESULTS/dpsw.txt"
if [ -n "$DPSW" ]; then
  echo "PASS step 3: the switched-domain dpsw ($DPSW) stands in dprc.1"
else
  echo "FAIL step 3: no dpsw after the switched convergence" >&2
fi

# step 3a (scenario 1 companions, DPSW-I3): the dpsw draws exactly 1 dpmcp + 1 dpbp from
# its own container, 0 dpcon, 0 dpio. The pool deltas (pool-baseline vs pool-after-converge)
# are the evidence; recorded here, read against the plan offline.
if [ -n "$DPSW" ]; then
  probe step-3-dpsw-info.txt restool dpsw info "$DPSW"
  echo "RECORD DPSW-I3: compare pool-baseline.txt vs pool-after-converge.txt — expect the dpsw drew exactly 1 dpmcp + 1 dpbp, 0 dpcon, 0 dpio (shared kernel dpio service)"
fi

# step 3b (scenario 1 bind, DPSW-I1): the dpaa2-switch bind is judged SETTLED through the
# bind_settle discipline (never an immediate read) — poll the driver link, drivers_probe
# kick, dprc sync kick. A bound link is the positive DPSW-I1 face (V-DPSW-1 rev 2, cited).
if [ -n "$DPSW" ]; then
  if bind_settle "/sys/bus/fsl-mc/devices/$DPSW/driver" "$DPSW"; then
    drv="$(readlink "/sys/bus/fsl-mc/devices/$DPSW/driver" 2>/dev/null)"
    echo "PASS step 3b: $DPSW bound (driver=${drv:-?}) — dpaa2-switch took the bindable switch (DPSW-I1, bind_settle)"
  else
    echo "FAIL step 3b: $DPSW did not bind after bind_settle (no dpaa2-switch driver link) — a finding" >&2
    probe step-3b-dmesg-tail.txt sh -c "dmesg | tail -60"
  fi
fi

# step 3c (scenario 2 sever + no driverless interval): the flat dpni was re-homed off
# dpmac.7 onto the switch, and dpmac.7 is now the dpsw uplink — no interval left the port
# driverless (ADR-0008 §8; dpsw-typestate design D10). Read dpsw info's per-interface
# endpoints: one interface faces dpmac.7 (uplink), one faces the member dpni.
if [ -n "$DPSW" ]; then
  up_ok=no; mem_ok=no
  grep -qE 'dpmac\.7($|[^0-9])' "$RESULTS/step-3-dpsw-info.txt" && up_ok=yes
  grep -qE 'dpni\.[0-9]+' "$RESULTS/step-3-dpsw-info.txt" && mem_ok=yes
  echo "dpsw interfaces: uplink-faces-dpmac.7=$up_ok member-faces-dpni=$mem_ok" | tee "$RESULTS/step-3c-ifaces.txt"
  if [ "$up_ok" = yes ] && [ "$mem_ok" = yes ]; then
    echo "PASS step 3c: the dpsw uplink faces dpmac.7 and a member interface faces a dpni — the interposition wired both ends (dpsw-typestate design D10)"
  else
    echo "FAIL step 3c: the dpsw interfaces are not wired as uplink dpmac.7 + member dpni" >&2
  fi
  # the flat dpni must no longer face dpmac.7 directly — the edge was severed, not duplicated.
  if [ -z "$FLAT_DPNI" ]; then
    echo "RECORD step 3c: the sever cannot be independently confirmed — the flat dpni was never resolved before the switched convergence"
  elif restool dpni info "$FLAT_DPNI" 2>/dev/null | grep -qE 'dpmac\.7($|[^0-9])'; then
    echo "RECORD step 3c: $FLAT_DPNI still names dpmac.7 directly — inspect whether the sever re-homed the same dpni or the plan built a fresh member (a finding, read against the plan)"
  else
    echo "PASS step 3c: the flat dpni no longer faces dpmac.7 directly — the edge was severed before the switch wired it (ADR-0008 §8)"
  fi
fi

# step 4 (scenario 1 read-back census): the board census equals the compiled plan. The
# convergence is census + read-back only (DPSW-I7; dpsw-typestate design D6), so idempotence
# is the proof: a re-run dry-run plans zero, a second ensure dispatches nothing.
run 4 "$DPAA2CTL" --config "$INTENT" dry-run
expect_zero 4 "post-converge dry-run: the board is converged"
expect_out 4 "0 planned transition(s)" "the converged dry-run plans zero transitions (read-back census == compiled plan)"
run 5 "$DPAA2CTL" --config "$INTENT" ensure --no-link --allow disruptive
expect_zero 5 "second ensure dispatches nothing (idempotent, census converged)"

# ---- reachability traffic (V-TRAF-0 pattern) — counted burst, dpni statistics oracle ----
# Resolve the member dpni (the switch interface facing a dpni, step 3c). The peer sends a
# small counted broadcast burst on dpmac.7; it floods the switch to the member dpni, whose
# ingress_all_frames must advance by the count. No rate target; nothing touches the peer's
# config beyond running its existing burst generator (dpsw-typestate design D9).
MEMBER_DPNI="$(restool dpsw info "${DPSW:-dpsw.0}" 2>/dev/null | grep -oE 'dpni\.[0-9]+' | head -1)"
echo "member dpni (switch frame oracle): ${MEMBER_DPNI:-<none>}" | tee "$RESULTS/member-dpni.txt"

# Bridge ONLY the switch port netdevs so unbridged standalone ports forward (bead dbf.19); the
# member stays OUT of the bridge — its hardware wire to switch if1 already carries the frames, so
# enslaving it would build an STP-less flood loop. Missing netdevs are RECORDED, not fatal.
# The bridge is VLAN-aware (vlan_filtering) — the switch driver enslaves ports into a
# VLAN-aware bridge only (bead dbf.19). Every ip link keeps its stderr so a refusal names
# its reason in the log.
echo "+ ip link add vdpsw5br type bridge vlan_filtering 1"
err="$(ip link add vdpsw5br type bridge vlan_filtering 1 2>&1)" || echo "RECORD bridge: could not create vdpsw5br: $err"
found=no
for nd in "${DPSW:+/sys/bus/fsl-mc/devices/$DPSW}"/net/*; do
  [ -n "$DPSW" ] && [ -e "$nd" ] || continue
  found=yes
  n="$(basename "$nd")"
  echo "+ ip link set $n master vdpsw5br up"
  err="$(ip link set "$n" master vdpsw5br up 2>&1)" || echo "RECORD bridge: could not enslave $n: $err"
done
[ "$found" = yes ] || echo "RECORD bridge: no switch-port netdev under $DPSW — switch not kernel-bound this run"
# Give the member dpni's bind the same settle discipline as the switch (bead dbf.19):
# wait for the driver link, then poll the netdev for a few seconds, before judging it
# missing. When still missing, bank sysfs + dmesg diagnostics so the next sitting self-attributes.
if [ -n "$MEMBER_DPNI" ]; then
  bind_settle "/sys/bus/fsl-mc/devices/$MEMBER_DPNI/driver" "$MEMBER_DPNI" || true
  mem_found=no
  i=0
  while [ "$i" -lt 5 ]; do
    for nd in "/sys/bus/fsl-mc/devices/$MEMBER_DPNI"/net/*; do
      [ -e "$nd" ] || continue
      mem_found=yes
      n="$(basename "$nd")"
      echo "+ ip link set $n up"
      err="$(ip link set "$n" up 2>&1)" || echo "RECORD bridge: could not bring up $n: $err"
    done
    [ "$mem_found" = yes ] && break
    sleep 1; i=$((i+1))
  done
  if [ "$mem_found" = no ]; then
    echo "RECORD bridge: no member netdev under $MEMBER_DPNI — member not kernel-bound this run"
    mem_child="$(restool dprc list 2>/dev/null | grep -oE 'dprc\.[0-9]+' | grep -vxE 'dprc\.1' | head -1)"
    probe member-bind-sysfs.txt sh -c "ls /sys/bus/fsl-mc/devices; [ -n \"$mem_child\" ] && readlink \"/sys/bus/fsl-mc/devices/$mem_child/driver\""
    probe member-bind-dmesg.txt sh -c "dmesg | tail -60"
  fi
fi
echo "+ ip link set vdpsw5br up"
err="$(ip link set vdpsw5br up 2>&1)" || echo "RECORD bridge: could not bring vdpsw5br up: $err"

if [ -n "$MEMBER_DPNI" ]; then
  in0="$(dpni_frames "$MEMBER_DPNI" ingress_all_frames)"
  printf '\n>> on the peer facing dpmac.7: send COUNT=16 MODE=broadcast (the peer'\''s existing burst generator); enter when done: '
  read -r _
  in1="$(dpni_frames "$MEMBER_DPNI" ingress_all_frames)"
  # Snapshot the switch's own interface counters so a zero localizes peer-vs-switch next time.
  probe traffic-dpsw-info.txt restool dpsw info "$DPSW"
  echo "member dpni ingress_all_frames $in0 -> $in1 (want +16)" | tee "$RESULTS/traffic.txt"
  if [ -n "$in0" ] && [ -n "$in1" ] && [ "$((in1 - in0))" -ge 16 ]; then
    echo "PASS traffic: the member dpni counted the broadcast burst across the switch (+$((in1 - in0)), want +16) — reachability witnessed (V-TRAF-0 pattern)"
  else
    echo "RECORD traffic: member dpni ingress delta $in0 -> $in1 (want +16) — read against the peer burst count; reachability-level only, no rate target" | tee -a "$RESULTS/traffic.txt"
  fi
else
  echo "RECORD traffic: no member dpni resolved from the dpsw interfaces — reachability oracle unavailable this run"
fi

# ---- scenario (3) teardown: the typed destroy path converges to the baseline census ----

# step 6: the empty intent reconciles the switched domain down to zero through the typed
# destroy path (the dpsw edge kinds carry no teardown witness, dpsw-typestate design D5).
printf '[intent]\nschema = 1\n' > "$RESULTS/step-6-empty.toml"
run 6 "$DPAA2CTL" --config "$RESULTS/step-6-empty.toml" ensure --prune --allow disruptive --no-link
expect_zero 6 "empty-intent ensure tears the switched domain down"
sleep 2
if [ -n "$DPSW" ] && [ ! -e "/sys/bus/fsl-mc/devices/$DPSW" ]; then
  echo "PASS step 6: the dpsw $DPSW is gone (sysfs device absent)"
elif [ -n "$DPSW" ] && ! restool dpsw info "$DPSW" >/dev/null 2>&1; then
  echo "PASS step 6: the dpsw $DPSW is gone (does not exist — read-back, not exit code, DPSW-I7)"
elif [ -n "$DPSW" ]; then
  echo "FAIL step 6: the dpsw $DPSW survives teardown" >&2
fi

# step 7: closing census — diffed offline against step 0's baseline. The container set
# must return to the pre-suite baseline; the managed root pool + grow-only dpio seats
# remain as reboot-required residue until the closing reboot (ADR-0003 §7).
run 7 restool dprc list
expect_zero 7 "post-suite census: container list"
probe step-7-show-dprc1.txt restool dprc show dprc.1
pool_capture pool-post.txt
echo "census diff vs step 0: the container set must match the baseline; the managed root pool plus the grow-only dpio seats remain as reboot-required residue (ADR-0003 §7); the closing reboot restores the DPL baseline"

echo "suite V-DPSW-5 switched-domain convergence + interposition + teardown complete"
