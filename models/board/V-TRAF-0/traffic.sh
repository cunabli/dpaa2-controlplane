# V-TRAF-0 traffic face. Sourced by the generated V-TRAF-0.sh after its
# last step (link up on dpmac.7) and before its teardown trap: the scratch
# group is standing here and is torn down whatever happens below.
# From the script: $OBJ_dpni_2 = the scratch dpni's board name, $RESULTS.
dpni="$OBJ_dpni_2"
netdev="$(ls /sys/bus/fsl-mc/devices/"$dpni"/net/)"
count() { restool dpni info "$dpni" | awk -v k="$1:" '$1 == k {print $2}'; }
ask() { printf '\n>> %s\n   enter when done: ' "$1"; read -r _; }
# verdict NAME "reading" delta want: PASS/FAIL line, kept in traffic.txt; false on FAIL
verdict() {
  if [ "$3" -ge "$4" ]; then r=PASS; else r=FAIL; fi
  echo "$r $1: $2 (+$3, want +$4)" | tee -a "$RESULTS/traffic.txt"; [ $r = PASS ]
}
# retry FACE: run the face until it passes or the operator moves on
retry() { until "$1"; do printf '   r=retry, enter=continue: '; read -r k; [ "$k" = r ] || break; done; }

# Inbound: the peer sends 16 broadcast frames; the dpni must count them in.
inbound() {
  in0=$(count ingress_all_frames)
  ask "on the peer: COUNT=16 MODE=broadcast ./scripts/cn10k-pktgen.sh start ; ./scripts/cn10k-pktgen.sh stop"
  in1=$(count ingress_all_frames)
  verdict inbound "ingress_all_frames $in0 -> $in1" $((in1 - in0)) 16
}

# Outbound: 8 pings to the peer port's MAC (static neighbour, so no ARP);
# the dpni must count them out, the peer counts them in as ip4 + drops
# (no address on its port: counted, then dropped).
outbound() {
  out0=$(count egress_all_frames)
  ping -c 8 -i 0.5 -W 1 -I "$netdev" 100.96.0.2 > "$RESULTS/traffic-ping.txt" 2>&1 || true   # no reply expected
  out1=$(count egress_all_frames)
  verdict outbound "egress_all_frames $out0 -> $out1" $((out1 - out0)) 8
}

echo; echo "traffic face: $dpni on $netdev"
ip addr replace 100.96.0.1/24 dev "$netdev"     # a source address for ping, nothing more
retry inbound
printf '\n>> peer port MAC (vppctl show hardware-interfaces TenGigaEthernet0/0, "Ethernet address"): '
read -r mac
ip neigh replace 100.96.0.2 lladdr "$mac" dev "$netdev"
ask "on the peer, note ip4 and drops: vppctl show interface TenGigaEthernet0/0"
retry outbound
printf '\n>> on the peer, read ip4 and drops again; type both deltas (want 8 8): '
read -r peer
echo "peer ip4/drops deltas: $peer (want 8 8)" | tee -a "$RESULTS/traffic.txt"
