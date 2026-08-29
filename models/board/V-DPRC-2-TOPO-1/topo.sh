# V-DPRC-2-TOPO-1 child-scoped topology face. Sourced by the generated
# V-DPRC-2-TOPO-1.sh after its last step (the root-rendered connect) and
# before its teardown trap: dprc.2 (built with TOPOLOGY_CHANGES) and its
# two connected dpcis (dpci.0, dpci.1) are standing. From the script:
# $OBJ_dprc_2, $OBJ_dpci_0, $OBJ_dpci_1, $RESULTS. No prompts, no operator.
#
# The positive control for V-DPCI-1's No-privilege refusal: a
# default-created container's own portal cannot connect anything, so every
# generated connect renders against the root ancestor. dprc.2 holds
# DPRC_CFG_OPT_TOPOLOGY_CHANGES_ALLOWED, so a connect issued ON dprc.2
# should now succeed. The hook disconnects the pair through dprc.2's own
# portal, then re-connects it through dprc.2 — both child-scoped — and PASS
# is the child-scoped connect succeeding with each dpci naming the other.
# The pair is left connected, matching the trace's final state.
R="$RESULTS/topo.txt"
mc_status() { grep -o 'MC error:.*' "$1" | head -1; }
# peer_is TAG NAME: exit 0 iff the TAG-side `dpci info` names NAME as its
# connected peer (restool prints `connected peer: <name>`).
peer_is() { grep -q "connected peer: $2$" "$RESULTS/topo-info-$1.txt"; }

# (a) Disconnect the pair through dprc.2's own portal (topology-change
# privilege). RECORD the status; the reconnect below is the real probe.
restool dprc disconnect "${OBJ_dprc_2}" --endpoint="${OBJ_dpci_0}" > "$RESULTS/topo-disconnect.txt" 2>&1 || true
echo "RECORD child-scoped disconnect status: $(mc_status "$RESULTS/topo-disconnect.txt" || echo '<none on stderr>')" | tee -a "$R"

# (b) Re-connect the pair through dprc.2's own portal — the control probe.
# A default container is refused No privilege here (V-DPCI-1); with
# TOPOLOGY_CHANGES the child portal drives the connect itself.
restool dprc connect "${OBJ_dprc_2}" --endpoint1="${OBJ_dpci_0}" --endpoint2="${OBJ_dpci_1}" > "$RESULTS/topo-connect.txt" 2>&1
cc=$?
echo "RECORD child-scoped connect status: $(mc_status "$RESULTS/topo-connect.txt" || echo '<none on stderr>')" | tee -a "$R"

# Read both ends back: each should name the other as its peer.
restool dpci info "${OBJ_dpci_0}" > "$RESULTS/topo-info-dpci0.txt" 2>&1 || true
restool dpci info "${OBJ_dpci_1}" > "$RESULTS/topo-info-dpci1.txt" 2>&1 || true
if [ "$cc" -eq 0 ] && peer_is dpci0 "${OBJ_dpci_1}" && peer_is dpci1 "${OBJ_dpci_0}"; then r=PASS; else r=FAIL; fi
echo "$r child-scoped connect on dprc.2 succeeds (TOPOLOGY_CHANGES lets its own portal connect)" | tee -a "$R"

echo; echo "topo face: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
