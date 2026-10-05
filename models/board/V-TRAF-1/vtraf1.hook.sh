# V-TRAF-1 face 6 teardown-law refusals (cross-dprc-links task 6.1, design D10).
# The two refusals cannot ride a forward MBT trace — a disabled (.fail) action
# cannot step, so a refusal is only ever a run's terminal step (vtraf1.qnt
# header). They run here as directed probes, sourced after the last trace step
# and under the suite's teardown trap. Never silent: each illegal command's MC
# status is recorded for the operator to diff. From the script: $RESULTS.
#
# No dpmac participates (finding 49): both ends are scratch dpnis connected at
# the root ancestor, the one container holding the topology-change privilege.

# cross-dprc-links task 6.2 (design D10): a frame witness then a saturation
# smoke on the standing dpni<->dpni pair, run before the sealed refusal probes.
# Both ends are scratch dpnis at the root (finding 49) — no dpmac, no frame
# leaves the pair. From the script: $OBJ_dpni_600, $OBJ_dpni_601, $RESULTS.
T="$RESULTS/vtraf1-traffic.txt"
TE="$RESULTS/vtraf1-traffic.err"
: > "$T"
: > "$TE"
# count OBJ KEY: one supported counter page (dpni.md: only ingress_all_frames
# and egress_all_frames move exactly; every other page reads zero, never judged).
count() { restool dpni info "$1" 2>>"$TE" | awk -v k="$2:" '$1 == k {print $2}'; }
# drops NS DEV: RX+TX errors+dropped off `ip -s link` — the discard oracle
# (dpni.md: restool discard pages read zero, so they are never judged).
drops() { ip -n "$1" -s link show "$2" | awk '/RX:|TX:/ {getline; e+=$3; d+=$4} END {print e+d}'; }
# verdict NAME TEXT delta want: exact-equality PASS/FAIL line, scanned by the plan.
verdict() {
  if [ "$3" -eq "$4" ]; then r=PASS; else r=FAIL; fi
  echo "$r $1: $2 (+$3, want +$4)" | tee -a "$T"; [ "$r" = PASS ]
}
# mono NAME CUR PREV: monotone PASS/FAIL line (a sample never regresses).
mono() {
  if [ "$2" -ge "$3" ]; then r=PASS; else r=FAIL; fi
  echo "$r $1: $2 >= $3" | tee -a "$T"; [ "$r" = PASS ]
}
# retry FACE: rerun a failed face while the operator sits (V-TRAF-0 idiom).
retry() { until "$1"; do printf '   r=retry, enter=continue: '; read -r k; [ "$k" = r ] || break; done; }
# net_cleanup: best-effort, safe if the rig half-built (set -u). Deleting a
# netns returns its netdev to the init ns; disconnect the pair so the generated
# teardown's destroy is not refused (LINK-I1 destroy-of-connected-end), then
# unplug both ends and destroy the six allocatables this hook provisioned — the
# generated teardown only knows the trace objects. Each command is echoed into
# $TE before it runs (the teardown-log attribution idiom).
net_cleanup() {
  ip netns del vtraf1-a 2>>"$TE" || true
  ip netns del vtraf1-b 2>>"$TE" || true
  [ -n "${OBJ_dpni_600:-}" ] && restool dprc disconnect dprc.1 --endpoint="$OBJ_dpni_600" 2>>"$TE" || true
  for o in "${OBJ_dpni_600:-}" "${OBJ_dpni_601:-}"; do
    [ -n "$o" ] || continue
    echo "+ restool dprc assign dprc.1 --object=$o --plugged=0" >> "$TE"
    restool dprc assign dprc.1 --object="$o" --plugged=0 2>>"$TE" || true
  done
  for id in "${CON2:-}" "${BP2:-}" "${MCP2:-}" "${CON1:-}" "${BP1:-}" "${MCP1:-}"; do
    [ -n "$id" ] || continue
    fam="${id%%.*}"
    echo "+ restool $fam destroy $id" >> "$TE"
    restool "$fam" destroy "$id" 2>>"$TE" || true
  done
}
# Re-arm the one EXIT trap: net cleanup first, then the generated teardown.
trap 'net_cleanup; teardown' EXIT

