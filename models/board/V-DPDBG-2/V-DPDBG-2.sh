#!/bin/sh
# suite: V-DPDBG-2
# class: hand-authored, operator-run — no generator, no plan diff.
# purpose: DPDBG-I4 has two faces. The `dprc show` face is verified
# (V-DPDBG-1, 2026-08-24: create/plug/destroy all read back); the sysfs
# enumeration face is unprobed — the adapter takes no bus-visibility
# observation yet. This suite settles that face (dprc-encapsulation task 6.1):
# it creates the root-only dpdbg singleton, observes its fsl-mc bus node
# and its driver-lessness, exercises a plug that no kernel driver can
# bind, and tears it down. Anchors: DPDBG-I4 and the
# family facts in docs/baseline/dpdbg.md; the read-back conformance and
# unconditional-teardown laws of ADR-0003 §2 and §6.
# operator: run as root — create/destroy and assign issue MC commands the
# kernel gates on CAP_NET_ADMIN (docs/baseline/mc-ioctl-policy.md).
# No repo build artifact is needed: the suite drives restool and sysfs
# only, POSIX sh, no project binary.
#   usage: sudo sh models/board/V-DPDBG-2/V-DPDBG-2.sh <results-dir>
# dpdbg create/destroy take no arguments: restool hardcodes the root
# container and pins the id to 0 (docs/baseline/dpdbg.md, DPDBG-I1), so
# the object is always dpdbg.0. `restool dpdbg destroy` prints success
# unconditionally, so teardown is judged on read-back, never on its exit
# or message (docs/baseline/dpdbg.md silent-failure note).
set -u
RESULTS="${1:?usage: $0 <results-dir>}"
mkdir -p "$RESULTS"

# residents helper is sourced by absolute-safe path so the suite does not
# depend on the caller's cwd (models/board/residents.sh, ADR-0008 §4–§5).
. "$(dirname "$0")/../residents.sh"

# --- kernel-log window (ADR-0008) ---
# A marker stamps the sitting's start in the kernel log; the teardown
# saves everything after it to dmesg.txt, so bus rescan markers and any
# probe refusals are files, not operator memory.
KMSG="dpaa2-verify V-DPDBG-2 pid $$"
echo "$KMSG start" > /dev/kmsg 2>/dev/null || true
save_dmesg() {
  dmesg 2>/dev/null | awk -v m="$KMSG start" 'w || index($0, m) { w = 1 } w' > "$RESULTS/dmesg.txt"
  [ -s "$RESULTS/dmesg.txt" ] || dmesg > "$RESULTS/dmesg.txt" 2>&1 || true
}

# --- independent safety self-check (ADR-0003 §4) ---
# The execution side refuses total-deny references even if a script was
# hand-edited after generation.
if grep -nE 'dpmac[.]3([^0-9]|$)|dpmac[.]17([^0-9]|$)|dpni[.]0([^0-9]|$)' "$0" | grep -v safety-self-check; then
  echo "refusing: total-deny object referenced in this script" >&2  # safety-self-check
  exit 1
fi

# --- reference pair assertion (ADR-0003 §2) ---
# Evidence is only valid against the stamped pair; refuse anything else.
mc="$(restool -m 2>/dev/null || true)"
case "$mc" in *10.39.0*) ;; *) echo "refusing: MC firmware is not 10.39.0: $mc" >&2; exit 1 ;; esac
kernel="$(uname -r)"
case "$kernel" in 6.6.52*) ;; *) echo "refusing: kernel is not 6.6.52: $kernel" >&2; exit 1 ;; esac

# --- constants and helpers (vfio.sh face style) ---
R="$RESULTS/dpdbg.txt"           # the PASS/FAIL/RECORD face log, counted at close
DEV=/sys/bus/fsl-mc/devices
DPDBG=dpdbg.0                    # the pinned singleton (id 0, root container)
route=""                        # the route that surfaced the bus node (d)

log() { echo "$1" | tee -a "$R"; }
# Settle after every bus-affecting op — the rescan race (ADR-0008), same
# duration as the teardown.
settle() { sleep 2; }
node_present() { [ -e "$DEV/$DPDBG" ]; }
driver_of() { drv="$(readlink "$DEV/$DPDBG/driver" 2>/dev/null)"; echo "${drv##*/}"; }
# run_cmd FILE cmd...: echo the command, run it capturing combined output
# (restool prints its MC status text on stderr) to FILE, return its exit.
run_cmd() { f="$1"; shift; echo "+ $*"; "$@" > "$RESULTS/$f" 2>&1; }
show() { restool dprc show dprc.1 > "$RESULTS/$1" 2>&1 || true; }
# has_dpdbg FILE: whether a `dprc show` capture lists a dpdbg row.
has_dpdbg() { awk '$1 ~ /^dpdbg\./ { f=1 } END { exit f?0:1 }' "$1"; }
# plug_state FILE: the plugged-state column (last field) of the dpdbg row.
plug_state() { awk '$1 ~ /^dpdbg\./ { print $NF }' "$1"; }
# pool_capture FILE: snapshot MC-global resource pools (ADR-0011),
# best-effort; the mcp pool count is RECORD-only context here, never
# judged (ADR-0011 open portal-count question).
pool_capture() { restool dprc show mc.global --resources > "$RESULTS/$1" 2>/dev/null || true; }

