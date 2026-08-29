# V-CEIL-1 resource-ceiling face. Sourced by V-CEIL-1.sh after its one
# create (the scratch child dprc.2, which has the ALLOC bit and so draws
# from the same MC pools as the boot residents without starving them).
# Per family, in a fixed order, create in the child until restool is
# refused or a per-family cap of 64 is reached; record the count and the
# MC status text; read mc.global --resources before, at the ceiling, and
# after destroying what was made. dpbp is predicted refused at the 63rd
# (bp 63, one drawn at boot) with No resources; the others meet a pool or
# the cap. dpio/dpseci/dpsw/dpdmux/dprtc are out (boot-owned seats, per-
# create endpoint counts, or singletons).
# From the script: $OBJ_dprc_2, $RESULTS.
. "$(dirname "$0")/../residents.sh"
R="$RESULTS/ceil.txt"
DPRC="$OBJ_dprc_2"
CAP=64                     # per-family bound
abort=0

log() { echo "$1" | tee -a "$R"; }
resources() { restool dprc show mc.global --resources > "$RESULTS/ceil-res-$1.txt" 2>&1 || true; }
status_of() { grep -oE '\(status 0x[0-9a-fA-F]+\)|No resources|No memory available|No privilege|does not exist' "$1" 2>/dev/null | head -1; }

# sweep FAM [create-args...]: create until refused or the cap, record, read
# resources, destroy what was made, re-read, and check the pool returned.
sweep() {
  [ "$abort" = 0 ] || return 0
  fam="$1"; shift
  resources "before-$fam"
  ids=""; n=0; refusal=""
  while [ "$n" -lt "$CAP" ]; do
    err="$RESULTS/ceil-$fam-$((n + 1)).err"
    id=$(restool --script "$fam" create "$@" --container="$DPRC" 2>"$err")
    if [ -z "$id" ] || grep -qiE 'error|does not exist|status 0x' "$err"; then
      refusal="$(status_of "$err")"
      break
    fi
    ids="$ids $id"
    n=$((n + 1))
  done
  resources "at-ceiling-$fam"
  log "RECORD $fam: created $n before refusal '${refusal:-<cap reached>}'"

  # Abort on an MC-short status (No memory available 0x9, timeout 0x7):
  # that says the MC itself, not a pool, is short.
  case "$refusal" in
    *0x9* | *0x7* | *"No memory available"*)
      abort=1
      log "FAIL abort: $fam refusal is MC-short ($refusal) - stopping the hook"
      ;;
  esac

  # Destroy what this family made, in the child, never the root.
  for id in $ids; do restool "$fam" destroy "$id" 2>>"$RESULTS/ceil-$fam-destroy.err" || true; done
  resources "restored-$fam"

  # Leak check: the resources reading returns to its pre-family value.
  if diff -q "$RESULTS/ceil-res-before-$fam.txt" "$RESULTS/ceil-res-restored-$fam.txt" > /dev/null 2>&1; then
    log "PASS $fam: resources returned to their pre-family value after the destroys"
  else
    log "FAIL $fam: resources did not return to pre-family value (leak)"
  fi

  # restool must still answer between families.
  if restool dprc show dprc.1 > /dev/null 2>&1; then :; else
    log "FAIL $fam: restool dprc show dprc.1 did not answer after the sweep"
    abort=1
  fi
}

residents_pre
sweep dpbp
sweep dpcon --num-priorities=2
sweep dpmcp
sweep dpci
sweep dpdmai
sweep dpdcei --engine=DPDCEI_ENGINE_DECOMPRESSION --priority=1
sweep dpni
residents_post
echo; echo "ceil face: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
