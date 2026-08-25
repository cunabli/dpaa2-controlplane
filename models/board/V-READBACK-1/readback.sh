# V-READBACK-1 read-back face. Sourced by the generated V-READBACK-1.sh
# after its last step (dpdcei create) and before its teardown trap: the
# five bare objects and their scratch group are standing here and are torn
# down whatever happens below. From the script: $OBJ_<fam>_<num> = each
# object's board name, $RESULTS. No prompts, no operator: pure read-back.
info() { restool "$1" info "$2" > "$RESULTS/readback-$1.txt" 2>&1; }   # whole output kept: every line is evidence
field() { awk -F': ' -v k="$2" '$1 == k {print $2; exit}' "$RESULTS/readback-$1.txt"; }
# expect FAM "label" want: PASS/FAIL line in readback.txt, from a predicted default
expect() { got="$(field "$1" "$2")"; if [ "$got" = "$3" ]; then r=PASS; else r=FAIL; fi; echo "$r $1 $2: $got (want $3)" | tee -a "$RESULTS/readback.txt"; }
# record FAM "label": RECORD line — the value is the unknown; the ledger settles it
record() { echo "RECORD $1 $2: $(field "$1" "$2")" | tee -a "$RESULTS/readback.txt"; }

# DPNI-I7: MC defaults of a bare dpni create.
info dpni "$OBJ_dpni_0"
expect dpni num_queues 1
expect dpni num_rx_tcs 1
expect dpni num_tx_tcs 1
expect dpni num_cgs 1
expect dpni mac_entries 16   # 80 is restool's maximum; MC 10.39.0 defaults to 16 (sitting 2026-08-25)
expect dpni vlan_entries 0
expect dpni qos_entries 0    # no QoS table on a single-TC dpni; 64 is restool's maximum (sitting 2026-08-25)
expect dpni fs_entries 64
record dpni "dpni_attr.options value is"
record dpni "dpni version"

# DPDMAI-I5: the queue count MC picks for a bare dpdmai.
info dpdmai "$OBJ_dpdmai_0"
record dpdmai "number of queues"
record dpdmai "number of priorities"
record dpdmai "dpdmai version"

# DPIO-I3: what MC reports for a DPIO_NO_CHANNEL dpio.
info dpio "$OBJ_dpio_0"
expect dpio "dpio channel mode is" DPIO_NO_CHANNEL
record dpio "number of priorities is"   # restool prints it in hex, e.g. 0x8 — record as-is
record dpio "dpio version"

# DPBP-I5: does bpid equal object id.
info dpbp "$OBJ_dpbp_0"
record dpbp "dpbp id"
record dpbp "buffer pool id"
echo "RECORD dpbp bpid equals id: $([ "$(field dpbp 'dpbp id')" = "$(field dpbp 'buffer pool id')" ] && echo yes || echo no)" | tee -a "$RESULTS/readback.txt"
record dpbp "dpbp version"

# DPDCEI-I1: the dpdcei info API version (engine is the flag we set).
info dpdcei "$OBJ_dpdcei_0"
expect dpdcei "DPDCEI engine" DPDCEI_ENGINE_DECOMPRESSION
record dpdcei "dpdcei version"

echo; echo "read-back face: $(grep -c ^PASS "$RESULTS/readback.txt") PASS, $(grep -c ^FAIL "$RESULTS/readback.txt") FAIL, $(grep -c ^RECORD "$RESULTS/readback.txt") RECORD"
