#!/bin/sh
# suite: V-DPSECI-3
# class: lifecycle  (crypto-only scratch tenant; touches no wired port, asserts no link)
# HAND-AUTHORED — not emitted by dpaa2-verify. The steps are `dpaa2ctl`
# invocations (dry-run -> ensure -> status --detail hooks -> teardown), not
# model-trace restool verbs, so there is no generator to regenerate from. Edit
# directly.
#
# Suite A of dpseci-typestate (task 5.1, bead dpaa2-controlplane-lbk.9;
# mbt-harness spec "Suite A is generated inside the safety envelope" /
# system-integration spec "One crypto intent converges end to end on the board";
# dpseci-typestate design D6). The full product path for the crypto surface: one
# `dpaa2ctl ensure` over models/board/V-DPSECI-3/intent-a.toml converges a scratch
# tenant's `[[crypto]]` block into a child DPRC holding one dpseci, which the
# child then VFIO-binds (RemoteOwned). Between converge and teardown the
# read-only hooks witness the two transports the deciding hazards live on:
#   (a) V-DPSECI-2 rev 1, the dual-transport read-back of the scratch dpseci:
#       restool `dpseci info` shows queues = flows and all-2 priorities (the
#       restool transport), AND the raw GET_ATTR face via `dpaa2ctl status
#       --detail` shows the options mask carries HAS_CG plus the GET_API_VERSION
#       column — the observable restool discards at print (DPSECI-I3;
#       dpseci-typestate design D5);
#   (b) the VFIO RemoteOwned leg, read across the container boundary as RECORDED
#       findings, not PASS/FAIL (the V-DPMAC-3 RemoteOwned precedent; root scope
#       cannot derive a child-container arbitration).
# The teardown runs the typed reverse path (ADR-0008): an empty intent derives
# zero, and `ensure --prune` destroys the surplus scratch dpseci and prunes its
# child DPRC — the signature-multiset census of dpseci-typestate design D9,
# closing clean (verdict V-DPSECI-3 rev 1, the e2e verdict). The operand's derived
# cfg is pinned offline by crates/dpaa2-tools/tests/vdpseci3_intents.rs.
#
# Read-only boot-object hooks ride the same script (dpseci-typestate design D6),
# each answering a docs/baseline/dpseci.md unknown, each RECORDED, never gating:
#   - GET_API_VERSION on a dpseci (baseline unknown #2, expected 5.4) — the
#     portal version column in `status --detail`;
#   - GET_ATTR on the boot/kernel dpseci (baseline unknown #3: does it carry
#     HAS_CG) — the portal options column in `status --detail`;
#   - the boot dpseci driver-link read (sysfs, read-only).
# Hooks observe; they never gate convergence (the engine proof is
# crates/dpaa2-tools/tests/dpseci_detail.rs). An Unobservable portal read on an
# unprivileged path is a RECORDED shape, not a failure (absence of evidence is
# never drift, dpseci-typestate design D5 and design D9).
#
# SAFETY LAWS (non-negotiable, ADR-0003 §4; dpseci-typestate design D6):
#   - Every mutation is scoped to the scratch tenant that intent-a.toml declares.
#     The ensure legs converge only its child DPRC and dpseci; the teardown prunes
#     only what the engine owns (the one-label law, V-MVP-1 — DPL-born and foreign
#     objects are exempt).
#   - The production VPP child container is NEVER touched — `peer-is-production`
#     discipline applies to the whole live tenant; no step names it.
#   - The boot/kernel dpseci is READ, NEVER unbound: an unbind would risk kernel
#     crypto for the boot (dpseci-typestate design D6). The hooks only readlink its
#     driver and read its portal face; no step writes a dpseci driver node.
#
# Banked verdicts are CITED, never re-run:
#   - V-LIFE-DPSECI-1 rev 2 (models/board/README.md): the crypto-API algorithm
#     namespace is global and the boot dpseci claims it, so a later-created dpseci
#     never kernel-binds for the rest of that boot. The scratch dpseci here is
#     VFIO-side (child container, bound to vfio-fsl-mc), so staying unbound on the
#     kernel side is the EXPECTED shape — recorded, not a divergence.
#   - V-DPSECI-1 rev 1 (models/board/README.md): restool's own parser refuses
#     priority 0, a priority above 8, and a count != num-queues (exit 234) before
#     any MC command; this suite derives an in-range all-2 vector, so that refusal
#     surface is never re-exercised.
#
# OPERATOR, READ FIRST:
#   - Run as root — the ensure legs issue MC creates/destroys and a VFIO bind that
#     gate on CAP_NET_ADMIN (docs/baseline/mc-ioctl-policy.md), and the raw
#     GET_ATTR portal read needs /dev/dprc.N access (a non-root run degrades the
#     portal line to no-observable — recorded, never fatal
#     (dpseci-typestate design D5).
#   - The ensure grows the kernel's root pool; dpio seats are grow-only
#     (ADR-0003 §7) and this suite CANNOT reclaim them at teardown — run
#     V-DPSECI-3 LAST in a sitting and REBOOT after. The post-suite census shows
#     the grow-only residue until that reboot.
set -u
RESULTS="${1:?usage: $0 <results-dir>}"
case "$RESULTS" in /*) ;; *) RESULTS="$PWD/$RESULTS" ;; esac
mkdir -p "$RESULTS"

# --- repo-root cd (dpseci-typestate design D6; ADR-0003 §2) ---
# The dpaa2ctl steps name the operand relative to the repo root. SELF is resolved
# absolute BEFORE the cd so the never-unbind self-check greps the right file.
SELF="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")"
cd "$(dirname "$SELF")/../../.." || { echo "refusing: cannot reach the repo root from $SELF" >&2; exit 1; }
INTENT="models/board/V-DPSECI-3/intent-a.toml"
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
KMSG="dpaa2-verify V-DPSECI-3 pid $$"
echo "$KMSG start" > /dev/kmsg 2>/dev/null || true
save_dmesg() {
  dmesg 2>/dev/null | awk -v m="$KMSG start" 'w || index($0, m) { w = 1 } w' > "$RESULTS/dmesg.txt"
  [ -s "$RESULTS/dmesg.txt" ] || dmesg > "$RESULTS/dmesg.txt" 2>&1 || true
}

# --- independent safety self-check (ADR-0003 §4; dpseci-typestate design D6) ---
# The boot dpseci must be READ, never severed from its kernel driver — a sever
# risks kernel crypto. This suite only readlinks driver nodes, so refuse if it ever
# carries a sysfs WRITE into a bus driver bind/driver_override node, or names the
# zeroth-dpni management interface literal (ADR-0003 §4 total-deny).
if grep -nE '>[^<>]*/(unbind|bind|driver_override)|dpni[.]0([^0-9]|$)' "$SELF" | grep -v safety-self-check; then
  echo "refusing: a forbidden write (bus driver node or total-deny object) is present in this script" >&2  # safety-self-check
  exit 1
