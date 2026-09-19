# V-DPNI-6 kernel option-profile read-back. Sourced by V-DPNI-6.sh with
# the kernel-profile dpni ($OBJ_dpni_0) standing; the generated teardown
# trap reclaims it. docs/baseline/dpni.md production kernel profile
# (HAS_KEY_MASKING only; 1q/1tc; everything else at the MC default).
# Read-back is the only observation (dpni-typestate design D7).
R="$RESULTS/readback.txt"
info() { restool dpni info "$OBJ_dpni_0" > "$RESULTS/readback-dpni.txt" 2>&1; }
field() { awk -F': ' -v k="$1" '$1 == k { print $2; exit }' "$RESULTS/readback-dpni.txt"; }
expect() { got="$(field "$1")"; if [ "$got" = "$2" ]; then r=PASS; else r=FAIL; fi; echo "$r dpni $1: $got (want $2)" | tee -a "$R"; }
record() { echo "RECORD dpni $1: $(field "$1")" | tee -a "$R"; }
opts() { echo "RECORD dpni options rendering: $(grep -iE 'option' "$RESULTS/readback-dpni.txt" | tr '\n' ';')" | tee -a "$R"; }

info
expect num_queues 1
expect num_tx_tcs 1
expect num_rx_tcs 1
record num_cgs               # default one CG per TC (unknown-register #3)
opts                         # expect HAS_KEY_MASKING only, no PFDR_IN_PEB/SINGLE_SENDER

echo; echo "kernel read-back: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
