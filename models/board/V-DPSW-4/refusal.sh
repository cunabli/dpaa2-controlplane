# V-DPSW-4 refusal face. Sourced by the generated V-DPSW-4.sh after its
# last step (the bus rescan) and before its teardown trap: the dpsw, its
# dpmcp/dpbp companions and its uplink edge are standing here and are torn
# down whatever happens below. The switch was built the way restool's
# silent defaults build it (flooding PER_VLAN, broadcast PER_OBJECT) — the
# shape the reference kernel's dpaa2-switch driver refuses at probe — so
# nothing binds it. This face reads that negative outcome back. From the
# script: $OBJ_dpsw_0 = the switch's board name, $RESULTS. No prompts, no
# operator: pure read-back.
R="$RESULTS/refusal.txt"

# ADR-0008: give the bus rescan and the driver's probe attempt time to
# settle before reading the outcome — longer than the positive suite's 1s,
# because here the pass is a bind that must never appear.
sleep 3

# DPSW-I1 refusal face: the driver leaves the object unbound. An empty
# driver link is the pass; any target means the driver took it after all.
link="$(readlink "/sys/bus/fsl-mc/devices/${OBJ_dpsw_0}/driver" 2>/dev/null || true)"
echo "readlink /sys/bus/fsl-mc/devices/${OBJ_dpsw_0}/driver -> ${link:-<empty>}" > "$RESULTS/refusal-driverlink.txt"
if [ -z "$link" ]; then r=PASS; else r=FAIL; fi
echo "$r dpsw driver link empty (unbound): ${link:-<empty>}" | tee -a "$R"

# DPSW-I2: the driver logs why it refused. dpaa2_switch_supports_cpu_traffic
# bails on the first unacceptable domain, and a default-built switch has
# its flooding scoped per VLAN, so the logged line is
# "Flooding domain is not per FDB, cannot probe".
dmesg 2>/dev/null | grep "$OBJ_dpsw_0" > "$RESULTS/refusal-dmesg.txt" 2>&1 || true
if grep -q "Flooding domain is not per FDB" "$RESULTS/refusal-dmesg.txt"; then r=PASS; else r=FAIL; fi
echo "$r dpsw probe refusal logged (Flooding domain is not per FDB): $(wc -l < "$RESULTS/refusal-dmesg.txt") matching kernel-log lines" | tee -a "$R"

# The flooding/broadcast configuration restool reports for the switch —
# the two fields the driver gates on. Whole output kept; values recorded.
restool dpsw info "$OBJ_dpsw_0" > "$RESULTS/refusal-dpsw-info.txt" 2>&1 || true
fcfg="$(awk -F': ' '$1 == "flooding cfg" {print $2; exit}' "$RESULTS/refusal-dpsw-info.txt")"
bcfg="$(awk -F': ' '$1 == "broadcast cfg" {print $2; exit}' "$RESULTS/refusal-dpsw-info.txt")"
echo "RECORD dpsw flooding cfg: ${fcfg:-<none>}" | tee -a "$R"
echo "RECORD dpsw broadcast cfg: ${bcfg:-<none>}" | tee -a "$R"

echo; echo "refusal face: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
