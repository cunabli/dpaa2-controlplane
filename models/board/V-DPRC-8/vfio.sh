# V-DPRC-8 VFIO face. Sourced by V-DPRC-8.sh after its last trace step,
# with the scratch child dprc.2 standing and one owned dpbp.0 created in
# it. The core machine cannot render a Dprc VFIO bind (machine.qnt:340
# `o.fam != Dprc`) and the generator has no driver_override read-back, so
# the whole VFIO program is here: surface the child's bus node, override +
# bind it (unplugged, as the board's own VPP child runs), read the
# propagation of the override to a post-bind resident, and check the MC
# census is unmoved across bind (DPRC-I7). bind/unbind mirror
# models/families/dprc.qnt `bindVfio`/`unbindVfio` on the `Plugged` face.
# From the script: $OBJ_dprc_2, $OBJ_dpbp_0, $RESULTS.
. "$(dirname "$0")/../residents.sh"
R="$RESULTS/vfio.txt"
DEV=/sys/bus/fsl-mc/devices
CHILD="$OBJ_dprc_2"
DPBP="$OBJ_dpbp_0"
VFIO_DPBP2=""            # the post-bind resident (h); set once created
route=""                 # the node-surfacing route that worked (c)

log() { echo "$1" | tee -a "$R"; }
# Settle after every bus-affecting op — the rescan race (ADR-0008), same
# duration as the generated teardown.
settle() { sleep 2; }
node_present() { [ -e "$DEV/$1" ]; }
driver_of() { drv="$(readlink "$DEV/$1/driver" 2>/dev/null)"; echo "${drv##*/}"; }
census() {
  restool dprc show "$CHILD" > "$RESULTS/vfio-census-$1-child.txt" 2>&1 || true
  restool dprc show dprc.1  > "$RESULTS/vfio-census-$1-root.txt"  2>&1 || true
}
census_eq() {
  diff -q "$RESULTS/vfio-census-$1-child.txt" "$RESULTS/vfio-census-$2-child.txt" >/dev/null 2>&1 &&
  diff -q "$RESULTS/vfio-census-$1-root.txt"  "$RESULTS/vfio-census-$2-root.txt"  >/dev/null 2>&1
}
# surface: replay the route that first surfaced the child node (c) so a
# post-bind resident's node appears the same way.
surface() {
  case "$route" in
    sync)   restool dprc sync > "$RESULTS/vfio-surface.txt" 2>&1 || true ;;
    rescan) echo 1 > /sys/bus/fsl-mc/rescan 2>>"$RESULTS/vfio-rescan.log" || true ;;
  esac
  settle
}

# Cleanup on every exit path: best-effort unbind of the child if still
# bound, clear the override on the child and on any resident it reached.
# Never destroys MC objects — the generated teardown destroys the trace
# objects and the container cascade reclaims the hook's post-bind dpbp.
_vfio_cleanup() {
  [ -e "$DEV/$CHILD/driver" ] && echo "$CHILD" > "$DEV/$CHILD/driver/unbind" 2>>"$RESULTS/vfio-cleanup.log" || true
  for o in "$CHILD" "$DPBP" "$VFIO_DPBP2"; do
    [ -n "$o" ] && [ -e "$DEV/$o/driver_override" ] && echo "" > "$DEV/$o/driver_override" 2>>"$RESULTS/vfio-cleanup.log" || true
  done
  teardown
}
trap _vfio_cleanup EXIT

residents_pre

# (b) A fresh restool-created child has no bus node (V-POOL-1 rev 1 re-anchor).
log "RECORD (b) child $CHILD bus node at create: $(node_present "$CHILD" && echo present || echo absent)"

# (c) Route hunt: dprc sync first, then the bus rescan; record which (if
# either) surfaces the child node. Do not predict.
restool dprc sync > "$RESULTS/vfio-sync.txt" 2>&1 || true
settle
node_present "$CHILD" && route="sync"
log "RECORD (c) child node after 'restool dprc sync': $(node_present "$CHILD" && echo present || echo absent)"
if [ -z "$route" ]; then
  echo 1 > /sys/bus/fsl-mc/rescan 2>>"$RESULTS/vfio-rescan.log" || true
  settle
  node_present "$CHILD" && route="rescan"
  log "RECORD (c) child node after 'echo 1 > /sys/bus/fsl-mc/rescan': $(node_present "$CHILD" && echo present || echo absent)"
fi

if [ -z "$route" ]; then
  log "RECORD (c) bind face unreachable via sync and rescan - the ledger judges (V-POOL-1 skipped-faces precedent, no FAIL for a conforming board); faces (d)-(j) skipped"
