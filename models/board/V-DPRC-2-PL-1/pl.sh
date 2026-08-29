# V-DPRC-2-PL-1 option-echo face. Sourced by the generated V-DPRC-2-PL-1.sh
# after its last step (the dpbp create) and before its teardown trap: dprc.2
# (built with PL_ALLOWED added to the default mask) and the unplugged dpbp
# in it are standing. From the script: $OBJ_dprc_2, $RESULTS. No prompts, no
# operator.
#
# dprc.md unknowns 3 and 6, PL_ALLOWED half: does DPRC_CFG_OPT_PL_ALLOWED
# read back on the container, and does anything else in the options change?
# `dprc info --verbose` echoes the decoded option names (restool's
# print_dprc_options, one DPRC_CFG_OPT_* per line). The hook only captures
# the option lines — the by-eye diff against the requested mask is done
# offline in the fold — and RECORDs whether PL_ALLOWED is among them.
R="$RESULTS/pl.txt"

# Read the container's decoded options. PASS on the read exiting 0.
restool dprc info "${OBJ_dprc_2}" --verbose > "$RESULTS/pl-info.txt" 2>&1
si=$?
if [ "$si" -eq 0 ]; then r=PASS; else r=FAIL; fi
echo "$r dprc info --verbose on dprc.2 (exit $si)" | tee -a "$R"

# Capture the decoded option lines verbatim for the offline fold.
opts="$(grep -o 'DPRC_CFG_OPT_[A-Z_]*' "$RESULTS/pl-info.txt" | sort -u | tr '\n' ' ')"
echo "RECORD decoded options: ${opts:-<none parsed>}" | tee -a "$R"
if grep -q 'DPRC_CFG_OPT_PL_ALLOWED' "$RESULTS/pl-info.txt"; then p=yes; else p=no; fi
echo "RECORD PL_ALLOWED reads back: $p" | tee -a "$R"

echo; echo "pl face: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
