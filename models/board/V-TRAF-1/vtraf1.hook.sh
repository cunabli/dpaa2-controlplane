# V-TRAF-1 face 6 teardown-law refusals (cross-dprc-links task 6.1, design D10).
# The two refusals cannot ride a forward MBT trace — a disabled (.fail) action
# cannot step, so a refusal is only ever a run's terminal step (vtraf1.qnt
# header). They run here as directed probes, sourced after the last trace step
# and under the suite's teardown trap. Never silent: each illegal command's MC
# status is recorded for the operator to diff. From the script: $RESULTS.
#
# No dpmac participates (finding 49): both ends are scratch dpnis connected at
# the root ancestor, the one container holding the topology-change privilege.
R="$RESULTS/vtraf1-refusals.txt"
E="$RESULTS/vtraf1-refusals.err"
: > "$R"
: > "$E"
log() { echo "$1" | tee -a "$R"; }

# A connected dpni↔dpni pair at the root to probe the laws on.
A="$(restool --script dpni create --container=dprc.1 2>>"$E")"
B="$(restool --script dpni create --container=dprc.1 2>>"$E")"
restool dprc connect dprc.1 --endpoint1="$A" --endpoint2="$B" 2>>"$E"

# LINK-I1 disconnect-before-destroy: the MC refuses destroy of a still-connected
# end (finding 34 generalized). Record the refusal; a success is the failure.
if restool dpni destroy "$A" >>"$R" 2>&1; then
  log "FAIL LINK-I1: destroy of still-connected end $A was NOT refused"
else
  log "RECORD LINK-I1: destroy of connected end $A refused (disconnect-before-destroy)"
fi

# DPRC-I5 double-connect: an endpoint holds at most one peer, so connecting an
# already-connected end to a new peer is refused (cardinality-one). Record it.
C="$(restool --script dpni create --container=dprc.1 2>>"$E")"
if restool dprc connect dprc.1 --endpoint1="$A" --endpoint2="$C" >>"$R" 2>&1; then
  log "FAIL DPRC-I5: double-connect of $A was NOT refused"
else
  log "RECORD DPRC-I5: double-connect of already-connected $A refused (disconnect-before-reconnect)"
fi

# Clean up what this hook created; the generated trap tears down only the trace's
# objects, so the hook destroys its own, in order, best-effort.
restool dprc disconnect dprc.1 --endpoint="$A" 2>>"$E" || true
for id in "$C" "$B" "$A"; do
  [ -n "$id" ] && restool dpni destroy "$id" 2>>"$E" || true
done

echo "vtraf1 refusal probes: $(grep -c '^RECORD ' "$R") recorded, $(grep -c '^FAIL ' "$R") failed"
