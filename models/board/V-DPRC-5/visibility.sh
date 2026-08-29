# V-DPRC-5 bus-visibility face. Sourced by the generated V-DPRC-5.sh
# after its last step (the root dpci create) and before its teardown
# trap: the scratch dprc.2, the child dpci in it, and the root dpci are
# standing. DPRC-I6 (a bus rescan reaches root containers only, so a
# child container's residents are never bus-visible) and DPCI-I3 (a dpci
# create performs no rescan, so sysfs lags the MC until an explicit one).
# From the script: $OBJ_dprc_2, $OBJ_dpci_0 (child, in dprc.2),
# $OBJ_dpci_1 (root, in dprc.1), $RESULTS. No prompts, no operator: pure
# observation plus the single rescan the face turns on. Nothing here is
# driver-bound (dpci has no kernel driver) and no destroy precedes the
# rescan, so the sync is safe (ADR-0008).
R="$RESULTS/visibility.txt"
DEV=/sys/bus/fsl-mc/devices

# present NAME: exit 0 iff the object has a sysfs device node.
present() { [ -e "$DEV/$1" ]; }
# listed FILE NAME: exit 0 iff a `dprc show` capture lists the object as
# one of its rows (the name is the first field of its row).
listed() { awk -v o="$2" '$1 == o { f = 1 } END { exit !f }' "$1"; }

# The autorescan policy a child's own refresh path keys off (dprc.md
# unknown 12): recorded, not asserted — it frames whether a child ever
# refreshes on its own.
echo "RECORD /sys/bus/fsl-mc/autorescan: $(cat /sys/bus/fsl-mc/autorescan 2>/dev/null || echo '<unreadable>')" | tee -a "$R"

# (a) Before any rescan. The root dpci the last trace step just created is
# MC-present but the create triggered no rescan (DPCI-I3), so it is not
# yet a sysfs node — recorded as the oracle (expected absent). The child
# dpci never becomes one at all (DPRC-I6): a rescan reaches root
# containers only, and dprc.2 is not the root.
ls "$DEV" > "$RESULTS/visibility-devs-pre.txt" 2>&1 || true
echo "RECORD root dpci ${OBJ_dpci_1} sysfs node before rescan: $(present "${OBJ_dpci_1}" && echo present || echo absent)" | tee -a "$R"
if ! present "${OBJ_dpci_0}"; then r=PASS; else r=FAIL; fi
echo "$r child dpci ${OBJ_dpci_0} absent from the bus (DPRC-I6, child resident never bus-visible)" | tee -a "$R"

# (b) The rescan. `dprc sync` re-walks the root container; it binds
# nothing (dpci is driver-less) and no destroy precedes it, so the walk is
# safe.
restool dprc sync > "$RESULTS/visibility-sync.txt" 2>&1 || true

# (c) After the rescan the root dpci is a sysfs node and the child still
# is not: the rescan reached the root container only.
ls "$DEV" > "$RESULTS/visibility-devs-post.txt" 2>&1 || true
if present "${OBJ_dpci_1}" && ! present "${OBJ_dpci_0}"; then r=PASS; else r=FAIL; fi
echo "$r after rescan: root dpci ${OBJ_dpci_1} present, child dpci ${OBJ_dpci_0} still absent" | tee -a "$R"

# (d) The MC held the child dpci all along — `dprc show dprc.2` lists it,
# so the bus-absence above is a visibility fact, not an MC one.
restool dprc show "${OBJ_dprc_2}" > "$RESULTS/visibility-show2.txt" 2>&1 || true
if listed "$RESULTS/visibility-show2.txt" "${OBJ_dpci_0}"; then r=PASS; else r=FAIL; fi
echo "$r MC lists child dpci ${OBJ_dpci_0} in dprc.2 (bus-absent, MC-present)" | tee -a "$R"

echo; echo "visibility face: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
