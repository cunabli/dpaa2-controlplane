# V-LINK-4 peer-request-channel face. Sourced by V-LINK-4.sh after its
# last step (link up on dpmac.7) and before its teardown trap: the scratch
# dpni is standing, bound and link-up, and no frames are sent. DPMAC-I4 —
# the MC keeps two directional link channels: requests flow down from the
# dpni (ethtool -A writes them, restool cannot read them) and PHY reality
# flows up (ethtool -a reads it). A request write that does not reappear
# in the reality read-back is the two channels being distinct.
# From the script: $OBJ_dpni_2 (the scratch dpni on dpmac.7), $RESULTS.
# The suite's own bound dpni evicts fsl_dpaa2_mac from dpmac.7 (ADR-0008
# §8); the residents rule records that rather than failing on it, and the
# teardown's disconnect-before-unbind hands the driver back.
RESIDENTS_EXPECT_UNBOUND="dpmac.7"
. "$(dirname "$0")/../residents.sh"
R="$RESULTS/link.txt"
DEV=/sys/bus/fsl-mc/devices
DPNI="$OBJ_dpni_2"
nd="$(ls "$DEV/$DPNI/net/" 2>/dev/null | head -1)"

log() { echo "$1" | tee -a "$R"; }
pause_rx() { ethtool -a "$nd" 2>/dev/null | awk 'tolower($1) == "rx:" { print tolower($2) }'; }
pause_tx() { ethtool -a "$nd" 2>/dev/null | awk 'tolower($1) == "tx:" { print tolower($2) }'; }
opp() { [ "$1" = on ] && echo off || echo on; }
mc_link() { restool dpni info "$DPNI" 2>/dev/null | awk -F: 'tolower($0) ~ /link status/ { gsub(/ /, "", $2); print $2 }'; }
carrier() { cat "/sys/class/net/$nd/carrier" 2>/dev/null; }
ask() { printf '\n>> %s\n   enter when done: ' "$1"; read -r _; }

residents_pre

# (a) Record the pause the netdev came up with. Rev 1 assumed the driver
# forces pause on at probe, but the PHY's negotiated reality overwrites
# that request at the first link event — on this wiring the board came up
# "flow control off", so writing off onto off observed nothing. Rev 2
# reads (a) first and writes its opposite.
ethtool -a "$nd" > "$RESULTS/link-ethtool-a-0.txt" 2>&1 || true
arx="$(pause_rx)"; atx="$(pause_tx)"
log "RECORD (a) ethtool -a as bound: rx=$arx tx=$atx"

# (b) Write the *opposite* of (a) on both directions and immediately read
# the reality channel back. A read-back that flips to match the write says
# `ethtool -a` echoes the cached request; whether that request is the MC's
# state is what the bounce in (c) settles — the request and the reality are
# distinct channels (DPMAC-I4) only if the bounce can override the request.
wrx="$(opp "$arx")"; wtx="$(opp "$atx")"
ethtool -A "$nd" rx "$wrx" tx "$wtx" 2>>"$RESULTS/link.log" || true
ethtool -a "$nd" > "$RESULTS/link-ethtool-a-b.txt" 2>&1 || true
brx="$(pause_rx)"; btx="$(pause_tx)"
log "RECORD (b) after 'ethtool -A rx $wrx tx $wtx': ethtool -a rx=$brx tx=$btx, dpni link status=$(mc_link)"
dmesg 2>/dev/null | awk '/dpmac.7/' | tail -n 20 > "$RESULTS/link-kmsg-b.txt" || true
if [ "$brx" = "$wrx" ] && [ "$btx" = "$wtx" ]; then
  log "RECORD (b) ethtool -a now echoes the written request (rx=$brx tx=$btx); the bounce decides whether reality follows"
else
  log "RECORD (b) ethtool -a did not take the request (rx=$brx tx=$btx vs written $wrx/$wtx)"
fi

# (c) A bounce is a PHY-reality push. Ask the operator to bounce the peer
# port, wait for the local carrier and restool's link read-back to agree
# again (V-LINK-2's acknowledgment), then read the reality channel once
# more — the read that shows what reality carries. Reality reverting to (a)
# through a link event, over the opposite request written in (b), is the
# two channels being distinct (DPMAC-I4).
ask "bounce the peer port facing dpmac.7 (admin-down then admin-up on the peer), then confirm both faces agree: cat /sys/class/net/$nd/carrier reads 1 AND restool dpni info $DPNI shows link status: 1"
log "RECORD (c) carrier=$(carrier) dpni link status=$(mc_link)"
ethtool -a "$nd" > "$RESULTS/link-ethtool-a-c.txt" 2>&1 || true
crx="$(pause_rx)"; ctx="$(pause_tx)"
log "RECORD (c) reality channel after the bounce: rx=$crx tx=$ctx"
if [ "$crx" = "$arx" ] && [ "$ctx" = "$atx" ]; then
  log "PASS (c) the bounce reverted pause to the PHY's reality ($arx/$atx), overriding the request written in (b) — two distinct channels (DPMAC-I4)"
else
  log "RECORD (c) the bounce did not revert to (a)'s reading (rx=$crx tx=$ctx vs $arx/$atx)"
fi

# (d) Restore (a)'s reading and read it back before returning.
ethtool -A "$nd" rx "$arx" tx "$atx" 2>>"$RESULTS/link.log" || true
ethtool -a "$nd" > "$RESULTS/link-ethtool-a-d.txt" 2>&1 || true
if [ "$(pause_rx)" = "$arx" ] && [ "$(pause_tx)" = "$atx" ]; then
  log "PASS (d) pause restored to (a)'s reading and read back (rx=$arx tx=$atx)"
else
  log "FAIL (d) pause not restored: rx=$(pause_rx) tx=$(pause_tx) vs (a) $arx/$atx"
fi

residents_post
echo; echo "link face: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
