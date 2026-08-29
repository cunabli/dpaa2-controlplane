# V-DPDMUX-2 uplink face, rev 5. Sourced by the generated V-DPDMUX-2.sh
# after its last step (the dpni create) and before its teardown trap: the
# dpdmux and the dpni are standing, unplugged, in the Linux root, and are
# torn down whatever happens below. From the script: $OBJ_dpdmux_0,
# $OBJ_dpni_0, $RESULTS. No prompts, no operator: the connects are
# hand-issued here because a refused connect is a disabled action the trace
# cannot express.
#
# Rev 5 exists because every earlier rev started from a board that had
# already been connected once, and on MC 10.39 a connection cannot be
# undone from either end (Configuration error 0x6) — so we could never tell
# whether the state we read was a genuine fresh answer or a ghost left by an
# earlier connect. Rev 2 connected a dpni onto the bare uplink and could not
# free it. Rev 3 (the first dpdmux of a fresh boot) asked for a downlink
# connect and was refused, yet interface 0 read the dpni anyway. Rev 4 (same
# boot, objects destroyed and re-created with the same ids) read the dpni on
# interface 0 BEFORE any connect in the run, and every connect was refused —
# as if the MC kept the connection record across destroy and re-create of
# same-id objects. Two hypotheses follow: (1) the record survives destroy +
# re-create of the same ids; (2) it even survives a reboot.
#
# So rev 5 MUST run right after a reboot, before anything has connected this
# dpdmux, and it reads both sides before it touches a thing. It uses no dpmac
# at all. The phases answer, in order:
#   phase 0  fresh boot: do both sides read unpaired before any connect?
#            (a "no" here means the record survived the reboot — hypothesis 2)
#   phase 1  rev 3's downlink-first connect, with two-sided read-backs: does
#            the firmware's yes/no answer match the state it actually leaves?
#   phase 2  free the dpni again, trying each end in turn, so the next phase
#            starts clean.
#   phase 3  rev 2's bare-uplink face, only on a clean slate: does the
#            uplink dpni-connect's answer match its state?
#
# Rev 5 as run also carried a fourth phase that destroyed both objects and
# re-created them with the same ids to answer hypothesis 1 in-run. It is
# removed: a hook never destroys or re-creates in the root — only the
# script's spaced teardown does, once per run (ADR-0003 §6, ADR-0008 §6–7).
# On 2026-08-29 that phase destroyed a connected pair in the root twice in
# one run (here and again in the EXIT-trap teardown); the fsl-mc bus rescan
# race (ADR-0008 §4) then released the boot dpni's driver and took the
# management interface down.
R="$RESULTS/uplink.txt"

# The connection restool reports for one interface of the demux's
# per-interface `info` block: `interface N:` at column 0, an indented
# `connection: <peer|none>` under it (dpdmux_commands.c print_dpdmux_endpoint).
iface_conn() { # file interface_num -> the connection token, or empty
  awk -v want="interface $2:" '
    { line = $0; sub(/^[ \t]+/, "", line) }
    line == want { inb = 1; next }
    inb && line ~ /^interface / { inb = 0 }
    inb { c = line; if (sub(/^connection:[ \t]*/, "", c)) { print c; exit } }
  ' "$1"
}
# The MC status line restool prints to stderr on a refusal, or empty.
mc_status() { grep -o 'MC error:.*' "$1" | head -1; }
# The dpni's own view of its peer: `endpoint: No object associated` when
# free, else `endpoint: <peer>[, link is ...]` (dpni_commands.c
# print_dpni_endpoint; a dpdmux peer prints as `dpdmux.N.if_id`). Returns
# the peer token, or `none` when free.
dpni_peer() { # file -> peer object token, or none
  e="$(grep '^endpoint:' "$1" | head -1)"
  case "$e" in
    *"No object associated"*|"") echo none ;;
    *) p="${e#endpoint: }"; echo "${p%%,*}" ;;
  esac
}

