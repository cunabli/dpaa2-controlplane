# V-POOL-2 pool-object-across-owners face. Sourced by V-POOL-2.sh after
# its last plug, with the same scratch child V-POOL-1 stands, *plugged*.
# DPBP-I3 / DPCON-I5 (a freed pool object is not clean: the kernel frees a
# dpbp as drain -> disable -> close, no reset) and DPMCP-I3 (no reset
# anywhere; restool exposes no portal state, so that half is unobservable
# here). The board shows the dpbp half through `dpbp info`; the free path
# is Linux-side and never reaches the MC object, so the only judged line
# is that the dpbp stays plugged and MC-listed through the cycle.
# Rev 2: the child's plug is refused by restool (see pool1.sh), so the
# child dpni never binds here either; the suite retires at rev 2.
# From the script: $OBJ_dprc_2, $OBJ_dpni_0, $OBJ_dpbp_0, $RESULTS.
. "$(dirname "$0")/../residents.sh"
R="$RESULTS/pool.txt"
DEV=/sys/bus/fsl-mc/devices
DPRC="$OBJ_dprc_2"; DPNI="$OBJ_dpni_0"; DPBP="$OBJ_dpbp_0"

log() { echo "$1" | tee -a "$R"; }
bound() { [ -e "$DEV/$1/driver" ]; }
bpstate() { restool dpbp info "$DPBP" 2>/dev/null | awk -F: '/plugged state/ { gsub(/ /, "", $2); print $2 }'; }
listed() { restool dprc show "$DPRC" 2>/dev/null | awk -v o="$DPBP" '$1 == o { f = 1 } END { exit !f }'; }

# A plugged, driver-bound child dprc cannot be destroyed by the generated
# teardown (restool refuses "unbind it first"), and the teardown does not
# unbind dprcs. Its own EXIT trap unbinds the child, then hands off to the
# script's teardown — so the child is unbound before teardown even on an
# early exit.
_pool_cleanup() {
  echo "$DPRC" > /sys/bus/fsl-mc/drivers/fsl_mc_dprc/unbind 2>>"$RESULTS/pool-child-unbind.log" || true
  teardown
}
trap _pool_cleanup EXIT

residents_pre

# The child was plugged as the trace's last step; wait (bounded) for the
# kernel's autorescan to bind it to fsl_mc_dprc, so the kernel probes its
# residents and the first dpni claims a dpbp from the child pool.
restool dprc sync > "$RESULTS/pool-sync.txt" 2>&1 || true   # a plug may not rescan on its own; no destroy precedes this
tries=0
while [ "$tries" -lt 15 ] && [ ! -e "$DEV/$DPRC/driver" ]; do tries=$((tries + 1)); sleep 1; done

if ! bound "$DPNI"; then
  log "RECORD child dpni ${DPNI} never bound (no dpio of its own, DPRC-I1); the free-path cycle is unobservable, recorded"
else
  # Record the dpbp before, across the kernel's drain-and-free (dpni
  # unbind), and after a rebind. Every line here is a RECORD.
  restool dpbp info "$DPBP" > "$RESULTS/pool-dpbp-1-bound.txt" 2>&1 || true
  log "RECORD dpbp ${DPBP} while dpni bound: plugged=$(bpstate)"
  echo "$DPNI" > "$DEV/$DPNI/driver/unbind" 2>>"$RESULTS/pool-dpni-unbind.log" || true
  sleep 2
  restool dpbp info "$DPBP" > "$RESULTS/pool-dpbp-2-freed.txt" 2>&1 || true
  log "RECORD dpbp ${DPBP} after dpni unbind (drain->disable->close): plugged=$(bpstate)"
  echo "$DPNI" > /sys/bus/fsl-mc/drivers/fsl_dpaa2_eth/bind 2>>"$RESULTS/pool-dpni-rebind.log" || true
  sleep 3
  restool dpbp info "$DPBP" > "$RESULTS/pool-dpbp-3-rebound.txt" 2>&1 || true
  log "RECORD dpbp ${DPBP} after dpni rebind (next allocator resets it): plugged=$(bpstate)"
fi

# The only judged line: the dpbp stays plugged and MC-listed throughout,
# because the free path never reaches the MC object.
if listed && [ "$(bpstate)" = plugged ]; then
  log "PASS dpbp ${DPBP} stayed plugged and MC-listed through the cycle (free path is not MC-observable)"
else
  log "FAIL dpbp ${DPBP} listed=$(listed && echo y || echo n) plugged=$(bpstate)"
fi

# Unbind the child so the generated teardown finds it as the trace left
# it (residents unbound, no rescan race with the destroys).
echo "$DPRC" > /sys/bus/fsl-mc/drivers/fsl_mc_dprc/unbind 2>>"$RESULTS/pool-child-unbind.log" || true
sleep 2

residents_post
echo; echo "pool face: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
