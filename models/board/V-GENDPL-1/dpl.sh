# V-GENDPL-1 generate-dpl round-trip face. Sourced by the generated
# V-GENDPL-1.sh after its last step (the dpci create) and before its
# teardown trap: the scratch dprc.2 and its dpdcei (engine DECOMPRESSION,
# priority 2), dpdmai (priorities 2,4) and dpci (2 priorities) are
# standing. DPDCEI-I2 / DPDMAI-I4: generate-dpl is not a round-trip —
# write-only create fields are dropped and dpdmai priorities are mangled,
# so the emitted node does not reconstruct the object. From the script:
# $OBJ_dprc_2, $OBJ_dpdcei_0, $OBJ_dpdmai_0, $OBJ_dpci_0, $RESULTS. No
# prompts, no operator: generate-dpl is a read-only query walk of live MC
# state (a legal hook verb), and the hook mutates nothing. It captures the
# DPL and the read-backs; the emitted-vs-requested diff is done offline in
# the fold.
R="$RESULTS/dpl.txt"
DTS="$RESULTS/dprc2.dpl.dts"

# node NAME: RECORD the emitted DPL node header(s) for the object, grepped
# from the dts (e.g. `dpdcei@0`). Empty capture means the family was not
# emitted at all — itself a finding for the fold.
node() { echo "RECORD emitted $1 node: $(grep -n "$2" "$DTS" 2>/dev/null | head -3 | tr '\n' ';' || true)" | tee -a "$R"; }

# Generate the DPL of dprc.2 and read each object back.
restool dprc generate-dpl "${OBJ_dprc_2}" > "$DTS" 2>"$RESULTS/dpl-gen-err.txt"; g=$?
restool dpdcei info "${OBJ_dpdcei_0}" > "$RESULTS/dpl-dpdcei-info.txt" 2>&1; ce=$?
restool dpdmai info "${OBJ_dpdmai_0}" > "$RESULTS/dpl-dpdmai-info.txt" 2>&1; me=$?
restool dpci   info "${OBJ_dpci_0}"   > "$RESULTS/dpl-dpci-info.txt"   2>&1; pe=$?

if [ "$g" -eq 0 ] && [ "$ce" -eq 0 ] && [ "$me" -eq 0 ] && [ "$pe" -eq 0 ]; then r=PASS; else r=FAIL; fi
echo "$r generate-dpl and all three info read-backs exited 0 (gen $g, dpdcei $ce, dpdmai $me, dpci $pe)" | tee -a "$R"

# The emitted nodes — the fold diffs these against the requested
# attributes (dpdcei priority 2, dpdmai priorities 2,4, dpci 2
# priorities). Write-only fields dropped and dpdmai priorities mangled are
# what the by-eye diff looks for.
node dpdcei 'dpdcei@'
node dpdmai 'dpdmai@'
node dpci   'dpci@'

echo; echo "dpl face: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
