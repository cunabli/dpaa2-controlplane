# V-POOL-7 pool free/drain face at ROOT scope. Sourced by V-POOL-7.sh after
# its kernel bind, with one root-bound scratch dpni standing and its three
# scratch companions (dpmcp, dpbp, dpcon) drawn. Observes, never drives, the
# Linux-side free path DPBP-I3/DPCON-I5/DPMCP-I3 leave open: the bind's
# allocator draw (DPBP-I2 root-kernel half), the unbind's return, and a
# re-bind proving the freed units are re-drawable. The free path is not
# restool-observable (COVERAGE), so these are RECORD lines; the one judged
# claim is the re-bind (a freed companion set is drawable again).
# From the script: $OBJ_dpni_0, $OBJ_dpmcp_0, $OBJ_dpbp_0, $OBJ_dpcon_0, $RESULTS.
. "$(dirname "$0")/../residents.sh"
R="$RESULTS/drain.txt"
DEV=/sys/bus/fsl-mc/devices
DPNI="$OBJ_dpni_0"

log() { echo "$1" | tee -a "$R"; }
netdev() { ls "$DEV/$DPNI/net/" 2>/dev/null | head -1; }
bound() { [ -e "$DEV/$1/driver" ]; }
drv() { d="$(readlink "$DEV/$1/driver" 2>/dev/null)"; d="${d##*/}"; echo "${d:-<none>}"; }
pools() { restool dprc show mc.global --resources > "$RESULTS/drain-res-$1.txt" 2>&1 || true; }
# delta BEFORE AFTER: the pool lines that moved, as "key old->new" pairs.
delta() { awk 'NR==FNR{a[$1]=$2;next} a[$1]!=$2{printf "%s %s->%s ", $1, a[$1], $2}' "$1" "$2"; }
companions() {
  for o in "$OBJ_dpmcp_0" "$OBJ_dpbp_0" "$OBJ_dpcon_0"; do
    log "RECORD ($1) companion $o driver=$(drv "$o") node=$([ -e "$DEV/$o" ] && echo y || echo n)"
  done
}

residents_pre

# (a) Drawn: the dpni bound in the trace, so the companions are claimed. Record
# the bound netdev, the companion driver links and the pool reading (the draw).
pools drawn
nd="$(netdev)"
if bound "$DPNI"; then
  log "PASS (a) dpni ${DPNI} is kernel-bound (netdev ${nd:-<none>}, driver $(drv "$DPNI"))"
else
  log "FAIL (a) dpni ${DPNI} did not bind in the trace — the free/drain faces cannot be observed"
fi
companions drawn

# (b) Free/return: unbind the dpni through sysfs (the clean remove path). The
# driver releases the companions (DPBP-I3 dirty return, DPCON-I5 free path,
# DPMCP-I3 observable half). Record the pool movement and the companion state.
echo "$DPNI" > "$DEV/$DPNI/driver/unbind" 2>>"$RESULTS/drain.log" || true
sleep 3
pools returned
if ! bound "$DPNI"; then
  log "PASS (b) dpni ${DPNI} unbound (netdev gone: $([ -z "$(netdev)" ] && echo y || echo n))"
else
  log "FAIL (b) dpni ${DPNI} still bound after the sysfs unbind"
fi
log "RECORD (b) pool movement unbind (drawn->returned): $(delta "$RESULTS/drain-res-drawn.txt" "$RESULTS/drain-res-returned.txt")"
companions returned

# (c) Re-drawable: rebind through sysfs; the freed companion set is drawn
# again. The judged claim — a return that cannot be re-drawn is a leak.
echo "$DPNI" > /sys/bus/fsl-mc/drivers/fsl_dpaa2_eth/bind 2>>"$RESULTS/drain.log" || true
sleep 3
pools redrawn
if bound "$DPNI"; then
  log "PASS (c) dpni ${DPNI} re-bound — the freed companions are re-drawable (netdev ${nd:-<none>})"
else
  log "FAIL (c) dpni ${DPNI} did not re-bind — a freed companion did not return to the pool"
fi
log "RECORD (c) pool movement rebind (returned->redrawn): $(delta "$RESULTS/drain-res-returned.txt" "$RESULTS/drain-res-redrawn.txt")"
companions redrawn

# Unbind before handing back to the generated teardown, so the destroys do not
# race the driver's own release path (ADR-0008 §4).
echo "$DPNI" > "$DEV/$DPNI/driver/unbind" 2>>"$RESULTS/drain.log" || true
sleep 2

residents_post
echo; echo "drain face: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
