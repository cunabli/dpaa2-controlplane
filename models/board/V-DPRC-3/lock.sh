# V-DPRC-3 set-locked face. Sourced by the generated V-DPRC-3.sh after
# its last step (the dpbp create) and before its teardown trap: the
# scratch dprc.2 and the unplugged dpbp in it are standing. DPRC-I11 and
# dprc.md unknown 4 — set-locked strips create/destroy/assign/unassign/
# lock from the whole sub-hierarchy, and only an ancestor above the
# container may toggle the lock. From the script: $OBJ_dprc_2,
# $OBJ_dpbp_0, $RESULTS. No prompts, no operator. restool's set-locked
# opens the child's PARENT-container portal (the root for a top-level
# scratch child), so the lock and unlock below both run through the root.
LABEL=vdprc3lock   # the label the refused set-label tries to write
R="$RESULTS/lock.txt"

# The MC/restool status line on a refusal, or empty.
mc_status() { grep -o 'MC error:.*' "$1" | head -1; }
# plug_state NAME: the plugged-state column (the last field) of the
# object's row in a `dprc show` capture — "plugged" or "unplugged".
plug_state() { awk -v o="$2" '$1 == o { print $NF }' "$1"; }

# (a) Lock dprc.2 from the root. Expected to succeed (issued from above
# the container).
restool dprc set-locked "${OBJ_dprc_2}" --locked=1 > "$RESULTS/lock-set1.txt" 2>&1
if [ $? -eq 0 ]; then r=PASS; else r=FAIL; fi
echo "$r set-locked dprc.2 --locked=1 accepted from the root" | tee -a "$R"

# (b) Plug the dpbp under lock. The strip removes assign, so this is
# refused; the status is recorded (the register predicts No privilege,
# not asserted here), and the pass is read back from the dpbp staying
# unplugged.
restool dprc assign "${OBJ_dprc_2}" --object="${OBJ_dpbp_0}" --plugged=1 > "$RESULTS/lock-plug.txt" 2>&1 || true
echo "RECORD locked-plug status: $(mc_status "$RESULTS/lock-plug.txt" || echo '<none on stderr>')" | tee -a "$R"
restool dprc show "${OBJ_dprc_2}" > "$RESULTS/lock-show-b.txt" 2>&1 || true
if [ "$(plug_state "$RESULTS/lock-show-b.txt" "${OBJ_dpbp_0}")" = "unplugged" ]; then r=PASS; else r=FAIL; fi
echo "$r dpbp still unplugged after the locked plug (assign stripped by the lock)" | tee -a "$R"

# (c) set-label on the dpbp under lock. The lock strips assign, not
# labels, so set-label is accepted under the lock (rev 1 sitting); status
# recorded, and the written label reads back present.
restool dprc set-label "${OBJ_dpbp_0}" --label="$LABEL" > "$RESULTS/lock-label.txt" 2>&1 || true
echo "RECORD locked-set-label status: $(mc_status "$RESULTS/lock-label.txt" || echo '<none on stderr>')" | tee -a "$R"
restool dprc show "${OBJ_dprc_2}" > "$RESULTS/lock-show-c.txt" 2>&1 || true
if grep -q "$LABEL" "$RESULTS/lock-show-c.txt"; then r=PASS; else r=FAIL; fi
echo "$r set-label is accepted under the lock (labels are not stripped; label \"$LABEL\" present)" | tee -a "$R"

# (d) Reads survive the lock: dprc show and dpbp info both exit 0.
restool dprc show "${OBJ_dprc_2}" > "$RESULTS/lock-show-d.txt" 2>&1
sd=$?
restool dpbp info "${OBJ_dpbp_0}" > "$RESULTS/lock-dpbp-info.txt" 2>&1
si=$?
if [ "$sd" -eq 0 ] && [ "$si" -eq 0 ]; then r=PASS; else r=FAIL; fi
echo "$r reads survive under lock (dprc show exit $sd, dpbp info exit $si)" | tee -a "$R"

# (e) Unlock from the root, then re-issue the plug — now it succeeds —
# and unplug again so the trace's final unplugged state stands.
restool dprc set-locked "${OBJ_dprc_2}" --locked=0 > "$RESULTS/lock-set0.txt" 2>&1
if [ $? -eq 0 ]; then r=PASS; else r=FAIL; fi
echo "$r set-locked dprc.2 --locked=0 accepted from the root" | tee -a "$R"
restool dprc assign "${OBJ_dprc_2}" --object="${OBJ_dpbp_0}" --plugged=1 > "$RESULTS/lock-replug.txt" 2>&1 || true
restool dprc show "${OBJ_dprc_2}" > "$RESULTS/lock-show-e.txt" 2>&1 || true
if [ "$(plug_state "$RESULTS/lock-show-e.txt" "${OBJ_dpbp_0}")" = "plugged" ]; then r=PASS; else r=FAIL; fi
echo "$r dpbp plugs once unlocked (assign restored)" | tee -a "$R"
restool dprc assign "${OBJ_dprc_2}" --object="${OBJ_dpbp_0}" --plugged=0 > "$RESULTS/lock-reunplug.txt" 2>&1 || true

# (f) Unconditionally leave the lock off (idempotent) so the teardown's
# destroy is never refused by a lock left set — impossible to miss if it
# fails.
if restool dprc set-locked "${OBJ_dprc_2}" --locked=0 > "$RESULTS/lock-final0.txt" 2>&1; then
  :
else
  echo "FAIL lock left set — teardown will refuse" | tee -a "$R"
fi

echo; echo "lock face: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
