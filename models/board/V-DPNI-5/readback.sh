# V-DPNI-5 PMD option-profile read-back. Sourced by V-DPNI-5.sh with the
# PMD-profile dpni ($OBJ_dpni_0) standing; the generated teardown trap
# reclaims it. docs/baseline/dpni.md production PMD profile
# (SINGLE_SENDER, CUSTOM_CG, HAS_KEY_MASKING, HAS_OPR, OPR_PER_TC,
# 0x80000000; 16q/16tc; num_cgs = queues + 8 = 24). Read-back is the only
# observation (dpni-typestate design D7); a wrong prediction here is
# evidentiary and feeds the model, not patched in the harness.
R="$RESULTS/readback.txt"
info() { restool dpni info "$OBJ_dpni_0" > "$RESULTS/readback-dpni.txt" 2>&1; }
field() { awk -F': ' -v k="$1" '$1 == k { print $2; exit }' "$RESULTS/readback-dpni.txt"; }
expect() { got="$(field "$1")"; if [ "$got" = "$2" ]; then r=PASS; else r=FAIL; fi; echo "$r dpni $1: $got (want $2)" | tee -a "$R"; }
record() { echo "RECORD dpni $1: $(field "$1")" | tee -a "$R"; }
opts() { echo "RECORD dpni options rendering: $(grep -iE 'option' "$RESULTS/readback-dpni.txt" | tr '\n' ';')" | tee -a "$R"; }

info
expect num_queues 16
expect num_tx_tcs 16
record num_rx_tcs            # >8 TCs: baseline holds Rx TCs cap at 8 [read]
expect num_cgs 24            # the deployed queues+8 heuristic (unknown-register #3)
expect vlan_entries 16
expect qos_entries 64
expect fs_entries 1
record mac_entries           # PMD sets none; a bare default is 16 (V-READBACK-1)
record num_opr               # label and honored value unknown (unknown-register #3)
opts

echo; echo "PMD read-back: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
