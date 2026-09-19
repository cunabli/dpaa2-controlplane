# V-DPNI-9 HAS_REPLICATION (0x4000) accept/reject probe. Sourced by
# V-DPNI-9.sh with one scratch container ($OBJ_dprc_2) standing. restool's
# option map knows DPNI_OPT_HAS_REPLICATION, but the MC 10.39.0 flib
# header does not list 0x4000 (docs/baseline/dpni.md unknown-register #8):
# the create is issued with the raw bit and whatever the MC does is the
# answer. An MC refusal is recorded as the observed verdict and cleanup
# continues; the run is not failed by the refusal itself (mbt-harness spec
# "A probe refusal is an answer"). The probe emits no PASS/FAIL — only a
# RECORD — precisely so acceptance and rejection are both conforming.
R="$RESULTS/replication.txt"
NAME="$(restool --script dpni create --options=0x4000 --container="$OBJ_dprc_2" 2>"$RESULTS/replication.err")"; RC=$?
status="$(cat "$RESULTS/replication.err" 2>/dev/null)"
cat "$RESULTS/replication.err" >> "$RESULTS/replication.log" 2>/dev/null || true

if [ -n "$NAME" ] && restool dpni info "$NAME" > "$RESULTS/replication-info.txt" 2>/dev/null; then
  echo "RECORD dpni 0x4000: ACCEPTED — $NAME created (rc=$RC), reads back present (unknown-register #8: the MC option is real)" | tee -a "$R"
  restool dpni destroy "$NAME" >> "$RESULTS/replication.log" 2>&1 || true
  sleep 2
else
  echo "RECORD dpni 0x4000: REJECTED — rc=$RC, nothing reads back, MC status: ${status:-none} (unknown-register #8: restool ahead of firmware)" | tee -a "$R"
fi

echo; echo "replication face: $(grep -c '^RECORD ' "$R") RECORD (probe refusal is an answer; run not failed)"
