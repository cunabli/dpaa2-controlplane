# V-POOL-4 companion-draw face. Sourced by V-POOL-4.sh after its one
# create (the scratch child dprc.2, restool's default create, which draws
# from the same MC pools as the boot residents without starving them).
# For dpio and then dpni, in that order: read mc.global --resources,
# create three objects one at a time reading after each, then destroy them
# one at a time reading after each. Per consecutive pair of readings a
# RECORD line carries the changed pool lines (an awk join of the two files
# — the board has no jq); the PASS lines are `mcp` unchanged across the
# family, the second and third per-object deltas equal to the first (a
# linear draw), and the pools back at their pre-family reading after the
# destroys. Every create is in the child and every destroy is an id this
# hook captured from its own create (ADR-0008 §7).
# From the script: $OBJ_dprc_2, $RESULTS.
. "$(dirname "$0")/../residents.sh"
R="$RESULTS/pool.txt"
DPRC="$OBJ_dprc_2"
N=3                        # objects per family
abort=0

log() { echo "$1" | tee -a "$R"; }
resources() { restool dprc show mc.global --resources > "$RESULTS/pool4-res-$1.txt" 2>&1 || true; }
status_of() { grep -oE '\(status 0x[0-9a-fA-F]+\)|No resources|No memory available|No privilege|does not exist' "$1" 2>/dev/null | head -1; }
# delta BEFORE AFTER: the changed pool lines as "key old->new " pairs.
delta() { awk 'NR==FNR{a[$1]=$2;next} a[$1]!=$2{printf "%s %s->%s ", $1, a[$1], $2}' "$1" "$2"; }
# draw BEFORE AFTER: the same lines as "key signed-difference " pairs —
# what the linearity check compares. Rev 1 compared the delta strings,
# whose absolute readings differ at every step by construction, and so
# failed two draws that were linear to the unit.
draw() { awk 'NR==FNR{a[$1]=$2;next} a[$1]!=$2{printf "%s %d ", $1, $2-a[$1]}' "$1" "$2"; }
mcp_of() { awk '$1=="mcp:"{print $2}' "$1"; }
# record LABEL DELTA: a RECORD line, or the empty marker when nothing moved.
record() { if [ -n "$2" ]; then log "RECORD $1: $2"; else log "RECORD $1: <no pool line moved>"; fi; }
# liveness FAM: restool must still answer between and after the families.
liveness() {
  if restool dprc show dprc.1 > /dev/null 2>&1; then :; else
    log "FAIL $1: restool dprc show dprc.1 did not answer after the family"
    abort=1
  fi
}

# family FAM: read before, create N reading after each, destroy reading
# after each, then RECORD every pair and PASS/FAIL the three properties.
family() {
  [ "$abort" = 0 ] || return 0
  fam="$1"
  b="$RESULTS/pool4-res-before-$fam.txt"
  resources "before-$fam"

  ids=""; made=0; refusal=""
  k=0
  while [ "$k" -lt "$N" ]; do
    k=$((k + 1))
    err="$RESULTS/pool4-$fam-create-$k.err"
    id=$(restool --script "$fam" create --container="$DPRC" 2>"$err")
    if [ -z "$id" ] || grep -qiE 'error|does not exist|status 0x' "$err"; then
      refusal="$(status_of "$err")"
      k=$((k - 1))
      break
    fi
    ids="$ids $id"; made=$((made + 1))
    resources "$fam-create-$k"
  done

  # A refusal ends this family's series after destroying what was made.
  if [ -n "$refusal" ]; then
    log "FAIL $fam: create refused with '${refusal:-<unknown>}'"
    case "$refusal" in
      *0x9* | *0x7* | *"No memory available"*)
        abort=1
        log "FAIL abort: $fam refusal is MC-short ($refusal) - stopping the hook"
        ;;
    esac
    for id in $ids; do restool "$fam" destroy "$id" 2>>"$RESULTS/pool4-$fam-destroy.err" || true; done
    liveness "$fam"
    return 0
  fi

  # Per-create RECORD lines and their delta strings.
  c1="$RESULTS/pool4-res-$fam-create-1.txt"
  c2="$RESULTS/pool4-res-$fam-create-2.txt"
  c3="$RESULTS/pool4-res-$fam-create-3.txt"
  record "$fam create 1" "$(delta "$b" "$c1")"
  record "$fam create 2" "$(delta "$c1" "$c2")"
  record "$fam create 3" "$(delta "$c2" "$c3")"
  dc1="$(draw "$b" "$c1")"; dc2="$(draw "$c1" "$c2")"; dc3="$(draw "$c2" "$c3")"

  # Destroy one at a time, reading after each.
  k=0
  for id in $ids; do
    k=$((k + 1))
    restool "$fam" destroy "$id" 2>>"$RESULTS/pool4-$fam-destroy.err" || true
    resources "$fam-destroy-$k"
  done
  d1="$RESULTS/pool4-res-$fam-destroy-1.txt"
  d2="$RESULTS/pool4-res-$fam-destroy-2.txt"
  d3="$RESULTS/pool4-res-$fam-destroy-3.txt"
  record "$fam destroy 1" "$(delta "$c3" "$d1")"
  record "$fam destroy 2" "$(delta "$d1" "$d2")"
  record "$fam destroy 3" "$(delta "$d2" "$d3")"

  # (a) mcp unchanged between the pre-family reading and every reading.
  mcp_b="$(mcp_of "$b")"; mcp_bad=""
  for f in "$c1" "$c2" "$c3" "$d1" "$d2" "$d3"; do
    m="$(mcp_of "$f")"
    [ "$m" = "$mcp_b" ] || mcp_bad="$mcp_bad ${f##*/}=$m"
  done
  if [ -z "$mcp_bad" ]; then
    log "PASS $fam: mcp unchanged across the family"
  else
    log "FAIL $fam: mcp moved (before=$mcp_b;$mcp_bad)"
  fi

  # (b) the second and third per-object draws equal the first (linear).
  if [ "$dc2" = "$dc1" ] && [ "$dc3" = "$dc1" ]; then
    log "PASS $fam: per-object draw is linear ($dc1)"
  else
    log "FAIL $fam: first-object draw differs from the rest (create1='$dc1' create2='$dc2' create3='$dc3')"
  fi

  # (c) the reading after the last destroy returns to the pre-family value.
  if diff -q "$b" "$d3" > /dev/null 2>&1; then
    log "PASS $fam: resources returned to their pre-family value after the destroys"
  else
    log "FAIL $fam: resources did not return to pre-family value (leak)"
  fi

  liveness "$fam"
}

residents_pre
family dpio
family dpni
residents_post
echo; echo "pool face: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