fi

# --- helpers ---
# run N cmd...: echo, execute, capture stdout, stderr and the exit code.
run() { n="$1"; shift; echo "+ $*"; "$@" >"$RESULTS/step-$n-out.txt" 2>"$RESULTS/step-$n-err.txt"; echo $? > "$RESULTS/step-$n-exit.txt"; }
probe() { f="$1"; shift; echo "+ (probe) $*"; "$@" > "$RESULTS/$f" 2>/dev/null || true; }
expect_zero() { rc=$(cat "$RESULTS/step-$1-exit.txt"); if [ "$rc" = 0 ]; then echo "PASS step $1: $2"; else echo "FAIL step $1: $2 (exit $rc)" >&2; fi; }
# expect_out N needle label: PASS iff step N's captured output carries the needle.
expect_out() { if grep -qF "$2" "$RESULTS/step-$1-out.txt" "$RESULTS/step-$1-err.txt" 2>/dev/null; then echo "PASS step $1: $3"; else echo "FAIL step $1: $3 (missing '$2')" >&2; fi; }
# detail_row OBJECT: the two-line `status --detail` block for one dpseci (object
# line + its indented portal line), from the step-3 capture.
detail_row() { grep -A1 -E "^[[:space:]]*$1 queues " "$RESULTS/step-3-out.txt" 2>/dev/null; }

# --- reference pair assertion (ADR-0003 §2) ---
mc="$(restool -m 2>/dev/null || true)"
case "$mc" in *10.39.0*) ;; *) echo "refusing: MC firmware is not 10.39.0: $mc" >&2; exit 1 ;; esac
kernel="$(uname -r)"
case "$kernel" in 6.6.52*) ;; *) echo "refusing: kernel is not 6.6.52: $kernel" >&2; exit 1 ;; esac

