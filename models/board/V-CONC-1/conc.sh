# V-CONC-1 two-writer face. Sourced by V-CONC-1.sh after its one create
# (the scratch child dprc.2). ADR-0006 assumes one initiating writer per
# pass; this run learns whether that is load-bearing at the MC. Every
# restool invocation opens its own portal, so two restool loops are two
# writers. All three faces run inside the child; the hook destroys
# everything it made, in the child, before returning (ADR-0008 §7).
# From the script: $OBJ_dprc_2, $RESULTS.
. "$(dirname "$0")/../residents.sh"
R="$RESULTS/conc.txt"
DPRC="$OBJ_dprc_2"
N=32                       # loops are bounded (32 iterations each)

log() { echo "$1" | tee -a "$R"; }
count_dpbp() { restool dprc show "$DPRC" 2>/dev/null | awk '$1 ~ /^dpbp\./ { n++ } END { print n + 0 }'; }
# scan_abort FILE: a "Device is busy" (0xA) or timeout (0x7) status stops
# the hook — the MC itself, not a pool, is short (match on the status
# text, docs/baseline/mc-status.md).
abort=0
scan_abort() {
  if grep -qE 'Device is busy|status 0x7|status 0xa|status 0xA' "$1" 2>/dev/null; then
    abort=1
    log "FAIL abort: MC busy/timeout status in $1 - the single-writer contract is load-bearing at the MC (ADR-0006)"
  fi
}

residents_pre

# (a) Two writers, 32 dpbp creates each, concurrent. Expect 64 objects
# listed, 64 distinct ids, no MC error.
: > "$RESULTS/conc-a-1.txt"; : > "$RESULTS/conc-a-2.txt"
: > "$RESULTS/conc-a-1.err"; : > "$RESULTS/conc-a-2.err"
mkloop() {
  w="$1"; i=0
  while [ "$i" -lt "$N" ]; do
    i=$((i + 1))
    restool --script dpbp create --container="$DPRC" >> "$RESULTS/conc-a-$w.txt" 2>>"$RESULTS/conc-a-$w.err"
  done
}
mkloop 1 & p1=$!
mkloop 2 & p2=$!
wait "$p1" "$p2"
scan_abort "$RESULTS/conc-a-1.err"; scan_abort "$RESULTS/conc-a-2.err"
made=$(cat "$RESULTS/conc-a-1.txt" "$RESULTS/conc-a-2.txt" | grep -c 'dpbp\.')
distinct=$(cat "$RESULTS/conc-a-1.txt" "$RESULTS/conc-a-2.txt" | grep -o 'dpbp\.[0-9]*' | sort -u | wc -l | tr -d ' ')
listed=$(count_dpbp)
if [ "$abort" = 0 ] && [ "$made" -eq $((N * 2)) ] && [ "$distinct" -eq $((N * 2)) ] && [ "$listed" -eq $((N * 2)) ]; then
  log "PASS (a) two writers made $made dpbps, $distinct distinct, $listed listed - none lost, no id collision"
else
  log "FAIL (a) made=$made distinct=$distinct listed=$listed (want $((N * 2)) each), abort=$abort"
fi

# (b) One writer churns a dpbp (create then destroy) N times while the
# other lists the child N times; every listing must succeed and the final
# count must equal the pre-face count.
if [ "$abort" = 0 ]; then
  before_b=$(count_dpbp)
  : > "$RESULTS/conc-b.err"
  churn() {
    i=0
    while [ "$i" -lt "$N" ]; do
      i=$((i + 1))
      id=$(restool --script dpbp create --container="$DPRC" 2>>"$RESULTS/conc-b.err")
      [ -n "$id" ] && restool dpbp destroy "$id" 2>>"$RESULTS/conc-b.err"
    done
  }
  lister() {
    i=0; fail=0
    while [ "$i" -lt "$N" ]; do
      i=$((i + 1))
      restool dprc show "$DPRC" > /dev/null 2>>"$RESULTS/conc-b.err" || fail=$((fail + 1))
    done
    echo "$fail" > "$RESULTS/conc-b-listfail.txt"
  }
  churn & cp=$!
  lister & lp=$!
  wait "$cp" "$lp"
  scan_abort "$RESULTS/conc-b.err"
  listfail=$(cat "$RESULTS/conc-b-listfail.txt" 2>/dev/null || echo '?')
  after_b=$(count_dpbp)
  if [ "$abort" = 0 ] && [ "$listfail" = 0 ] && [ "$after_b" -eq "$before_b" ]; then
    log "PASS (b) $N listings all succeeded during churn; count returned to $before_b"
  else
    log "FAIL (b) listfail=$listfail before=$before_b after=$after_b abort=$abort"
  fi
fi

# (c) One writer destroys a dpbp while the other reads it: each read must
# resolve to the object or "does not exist", never hang. Count how often a
# destroyed id is minted again (ADR-0010 lowest-free reuse).
if [ "$abort" = 0 ]; then
  : > "$RESULTS/conc-c.err"
  reused=0; badread=0; prev=""; i=0
  while [ "$i" -lt "$N" ]; do
    i=$((i + 1))
    id=$(restool --script dpbp create --container="$DPRC" 2>>"$RESULTS/conc-c.err")
    ( restool dpbp info "$id" > "$RESULTS/conc-c-read-$i.txt" 2>&1 ) & rp=$!
    restool dpbp destroy "$id" 2>>"$RESULTS/conc-c.err"
    wait "$rp"
    grep -qiE 'does not exist|dpbp id' "$RESULTS/conc-c-read-$i.txt" || { badread=$((badread + 1)); log "RECORD (c) read $i: $(head -1 "$RESULTS/conc-c-read-$i.txt")"; }
    [ "$id" = "$prev" ] && reused=$((reused + 1))
    prev="$id"
  done
  scan_abort "$RESULTS/conc-c.err"
  log "RECORD (c) destroyed id minted again $reused/$N times (ADR-0010 lowest-free reuse)"
  if [ "$abort" = 0 ] && [ "$badread" = 0 ]; then
    log "PASS (c) every read resolved to the object or 'does not exist', no hang"
  else
    log "FAIL (c) badread=$badread abort=$abort"
  fi
fi

# Destroy everything this hook made and left standing (face (a)'s 64), in
# the child, before the generated teardown destroys the now-empty child.
for id in $(cat "$RESULTS/conc-a-1.txt" "$RESULTS/conc-a-2.txt" 2>/dev/null | grep -o 'dpbp\.[0-9]*'); do
  restool dpbp destroy "$id" 2>>"$RESULTS/conc-cleanup.err" || true
done

residents_post
echo; echo "conc face: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
