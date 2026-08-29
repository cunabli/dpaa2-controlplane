# V-DPRC-6 container-move refusal face. Sourced by the generated
# V-DPRC-6.sh after its last step (the dpbp plugged in dprc.2) and before
# its teardown trap: the two scratch containers and the plugged dpbp are
# standing here. Nothing is kernel-bound — a child container's residents
# never are (DPRC-I6) — so the refusals below are the MC's own gate, not a
# driver's. From the script: $OBJ_dprc_2, $OBJ_dprc_3, $OBJ_dpbp_0,
# $RESULTS. No prompts, no operator: a refused move is a disabled action
# the trace cannot express, so all three moves are hand-issued here.
R="$RESULTS/moves.txt"

# The MC status line restool prints to stderr on a refusal, or empty.
mc_status() { grep -o 'MC error:.*' "$1" | head -1; }
# Whether a `dprc show` capture lists an object as one of its rows (the
# object name is the first field of its row, the same read-back the trace
# path uses).
listed() { awk -v o="$2" '$1 == o { f = 1 } END { exit !f }' "$1"; }

# (a) DPRC-I3: a plugged object cannot be moved. Pull the *plugged* dpbp
# one hop up to the root — the child->parent rendering V-DPRC-1 used
# (unassign at the parent, --child the source). The status is not in the
# register yet, so it is recorded; the pass is read back from where the
# dpbp actually sits.
restool dprc unassign dprc.1 --object="${OBJ_dpbp_0}" --child="${OBJ_dprc_2}" > "$RESULTS/moves-up.txt" 2>&1 || true
sa="$(mc_status "$RESULTS/moves-up.txt")"
echo "RECORD plugged-move-up status: ${sa:-<none on stderr>}" | tee -a "$R"
restool dprc show "${OBJ_dprc_2}" > "$RESULTS/moves-a-show2.txt" 2>&1 || true
restool dprc show dprc.1          > "$RESULTS/moves-a-show1.txt" 2>&1 || true
if listed "$RESULTS/moves-a-show2.txt" "${OBJ_dpbp_0}" && ! listed "$RESULTS/moves-a-show1.txt" "${OBJ_dpbp_0}"; then r=PASS; else r=FAIL; fi
echo "$r plugged dpbp stayed in dprc.2, absent from dprc.1 (move up refused)" | tee -a "$R"
# Self-clean: if the move went through, the dpbp is now in the root — put
# it back so the teardown finds the trace's final state.
if listed "$RESULTS/moves-a-show1.txt" "${OBJ_dpbp_0}"; then
  restool dprc assign dprc.1 --object="${OBJ_dpbp_0}" --child="${OBJ_dprc_2}" > "$RESULTS/moves-a-undo.txt" 2>&1 || true
  echo "RECORD move-up unexpected success undone (reassigned to dprc.2)" | tee -a "$R"
fi

# (b) unplug the dpbp, so the sibling move below is refused for the
# container reason and not the plugged one.
restool dprc assign "${OBJ_dprc_2}" --object="${OBJ_dpbp_0}" --plugged=0 > "$RESULTS/moves-unplug.txt" 2>&1 || true

# (c) The sibling move dprc.2 -> dprc.3 in one command — the exact
# rendering V-DPRC-1 rev 1 used (assign at the source container, --child
# the destination), which exited 255. The register predicts No privilege
# from that exit code; this fills the status text in.
restool dprc assign "${OBJ_dprc_2}" --object="${OBJ_dpbp_0}" --child="${OBJ_dprc_3}" > "$RESULTS/moves-sibling.txt" 2>&1 || true
status="$(mc_status "$RESULTS/moves-sibling.txt")"
echo "RECORD sibling-move status: ${status:-<none on stderr>}" | tee -a "$R"
case "$status" in *"No privilege"*) r=PASS ;; *) r=FAIL ;; esac
echo "$r sibling move refused with No privilege: ${status:-<none>}" | tee -a "$R"
restool dprc show "${OBJ_dprc_2}" > "$RESULTS/moves-c-show2.txt" 2>&1 || true
if listed "$RESULTS/moves-c-show2.txt" "${OBJ_dpbp_0}"; then r=PASS; else r=FAIL; fi
echo "$r dpbp still in dprc.2 after refused sibling move" | tee -a "$R"
# Self-clean: if the sibling move went through, the dpbp is in dprc.3 — put
# it back so the teardown (which reclaims it from dprc.2) copes.
if ! listed "$RESULTS/moves-c-show2.txt" "${OBJ_dpbp_0}"; then
  restool dprc assign "${OBJ_dprc_3}" --object="${OBJ_dpbp_0}" --child="${OBJ_dprc_2}" > "$RESULTS/moves-c-undo.txt" 2>&1 || true
  echo "RECORD sibling-move unexpected success undone (reassigned to dprc.2)" | tee -a "$R"
fi

echo; echo "moves face: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