# --- unconditional teardown (ADR-0003 §6/§7) — the backstop, idempotent with step 6 ---
# Reconcile the crypto population back to baseline through the reconciler's own
# reverse path: an empty intent derives zero dpsecis, and `ensure --prune` destroys
# the surplus scratch dpseci and prunes its child DPRC (the signature-multiset
# census delta of dpseci-typestate design D9).
# The boot and production dpsecis are DPL-born / foreign and exempt (the one-label
# law, V-MVP-1). dpio seats are grow-only and NOT reclaimed here — the closing
# reboot restores them (ADR-0003 §7).
teardown() {
  printf '[intent]\nschema = 1\n' > "$RESULTS/teardown-empty.toml"
  "$DPAA2CTL" --config "$RESULTS/teardown-empty.toml" ensure --prune --allow disruptive --no-link \
    > "$RESULTS/teardown-ensure.txt" 2>>"$RESULTS/teardown.log" || true
  sleep 2
  save_dmesg
  echo "teardown: scratch dpseci destroyed and its child DPRC pruned; dpio seats are grow-only — reboot to reclaim (ADR-0003 §7)"
}
trap teardown EXIT

# step 0: pre-suite census — the baseline the post-suite state is diffed against.
run 0 restool dprc list
expect_zero 0 "pre-suite census: container list"
probe step-0-show-dprc1.txt restool dprc show dprc.1

# Resolve the boot/kernel dpseci: the root-bus-visible dpseci with a driver link
# (the scratch dpseci lives in a VFIO child and is off the root bus, DPRC-I6). This
# is the never-unbind object — only READ here (driver-link + portal face).
BOOT_DPSECI=""
BOOT_DRV=""
for d in /sys/bus/fsl-mc/devices/dpseci.*; do
  [ -e "$d" ] || continue
  drv="$(readlink "$d/driver" 2>/dev/null)"
  if [ -n "$drv" ]; then BOOT_DPSECI="$(basename "$d")"; BOOT_DRV="$drv"; break; fi
done
echo "boot/kernel dpseci: ${BOOT_DPSECI:-<none>} driver=${BOOT_DRV:-<none>}" | tee "$RESULTS/boot-dpseci.txt"

# step 1: dry-run intent-a — headlines the convergence plan, dispatching nothing.
run 1 "$DPAA2CTL" --config "$INTENT" dry-run
expect_zero 1 "dry-run headlines the convergence plan"

# step 2: converge under the disruptive gate — creates the scratch child DPRC,
# creates + populates + VFIO-binds the one dpseci sized by the crypto block's flows.
# CreateContainer / pool grow / dpseci create are Class::Disruptive.
run 2 "$DPAA2CTL" --config "$INTENT" ensure --no-link --allow disruptive
expect_zero 2 "ensure --allow disruptive converges the crypto intent"
probe step-2-list.txt restool dprc list

# Resolve the scratch child DPRC and its dpseci (only the scratch tenant carries a
# crypto block, so the child DPRC that holds a dpseci is the scratch one).
SCRATCH_DPRC=""
SCRATCH_DPSECI=""
for c in $(restool dprc list 2>/dev/null | grep -oE 'dprc\.[0-9]+' | sort -u); do
  [ "$c" = dprc.1 ] && continue
  if restool dprc show "$c" 2>/dev/null | grep -qE 'dpseci\.[0-9]+'; then
    SCRATCH_DPRC="$c"
    SCRATCH_DPSECI="$(restool dprc show "$c" 2>/dev/null | grep -oE 'dpseci\.[0-9]+' | head -1)"
    break
  fi
done
echo "scratch child DPRC=${SCRATCH_DPRC:-<none>} scratch dpseci=${SCRATCH_DPSECI:-<none>}" | tee "$RESULTS/scratch.txt"
probe step-2-show-scratch.txt restool dprc show "${SCRATCH_DPRC:-dprc.1}"
if [ -n "$SCRATCH_DPSECI" ]; then
  echo "PASS step 2: the scratch child DPRC holds the derived dpseci ($SCRATCH_DPSECI)"
else
  echo "FAIL step 2: no dpseci under a scratch child DPRC after convergence" >&2
fi