# reread FILETAG PHRASE: a fresh two-sided read into
# <FILETAG>-{dpdmux,dpni}-info.txt; sets i0, i1 (the demux interfaces) and
# dep (the dpni's endpoint), and RECORDs the three on one line under PHRASE.
reread() {
  restool dpdmux info "${OBJ_dpdmux_0}" > "$RESULTS/$1-dpdmux-info.txt" 2>&1 || true
  i0="$(iface_conn "$RESULTS/$1-dpdmux-info.txt" 0)"
  i1="$(iface_conn "$RESULTS/$1-dpdmux-info.txt" 1)"
  restool dpni info "${OBJ_dpni_0}" > "$RESULTS/$1-dpni-info.txt" 2>&1 || true
  dep="$(dpni_peer "$RESULTS/$1-dpni-info.txt")"
  echo "RECORD $2: interface 0 ${i0:-<none read>}, interface 1 ${i1:-<none read>}, dpni endpoint $dep" | tee -a "$R"
}
# still_paired: true if either side (either demux interface, or the dpni)
# currently reads a real peer. Judges on the globals reread last set.
still_paired() {
  { [ "$i0" != none ] && [ -n "$i0" ]; } || { [ "$i1" != none ] && [ -n "$i1" ]; } || [ "$dep" != none ]
}

# Phase 0 — before any connect (fresh boot). Read both sides with nothing
# done. A pairing here is the record surviving the reboot (hypothesis 2).
upt="$(cat /proc/uptime)"
echo "RECORD seconds since boot: ${upt%% *}" | tee -a "$R"
reread pre "before any connect"
if [ "$i0" = none ] && [ "$dep" = none ]; then r=PASS; else r=FAIL; fi
echo "$r no pairing exists before any connect (fresh boot)" | tee -a "$R"

# Phase 1 — downlink first (rev 3's sequence). Connect the dpni onto the
# downlink (interface 1) and read both sides back.
restool dprc connect dprc.1 --endpoint1="${OBJ_dpdmux_0}.1" --endpoint2="${OBJ_dpni_0}" > "$RESULTS/downlink-connect.txt" 2>&1 || true
s="$(mc_status "$RESULTS/downlink-connect.txt")"
echo "RECORD downlink connect status: ${s:-<accepted, none on stderr>}" | tee -a "$R"
reread downlink "after downlink connect"
# Answer alone: accepted and the dpni landed where asked (interface 1).
case "$i1" in dpni.*) i1_dpni=1 ;; *) i1_dpni=0 ;; esac
if [ -z "$s" ] && [ "$i1_dpni" = 1 ]; then r=PASS; else r=FAIL; fi
echo "$r downlink connect accepted (interface 1 is the dpni)" | tee -a "$R"
# Answer vs state: a yes must leave the pairing, a no must leave none.
case "$dep" in "${OBJ_dpdmux_0}".*) dep_mux=1 ;; *) dep_mux=0 ;; esac
if { [ -z "$s" ] && [ "$i1_dpni" = 1 ] && [ "$dep_mux" = 1 ]; } || \
   { [ -n "$s" ] && [ "$i0" = none ] && [ "$i1" = none ] && [ "$dep" = none ]; }; then r=PASS; else r=FAIL; fi
echo "$r the firmware's answer and its state agree" | tee -a "$R"

# Phase 2 — free the dpni, trying each end until both sides read none, so
# phase 3 starts on a clean slate.
if still_paired; then
  restool dprc disconnect dprc.1 --endpoint="${OBJ_dpni_0}" > "$RESULTS/free-disconnect-dpni.txt" 2>&1 || true
  d="$(mc_status "$RESULTS/free-disconnect-dpni.txt")"
  echo "RECORD free: disconnect from the dpni end: ${d:-<accepted, none on stderr>}" | tee -a "$R"
  reread free-dpni "after dpni-end disconnect"
  if still_paired; then
    restool dprc disconnect dprc.1 --endpoint="${OBJ_dpdmux_0}.1" > "$RESULTS/free-disconnect-mux1.txt" 2>&1 || true
    d="$(mc_status "$RESULTS/free-disconnect-mux1.txt")"
    echo "RECORD free: disconnect from the demux downlink end: ${d:-<accepted, none on stderr>}" | tee -a "$R"
    reread free-mux1 "after demux-downlink-end disconnect"
    if still_paired; then
      restool dprc disconnect dprc.1 --endpoint="${OBJ_dpdmux_0}" > "$RESULTS/free-disconnect-mux0.txt" 2>&1 || true
      d="$(mc_status "$RESULTS/free-disconnect-mux0.txt")"
      echo "RECORD free: disconnect from the demux uplink end: ${d:-<accepted, none on stderr>}" | tee -a "$R"
      reread free-mux0 "after demux-uplink-end disconnect"
    fi
  fi
  if still_paired; then r=FAIL; else r=PASS; fi
  echo "$r dpni freed by disconnect" | tee -a "$R"
