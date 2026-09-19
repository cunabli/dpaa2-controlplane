# V-DPNI-7 unread-flag read-back. Sourced by V-DPNI-7.sh with a dpni
# ($OBJ_dpni_0) carrying DPNI_OPT_TX_FRM_RELEASE, HAS_POLICING and
# SHARED_CONGESTION standing; the generated teardown trap reclaims it.
# docs/baseline/dpni.md unknown-register #6: no consumer reads these
# flags, so the open question is whether the MC accepts them and how
# `dpni info` renders them. The create step already read the object
# present; here the option rendering is recorded (read-back is the only
# observation, dpni-typestate design D7).
R="$RESULTS/readback.txt"
info() { restool dpni info "$OBJ_dpni_0" > "$RESULTS/readback-dpni.txt" 2>&1; }
field() { awk -F': ' -v k="$1" '$1 == k { print $2; exit }' "$RESULTS/readback-dpni.txt"; }
record() { echo "RECORD dpni $1: $(field "$1")" | tee -a "$R"; }
opts() { echo "RECORD dpni options rendering: $(grep -iE 'option' "$RESULTS/readback-dpni.txt" | tr '\n' ';')" | tee -a "$R"; }

info
opts                         # how the MC renders the three unread flags (unknown-register #6)
record num_queues
record num_tx_tcs

echo; echo "unread-flag read-back: $(grep -c '^RECORD ' "$R") RECORD (the rendering is the answer)"
