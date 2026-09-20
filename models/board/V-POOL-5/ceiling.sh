# V-POOL-5 dpbp census/ceiling face at ROOT scope. Sourced by V-POOL-5.sh
# after its one create (OBJ_dpbp_0, a dpbp in dprc.1) and before the teardown
# trap. Two faces: DPBP-I7 (a dpbp create is refused with No resources exactly
# when the buffer-pool free count reaches zero — the census predicts the
# refusal to the object) taken against the ROOT's own listing, and DPBP-I5
# (does a runtime dpbp's bpid equal its object id) read back on OBJ_dpbp_0.
#
# ROOT scope by pool-objects design D5: the pool the reconciler manages is
# dprc.1, so the census the convergence loop reads is this pool's free count.
# Unlike V-CEIL-1 (scratch child), this sweep creates and destroys in the
# ROOT, so it settles after every destroy and guards the boot residents
# (residents.sh) — the destroy rescan race (ADR-0008 §4) is caught, not
# silent. The 4.2 operator: this sweep briefly draws the root buffer-pool to
# zero and destroys ~63 dpbps in the Linux root; run it from a clean boot and
# read residents.txt before trusting a later sitting.
# From the script: $OBJ_dpbp_0, $RESULTS.
. "$(dirname "$0")/../residents.sh"
R="$RESULTS/ceiling.txt"
DPRC=dprc.1
CAP=200                    # a bound well above the ~63 bp floor; a runaway stops here

log() { echo "$1" | tee -a "$R"; }
resources() { restool dprc show mc.global --resources > "$RESULTS/ceiling-res-$1.txt" 2>&1 || true; }
status_of() { grep -oE '\(status 0x[0-9a-fA-F]+\)|No resources|No memory available|No privilege|does not exist' "$1" 2>/dev/null | head -1; }
# bp_free FILE: the free count restool prints on the `bp:` line of --resources.
bp_free() { awk '$1=="bp:"{print $2}' "$1"; }
# field FAM FILE KEY: the value restool prints after "KEY: " in an info dump.
field() { awk -F': ' -v k="$2" '$1 == k {print $2; exit}' "$1"; }

residents_pre

# DPBP-I5: read OBJ_dpbp_0 back and compare its object id to its buffer pool id.
restool dpbp info "$OBJ_dpbp_0" > "$RESULTS/ceiling-dpbp-info.txt" 2>&1 || true
id="$(field dpbp "$RESULTS/ceiling-dpbp-info.txt" 'dpbp id')"
bpid="$(field dpbp "$RESULTS/ceiling-dpbp-info.txt" 'buffer pool id')"
log "RECORD (I5) OBJ_dpbp_0 dpbp id=$id buffer pool id=$bpid bpid-equals-id=$([ "$id" = "$bpid" ] && echo yes || echo no)"

# DPBP-I7: read the census, then create dpbp in the ROOT until refused; the
# count created before the refusal must equal the free count the census read.
resources before
predicted="$(bp_free "$RESULTS/ceiling-res-before.txt")"
log "RECORD (I7) census predicts refusal after $predicted dpbp creates (bp free count at sweep start)"

ids=""; n=0; refusal=""
while [ "$n" -lt "$CAP" ]; do
  err="$RESULTS/ceiling-create-$((n + 1)).err"
  new=$(restool --script dpbp create --container="$DPRC" 2>"$err")
  if [ -z "$new" ] || grep -qiE 'error|does not exist|status 0x' "$err"; then
    refusal="$(status_of "$err")"
    break
  fi
  ids="$ids $new"
  n=$((n + 1))
done
resources at-ceiling
log "RECORD (I7) created $n before refusal '${refusal:-<cap reached>}'"

# The refusal-is-an-answer convention (mbt-harness spec): a refusal is the
# observed verdict, not a failure. PASS iff it landed at the census prediction
# with No resources.
if [ "$n" = "$predicted" ] && printf '%s' "$refusal" | grep -q "No resources"; then
  log "PASS (I7) refusal at the census prediction ($n) with No resources"
else
  log "FAIL (I7) refusal at $n (predicted $predicted), status '${refusal:-<none>}'"
fi

# Destroy what this sweep made, in the ROOT, settling after each (ADR-0008 §4).
for did in $ids; do
  restool dpbp destroy "$did" 2>>"$RESULTS/ceiling-destroy.err" || true
  sleep 1
done
resources restored

# Every unit returns: the free count is back at the pre-sweep prediction.
after="$(bp_free "$RESULTS/ceiling-res-restored.txt")"
if [ "$after" = "$predicted" ]; then
  log "PASS (I7) every dpbp returned its unit (bp free $after back to $predicted)"
else
  log "FAIL (I7) bp free $after did not return to the pre-sweep $predicted (leak)"
fi

residents_post
echo; echo "ceiling face: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