else
  # (d) Census A — before any bind.
  census A

  # (e) Bind: driver_override then bind. The override bypasses the bus
  # plugged check, so an unplugged child binds (as dprc.2/VPP runs).
  echo vfio-fsl-mc > "$DEV/$CHILD/driver_override" 2>>"$RESULTS/vfio-bind.log" || true
  echo "$CHILD" > /sys/bus/fsl-mc/drivers/vfio-fsl-mc/bind 2>>"$RESULTS/vfio-bind.log" || true
  settle
  if [ "$(driver_of "$CHILD")" = vfio-fsl-mc ]; then
    log "PASS (e) child $CHILD bound to vfio-fsl-mc"
  else
    log "FAIL (e) child $CHILD not bound to vfio-fsl-mc (driver='$(driver_of "$CHILD")')"
  fi
  log "RECORD (e) child iommu_group: $(readlink "$DEV/$CHILD/iommu_group" 2>/dev/null | sed 's#.*/##')"

  # (f) Census B — a bind is bus-only, the MC census must be unmoved (DPRC-I7).
  census B
  if census_eq A B; then
    log "PASS (f) DPRC-I7: MC census unmoved across bind (A==B)"
  else
    log "FAIL (f) DPRC-I7: MC census moved across bind (A!=B)"
  fi

  # (g) Pre-bind resident face: whether the trace's dpbp now has a bus node
  # and what its override reads (the bind-time container scan face).
  if node_present "$DPBP"; then
    log "RECORD (g) trace dpbp $DPBP bus node present, driver_override='$(cat "$DEV/$DPBP/driver_override" 2>/dev/null)'"
  else
    log "RECORD (g) trace dpbp $DPBP has no bus node (child residents not bus-visible, DPRC-I6)"
  fi

  # (h) Post-bind add: create a second dpbp in the child with the exact
  # rendering the generated trace step uses, surface it by the route from
  # (c), and read the override. A bound container propagates the override
  # to a subsequently added resident (dprc.md:230-234) — the PASS/FAIL line.
  VFIO_DPBP2="$(restool --script dpbp create --container=${CHILD} 2>>"$RESULTS/vfio-add.log")"
  log "RECORD (h) post-bind create: dpbp $VFIO_DPBP2 in $CHILD"
  if [ -z "$VFIO_DPBP2" ]; then
    # A refused create into a VFIO-bound container is its own board answer;
    # do not run the propagation probe with an empty name.
    log "FAIL (h) post-bind dpbp create refused in bound container $CHILD (see vfio-add.log)"
  else
    surface
    if node_present "$VFIO_DPBP2"; then
      ov2="$(cat "$DEV/$VFIO_DPBP2/driver_override" 2>/dev/null)"
      if [ "$ov2" = vfio-fsl-mc ]; then
        log "PASS (h) override propagated to post-bind resident $VFIO_DPBP2 (driver_override='vfio-fsl-mc')"
      else
        log "FAIL (h) post-bind resident $VFIO_DPBP2 driver_override='$ov2' not vfio-fsl-mc"
      fi
    else
      log "FAIL (h) post-bind resident $VFIO_DPBP2 never got a bus node (propagation unobservable)"
    fi
  fi

  # (i) Census C, then unbind and clear the override.
  census C
  echo "$CHILD" > "$DEV/$CHILD/driver/unbind" 2>>"$RESULTS/vfio-unbind.log" || true
  echo "" > "$DEV/$CHILD/driver_override" 2>>"$RESULTS/vfio-unbind.log" || true
  settle
  if [ -z "$(driver_of "$CHILD")" ]; then
    log "PASS (i) child $CHILD unbound (driver symlink gone)"
  else
    log "FAIL (i) child $CHILD still bound after unbind (driver='$(driver_of "$CHILD")')"
  fi
  log "RECORD (i) child driver_override after clear: '$(cat "$DEV/$CHILD/driver_override" 2>/dev/null)'"
  census D
  if census_eq C D; then
    log "PASS (i) DPRC-I7: MC census unmoved across unbind (C==D)"
  else
    log "FAIL (i) DPRC-I7: MC census moved across unbind (C!=D)"
  fi

  # (j) Eligibility: re-set override, re-bind, then unbind and clear again.
  echo vfio-fsl-mc > "$DEV/$CHILD/driver_override" 2>>"$RESULTS/vfio-elig.log" || true
  echo "$CHILD" > /sys/bus/fsl-mc/drivers/vfio-fsl-mc/bind 2>>"$RESULTS/vfio-elig.log" || true
  settle
  if [ "$(driver_of "$CHILD")" = vfio-fsl-mc ]; then
    log "PASS (j) eligibility restored: child $CHILD re-bound after unbind"
  else
    log "FAIL (j) child $CHILD did not re-bind (driver='$(driver_of "$CHILD")')"
  fi
  echo "$CHILD" > "$DEV/$CHILD/driver/unbind" 2>>"$RESULTS/vfio-elig.log" || true
  echo "" > "$DEV/$CHILD/driver_override" 2>>"$RESULTS/vfio-elig.log" || true
  settle
fi

residents_post
echo; echo "vfio face: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