else
  echo "RECORD phase 2 free the dpni: nothing to free" | tee -a "$R"
  echo "PASS dpni freed by disconnect" | tee -a "$R"
fi

# Phase 3 — bare uplink face (rev 2's), only on a clean slate. If phase 2
# could not free the dpni, every judged line below records a skip instead.
if still_paired; then
  echo "RECORD uplink connect status: skipped: still paired" | tee -a "$R"
  echo "RECORD uplink dpni-connect answer and state agree: skipped: still paired" | tee -a "$R"
  echo "RECORD uplink disconnect from the demux end: skipped: still paired" | tee -a "$R"
  echo "RECORD uplink disconnect from the dpni end: skipped: still paired" | tee -a "$R"
  echo "RECORD uplink pairing after both disconnects: skipped: still paired" | tee -a "$R"
else
  restool dprc connect dprc.1 --endpoint1="${OBJ_dpdmux_0}" --endpoint2="${OBJ_dpni_0}" > "$RESULTS/uplink-connect.txt" 2>&1 || true
  s="$(mc_status "$RESULTS/uplink-connect.txt")"
  echo "RECORD uplink connect status: ${s:-<accepted, none on stderr>}" | tee -a "$R"
  restool dpdmux info "${OBJ_dpdmux_0}" > "$RESULTS/uplink-dpdmux-info.txt" 2>&1 || true
  i0="$(iface_conn "$RESULTS/uplink-dpdmux-info.txt" 0)"
  restool dpni info "${OBJ_dpni_0}" > "$RESULTS/uplink-dpni-info.txt" 2>&1 || true
  dep="$(dpni_peer "$RESULTS/uplink-dpni-info.txt")"
  echo "RECORD uplink interface 0: ${i0:-<none read>}" | tee -a "$R"
  echo "RECORD uplink dpni endpoint: $dep" | tee -a "$R"
  # Answer vs state, same rule as phase 1 with interface 0 for the port.
  case "$i0" in dpni.*) i0_dpni=1 ;; *) i0_dpni=0 ;; esac
  case "$dep" in "${OBJ_dpdmux_0}".*) dep_mux=1 ;; *) dep_mux=0 ;; esac
  if { [ -z "$s" ] && [ "$i0_dpni" = 1 ] && [ "$dep_mux" = 1 ]; } || \
     { [ -n "$s" ] && [ "$i0" = none ] && [ "$dep" = none ]; }; then r=PASS; else r=FAIL; fi
  echo "$r uplink dpni-connect: answer and state agree" | tee -a "$R"
  # Try to free it again from each end; these are the record, not a verdict.
  restool dprc disconnect dprc.1 --endpoint="${OBJ_dpdmux_0}" > "$RESULTS/uplink-disconnect-mux.txt" 2>&1 || true
  dm="$(mc_status "$RESULTS/uplink-disconnect-mux.txt")"
  echo "RECORD uplink disconnect from the demux end: ${dm:-<accepted, none on stderr>}" | tee -a "$R"
  restool dprc disconnect dprc.1 --endpoint="${OBJ_dpni_0}" > "$RESULTS/uplink-disconnect-dpni.txt" 2>&1 || true
  dn="$(mc_status "$RESULTS/uplink-disconnect-dpni.txt")"
  echo "RECORD uplink disconnect from the dpni end: ${dn:-<accepted, none on stderr>}" | tee -a "$R"
  restool dpdmux info "${OBJ_dpdmux_0}" > "$RESULTS/uplink-info-final.txt" 2>&1 || true
  i0f="$(iface_conn "$RESULTS/uplink-info-final.txt" 0)"
  restool dpni info "${OBJ_dpni_0}" > "$RESULTS/uplink-dpni-final.txt" 2>&1 || true
  depf="$(dpni_peer "$RESULTS/uplink-dpni-final.txt")"
  echo "RECORD uplink pairing after both disconnects: ${i0f:-<none read>} / dpni endpoint $depf" | tee -a "$R"
fi

# Teardown: the EXIT-trap teardown in the generated V-DPDMUX-2.sh destroys
# both objects, once per run. An empty teardown.log is the evidence the
# destroys were accepted while the pairing still stood.
echo; echo "uplink face: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
