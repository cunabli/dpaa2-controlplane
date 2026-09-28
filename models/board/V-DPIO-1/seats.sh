# V-DPIO-1 dpio seat/priority face at ROOT scope. Sourced by V-DPIO-1.sh after
# its one create (OBJ_dpio_0, a DPIO_NO_CHANNEL dpio at 8 priorities in
# dprc.1) and before the teardown trap. Three faces: the boot kernel dpios'
# seat/priority surface (read), the scratch dpio's num_priorities read back
# (mode-independent, the V-READBACK-1 settled half of DPIO-I3), and the kernel
# face for the scratch dpio — which at ROOT scope is expected UNREACHABLE (the
# boot layout fills every per-CPU seat, so a runtime dpio never binds,
# ADR-0008; V-LIFE-DPIO-1). The unreachable face is NAMED, not silently
# dropped (pool-objects spec loud-not-silent); the DPL-child escape (bead
# dpaa2-controlplane-960.13) is a separately gated task, not taken here.
# From the script: $OBJ_dpio_0, $RESULTS.
. "$(dirname "$0")/../residents.sh"
R="$RESULTS/seats.txt"
DEV=/sys/bus/fsl-mc/devices
DPIO="$OBJ_dpio_0"

log() { echo "$1" | tee -a "$R"; }
info() { restool dpio info "$1" > "$RESULTS/seats-info-$2.txt" 2>&1 || true; }
field() { awk -F': ' -v k="$2" '$1 == k {print $2; exit}' "$1"; }
drv() { d="$(readlink "$DEV/$1/driver" 2>/dev/null)"; d="${d##*/}"; echo "${d:-<none>}"; }

residents_pre

# (a) Boot kernel dpios' seat/priority surface — the per-CPU seats the runtime
# dpio contends with. dpio.0 and dpio.15 bracket the boot set (residents.sh).
for o in dpio.0 dpio.15; do
  f="$RESULTS/seats-info-boot-$o.txt"
  restool dpio info "$o" > "$f" 2>&1 || true
  log "RECORD (a) boot $o channel-mode='$(field "$f" 'dpio channel mode is')' num-priorities='$(field "$f" 'number of priorities is')' driver=$(drv "$o")"
done

# (b) The scratch dpio's num_priorities read back: DPIO-I3's reported half.
# V-READBACK-1 settled it at 0x8 for a NO_CHANNEL dpio at 8 priorities; the
# mode does not fold the priorities away.
info "$DPIO" scratch
mode="$(field "$RESULTS/seats-info-scratch.txt" 'dpio channel mode is')"
prio="$(field "$RESULTS/seats-info-scratch.txt" 'number of priorities is')"
if [ "$mode" = DPIO_NO_CHANNEL ]; then
  log "PASS (b) scratch dpio ${DPIO} reports channel mode DPIO_NO_CHANNEL"
else
  log "FAIL (b) scratch dpio ${DPIO} channel mode is '$mode', not DPIO_NO_CHANNEL"
fi
if [ "$prio" = 0x8 ] || [ "$prio" = 8 ]; then
  log "PASS (b) scratch dpio ${DPIO} reports 8 priorities ('$prio'), mode-independent"
else
  log "FAIL (b) scratch dpio ${DPIO} reports '$prio' priorities, not 8"
fi

# (c) Kernel face: give autorescan a moment, then read the scratch dpio's
# driver link. Expected empty — a runtime dpio takes no seat at root, so
# DPIO-I3's kernel half is unreachable here (named, not dropped).
restool dprc sync > "$RESULTS/seats-sync.txt" 2>&1 || true
sleep 3
d="$(drv "$DPIO")"
log "RECORD (c) scratch dpio ${DPIO} driver link: $d, bus node $([ -e "$DEV/$DPIO" ] && echo present || echo absent)"
case "$d" in
  fsl_mc_dpio)
    log "RECORD (c) UNEXPECTED: scratch dpio took a seat (fsl_mc_dpio) — DPIO-I3 kernel half reachable at root; feed to the 4.4 gate" ;;
  *)
    log "RECORD (c) DPIO-I3 kernel half UNREACHABLE at root: the runtime dpio holds no fsl_mc_dpio seat (boot seats full, ADR-0008) — the DPL-child escape (bead dpaa2-controlplane-960.13) is the route, not taken here" ;;
esac

residents_post
echo; echo "seats face: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