# --- (a) preconditions: nothing dpdbg exists yet, on either face ---
# The singleton is absent from every reference DPC (docs/baseline/dpdbg.md),
# so a clean board shows no dpdbg row and carries no bus node. A violation
# means a prior sitting leaked one; fail before creating anything.
show show-pre.txt
if has_dpdbg "$RESULTS/show-pre.txt"; then
  log "FAIL (a) a dpdbg already exists in dprc.1 before create"
  exit 1
fi
if node_present; then
  log "FAIL (a) a dpdbg bus node already exists before create"
  exit 1
fi
log "PASS (a) no dpdbg in dprc.1 and no dpdbg bus node before create"
pool_capture pool-pre.txt
residents_pre

# --- unconditional teardown (ADR-0003 §6) ---
# The repo law: suites never destroy inline; the trap is the one destroy
# site, run once, spaced. Post-state is judged on read-back, never on the
# destroy's exit or its unconditional "is destroyed" message.
teardown() {
  sleep 2
  run_cmd teardown.log restool dpdbg destroy || true
  sleep 2
  show show-post.txt
  if has_dpdbg "$RESULTS/show-post.txt"; then
    log "FAIL (h) dpdbg still listed in dprc.1 after destroy"
  else
    log "PASS (h) dpdbg absent from dprc.1 after destroy (read-back)"
  fi
  if node_present; then
    log "FAIL (i) dpdbg bus node still present after destroy"
  else
    log "PASS (i) no dpdbg bus node after destroy (read-back)"
  fi
  pool_capture pool-post.txt
  residents_post
  save_dmesg
  echo; echo "dpdbg sysfs face: $(grep -c '^PASS ' "$R") PASS, $(grep -c '^FAIL ' "$R") FAIL, $(grep -c '^RECORD ' "$R") RECORD"
}
trap teardown EXIT

# --- (b) create the singleton (no arguments) ---
if run_cmd create.txt restool dpdbg create; then
  log "PASS (b) restool dpdbg create exited 0"
else
  log "FAIL (b) restool dpdbg create failed (see create.txt)"
fi
settle

# --- (c) MC face: the singleton lists in dprc show ---
show show-create.txt
if has_dpdbg "$RESULTS/show-create.txt"; then
  log "PASS (c) dpdbg.0 listed in dprc.1"
else
  log "FAIL (c) dpdbg.0 not listed in dprc.1 after create"
fi
log "RECORD (c) dpdbg.0 plugged column: $(plug_state "$RESULTS/show-create.txt")"

# --- (d) bus face: surface the node, predicting no route (vfio.sh (c)) ---
# DPDBG-I4 says a created dpdbg enumerates on the fsl-mc bus; which action
# makes its node appear is a board fact, not a prediction. Record the node
# right after create+settle, then try `dprc sync`, then a bus rescan, and
# record which (if any) route surfaced it.
node_present && route="create"
log "RECORD (d) dpdbg.0 bus node right after create+settle: $(node_present && echo present || echo absent)"
if [ -z "$route" ]; then
  run_cmd sync.txt restool dprc sync || true
  settle
  node_present && route="sync"
  log "RECORD (d) dpdbg.0 bus node after 'restool dprc sync': $(node_present && echo present || echo absent)"
fi
if [ -z "$route" ]; then
  echo "+ echo 1 > /sys/bus/fsl-mc/rescan"
  echo 1 > /sys/bus/fsl-mc/rescan 2>>"$RESULTS/rescan.log" || true
  settle
  node_present && route="rescan"
  log "RECORD (d) dpdbg.0 bus node after 'echo 1 > /sys/bus/fsl-mc/rescan': $(node_present && echo present || echo absent)"
fi
log "RECORD (d) route that surfaced the bus node: ${route:-none}"

# --- (e) the DPDBG-I4 sysfs face: node present and driver-less ---
# The kernel registers fsl_mc_bus_dpdbg_type but ships no driver, so a
# created dpdbg enumerates with no driver link (docs/baseline/dpdbg.md,
# Kernel-side).
if node_present && [ -z "$(driver_of)" ]; then
  log "PASS (e) DPDBG-I4: dpdbg.0 bus node present and holds no driver link"
else
  log "FAIL (e) DPDBG-I4 sysfs face: node present=$(node_present && echo yes || echo no), driver='$(driver_of)'"
fi
log "RECORD (e) dpdbg.0 modalias: $(cat "$DEV/$DPDBG/modalias" 2>/dev/null)"

# --- (f) never kernel-bindable, even plugged ---
# Plugging is the kernel's bind lever (docs/baseline/dprc.md), but no
# driver exists for the dpdbg bus type, so the plugged node still binds
# nothing. Pass on a driver-less node; fail naming any driver that appears.
run_cmd plug.txt restool dprc assign dprc.1 --object=dpdbg.0 --plugged=1 || true
settle
drv="$(driver_of)"
if [ -z "$drv" ]; then
  log "PASS (f) plugged dpdbg.0 still holds no driver link (no driver for the type)"
else
  log "FAIL (f) plugged dpdbg.0 bound driver '$drv'"
fi
show show-plug.txt
log "RECORD (f) dpdbg.0 plugged column read-back: $(plug_state "$RESULTS/show-plug.txt")"

# --- (g) unplug so the trace's final state is unplugged ---
if run_cmd unplug.txt restool dprc assign dprc.1 --object=dpdbg.0 --plugged=0; then
  log "PASS (g) restool dprc assign --plugged=0 exited 0"
else
  log "FAIL (g) restool dprc assign --plugged=0 failed (see unplug.txt)"
fi

echo "suite V-DPDBG-2 body complete; teardown destroys the singleton"
