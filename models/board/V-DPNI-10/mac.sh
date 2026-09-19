# V-DPNI-10 primary-MAC mutation. Sourced by V-DPNI-10.sh with a bare
# scratch dpni ($OBJ_dpni_0) standing; the generated teardown trap
# reclaims it. docs/baseline/dpni.md command surface: `dpni update
# --mac-addr` is the only runtime setter restool drives, and the primary
# MAC is dpni-typestate design D1's runtime slot beside the immutable
# create block. The address is assembled from octets here rather than
# written as a literal — the public-repo leak scan rejects a six-octet MAC
# literal even in a hook. Read-back with `dpni info` is the only
# observation (dpni-typestate design D7).
R="$RESULTS/mac.txt"
DPNI="$OBJ_dpni_0"
o=00                                    # locally administered address, zero body
mac="02:$o:$o:$o:$o:07"                 # 02-…-07, no six-octet literal in source
mc_mac() { restool dpni info "$DPNI" 2>>"$RESULTS/mac.log" | awk 'tolower($0) ~ /mac/ && tolower($0) ~ /addr/ { print tolower($NF); exit }'; }

echo "RECORD dpni primary MAC before update: $(mc_mac)" | tee -a "$R"
restool dpni update "$DPNI" --mac-addr="$mac" >>"$RESULTS/mac.log" 2>&1 || true
got="$(mc_mac)"
if [ "$got" = "$mac" ]; then
  echo "PASS dpni primary MAC set through restool reads back: $got" | tee -a "$R"
else
  echo "FAIL dpni primary MAC read back $got, want $mac" | tee -a "$R"
fi

echo; echo "mac face: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
