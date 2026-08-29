# V-POOL-3 uapi-opener face. Sourced by V-POOL-3.sh after its one create
# (the scratch child is only there to give the suite a teardown and a
# results directory). DPMCP-I2: N simultaneous openers of /dev/dprc.1
# were predicted to need N-1 free dpmcps, failing ENXIO at exhaustion.
# Rev 1 (2026-08-29) found the ceiling is one: the second opener fails
# EINVAL while the first is held, from a device-link cycle in the uapi's
# portal allocation (ADR-0006 amendment). Rev 2 expects 1 held, K-1
# refused with that errno; a higher count re-opens DPMCP-I2.
#
# The openers are timer-bounded background processes: each holds the
# portal for $HOLD seconds and then exits no matter what the hook does, so
# the root pool is never held longer than that. No restool runs while the
# portal is held (restool is itself an opener) — the recovery read only
# runs after every opener has been released.
# From the script: $RESULTS.
. "$(dirname "$0")/../residents.sh"
R="$RESULTS/pool.txt"
HOLD=5        # seconds each opener holds the portal, then self-releases
K=120         # openers spawned at once, well above the ~53 expected ceiling

log() { echo "$1" | tee -a "$R"; }

residents_pre

# Spawn K openers simultaneously. Each writes a "try" marker, then
# `exec 9<>/dev/dprc.1`: a *failed* exec (a redirection with no command)
# ends the subshell on the spot, so the "ok" marker after it never lands —
# that is how the subshell's exit status is read back. Counting refusals
# from a `pool3-fail-*` file the else-branch writes was the rev-1 bug: the
# else never ran, the exec having already killed the subshell. The errno
# text lands in the subshell's stderr file either way.
rm -f "$RESULTS"/pool3-try-* "$RESULTS"/pool3-ok-* "$RESULTS"/pool3-err-*.txt 2>/dev/null
i=0
while [ "$i" -lt "$K" ]; do
  i=$((i + 1))
  (
    : > "$RESULTS/pool3-try-$i"
    exec 9<>/dev/dprc.1     # a failed exec ends this subshell here
    : > "$RESULTS/pool3-ok-$i"
    sleep "$HOLD"
  ) 2>"$RESULTS/pool3-err-$i.txt" &
done

# Let every opener attempt its open (well under the HOLD timer). No
# restool here — restool would itself need a portal.
sleep 2
tried=$(ls "$RESULTS"/pool3-try-* 2>/dev/null | wc -l | tr -d ' ')
opened=$(ls "$RESULTS"/pool3-ok-* 2>/dev/null | wc -l | tr -d ' ')
failed=$((tried - opened))
errno=$(cat "$RESULTS"/pool3-err-*.txt 2>/dev/null | grep -m1 . || echo '<none captured>')
log "RECORD openers: $opened held the portal, $failed refused (K=$K)"
log "RECORD first refusal errno text: $errno"
if [ "$opened" -gt 0 ] && [ "$failed" -gt 0 ]; then
  log "PASS portal ceiling observed: $opened opened then refused (DPMCP-I2)"
else
  log "RECORD no ceiling hit at K=$K (raise K and re-run); opened=$opened failed=$failed"
fi

# Release: wait for the timer-bounded openers to exit, then prove the
# pool recovered.
wait 2>/dev/null
if restool dprc show dprc.1 > "$RESULTS/pool3-recover.txt" 2>&1; then
  log "PASS pool recovered: restool dprc show dprc.1 answers after release"
else
  log "FAIL pool did not recover: restool dprc show dprc.1 failed after release"
fi

residents_post
echo; echo "pool face: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
