#!/bin/sh
# suite: V-MVP-1
# class: link-signaling  (explicitly flagged run — the kernel port takes wired dpmac.7)
# HAND-AUTHORED — not emitted by dpaa2-verify. The steps are `dpaa2ctl`
# invocations (dry-run -> ensure -> probe), not model-trace restool verbs, so
# there is no generator to regenerate from. Edit directly.
#
# The first MVP workable setup converged from ONE intent file (pool-objects task
# 4.3, bead dpaa2-controlplane-960.12; system-integration req 1 "One intent file
# converges the MVP setup end to end"; pool-objects designs D10/D11). A single `dpaa2ctl
# ensure` over models/board/V-MVP-1/intent.toml converges TWO regimes at once:
#   - a RESERVED-KERNEL interface — the kernel dpni on the wired 10G dpmac.7 in
#     dprc.1 with its regime-derived root pool companions, kernel-attached and
#     brought to a real carrier witness (Scenario A);
#   - an ISOLATED USERSPACE tenant — the `router` child dprc.N populated with a
#     dpni + its derived companions on the unwired 25G dpmac.5 and VFIO-bound,
#     consumable by any userspace dataplane (Scenario A part 2).
# The convergence is idempotent and level-triggered (Scenario "fresh board");
# drift heals in both directions (Scenario "drift heals"); teardown returns the
# baseline bar the grow-only dpio seats (Scenario "teardown returns").
#
# The mixed-dpmac choice (kernel on wired dpmac.7, router on unwired dpmac.5) is
# the operator's decision for this suite; its fuller rationale lives in
# models/board/V-MVP-1/intent.toml. In one line: a live carrier witness is only
# observable on a kernel-bound netdev, so the kernel port takes the flagged wired
# 10G dpmac.7, while the userspace child runs no dataplane here and takes the
# unwired lifecycle-safe 25G dpmac.5 — never asserted link-up. The operand's
# derived counts are pinned offline by crates/dpaa2-tools/tests/vmvp_intents.rs:
# root 18 dpmcp / 2 dpbp / 32 dpcon / 16 dpio; router child 12 dpio / 6 dpcon /
# 2 dpbp / 1 dpmcp.
#
# OPERATOR, READ FIRST:
#   - Run as root — the ensure legs issue MC creates/destroys and a VFIO bind
#     that gate on CAP_NET_ADMIN (docs/baseline/mc-ioctl-policy.md).
#   - The peer port facing dpmac.7 MUST be admin-up before you start, or the
#     Scenario A carrier witness times out (V-LINK-4 shape): dpmac.7 sees no
#     light until the peer drives the link.
#   - The grow leg creates the kernel's full root pool; dpio seats are grow-only
#     (pool-objects design D4) and this suite CANNOT reclaim them at teardown — run V-MVP-1
#     LAST in a sitting and REBOOT after (ADR-0003 §7 recovery guarantee is the
#     backstop). The post-suite census will show the leaked dpio seats until
#     that reboot.
set -u
RESULTS="${1:?usage: $0 <results-dir>}"
case "$RESULTS" in /*) ;; *) RESULTS="$PWD/$RESULTS" ;; esac
mkdir -p "$RESULTS"

# --- repo-root cd (design D12; ADR-0003 §2) ---
# The dpaa2ctl steps name the operand relative to the repo root. SELF is resolved
# absolute BEFORE the cd so the total-deny self-check greps the right file.
SELF="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")"
cd "$(dirname "$SELF")/../../.." || { echo "refusing: cannot reach the repo root from $SELF" >&2; exit 1; }
INTENT="models/board/V-MVP-1/intent.toml"
[ -f "$INTENT" ] || { echo "refusing: not the repo root ($INTENT missing); run this script from its checkout" >&2; exit 1; }

# --- dpaa2ctl from this checkout's build output (design D12; ADR-0003 §2) ---
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

# --- kernel-log window ---
KMSG="dpaa2-verify V-MVP-1 pid $$"
echo "$KMSG start" > /dev/kmsg 2>/dev/null || true
save_dmesg() {
  dmesg 2>/dev/null | awk -v m="$KMSG start" 'w || index($0, m) { w = 1 } w' > "$RESULTS/dmesg.txt"
  [ -s "$RESULTS/dmesg.txt" ] || dmesg > "$RESULTS/dmesg.txt" 2>&1 || true
}

# --- independent safety self-check (ADR-0003 §4) ---
# dpmac.7 is permitted here under the flagged link-signaling class; dpmac.5 is
# lifecycle-only. The deny set is unchanged; the sysfs and census globs below use
# dpni.* and variables, never the forbidden zeroth-dpni literal.
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
expect_nonzero() { rc=$(cat "$RESULTS/step-$1-exit.txt"); if [ "$rc" != 0 ]; then echo "PASS step $1: $2 (refused, exit $rc)"; else echo "FAIL step $1: $2 (exit 0 — the refusal did not fire)" >&2; fi; }
# expect_out N needle label: PASS iff step N's captured output carries the needle.
expect_out() { if grep -qF "$2" "$RESULTS/step-$1-out.txt" "$RESULTS/step-$1-err.txt" 2>/dev/null; then echo "PASS step $1: $3"; else echo "FAIL step $1: $3 (missing '$2')" >&2; fi; }
# expect_line N regex label: PASS iff a captured line of step N matches the regex.
# The census regexes anchor the ROOT block by its 2-space indent (render.rs line 260),
# so a converged child family (4-space indent, render.rs line 360) cannot false-PASS.
expect_line() { if grep -qE "$2" "$RESULTS/step-$1-out.txt" "$RESULTS/step-$1-err.txt" 2>/dev/null; then echo "PASS step $1: $3"; else echo "FAIL step $1: $3 (no line matching /$2/)" >&2; fi; }
# count_family file fam: how many `fam.N` object rows a capture holds.
count_family() { awk -v f="$2" '$1 ~ "^" f "\\." { c++ } END { print c+0 }' "$RESULTS/$1" 2>/dev/null; }
# expect_family file fam want label: PASS iff the capture holds exactly `want` rows of `fam`.
expect_family() { got=$(count_family "$1" "$2"); if [ "$got" = "$3" ]; then echo "PASS: $4 ($2 = $3)"; else echo "FAIL: $4 ($2 = $got, want $3)" >&2; fi; }

# --- reference pair assertion (ADR-0003 §2) ---
mc="$(restool -m 2>/dev/null || true)"
case "$mc" in *10.39.0*) ;; *) echo "refusing: MC firmware is not 10.39.0: $mc" >&2; exit 1 ;; esac
kernel="$(uname -r)"
case "$kernel" in 6.6.52*) ;; *) echo "refusing: kernel is not 6.6.52: $kernel" >&2; exit 1 ;; esac

# --- unconditional teardown (ADR-0003 §6/§7) — the backstop, idempotent with step 9 ---
# Reconcile both regimes back to the baseline through the reconciler's own reverse
# path: an empty intent derives zero, and ensure runs the grow-first / shrink-last
# walk — consumers (the kernel dpni, the populated child dprc) are torn down FIRST,
# which releases their draws, so the shrink half then reclaims every free-managed
# trio object to 0 through the unplug probe. A foreign object left standing is
# destroyed best-effort. dpio seats are grow-only and NOT reclaimed here — the
# closing reboot restores them (ADR-0003 §7). The empty intent is written to the
# results dir so no throwaway operand is committed.
teardown() {
  printf '[intent]\nschema = 1\n' > "$RESULTS/teardown-empty.toml"
  "$DPAA2CTL" --config "$RESULTS/teardown-empty.toml" ensure --prune --allow disruptive --no-link \
    > "$RESULTS/teardown-ensure.txt" 2>>"$RESULTS/teardown.log" || true
  sleep 2
  if [ -n "${FOREIGN:-}" ]; then
    restool dpbp info "$FOREIGN" > /dev/null 2>&1 && restool dpbp destroy "$FOREIGN" 2>>"$RESULTS/teardown.log" || true
  fi
  pool_capture pool-teardown.txt
  save_dmesg
  echo "teardown: kernel dpni + child dprc + managed trio reconciled to empty; dpio seats are grow-only — reboot to reclaim (ADR-0003 §7)"
}
trap teardown EXIT

# --- baseline (ADR-0011) ---
pool_capture pool-baseline.txt

# step 0: pre-suite census — the baseline the post-suite state is diffed against.
run 0 restool dprc list
expect_zero 0 "pre-suite census: container list"
probe step-0-show-dprc1.txt restool dprc show dprc.1

# step 1: dry-run the intent — headlines the convergence plan (root pool grow +
# the router child population), dispatching nothing.
run 1 "$DPAA2CTL" --config "$INTENT" dry-run
expect_zero 1 "dry-run headlines the MVP convergence"
expect_out 1 "root pool convergence" "dry-run prints the root pool convergence block"
expect_out 1 "child population" "dry-run prints the router child population block"

# step 2: hitless-refusal leg — ensure WITHOUT --allow. Growing the root pool is
# Class::Disruptive, so the grow half refuses and changes nothing (exit FAILURE).
run 2 "$DPAA2CTL" --config "$INTENT" ensure --no-link
expect_nonzero 2 "hitless ensure refuses the disruptive root pool grow"
expect_out 2 "refused: root pool grow is" "the refusal names the root pool grow class"

# step 3: converge under the disruptive gate — grows the root pool, creates the
# kernel dpni on dpmac.7, creates + populates + VFIO-binds the router child dprc.
run 3 "$DPAA2CTL" --config "$INTENT" ensure --no-link --allow disruptive
expect_zero 3 "ensure --allow disruptive converges both regimes"
probe step-3-list.txt restool dprc list
probe step-3-show-dprc1.txt restool dprc show dprc.1
# Capture the new router child's board id for the later checks and teardown proof.
CHILD="$(restool dprc show dprc.1 2>/dev/null | awk '$1 ~ /^dprc\./ && $2 == "router" { print $1; exit }')"
echo "child dprc: ${CHILD:-<none>}" | tee "$RESULTS/step-3-child.txt"
if [ -n "$CHILD" ]; then echo "PASS step 3: the router child container exists ($CHILD)"; else echo "FAIL step 3: no router-labelled child dprc after convergence" >&2; fi
[ -n "$CHILD" ] && probe step-3-show-child.txt restool dprc show "$CHILD"
pool_capture pool-after-converge.txt

# step 4: Scenario A liveness — the kernel dpni is kernel-attached and brought to
# a real carrier witness. Find the dpni whose endpoint is dpmac.7, its netdev, and
# confirm the dpaa2-eth bind; then admin-up and WAIT (bounded, ~60s) until the
# kernel carrier flag and restool's link read-back AGREE (V-LINK-2 acknowledgment).
KERN_DPNI=""
for d in /sys/bus/fsl-mc/devices/dpni.*; do
  [ -e "$d" ] || continue
  n="$(basename "$d")"
  if restool dpni info "$n" 2>/dev/null | grep -qE 'dpmac\.7($|[^0-9])'; then KERN_DPNI="$n"; break; fi
done
echo "kernel dpni on dpmac.7: ${KERN_DPNI:-<none>}" | tee "$RESULTS/step-4-kern-dpni.txt"
if [ -n "$KERN_DPNI" ]; then
  echo "PASS step 4: the kernel dpni terminating dpmac.7 exists ($KERN_DPNI)"
  drv="$(readlink "/sys/bus/fsl-mc/devices/$KERN_DPNI/driver" 2>/dev/null)"
  case "$drv" in *fsl_dpaa2_eth) echo "PASS step 4: $KERN_DPNI is bound to dpaa2-eth (kernel-attached)";;
                 *) echo "FAIL step 4: $KERN_DPNI is not dpaa2-eth-bound (driver=${drv:-none})" >&2;; esac
  IF="$(ls "/sys/bus/fsl-mc/devices/$KERN_DPNI/net/" 2>/dev/null | head -1)"
  echo "netdev: ${IF:-<none>}" | tee "$RESULTS/step-4-netdev.txt"
  if [ -n "$IF" ]; then
    echo "PASS step 4: $KERN_DPNI exposes netdev $IF"
    ip link set "$IF" up 2>>"$RESULTS/step-4.log" || true
    i=0
    while [ "$i" -lt 60 ]; do
      c="$(cat "/sys/class/net/$IF/carrier" 2>/dev/null || echo 0)"
      # split on [:-]: restool renders `link status: N - word`; a bare -F: yields `1-up`, so the [ "$l" = 1 ] compare would silently never match.
      l="$(restool dpni info "$KERN_DPNI" 2>/dev/null | awk -F'[:-]' 'tolower($0) ~ /link status/ { gsub(/ /, "", $2); print $2 }')"
      if [ "$c" = 1 ] && [ "$l" = 1 ]; then break; fi
      sleep 1; i=$((i + 1))
    done
    { echo "carrier=$c dpni-link-status=$l after ${i}s"; } | tee "$RESULTS/step-4-link.txt"
    if [ "$c" = 1 ] && [ "$l" = 1 ]; then
      echo "PASS step 4: link-connected — kernel carrier and restool read-back agree (V-LINK-2 acknowledgment)"
    else
      echo "FAIL step 4: link witness timed out (carrier=$c link=$l) — is the peer port facing dpmac.7 admin-up?" >&2
    fi
  else
    echo "FAIL step 4: $KERN_DPNI exposes no netdev" >&2
  fi
else
  echo "FAIL step 4: no dpni terminates dpmac.7 after convergence" >&2
fi

# step 5: Scenario A part 2 — the router child is VFIO-bound and fully populated.
if [ -n "$CHILD" ]; then
  cdrv="$(readlink "/sys/bus/fsl-mc/devices/$CHILD/driver" 2>/dev/null)"
  echo "child driver: ${cdrv:-<none>}" | tee "$RESULTS/step-5-driver.txt"
  case "$cdrv" in *vfio-fsl-mc) echo "PASS step 5: $CHILD is VFIO-bound (driver ends in vfio-fsl-mc)";;
                  *) echo "FAIL step 5: $CHILD is not vfio-fsl-mc-bound (driver=${cdrv:-none})" >&2;; esac
  probe step-5-show-child.txt restool dprc show "$CHILD"
  expect_family step-5-show-child.txt dpni 1 "step 5: child holds its dpni"
  expect_family step-5-show-child.txt dpio 12 "step 5: child holds 12 dpio seats"
  expect_family step-5-show-child.txt dpcon 6 "step 5: child holds 6 dpcon"
  expect_family step-5-show-child.txt dpbp 2 "step 5: child holds 2 dpbp"
  expect_family step-5-show-child.txt dpmcp 1 "step 5: child holds 1 dpmcp"
else
  echo "FAIL step 5: no child dprc to probe for the VFIO bind and population" >&2
fi

# step 6: idempotence — a re-run dry-run plans zero (an all-hitless, empty plan),
# and a second ensure dispatches nothing (Converged, level-triggered).
run 6 "$DPAA2CTL" --config "$INTENT" dry-run
expect_zero 6 "post-converge dry-run: the board is converged"
expect_out 6 "0 planned transition(s)" "the converged dry-run plans zero port transitions"
run 7 "$DPAA2CTL" --config "$INTENT" ensure --no-link --allow disruptive
expect_zero 7 "second ensure dispatches nothing (idempotent, no-op re-run)"

# step 8: drift surplus — a foreign, undeclared, free dpbp in the root, the prune
# target (V-POOL-6 step-6 shape). FOREIGN is exported for the teardown.
FOREIGN="$(restool --script dpbp create 2>"$RESULTS/step-8-foreign-err.txt")"
echo "$FOREIGN" > "$RESULTS/step-8-foreign.txt"
if [ -n "$FOREIGN" ]; then echo "PASS step 8: foreign root dpbp created ($FOREIGN)"; else echo "FAIL step 8: could not create the foreign dpbp" >&2; fi
sleep 2   # space the disruptive edge (ADR-0008 §6)
run 8 "$DPAA2CTL" --config "$INTENT" ensure --no-link --allow disruptive
expect_zero 8 "ensure prunes the foreign dpbp and restores the derived counts"
run 9 restool dpbp info "$FOREIGN"
if [ "$(cat "$RESULTS/step-9-exit.txt")" != 0 ]; then
  echo "PASS step 8: the foreign dpbp $FOREIGN was pruned (info reports it absent)"
else
  echo "FAIL step 8: the foreign dpbp $FOREIGN still exists after the prune" >&2
fi
pool_capture pool-after-surplus-heal.txt
probe step-8-show-dprc1.txt restool dprc show dprc.1
# Counts back to the derived base — witnessed by the tool's OWN census, not a raw
# `dprc show` row count (DPL-born rows carry no managed label, so a bare count would
# fold them in). A converged dry-run reports dpbp at required=2 with zero deltas.
# The teardown's info guard skips a FOREIGN already pruned here, so it is not cleared.
run 12 "$DPAA2CTL" --config "$INTENT" dry-run
expect_zero 12 "post-surplus-heal dry-run reads the converged pool"
expect_line 12 '^  dpbp observed managed=[0-9]+/[0-9]+ required=2 \[hitless\] create=0 destroy=0 prune=0' "root dpbp converged to the derived 2 (surplus + foreign gone)"

# step 9 (drift deficit): destroy ONE free managed root-pool object out of band,
# then heal. Drawn-ness has no `dprc show` column (dpaa2-mc restool.rs: "the shim
# reports undrawn, the unplug probe discovers the real draw"), so the victim is
# selected with the tool's OWN unplug probe: walk the labelled (managed) dpcon rows
# high-index first — a labelled row (NF==3: object + label + plugged-state) is a
# managed pool object, an unlabelled row (NF==2) is DPL-born and never a target —
# and `assign --plugged=0` each: a free object unplugs, a drawn one refuses (-EBUSY,
# the ShrinkBelowDraw signal) and is skipped, so only a free managed object is
# destroyed. dpcon carries the widest free headroom of the trio (32 derived, ~16
# drawn), so a free victim is found. High-index-first keeps clear of any low-ordinal
# DPL-born object; dpbp.0 is never touched.
VICTIM=""
for obj in $(restool dprc show dprc.1 2>/dev/null | awk 'NF==3 && $3=="plugged" && $1 ~ /^dpcon\./ { print $1 }' | sort -t. -k2 -nr); do
  if restool dprc assign dprc.1 --object="$obj" --plugged=0 2>>"$RESULTS/step-9.log"; then
    if restool dpcon destroy "$obj" 2>>"$RESULTS/step-9.log"; then VICTIM="$obj"; break; fi
    restool dprc assign dprc.1 --object="$obj" --plugged=1 2>>"$RESULTS/step-9.log" || true  # destroy failed: re-plug
  fi
done
echo "deficit victim: ${VICTIM:-<none>}" | tee "$RESULTS/step-9-victim.txt"
if [ -n "$VICTIM" ]; then echo "PASS step 9: destroyed one free managed dpcon out of band ($VICTIM)"; else echo "FAIL step 9: found no free managed dpcon to destroy" >&2; fi
sleep 2   # space the disruptive edge before the heal (ADR-0008 §6)
run 10 "$DPAA2CTL" --config "$INTENT" ensure --no-link --allow disruptive
expect_zero 10 "ensure recreates the deficit — root dpcon back to the derived count"
probe step-9-show-dprc1.txt restool dprc show dprc.1
# Heal witnessed by the tool's census: dpcon recreated to 32, dpbp/dpmcp untouched.
run 13 "$DPAA2CTL" --config "$INTENT" dry-run
expect_zero 13 "post-deficit-heal dry-run reads the converged pool"
expect_line 13 '^  dpcon observed managed=[0-9]+/[0-9]+ required=32 \[hitless\] create=0 destroy=0 prune=0' "step 9: root dpcon healed to the derived 32 (deficit recreated)"
expect_line 13 '^  dpbp observed managed=[0-9]+/[0-9]+ required=2 \[hitless\] create=0 destroy=0 prune=0' "step 9: root dpbp untouched at the derived 2"
expect_line 13 '^  dpmcp observed managed=[0-9]+/[0-9]+ required=18 \[hitless\] create=0 destroy=0 prune=0' "step 9: root dpmcp untouched at the derived 18"
if grep -qE '(^| )dpbp\.0( |$)' "$RESULTS/step-9-show-dprc1.txt" 2>/dev/null; then
  echo "PASS step 9: the DPL-born dpbp.0 is untouched (present after the heal)"
else
  echo "RECORD step 9: dpbp.0 not seen in the root listing — confirm the DPL-born boot pool from the census diff"
fi
pool_capture pool-after-deficit-heal.txt

# step 10 (teardown scenario — the ASSERTED leg): empty intent + prune tears both
# regimes down. Expect the kernel dpni gone (netdev absent), the child dprc gone
# from the root listing, the runtime trio reclaimed, and the typed dpio residue
# printed (residue:) — the grow-only seats a teardown cannot reclaim (ADR-0003 §7).
printf '[intent]\nschema = 1\n' > "$RESULTS/step-10-empty.toml"
run 11 "$DPAA2CTL" --config "$RESULTS/step-10-empty.toml" ensure --prune --allow disruptive --no-link
expect_zero 11 "empty-intent ensure tears both regimes down"
expect_out 11 "residue:" "teardown reports the typed grow-only dpio residue (ADR-0003 §7)"
sleep 2
# Kernel dpni gone — its sysfs device (and netdev) is absent.
if [ -n "$KERN_DPNI" ] && [ ! -e "/sys/bus/fsl-mc/devices/$KERN_DPNI" ]; then
  echo "PASS step 10: the kernel dpni $KERN_DPNI is gone (sysfs device absent)"
elif [ -n "$KERN_DPNI" ]; then
  echo "FAIL step 10: the kernel dpni $KERN_DPNI survives teardown" >&2
fi
# Child dprc gone from the root listing.
probe step-10-show-dprc1.txt restool dprc show dprc.1
if [ -n "$CHILD" ] && ! grep -qE "(^| )$CHILD( |\$)" "$RESULTS/step-10-show-dprc1.txt" 2>/dev/null; then
  echo "PASS step 10: the router child $CHILD is gone from dprc.1"
elif [ -n "$CHILD" ]; then
  echo "FAIL step 10: the router child $CHILD survives teardown" >&2
fi
# Runtime trio (every managed dpmcp/dpbp/dpcon) reclaimed to the DPL baseline —
# witnessed offline by the census diff below (a raw row count would fold in DPL-born
# rows). Only the grow-only dpio seats remain, which the closing reboot restores.
pool_capture pool-post.txt
echo "census diff vs step 0: only the grow-only dpio seats should remain (pool-objects design D4); the closing reboot restores them (ADR-0003 §7)"

echo "suite V-MVP-1 MVP end-to-end convergence complete"
