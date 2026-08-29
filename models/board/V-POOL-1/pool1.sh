# V-POOL-1 pool-mechanics face. Sourced by V-POOL-1.sh after its last
# plug, with the scratch child dprc.2 standing and *plugged*: two dpmcps,
# two dpcons, two dpbps and two unconnected dpnis, all plugged but the
# second dpbp. DPBP-I2 (plugged vs allocator), DPBP-I4 (exhaustion then
# top-up), DPRC-I8 (batch plug then probe). The trace's last step plugs
# the child itself, so the kernel's autorescan (=1) binds it to
# fsl_mc_dprc — rev 1's hand-bind wrote to a device that did not exist
# (an unplugged child has no bus node). This rev waits for that
# plug-driven bind instead of writing to `bind`. Rev 2 found the plug
# itself is refused by restool ("Cannot change plugged state of dprc",
# dprc_commands.c), so the child stays unplugged and (b) records it;
# the suite retires at rev 2 — a kernel-driven child needs the DPL or
# the raw command path (#10).
# From the script: $OBJ_dprc_2, $OBJ_dp*_{0,1}, $RESULTS.
. "$(dirname "$0")/../residents.sh"
R="$RESULTS/pool.txt"
DEV=/sys/bus/fsl-mc/devices
DPRC="$OBJ_dprc_2"

log() { echo "$1" | tee -a "$R"; }
present() { [ -e "$DEV/$1" ]; }                                   # sysfs device node exists
listed() { restool dprc show "$DPRC" 2>/dev/null | awk -v o="$1" '$1 == o { f = 1 } END { exit !f }'; }
bound() { [ -e "$DEV/$1/driver" ]; }

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

# (a) Rev 1 judged DPBP-I2's child half here (residents MC-listed, no
# sysfs node) on an unplugged child. The child is plugged before this hook
# runs now, so the bus may already hold its residents: recorded, not judged.
for o in "$OBJ_dpmcp_0" "$OBJ_dpcon_0" "$OBJ_dpbp_0" "$OBJ_dpni_0" "$OBJ_dpni_1"; do
  log "RECORD (a) $o listed=$(listed "$o" && echo y || echo n) node=$(present "$o" && echo y || echo n) at hook start"
done

# (b) The child was plugged as the trace's last step. 5.10 showed the root
# rescans on its own for a create; whether it does for a plug is unproven,
# so one explicit rescan (no destroy precedes it in this run, ADR-0008) and
# then a bounded wait for fsl_mc_dprc to hold the child. A bound child is
# kernel-driven — its own bus, pools and probes — so its residents must be
# bus-visible now: that is the new claim, the inverse of rev 1's.
restool dprc sync > "$RESULTS/pool-sync.txt" 2>&1 || true
tries=0
while [ "$tries" -lt 15 ] && [ ! -e "$DEV/$DPRC/driver" ]; do tries=$((tries + 1)); sleep 1; done
childdrv="$(readlink "$DEV/$DPRC/driver" 2>/dev/null)"
cd_name="${childdrv##*/}"; [ -n "$cd_name" ] || cd_name="<none>"
log "RECORD (b) $DPRC driver link after the plug: $cd_name"
if [ -n "$childdrv" ]; then
  sleep 3
  if present "$OBJ_dpni_0" && present "$OBJ_dpbp_0"; then
    log "PASS (b) child residents are bus-visible once the child is kernel-driven (${OBJ_dpni_0}, ${OBJ_dpbp_0} present)"
  else
    log "FAIL (b) child bound but residents absent from the bus: dpni=$(present "$OBJ_dpni_0" && echo y || echo n) dpbp=$(present "$OBJ_dpbp_0" && echo y || echo n)"
  fi
fi

if [ -z "$childdrv" ]; then
  log "RECORD (b) child stayed unbound - DPBP-I4/DPRC-I8 re-anchor to pool-objects (#6): restool refuses to plug a dprc, so a runtime child never gets a driver; faces (c)-(e) skipped"
else
  # (c) DPRC-I8/DPBP-I4 deferred probe: the first dpni binds, the second
  # does not (its dpbp draw is one short), with the exhaustion line in the
  # kernel log — a deferral, not a refusal.
  dmesg 2>/dev/null | tail -n 300 > "$RESULTS/pool-kmsg-defer.txt"
  if bound "$OBJ_dpni_0"; then log "PASS (c) first dpni ${OBJ_dpni_0} bound"; else log "FAIL (c) first dpni ${OBJ_dpni_0} did not bind"; fi
  if ! bound "$OBJ_dpni_1"; then log "PASS (c) second dpni ${OBJ_dpni_1} deferred (unbound)"; else log "FAIL (c) second dpni ${OBJ_dpni_1} bound despite the missing dpbp"; fi
  if grep -q "No more resources of type dpbp" "$RESULTS/pool-kmsg-defer.txt"; then
    log "PASS (c) kernel log carries 'No more resources of type dpbp'"
  else
    log "RECORD (c) exhaustion line not seen (a dpio-less child may never probe either dpni, DPRC-I1; recorded)"
  fi

  # (d) DPBP-I4 top-up: plug the second dpbp; the deferred dpni binds.
  restool dprc assign "$DPRC" --object="$OBJ_dpbp_1" --plugged=1 2>>"$RESULTS/pool-topup.log" || true
  sleep 3
  if bound "$OBJ_dpni_1"; then
    log "PASS (d) second dpni ${OBJ_dpni_1} bound after the dpbp top-up"
  else
    log "RECORD (d) second dpni ${OBJ_dpni_1} still unbound after top-up (recorded, not a divergence: it draws no dpio of its own, DPRC-I1)"
  fi

  # (e) Unbind the child dprc so the generated teardown finds it as the
  # trace left it (residents unbound, no rescan race with the destroys).
  echo "$DPRC" > /sys/bus/fsl-mc/drivers/fsl_mc_dprc/unbind 2>>"$RESULTS/pool-unbind.log" || true
  sleep 2
fi

residents_post
echo; echo "pool face: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
