# V-DPNI-3 netdev-runtime-state face. Sourced by V-DPNI-3.sh after its
# kernel bind, with one root-bound scratch dpni standing. DPNI-I2 (MC
# state set before a bind does not survive it — the probe resets), DPNI-I8
# (a clean unbind resets the object) and dpni.md unknown 4 (what the reset
# clears) as far as the primary MAC shows it. MTU is deliberately not the
# probe: the kernel never sends a max-frame-length change to the MC.
# From the script: $OBJ_dpni_0, $RESULTS.
. "$(dirname "$0")/../residents.sh"
R="$RESULTS/dpni3.txt"
DEV=/sys/bus/fsl-mc/devices
DPNI="$OBJ_dpni_0"
LAA=02:11:22:33:44:55        # a locally administered MAC set via the netdev
MC_MAC=02:aa:bb:cc:dd:ee     # a second MAC set on the MC object while unbound

log() { echo "$1" | tee -a "$R"; }
netdev() { ls "$DEV/$DPNI/net/" 2>/dev/null | head -1; }
mc_mac() { restool dpni info "$DPNI" 2>/dev/null | awk 'tolower($0) ~ /mac/ && tolower($0) ~ /addr/ { print tolower($NF); exit }'; }
frame_len() { restool dpni info "$DPNI" 2>/dev/null | awk -F: 'tolower($0) ~ /max frame/ { gsub(/ /, "", $2); print $2; exit }'; }
bound() { [ -e "$DEV/$DPNI/driver" ]; }

residents_pre
nd="$(netdev)"

# (a) The bound netdev's MAC and the MC object's, recorded.
log "RECORD (a) netdev $nd MAC: $(cat "/sys/class/net/$nd/address" 2>/dev/null)"
log "RECORD (a) dpni info MAC: $(mc_mac)"

# (b) Set a locally administered MAC on the netdev; the kernel pushes it
# to the MC (dpni_set_primary_mac_addr), so dpni info must show it.
ip link set dev "$nd" address "$LAA" 2>>"$RESULTS/dpni3.log" || true
sleep 1
if [ "$(mc_mac)" = "$LAA" ]; then
  log "PASS (b) dpni info shows the netdev-set MAC $LAA"
else
  log "FAIL (b) dpni info MAC is $(mc_mac), not $LAA"
fi

# (c) Unbind through sysfs (the clean remove path). DPNI-I8: the remove
# resets the object, so dpni info must no longer carry the set MAC.
# Record the max frame length while unbound.
echo "$DPNI" > "$DEV/$DPNI/driver/unbind" 2>>"$RESULTS/dpni3.log" || true
sleep 2
if [ "$(mc_mac)" != "$LAA" ]; then
  log "PASS (c) after unbind dpni info no longer carries $LAA (DPNI-I8 reset)"
else
  log "FAIL (c) dpni info still carries $LAA after unbind"
fi
log "RECORD (c) max frame length while unbound: $(frame_len)"

# (d) MC state set before a (re)bind: write a second MAC to the object
# directly while it is unbound.
restool dpni update "$DPNI" --mac-addr="$MC_MAC" 2>>"$RESULTS/dpni3.log" || true
log "RECORD (d) dpni info MAC after restool update while unbound: $(mc_mac)"

# (e) Rebind through sysfs. DPNI-I2: the probe resets the object, so
# neither the netdev nor dpni info should carry the second MAC — the dpni
# has no dpmac to inherit from (DPNI-I3), so the probe re-derives a random
# one.
echo "$DPNI" > /sys/bus/fsl-mc/drivers/fsl_dpaa2_eth/bind 2>>"$RESULTS/dpni3.log" || true
sleep 3
if ! bound "$DPNI"; then
  log "FAIL (e) dpni ${DPNI} did not rebind within the wait"
else
  nd="$(netdev)"
  nd_mac="$(cat "/sys/class/net/$nd/address" 2>/dev/null)"
  mc="$(mc_mac)"
  if [ "$mc" != "$MC_MAC" ] && [ "$nd_mac" != "$MC_MAC" ]; then
    log "PASS (e) after rebind neither dpni info ($mc) nor netdev ($nd_mac) carries $MC_MAC (DPNI-I2: probe reset it)"
  else
    log "FAIL (e) second MAC survived the rebind: dpni info=$mc netdev=$nd_mac"
  fi
fi

residents_post
echo; echo "dpni3 face: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
