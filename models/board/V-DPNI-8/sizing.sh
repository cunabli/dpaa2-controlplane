# V-DPNI-8 sizing-field walk. Sourced by V-DPNI-8.sh with one scratch
# container ($OBJ_dprc_2) standing; every fixture created below is
# destroyed in the same loop (with the ADR-0008 settle after each
# destroy), and the container is reclaimed by the generated teardown trap.
# docs/baseline/dpni.md unknown-register #3: the semantics of num_cgs,
# num_opr and dist_key_size, and the rationale of num_cgs = num_queues + 8.
# Each fixture is bare but for the one field under test, so its read-back
# isolates that field. Read-back is the only observation; a mid-walk MC
# refusal is recorded, not fatal (dpni-typestate design Risks).
R="$RESULTS/sizing.txt"
mk() { NAME="$(restool --script dpni create "$@" --container="$OBJ_dprc_2" 2>>"$RESULTS/sizing.log")"; RC=$?; }
info() { restool dpni info "$NAME" > "$RESULTS/.info" 2>>"$RESULTS/sizing.log"; }
field() { awk -F': ' -v k="$1" '$1 == k { print $2; exit }' "$RESULTS/.info"; }
gone() { restool dpni destroy "$NAME" >>"$RESULTS/sizing.log" 2>&1 || true; sleep 2; }

# num_cgs walk (restool range 1–128; deployed heuristic queues+8): does
# the read-back honor the requested count, and where (if anywhere) the MC
# caps it.
for v in 1 8 24 64 128; do
  mk --num-cgs="$v"
  if [ "$RC" -ne 0 ] || [ -z "$NAME" ]; then
    echo "RECORD dpni num_cgs=$v: create rc=$RC (refused — an answer)" | tee -a "$R"; continue
  fi
  info
  echo "RECORD dpni num_cgs=$v: read back num_cgs=$(field num_cgs)" | tee -a "$R"
  gone
done

# num_opr walk (restool range 1–128; default num_tcs×num_queues).
for v in 1 16 128; do
  mk --num-opr="$v"
  if [ "$RC" -ne 0 ] || [ -z "$NAME" ]; then
    echo "RECORD dpni num_opr=$v: create rc=$RC (refused — an answer)" | tee -a "$R"; continue
  fi
  info
  echo "RECORD dpni num_opr=$v: read back num_opr=$(field num_opr) (label may be absent)" | tee -a "$R"
  gone
done

# dist_key_size walk (restool range 1–56): write-only — absent from
# dpni_attr (dpni-typestate design D4; DPNI-I12), so the probe confirms it
# does not read back.
for v in 1 24 56; do
  mk --dist-key-size="$v"
  if [ "$RC" -ne 0 ] || [ -z "$NAME" ]; then
    echo "RECORD dpni dist_key_size=$v: create rc=$RC (refused — an answer)" | tee -a "$R"; continue
  fi
  info
  dks="$(field dist_key_size)"
  if [ -z "$dks" ]; then
    echo "PASS dpni dist_key_size=$v: write-only, absent from dpni info (DPNI-I12)" | tee -a "$R"
  else
    echo "FAIL dpni dist_key_size=$v: unexpectedly read back as $dks" | tee -a "$R"
  fi
  gone
done

echo; echo "sizing face: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
