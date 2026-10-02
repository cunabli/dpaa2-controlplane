#!/bin/sh
# suite: V-DPMAC-3
# class: link-signaling  (explicitly flagged run — the kernel port takes wired dpmac.7)
# HAND-AUTHORED — not emitted by dpaa2-verify. The steps are `dpaa2ctl`
# invocations (dry-run -> ensure -> status --detail hooks -> teardown), not
# model-trace restool verbs, so there is no generator to regenerate from. Edit
# directly.
#
# Suite A of dpmac-typestate (task 5.1, bead dpaa2-controlplane-0xu.10;
# mbt-harness spec "The board program renders Suite A and Suite B inside the
# envelope" / scenario "Suite A witnesses the typed teardown";
# dpmac-typestate design D7). The full product path for the port surface: one
# `dpaa2ctl ensure` over models/board/V-DPMAC-3/intent-a.toml converges the kernel
# regime on dpmac.7 (KernelOwned) and a cross-container VFIO child on dpmac.5
# (RemoteOwned). Between converge and teardown the read-only hooks witness, each
# citing its docs/baseline/dpmac.md observable:
#   (a) arbitration state (DPMAC-I6) via `dpaa2ctl status --detail`;
#   (b) MAC immutability + the inheritance positive face (DPMAC-I2 / DPNI-I3):
#       the dpni primary MAC equals the dpmac burned-in MAC after bind;
#   (c) attribute constancy across the cycle (DPMAC-I3 observe-only face);
#   (d) the 28-row counter read (DPMAC-I7): exactly 28 rows on MC 10.39, the row
#       count the only observable of a silent refusal;
#   (e) the carrier (dpmac-typestate design D6 sysfs route; restool dpni link line
#       as oracle).
# The teardown runs the typed sever-then-unbind path (ADR-0008 §8): the disconnect
# is issued while the dpni is bound, the unbind follows, and dpmac.7 reads back
# with the standalone driver re-attached — no driverless interval. The RemoteOwned
# leg is read-only across the container boundary (dpmac-typestate task 4.1: RemoteOwned is not
# derivable from root-scoped observation), so its readings are RECORDED findings,
# not PASS/FAIL. The operand's derived counts are pinned offline by
# crates/dpaa2-tools/tests/vdpmac3_intents.rs.
#
# Hooks observe; they never gate convergence (dpmac-typestate task 4.1
# display-only rule; the engine proof is crates/dpaa2-tools/tests/port_detail.rs).
#
# Banked verdicts are CITED, never re-run: V-LINK-2 rev 3 (the kernel `up` bit is
# effective with state_valid=0, with propagation lag; cable pull the only
# link-down stimulus), V-LINK-4 rev 2 (no kernel-side observable for DPMAC-I4's
# directional channels on a PHY port), V-DPMAC-1 rev 1 (the 28-of-62 counter
# vocabulary is firmware-wide, refusals silent).
#
# OPERATOR, READ FIRST:
#   - Run as root — the ensure legs issue MC creates/destroys and a VFIO bind
#     that gate on CAP_NET_ADMIN (docs/baseline/mc-ioctl-policy.md).
#   - dpmac.7 is a FLAGGED production-peer port (ADR-0003 §4): the peer facing it
#     MUST be admin-up before you start, or the carrier hook reads down/timed-out
#     (V-LINK-2 shape). This suite touches ONLY this change's own dpni and the
#     connection edge — never the peer host (dpmac-typestate Risks).
#   - KNOWN HAZARD, V-MVP-1 rev 5 finding-49: one phylink link-up Oops can fire at
#     the ordered teardown (phylink_resolve -> dpaa2_mac_link_up -> mc_send_command
#     Oops; the board keeps serving, a kworker dies). Recorded in
#     docs/upstream/phylink-dpni-churn-crash.md. If it fires, POWER-CYCLE the board
#     after the sitting — the ADR-0003 §7 recovery guarantee is the backstop.
#   - The ensure grows the kernel's root pool; dpio seats are grow-only
#     (pool-objects design D4) and this suite CANNOT reclaim them at teardown — run
#     V-DPMAC-3 LAST in a sitting and REBOOT after (ADR-0003 §7). The post-suite
#     census shows the leaked dpio seats until that reboot.
set -u
RESULTS="${1:?usage: $0 <results-dir>}"
case "$RESULTS" in /*) ;; *) RESULTS="$PWD/$RESULTS" ;; esac
mkdir -p "$RESULTS"

# --- repo-root cd (dpmac-typestate design D7; ADR-0003 §2) ---
# The dpaa2ctl steps name the operand relative to the repo root. SELF is resolved
# absolute BEFORE the cd so the total-deny self-check greps the right file.
SELF="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")"
cd "$(dirname "$SELF")/../../.." || { echo "refusing: cannot reach the repo root from $SELF" >&2; exit 1; }
INTENT="models/board/V-DPMAC-3/intent-a.toml"
[ -f "$INTENT" ] || { echo "refusing: not the repo root ($INTENT missing); run this script from its checkout" >&2; exit 1; }

# --- dpaa2ctl from this checkout's build output (ADR-0003 §2) ---
if [ -z "${DPAA2CTL:-}" ]; then
  if [ -x target/release/dpaa2ctl ]; then
    DPAA2CTL=target/release/dpaa2ctl
  elif [ -x target/debug/dpaa2ctl ]; then
    DPAA2CTL=target/debug/dpaa2ctl
  else
    echo "refusing: no dpaa2ctl in target/release or target/debug — run: cargo build -p dpaa2-tools" >&2
    exit 1
  fi
fi
{ echo "$DPAA2CTL"; sha256sum "$DPAA2CTL" 2>/dev/null || ls -l "$DPAA2CTL"; } > "$RESULTS/dpaa2ctl-provenance.txt"
echo "git rev: $(git rev-parse HEAD 2>/dev/null || echo unknown)" > "$RESULTS/git-rev.txt"

# --- kernel-log window ---
KMSG="dpaa2-verify V-DPMAC-3 pid $$"
echo "$KMSG start" > /dev/kmsg 2>/dev/null || true
save_dmesg() {
  dmesg 2>/dev/null | awk -v m="$KMSG start" 'w || index($0, m) { w = 1 } w' > "$RESULTS/dmesg.txt"
  [ -s "$RESULTS/dmesg.txt" ] || dmesg > "$RESULTS/dmesg.txt" 2>&1 || true
}

# --- independent safety self-check (ADR-0003 §4) ---
# dpmac.7 is permitted here under the flagged link-signaling class; dpmac.5 is
# lifecycle-only. The deny set is unchanged; sysfs globs use dpni.* and variables,
# never the forbidden zeroth-dpni literal.
if grep -nE 'dpmac[.]3([^0-9]|$)|dpmac[.]17([^0-9]|$)|dpni[.]0([^0-9]|$)' "$SELF" | grep -v safety-self-check; then
  echo "refusing: total-deny object referenced in this script" >&2  # safety-self-check
  exit 1
fi

# --- helpers ---
# run N cmd...: echo, execute, capture stdout, stderr and the exit code.
run() { n="$1"; shift; echo "+ $*"; "$@" >"$RESULTS/step-$n-out.txt" 2>"$RESULTS/step-$n-err.txt"; echo $? > "$RESULTS/step-$n-exit.txt"; }
probe() { f="$1"; shift; echo "+ (probe) $*"; "$@" > "$RESULTS/$f" 2>/dev/null || true; }
pool_capture() { restool dprc show mc.global --resources > "$RESULTS/$1" 2>/dev/null || true; }
expect_zero() { rc=$(cat "$RESULTS/step-$1-exit.txt"); if [ "$rc" = 0 ]; then echo "PASS step $1: $2"; else echo "FAIL step $1: $2 (exit $rc)" >&2; fi; }
# expect_out N needle label: PASS iff step N's captured output carries the needle.
expect_out() { if grep -qF "$2" "$RESULTS/step-$1-out.txt" "$RESULTS/step-$1-err.txt" 2>/dev/null; then echo "PASS step $1: $3"; else echo "FAIL step $1: $3 (missing '$2')" >&2; fi; }
# mac_of FILE: the lower-cased MAC address restool printed in a capture, or empty.
mac_of() { awk 'tolower($0) ~ /mac address/ { print tolower($NF); exit }' "$RESULTS/$1" 2>/dev/null; }

# --- reference pair assertion (ADR-0003 §2) ---
mc="$(restool -m 2>/dev/null || true)"
case "$mc" in *10.39.0*) ;; *) echo "refusing: MC firmware is not 10.39.0: $mc" >&2; exit 1 ;; esac
kernel="$(uname -r)"
case "$kernel" in 6.6.52*) ;; *) echo "refusing: kernel is not 6.6.52: $kernel" >&2; exit 1 ;; esac

# --- carrier-route reference-pair property (dpmac-typestate design D6; ADR-0008 class) ---
# CONFIG_FSL_DPAA2_MAC_NETDEVS gives a standalone-bound (Offered) dpmac a macN
# netdev — the route every carrier read depends on (crates/dpaa2-hal/src/sysfs.rs
# mac_netdev_present). dpmac.5 stays standalone the whole suite, so its macN is the
# witness. A BSP without the config degrades carrier reads to NoObservable — loud,
# not fatal.
if [ -d /sys/bus/fsl-mc/devices/dpmac.5/net ] && [ -n "$(ls -A /sys/bus/fsl-mc/devices/dpmac.5/net 2>/dev/null)" ]; then
  echo "PASS pre-flight: CONFIG_FSL_DPAA2_MAC_NETDEVS live (dpmac.5 exposes a macN netdev); the carrier route is observable"
else
  echo "RECORD pre-flight: no macN netdev on standalone dpmac.5 — CONFIG_FSL_DPAA2_MAC_NETDEVS absent; carrier reads degrade to NoObservable (dpmac-typestate design D6), continuing"
fi

# --- unconditional teardown (ADR-0003 §6/§7) — the backstop, idempotent with step 6 ---
# Reconcile both regimes back to the baseline through the reconciler's own reverse
# path: an empty intent derives zero, and ensure runs the typed teardown — the
# kernel dpni on dpmac.7 is severed (disconnect) then unbound then destroyed
# (ADR-0008 §8 sever-then-unbind), the remote child is pruned. dpio seats are
# grow-only and NOT reclaimed here — the closing reboot restores them (ADR-0003 §7).
teardown() {
  printf '[intent]\nschema = 1\n' > "$RESULTS/teardown-empty.toml"
  "$DPAA2CTL" --config "$RESULTS/teardown-empty.toml" ensure --prune --allow disruptive --no-link \
    > "$RESULTS/teardown-ensure.txt" 2>>"$RESULTS/teardown.log" || true
  sleep 2
  pool_capture pool-teardown.txt
  save_dmesg
  echo "teardown: kernel dpni severed-then-unbound and destroyed + remote child pruned; dpio seats are grow-only — reboot to reclaim (ADR-0003 §7)"
}
trap teardown EXIT

# --- pool baseline (ADR-0011) ---
pool_capture pool-baseline.txt

# step 0: pre-suite census — the baseline the post-suite state is diffed against,
# and the BEFORE half of the DPMAC-I3 attr-constancy and DPMAC-I7 counter reads
# (dpmac.7 is Offered/standalone here, pre-bind).
run 0 restool dprc list
expect_zero 0 "pre-suite census: container list"
probe step-0-show-dprc1.txt restool dprc show dprc.1
probe step-0-dpmac7-info.txt restool dpmac info dpmac.7
probe step-0-dpmac7-counters.txt restool dpmac info dpmac.7 --verbose
probe step-0-dpmac7-mac.txt restool dpmac info dpmac.7
# The DPC-born attribute projection (DPMAC-I3): max rate + link type are constant
# across connect/disconnect; captured now for the after-bind diff in step 3c.
grep -iE 'max rate|link type' "$RESULTS/step-0-dpmac7-info.txt" > "$RESULTS/step-0-dpmac7-attrs.txt" 2>/dev/null || true

# step 1: dry-run intent-a — headlines the convergence plan, dispatching nothing.
run 1 "$DPAA2CTL" --config "$INTENT" dry-run
expect_zero 1 "dry-run headlines the convergence plan"

# step 2: converge under the disruptive gate — grows the root pool, creates the
# kernel dpni on dpmac.7 (KernelOwned), creates + populates + VFIO-binds the remote
# child (RemoteOwned). CreateContainer / pool grow are Class::Disruptive.
run 2 "$DPAA2CTL" --config "$INTENT" ensure --no-link --allow disruptive
expect_zero 2 "ensure --allow disruptive converges both regimes"
probe step-2-list.txt restool dprc list
probe step-2-show-dprc1.txt restool dprc show dprc.1
pool_capture pool-after-converge.txt

# Resolve the kernel dpni whose endpoint is dpmac.7 (its netdev + carrier hook).
KERN_DPNI=""
for d in /sys/bus/fsl-mc/devices/dpni.*; do
  [ -e "$d" ] || continue
  n="$(basename "$d")"
  if restool dpni info "$n" 2>/dev/null | grep -qE 'dpmac\.7($|[^0-9])'; then KERN_DPNI="$n"; break; fi
done
echo "kernel dpni on dpmac.7: ${KERN_DPNI:-<none>}" | tee "$RESULTS/kern-dpni.txt"

# ---- hooks: read-only witnesses between converge and teardown (dpmac-typestate design D7) ----

# step 3a: arbitration (DPMAC-I6) — status --detail reads KernelOwned on dpmac.7
# from the driver-link read-back, never commanded (dpmac-typestate design D2).
run 3 "$DPAA2CTL" --config "$INTENT" status --detail
expect_zero 3 "status --detail reads the port surface"
if grep -E '^[[:space:]]*dpmac\.7 ' "$RESULTS/step-3-out.txt" | grep -q 'arbitration=KernelOwned'; then
  echo "PASS step 3a: dpmac.7 reads arbitration=KernelOwned (DPMAC-I6)"
else
  echo "FAIL step 3a: dpmac.7 is not KernelOwned after bind (DPMAC-I6)" >&2
fi

# step 3b: MAC immutability + inheritance positive face (DPMAC-I2 / DPNI-I3) — the
# dpni primary MAC equals the dpmac burned-in MAC after bind; status --detail
# classifies it Inherited. restool is the oracle (dpmac-typestate design D6).
if grep -E '^[[:space:]]*dpmac\.7 ' "$RESULTS/step-3-out.txt" | grep -q 'mac=Inherited'; then
  echo "PASS step 3b: dpmac.7 MAC relation reads Inherited (DPNI-I3 positive face)"
else
  echo "FAIL step 3b: dpmac.7 MAC relation is not Inherited" >&2
fi
probe step-3-dpmac7-mac.txt restool dpmac info dpmac.7
[ -n "$KERN_DPNI" ] && probe step-3-kerndpni-info.txt restool dpni info "$KERN_DPNI"
dpmac_mac="$(mac_of step-3-dpmac7-mac.txt)"
dpni_mac="$(mac_of step-3-kerndpni-info.txt)"
echo "dpmac.7 burned-in MAC=${dpmac_mac:-<none>} ; kernel dpni primary MAC=${dpni_mac:-<none>}" | tee "$RESULTS/step-3-mac-compare.txt"
if [ -n "$dpmac_mac" ] && [ "$dpmac_mac" = "$dpni_mac" ]; then
  echo "PASS step 3b: the dpni primary MAC equals the dpmac burned-in MAC (DPMAC-I2 immutable, inherited at bind)"
else
  echo "FAIL step 3b: dpni primary MAC ($dpni_mac) != dpmac burned-in MAC ($dpmac_mac)" >&2
fi

# step 3c: attribute constancy (DPMAC-I3 observe-only face) — the DPC-born attr
# projection (max rate + link type) is identical before (step 0) and after bind.
grep -iE 'max rate|link type' "$RESULTS/step-3-dpmac7-mac.txt" > "$RESULTS/step-3-dpmac7-attrs.txt" 2>/dev/null || true
if diff "$RESULTS/step-0-dpmac7-attrs.txt" "$RESULTS/step-3-dpmac7-attrs.txt" > "$RESULTS/step-3-attr-diff.txt" 2>&1; then
  echo "PASS step 3c: dpmac.7 DPC-born attributes constant across the bind (DPMAC-I3)"
else
  echo "FAIL step 3c: dpmac.7 attributes changed across the bind (see step-3-attr-diff.txt, DPMAC-I3)" >&2
fi

# step 3d: the 28-row counter read (DPMAC-I7) — status --detail renders exactly 28
# vocabulary rows on MC 10.39; the row count is the only observable of the silent
# refusal of the 34 counters 10.39 does not carry (V-DPMAC-1 rev 1, cited). restool
# --verbose is captured as the oracle, not re-counted here.
if grep -qF 'counters (28):' "$RESULTS/step-3-out.txt"; then
  echo "PASS step 3d: status --detail reads the 28-row 10.39 counter vocabulary (DPMAC-I7)"
else
  echo "FAIL step 3d: the counter read is not the 28-row 10.39 vocabulary (DPMAC-I7)" >&2
fi
probe step-3-dpmac7-counters.txt restool dpmac info dpmac.7 --verbose

# step 3e: carrier (dpmac-typestate design D6 sysfs route; restool dpni link line
# the oracle). Bring the kernel netdev up and wait bounded (~60s) for the carrier
# flag and restool's link read-back to agree (V-LINK-2 acknowledgment, cited). A
# carrier that stays down is a RECORDED finding — the hook never gates convergence.
if [ -n "$KERN_DPNI" ]; then
  IF="$(ls "/sys/bus/fsl-mc/devices/$KERN_DPNI/net/" 2>/dev/null | head -1)"
  echo "netdev: ${IF:-<none>}" | tee "$RESULTS/step-3-netdev.txt"
  if [ -n "$IF" ]; then
    ip link set "$IF" up 2>>"$RESULTS/step-3.log" || true
    i=0
    while [ "$i" -lt 60 ]; do
      c="$(cat "/sys/class/net/$IF/carrier" 2>/dev/null || echo 0)"
      # restool renders `link status: N - word`; split on [:-] so a bare value compares.
      l="$(restool dpni info "$KERN_DPNI" 2>/dev/null | awk -F'[:-]' 'tolower($0) ~ /link status/ { gsub(/ /, "", $2); print $2 }')"
      if [ "$c" = 1 ] && [ "$l" = 1 ]; then break; fi
      sleep 1; i=$((i + 1))
    done
    echo "carrier=$c dpni-link-status=$l after ${i}s" | tee "$RESULTS/step-3-link.txt"
    if grep -E '^[[:space:]]*dpmac\.7 ' "$RESULTS/step-3-out.txt" | grep -q 'carrier=up'; then
      echo "PASS step 3e: status --detail reads carrier=up on dpmac.7 (sysfs route; restool link agrees)"
    else
      echo "RECORD step 3e: dpmac.7 carrier not up at the status read (carrier=$c link=$l) — peer-dependent; re-read below"
      probe step-3e-status-recheck.txt "$DPAA2CTL" --config "$INTENT" status --detail
    fi
  else
    echo "RECORD step 3e: kernel dpni exposes no netdev yet — carrier NoObservable at this read"
  fi
else
  echo "FAIL step 3e: no dpni terminates dpmac.7 after convergence" >&2
fi

# step 4: idempotence — a re-run dry-run plans zero, a second ensure dispatches
# nothing (Converged, level-triggered; the V-DPRC-9 convention).
run 4 "$DPAA2CTL" --config "$INTENT" dry-run
expect_zero 4 "post-converge dry-run: the board is converged"
expect_out 4 "0 planned transition(s)" "the converged dry-run plans zero port transitions"
run 5 "$DPAA2CTL" --config "$INTENT" ensure --no-link --allow disruptive
expect_zero 5 "second ensure dispatches nothing (idempotent, no-op re-run)"

# step 5 (RemoteOwned leg): read the dpmac.5 VFIO child arrangement ACROSS the
# container boundary. dpmac-typestate task 4.1: RemoteOwned is not derivable from root-scoped
# observation (fsl_mc_get_endpoint returns -EPERM across the boundary), so root
# scope shows arbitration as observed and carrier as observed — both are RECORDED
# findings, never PASS/FAIL (dpmac-typestate design D7).
echo "+ RemoteOwned leg: dpmac.5 cross-boundary read (findings, not pass/fail)"
dpmac5_line="$(grep -E '^[[:space:]]*dpmac\.5 ' "$RESULTS/step-3-out.txt" 2>/dev/null)"
echo "RECORD RemoteOwned: status --detail dpmac.5 => ${dpmac5_line:-<no dpmac.5 line>}"
probe step-5-dpmac5-info.txt restool dpmac info dpmac.5
rmt_drv="$(readlink "/sys/bus/fsl-mc/devices/dpmac.5/driver" 2>/dev/null)"
echo "RECORD RemoteOwned: dpmac.5 kernel driver=${rmt_drv:-<none>} (standalone keeps the PHY while the datapath is remote; dpmac.md kernel-side behavior)" | tee "$RESULTS/step-5-driver.txt"

# step 6 (typed teardown — the spec scenario "Suite A witnesses the typed
# teardown"): the empty intent reconciles both regimes down. For the KernelOwned
# dpmac.7 the engine runs the sever-then-unbind path (ADR-0008 §8): disconnect
# while bound, then unbind, then destroy — so dpmac.7 reads back with the
# standalone driver re-attached and NO driverless interval.
printf '[intent]\nschema = 1\n' > "$RESULTS/step-6-empty.toml"
run 6 "$DPAA2CTL" --config "$RESULTS/step-6-empty.toml" ensure --prune --allow disruptive --no-link
expect_zero 6 "empty-intent ensure tears both regimes down (sever-then-unbind)"
sleep 2
# The kernel dpni is gone, and dpmac.7's standalone driver is re-attached — the
# teardown left no driverless port (ADR-0008 §8; V-LINK-4 checkpoint shape, cited).
if [ -n "$KERN_DPNI" ] && [ ! -e "/sys/bus/fsl-mc/devices/$KERN_DPNI" ]; then
  echo "PASS step 6: the kernel dpni $KERN_DPNI is gone (sysfs device absent)"
elif [ -n "$KERN_DPNI" ]; then
  echo "FAIL step 6: the kernel dpni $KERN_DPNI survives teardown" >&2
fi
dpmac7_drv="$(readlink "/sys/bus/fsl-mc/devices/dpmac.7/driver" 2>/dev/null)"
echo "dpmac.7 driver after teardown: ${dpmac7_drv:-<none>}" | tee "$RESULTS/step-6-driver.txt"
case "$dpmac7_drv" in
  *fsl_dpaa2_mac) echo "PASS step 6: dpmac.7 standalone driver re-attached, no driverless interval (ADR-0008 §8)";;
  *) echo "FAIL step 6: dpmac.7 is NOT standalone-bound after teardown (driver=${dpmac7_drv:-none}) — driverless port, the dpmac-typestate design D3 hazard" >&2;;
esac

# step 7: closing census — diffed offline against step 0's baseline. The managed
# root pool + grow-only dpio seats remain as reboot-required residue until the
# closing reboot (ADR-0020 decision 2; ADR-0003 §7); the container set must match.
run 7 restool dprc list
expect_zero 7 "post-suite census: container list"
probe step-7-show-dprc1.txt restool dprc show dprc.1
pool_capture pool-post.txt
echo "census diff vs step 0: the managed root pool plus the grow-only dpio seats remain as reboot-required residue (ADR-0020 decision 2); the closing reboot restores the DPL baseline (ADR-0003 §7)"

echo "suite V-DPMAC-3 typestate convergence + teardown complete"