# poll_netdev OBJ: wait up to 30s for the kernel to bind a netdev under the
# dpni's bus device; echo the netdev name and succeed, or time out non-zero.
poll_netdev() {
  i=0
  while [ "$i" -lt 30 ]; do
    nd="$(ls /sys/bus/fsl-mc/devices/"$1"/net/ 2>>"$TE" | head -n1)"
    [ -n "$nd" ] && { echo "$nd"; return 0; }
    i=$((i + 1)); sleep 1
  done
  return 1
}
# wait_link OBJ: bounded poll for the MC to report the end's link up; kept.
wait_link() {
  i=0
  while [ "$i" -lt 20 ]; do
    l="$(restool dpni info "$1" 2>>"$TE" | grep -i 'link status')"
    case "$l" in *[Uu]p*|*': 1'*) echo "link $1: $l" >> "$T"; return 0 ;; esac
    i=$((i + 1)); sleep 1
  done
  echo "link $1: ${l:-none} (never up)" >> "$T"; return 1
}
# witness: baseline the four counters only after the rig settled, ping 8, then
# require EXACT movement — sender egress/ingress and receiver ingress/egress all +8.
witness() {
  se0=$(count "$OBJ_dpni_600" egress_all_frames); si0=$(count "$OBJ_dpni_600" ingress_all_frames)
  re0=$(count "$OBJ_dpni_601" ingress_all_frames); rx0=$(count "$OBJ_dpni_601" egress_all_frames)
  ip netns exec vtraf1-a ping -c 8 -i 0.2 -W 1 192.0.2.2 >>"$T" 2>>"$TE" || true
  se1=$(count "$OBJ_dpni_600" egress_all_frames); si1=$(count "$OBJ_dpni_600" ingress_all_frames)
  re1=$(count "$OBJ_dpni_601" ingress_all_frames); rx1=$(count "$OBJ_dpni_601" egress_all_frames)
  a=0
  verdict witness-sender-egress "egress_all_frames $se0 -> $se1" $((se1 - se0)) 8 || a=1
  verdict witness-receiver-ingress "ingress_all_frames $re0 -> $re1" $((re1 - re0)) 8 || a=1
  verdict witness-receiver-egress "egress_all_frames $rx0 -> $rx1" $((rx1 - rx0)) 8 || a=1
  verdict witness-sender-ingress "ingress_all_frames $si0 -> $si1" $((si1 - si0)) 8 || a=1
  return "$a"
}
# smoke (cross-dprc-links task 6.2): monotone counters and zero new discards
# under load; NO rate is computed, printed, or compared
# (cross-dprc-links design D10: reachability, not performance).
smoke() {
  da0=$(drops vtraf1-a "$NDA"); db0=$(drops vtraf1-b "$NDB")
  pse=$(count "$OBJ_dpni_600" egress_all_frames); pre=$(count "$OBJ_dpni_601" ingress_all_frames)
  prx=$(count "$OBJ_dpni_601" egress_all_frames); psi=$(count "$OBJ_dpni_600" ingress_all_frames)
  b=0; s=0
  while [ "$s" -lt 3 ]; do
    ip netns exec vtraf1-a ping -f -c 1000 -s 1400 192.0.2.2 >>"$T" 2>>"$TE" || true
    cse=$(count "$OBJ_dpni_600" egress_all_frames); cre=$(count "$OBJ_dpni_601" ingress_all_frames)
    crx=$(count "$OBJ_dpni_601" egress_all_frames); csi=$(count "$OBJ_dpni_600" ingress_all_frames)
    mono smoke-sender-egress "$cse" "$pse" || b=1
    mono smoke-receiver-ingress "$cre" "$pre" || b=1
    mono smoke-receiver-egress "$crx" "$prx" || b=1
    mono smoke-sender-ingress "$csi" "$psi" || b=1
    pse=$cse; pre=$cre; prx=$crx; psi=$csi; s=$((s + 1))
  done
  da1=$(drops vtraf1-a "$NDA"); db1=$(drops vtraf1-b "$NDB")
  verdict smoke-discards-a "dropped+errors $da0 -> $da1" $((da1 - da0)) 0 || b=1
  verdict smoke-discards-b "dropped+errors $db0 -> $db1" $((db1 - db0)) 0 || b=1
  return "$b"
}

