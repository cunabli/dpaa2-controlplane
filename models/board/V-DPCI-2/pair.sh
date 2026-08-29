# V-DPCI-2 mismatched-priority pair face. Sourced by the generated
# V-DPCI-2.sh after its last step (the root-pair connect) and before its
# teardown trap: the sixteen 2-priority dpcis in dprc.2 (dpci.0..dpci.15)
# and the connected root pair are standing. dpci.md unknown 5: may a dpci
# connect to a peer with a different priority count? From the script:
# $OBJ_dpci_0 (a 2-priority dpci in dprc.2), $OBJ_dprc_2, $RESULTS.
#
# The hook creates ONE fixture dpci at num-priorities=1 inside dprc.2 —
# allowed because the CONNECT is the thing under test, not the create
# (ADR-0007 §3) — connects it on the root to the 2-priority dpci.0, reads
# both ends back, then disconnects it. The fixture is never destroyed
# here: it is an owned resident of dprc.2 and dies with the container at
# teardown.
R="$RESULTS/pair.txt"
mc_status() { grep -o 'MC error:.*' "$1" | head -1; }
# peer_is TAG NAME: exit 0 iff the TAG-side `dpci info` names NAME as its
# connected peer (restool prints `connected peer: <name>`).
peer_is() { grep -q "connected peer: $2$" "$RESULTS/pair-info-$1.txt"; }

# The 1-priority fixture in dprc.2 (dies with the container at teardown).
FIX="$(restool --script dpci create --num-priorities=1 --container="${OBJ_dprc_2}" 2>"$RESULTS/pair-fix-err.txt")"
echo "RECORD fixture dpci (1 priority) in dprc.2: ${FIX:-<create failed>}" | tee -a "$R"

# Connect the 1-priority fixture to the 2-priority dpci.0 on the root
# ancestor (the container that holds topology-change privilege). unknown
# 5: mismatched priority counts across the edge.
restool dprc connect dprc.1 --endpoint1="${FIX}" --endpoint2="${OBJ_dpci_0}" > "$RESULTS/pair-connect.txt" 2>&1 || true
echo "RECORD mismatched-priority connect status: $(mc_status "$RESULTS/pair-connect.txt" || echo '<none on stderr>')" | tee -a "$R"

# Read both ends back: each should name the other as its peer.
restool dpci info "${FIX}" > "$RESULTS/pair-info-fix.txt" 2>&1 || true
restool dpci info "${OBJ_dpci_0}" > "$RESULTS/pair-info-dpci0.txt" 2>&1 || true
if peer_is fix "${OBJ_dpci_0}" && peer_is dpci0 "${FIX}"; then r=PASS; else r=FAIL; fi
echo "$r 1-priority fixture and 2-priority dpci.0 connected, each names the other (unknown 5)" | tee -a "$R"

# Disconnect the fixture again (allowed) so it is unconnected when the
# container destroy reclaims it.
restool dprc disconnect dprc.1 --endpoint="${FIX}" > "$RESULTS/pair-disconnect.txt" 2>&1 || true

echo; echo "pair face: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