# step 3: the single read-only portal pass between converge and teardown — the
# GET_ATTR + GET_API_VERSION faces for every dpseci (scratch row + boot row below).
run 3 "$DPAA2CTL" --config "$INTENT" status --detail
expect_zero 3 "status --detail reads the dpseci surface"

# ---- V-DPSECI-2 rev 1: the dual-transport read-back of the scratch dpseci ----

# step 3a (restool transport): `dpseci info` shows queues = flows (2) and all-2
# priorities — the oracle for the queue shape (dpseci-typestate design D6; the
# options mask is NOT printed here, DPSECI-I3).
if [ -n "$SCRATCH_DPSECI" ]; then
  probe step-3-scratch-info.txt restool dpseci info "$SCRATCH_DPSECI"
  txq="$(awk -F: '/number of transmit queues:/ { gsub(/ /,"",$2); print $2 }' "$RESULTS/step-3-scratch-info.txt" 2>/dev/null)"
  prios="$(awk -F: '/tx priorities:/ { gsub(/ /,"",$2); print $2 }' "$RESULTS/step-3-scratch-info.txt" 2>/dev/null)"
  echo "scratch dpseci restool info: tx-queues=${txq:-<none>} tx-priorities=${prios:-<none>}" | tee "$RESULTS/step-3-scratch-restool.txt"
  if [ "$txq" = 2 ] && [ "$prios" = "2,2" ]; then
    echo "PASS step 3a: restool info reads queues=flows(2) and all-2 priorities (V-DPSECI-2 restool transport)"
  else
    echo "FAIL step 3a: restool info did not read queues=2 / priorities=2,2 (got tx-queues=$txq priorities=$prios)" >&2
  fi
else
  echo "FAIL step 3a: no scratch dpseci to read back (V-DPSECI-2 restool transport)" >&2
fi

# step 3b (portal transport): the raw GET_ATTR face via `status --detail` — the
# shipped render carries queues/priorities AND the options mask + GET_API_VERSION
# that restool discards (V-DPSECI-2 rev 1; dpseci-typestate design D5). An
# Unobservable portal on an unprivileged run is a RECORDED shape, never a failure.
if [ -n "$SCRATCH_DPSECI" ]; then
  detail_row "$SCRATCH_DPSECI" | tee "$RESULTS/step-3-scratch-detail.txt" >/dev/null
  if grep -qE "^[[:space:]]*$SCRATCH_DPSECI queues tx=2/rx=2 tx-priorities=\[2,2\]" "$RESULTS/step-3-scratch-detail.txt"; then
    echo "PASS step 3b: status --detail reads the scratch dpseci queue shape (tx=2/rx=2, all-2)"
  else
    echo "FAIL step 3b: status --detail did not read the 2-queue/all-2 shape on $SCRATCH_DPSECI" >&2
  fi
  if grep -q 'options=\[.*HasCg.*\]' "$RESULTS/step-3-scratch-detail.txt"; then
    echo "PASS step 3b: the GET_ATTR face shows HAS_CG in the scratch dpseci options mask (V-DPSECI-2 rev 1)"
  elif grep -q 'portal=no-observable' "$RESULTS/step-3-scratch-detail.txt"; then
    echo "RECORD step 3b: the portal was Unobservable this run (unprivileged path) — absence of evidence, not drift (dpseci-typestate design D5, design D9)"
  else
    echo "FAIL step 3b: the scratch dpseci options mask does not carry HAS_CG (V-DPSECI-2 rev 1)" >&2
  fi
fi

# step 4 (RemoteOwned leg, RECORDED findings — the V-DPMAC-3 precedent): the scratch
# dpseci lives in a VFIO-bound child DPRC, so root scope cannot derive its
# arbitration; what it reads across the boundary is recorded, never PASS/FAIL.
echo "+ RemoteOwned leg: scratch dpseci cross-boundary read (findings, not pass/fail)"
if [ -n "$SCRATCH_DPSECI" ]; then
  if [ -e "/sys/bus/fsl-mc/devices/$SCRATCH_DPSECI" ]; then
    echo "RECORD RemoteOwned: $SCRATCH_DPSECI is present on the root fsl-mc bus (unexpected — the child should hold it, DPRC-I6)"
  else
    echo "RECORD RemoteOwned: $SCRATCH_DPSECI is off the root fsl-mc bus (child-container resident, DPRC-I6) and VFIO-side — staying unbound on the kernel side is the EXPECTED shape, because the boot dpseci owns the crypto algorithm namespace (V-LIFE-DPSECI-1 rev 2, cited)"
  fi