# ----- SECTION A: frame witness (cross-dprc-links task 6.2) -----
# Connect BEFORE kernel bind — the blessed populate->connect->bind order; a
# post-bind connect trips the ENDPOINT_CHANGED law that belongs to face 4.
restool dprc connect dprc.1 --endpoint1="$OBJ_dpni_600" --endpoint2="$OBJ_dpni_601" 2>>"$TE"

# The kernel auto-probes each bus-visible dpni and draws 1 dpmcp + 1 dpbp +
# >=1 dpcon from dprc.1's pool; rev 1 showed a probe with no free census
# defers forever and no netdev ever appears. Provision two private census sets
# and plug one dpni at a time — a probing dpni greedily takes every free dpcon,
# so the pair must not see the pool together before the first is bound.
MCP1="$(restool --script dpmcp create --container=dprc.1 2>>"$TE")"
BP1="$(restool --script dpbp create --container=dprc.1 2>>"$TE")"
CON1="$(restool --script dpcon create --container=dprc.1 2>>"$TE")"
MCP2="$(restool --script dpmcp create --container=dprc.1 2>>"$TE")"
BP2="$(restool --script dpbp create --container=dprc.1 2>>"$TE")"
CON2="$(restool --script dpcon create --container=dprc.1 2>>"$TE")"

RIG_OK=1
NDA=""
NDB=""
for o in "$MCP1" "$BP1" "$CON1"; do
  restool dprc assign dprc.1 --object="$o" --plugged=1 2>>"$TE"
done
restool dprc sync 2>>"$TE"
restool dprc assign dprc.1 --object="$OBJ_dpni_600" --plugged=1 2>>"$TE"
if ! NDA="$(poll_netdev "$OBJ_dpni_600")"; then
  echo "FAIL witness-rig: $OBJ_dpni_600 never bound a netdev" | tee -a "$T"
  RIG_OK=0
fi
if [ "$RIG_OK" = 1 ]; then
  for o in "$MCP2" "$BP2" "$CON2"; do
    restool dprc assign dprc.1 --object="$o" --plugged=1 2>>"$TE"
  done
  restool dprc sync 2>>"$TE"
  restool dprc assign dprc.1 --object="$OBJ_dpni_601" --plugged=1 2>>"$TE"
  if ! NDB="$(poll_netdev "$OBJ_dpni_601")"; then
    echo "FAIL witness-rig: $OBJ_dpni_601 never bound a netdev" | tee -a "$T"
    RIG_OK=0
  fi
fi

# With both ends bound, stand the netns rig and run the witness and smoke. No
# command runs with an empty netdev variable: a failed bind skips the whole rig.
if [ "$RIG_OK" = 1 ]; then
  # One netns per side; IPv6 off BEFORE link-up so no ND/DAD frame pollutes the
  # exact count; static neigh both ways (peer MAC read back) so no ARP flows.
  ip netns add vtraf1-a 2>>"$TE"
  ip netns add vtraf1-b 2>>"$TE"
  ip link set "$NDA" netns vtraf1-a 2>>"$TE"
  ip link set "$NDB" netns vtraf1-b 2>>"$TE"
  ip netns exec vtraf1-a sysctl -w net.ipv6.conf.all.disable_ipv6=1 >>"$TE" 2>&1
  ip netns exec vtraf1-b sysctl -w net.ipv6.conf.all.disable_ipv6=1 >>"$TE" 2>&1
  ip -n vtraf1-a addr replace 192.0.2.1/24 dev "$NDA" 2>>"$TE"
  ip -n vtraf1-b addr replace 192.0.2.2/24 dev "$NDB" 2>>"$TE"
  MACA="$(ip -n vtraf1-a link show "$NDA" 2>>"$TE" | awk '/link\/ether/ {print $2}')"
  MACB="$(ip -n vtraf1-b link show "$NDB" 2>>"$TE" | awk '/link\/ether/ {print $2}')"
  ip -n vtraf1-a neigh replace 192.0.2.2 lladdr "$MACB" dev "$NDA" 2>>"$TE"
  ip -n vtraf1-b neigh replace 192.0.2.1 lladdr "$MACA" dev "$NDB" 2>>"$TE"
  ip -n vtraf1-a link set "$NDA" up 2>>"$TE"
  ip -n vtraf1-b link set "$NDB" up 2>>"$TE"
  wait_link "$OBJ_dpni_600" || true
  wait_link "$OBJ_dpni_601" || true
  retry witness
  # ----- SECTION B: saturation smoke (cross-dprc-links task 6.2) -----
  retry smoke
fi

R="$RESULTS/vtraf1-refusals.txt"
E="$RESULTS/vtraf1-refusals.err"
: > "$R"
: > "$E"
log() { echo "$1" | tee -a "$R"; }

# Create A, B and C up front, before any connect or destroy: a create recycles
# the lowest free id, so a C minted after A's destroy would reuse A's id and
# invalidate the fixture (rev 1: dpni_500 and dpni_600 both mapped to dpni.9).
A="$(restool --script dpni create --container=dprc.1 2>>"$E")"
B="$(restool --script dpni create --container=dprc.1 2>>"$E")"
C="$(restool --script dpni create --container=dprc.1 2>>"$E")"

# fixture_ok: A's `dpni info` names B on its endpoint line (the generated step
# 2/6 read-back idiom); a connect the MC silently dropped reads back no peer.
fixture_ok() { restool dpni info "$A" 2>>"$E" | grep -q "endpoint:.*$B"; }

# Establish and assert the A<->B wire the two laws probe; a SKIP is loud.
if restool dprc connect dprc.1 --endpoint1="$A" --endpoint2="$B" 2>>"$E" && fixture_ok; then
  # LINK-I1 disconnect-before-destroy: the MC refuses destroy of a still-
  # connected end (finding 34 generalized). A success is the failure AND tears
  # the fixture DPRC-I5 needs, so DPRC-I5 is then skipped.
  if restool dpni destroy "$A" >>"$R" 2>&1; then
    log "FAIL LINK-I1: destroy of still-connected end $A was NOT refused"
    log "SKIP DPRC-I5: LINK-I1 destroyed the connected end, precondition gone"
  else
    log "RECORD LINK-I1: destroy of connected end $A refused (disconnect-before-destroy)"
    # DPRC-I5 double-connect: an endpoint holds at most one peer. Re-assert the
    # fixture first so the law never runs against a stale wire.
    if fixture_ok; then
      if restool dprc connect dprc.1 --endpoint1="$A" --endpoint2="$C" >>"$R" 2>&1; then
        log "FAIL DPRC-I5: double-connect of $A was NOT refused"
      else
        log "RECORD DPRC-I5: double-connect of already-connected $A refused (disconnect-before-reconnect)"
      fi
    else
      log "SKIP DPRC-I5: fixture not established"
    fi
  fi
else
  log "SKIP LINK-I1: fixture not established"
  log "SKIP DPRC-I5: fixture not established"
fi

# Clean up what this hook created; the generated trap tears down only the trace's
# objects, so the hook destroys its own, in order, best-effort.
restool dprc disconnect dprc.1 --endpoint="$A" 2>>"$E" || true
for id in "$C" "$B" "$A"; do
  [ -n "$id" ] && restool dpni destroy "$id" 2>>"$E" || true
done

echo "vtraf1 refusal probes: $(grep -c '^RECORD ' "$R") recorded, $(grep -c '^FAIL ' "$R") failed, $(grep -c '^SKIP ' "$R") skipped"