fi

# ---- read-only boot-object hooks (dpseci-typestate design D6) — RECORDED, never gating ----

# hook (baseline unknown #2): GET_API_VERSION on a dpseci — the portal version
# column. Expected 5.4 on the 10.39 firmware (restool's pinned flib reads 5.3).
# hook (baseline unknown #3): GET_ATTR on the boot/kernel dpseci — does it carry
# HAS_CG? restool cannot show it; the portal options column is the only observable.
if [ -n "$BOOT_DPSECI" ]; then
  detail_row "$BOOT_DPSECI" | tee "$RESULTS/boot-detail.txt" >/dev/null
  boot_ver="$(sed -n 's/.*version=\([0-9.]*\).*/\1/p' "$RESULTS/boot-detail.txt" | head -1)"
  echo "RECORD boot hook (unknown #2): $BOOT_DPSECI GET_API_VERSION portal version=${boot_ver:-<no-observable>} (expected 5.4)"
  if grep -q 'options=\[.*HasCg.*\]' "$RESULTS/boot-detail.txt"; then
    echo "RECORD boot hook (unknown #3): the boot dpseci options mask carries HAS_CG"
  elif grep -q 'portal=no-observable' "$RESULTS/boot-detail.txt"; then
    echo "RECORD boot hook (unknown #3): the boot dpseci portal was Unobservable this run — HAS_CG unread (dpseci-typestate design D5)"
  else
    echo "RECORD boot hook (unknown #3): the boot dpseci options mask does NOT carry HAS_CG"
  fi
  # the boot dpseci driver-link read (sysfs, read-only) — the kernel consumer binding.
  echo "RECORD boot hook: $BOOT_DPSECI driver link = ${BOOT_DRV:-<none>} (read-only; never unbound — unbind risks kernel crypto, dpseci-typestate design D6)"
else
  echo "RECORD boot hook: no driver-bound dpseci on the root fsl-mc bus — boot-object faces unread this sitting"
fi

# step 5: idempotence — a re-run dry-run plans zero, a second ensure dispatches
# nothing (Converged, level-triggered; the dpseci-typestate design D9 census
# already matches).
run 5 "$DPAA2CTL" --config "$INTENT" dry-run
expect_zero 5 "post-converge dry-run: the board is converged"
expect_out 5 "0 planned transition(s)" "the converged dry-run plans zero transitions"
run 6 "$DPAA2CTL" --config "$INTENT" ensure --no-link --allow disruptive
expect_zero 6 "second ensure dispatches nothing (idempotent, no-op re-run)"

# step 7 (typed teardown — the e2e verdict V-DPSECI-3 rev 1): the empty intent
# reconciles the crypto population down to zero. `ensure --prune` destroys the
# scratch dpseci and prunes its child DPRC (the dpseci-typestate design D9 signature-multiset
# delta); the boot and production dpsecis are exempt (the one-label law).
printf '[intent]\nschema = 1\n' > "$RESULTS/step-7-empty.toml"
run 7 "$DPAA2CTL" --config "$RESULTS/step-7-empty.toml" ensure --prune --allow disruptive --no-link
expect_zero 7 "empty-intent ensure tears the crypto population down"
sleep 2
if [ -n "$SCRATCH_DPRC" ]; then
  if restool dprc list 2>/dev/null | grep -qE "^$SCRATCH_DPRC([^0-9]|$)|(^|[^0-9])$SCRATCH_DPRC([^0-9]|$)"; then
    echo "FAIL step 7: the scratch child DPRC $SCRATCH_DPRC survives teardown" >&2
  else
    echo "PASS step 7: the scratch child DPRC $SCRATCH_DPRC is pruned (dpseci destroyed with it)"
  fi
fi

# step 8: closing census — diffed offline against step 0's baseline. The managed
# root pool + grow-only dpio seats remain as reboot-required residue until the
# closing reboot (ADR-0003 §7); the container set must match the baseline.
run 8 restool dprc list
expect_zero 8 "post-suite census: container list"
probe step-8-show-dprc1.txt restool dprc show dprc.1
echo "census diff vs step 0: the managed root pool plus the grow-only dpio seats remain as reboot-required residue (ADR-0003 §7); the closing reboot restores the DPL baseline"

echo "suite V-DPSECI-3 crypto convergence + teardown complete"
