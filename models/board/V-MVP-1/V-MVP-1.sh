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
  # Clear BOTH step-8 foreigns on every abort path (info-guarded: a stray already pruned by
  # the ensure is skipped; the bare survivor is destroyed here — born unplugged, unlabeled).
  for f in "${FOREIGN:-}" "${FOREIGN_STRAY:-}"; do
    [ -n "$f" ] || continue
    restool dpbp info "$f" > /dev/null 2>&1 && restool dpbp destroy "$f" 2>>"$RESULTS/teardown.log" || true
  done
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

# step 8 (drift surplus): TWO foreign root dpbps exercise the one-label law under
# ADR-0020 decision 4. (1) FOREIGN_STRAY carries an undeclared label 'stray' ⇒ Foreign,
# and a fresh create is born unplugged, so root prune REACHES it (ADR-0020 decision 4;
# pool_lifecycle.rs foreign_free_unplugged). (2) FOREIGN is bare/unlabeled ⇒ the empty
# label is the DPL sentinel, prune-EXEMPT everywhere (one-label law; pool_lifecycle.rs:767
# membership) ⇒ it SURVIVES the ensure and only the EXIT trap clears it. Both are exported
# for the trap (it destroys each on every abort path, info-guarded).
FOREIGN_STRAY="$(restool --script dpbp create 2>"$RESULTS/step-8-stray-err.txt")"
echo "$FOREIGN_STRAY" > "$RESULTS/step-8-stray.txt"
[ -n "$FOREIGN_STRAY" ] && restool dprc set-label "$FOREIGN_STRAY" --label=stray 2>>"$RESULTS/step-8-stray-err.txt" || true
if [ -n "$FOREIGN_STRAY" ]; then echo "PASS step 8: labeled foreign root dpbp created ($FOREIGN_STRAY, label=stray)"; else echo "FAIL step 8: could not create the labeled foreign dpbp" >&2; fi
FOREIGN="$(restool --script dpbp create 2>"$RESULTS/step-8-foreign-err.txt")"
echo "$FOREIGN" > "$RESULTS/step-8-foreign.txt"
if [ -n "$FOREIGN" ]; then echo "PASS step 8: bare unlabeled foreign root dpbp created ($FOREIGN)"; else echo "FAIL step 8: could not create the bare foreign dpbp" >&2; fi
sleep 2   # space the disruptive edge (ADR-0008 §6)
run 8 "$DPAA2CTL" --config "$INTENT" ensure --no-link --allow disruptive
expect_zero 8 "ensure prunes the labeled foreign and leaves the derived counts"
run 9 restool dpbp info "$FOREIGN_STRAY"
if [ "$(cat "$RESULTS/step-9-exit.txt")" != 0 ]; then
  echo "PASS step 8: the labeled foreign dpbp $FOREIGN_STRAY was pruned (info reports it absent, ADR-0020 decision 4)"
else
  echo "FAIL step 8: the labeled foreign dpbp $FOREIGN_STRAY still exists after the prune" >&2
fi
# The bare unlabeled foreign is the empty-label DPL sentinel: prune-EXEMPT (one-label law),
# so it SURVIVES the ensure — info still succeeds. The EXIT trap clears it (born unplugged).
run 14 restool dpbp info "$FOREIGN"
expect_zero 14 "the bare unlabeled foreign $FOREIGN survives the ensure (empty-label DPL sentinel is prune-exempt, one-label law)"
pool_capture pool-after-surplus-heal.txt
probe step-8-show-dprc1.txt restool dprc show dprc.1
# Counts back to the derived base — witnessed by the tool's OWN census, not a raw `dprc show`
# row count (DPL-born rows, incl. the bare survivor, carry no managed label). A converged
# dry-run reports dpbp at required=2 with zero deltas; the pruned stray leaves prune=0.
run 12 "$DPAA2CTL" --config "$INTENT" dry-run
expect_zero 12 "post-surplus-heal dry-run reads the converged pool"
expect_line 12 '^  Dpbp observed managed=[0-9]+/[0-9]+ required=2 \[hitless\] create=0 destroy=0 prune=0' "root dpbp converged to the derived 2 (stray pruned, bare survivor DPL-exempt)"

# step 9 (drift surplus residue — the grow-only leg, ADR-0020 decisions 1-2): a root
# managed surplus is REPORTED as reboot-required residue, never reclaimed at runtime. A root
# deficit cannot be manufactured — every plugged root object is fsl_mc_allocator-bound and
# restool refuses the client-side unplug, so the old out-of-band victim walk found no victim
# (rev4, recorded — not re-derived here). Instead grow the root ABOVE intent with
# intent-highwater (+2 dpcon), then reconcile the PLAIN intent back down: the surplus is not
# destroyed (root grow-only, destroy=0), it renders as typed residue. The surplus persists to
# the closing reboot (this suite runs LAST, the sitting reboots after; ADR-0003 §7).
run 10 "$DPAA2CTL" --config models/board/V-MVP-1/intent-highwater.toml ensure --no-link --allow disruptive
expect_zero 10 "ensure intent-highwater grows the root dpcon +2 above the derived 32"
sleep 2   # space the disruptive edge (ADR-0008 §6)
probe step-9-show-dprc1.txt restool dprc show dprc.1
# dry-run the PLAIN intent: dpcon managed sits ABOVE required 32, so the 4-space reboot-
# required line fires (PoolResidue Display) and the census shows destroy=0 (grow-only, no
# runtime reclaim).
run 13 "$DPAA2CTL" --config "$INTENT" dry-run
expect_zero 13 "post-highwater dry-run reads the plain intent"
expect_line 13 '^    reboot-required: Dpcon pool: observed [0-9]+ exceed the required 32 ' "root dpcon surplus renders as reboot-required residue (ADR-0020 decisions 1-2)"
expect_line 13 '^  Dpcon observed managed=[0-9]+/[0-9]+ required=32 \[hitless\] create=0 destroy=0 prune=0' "root dpcon surplus is NOT reclaimed (destroy=0, grow-only)"
sleep 2   # space the disruptive edge (ADR-0008 §6)
# ensure the PLAIN intent: exits zero (residue is a report, not a failure) and prints the
# no-indent `residue:` marker (main.rs); the managed dpcon census stays at the grown count.
run 15 "$DPAA2CTL" --config "$INTENT" ensure --no-link --allow disruptive
expect_zero 15 "ensure plain intent exits zero over the root dpcon surplus (residue is a report, not a failure)"
expect_out 15 "residue: Dpcon pool:" "ensure prints the typed dpcon reboot-required residue (ADR-0020 decision 2)"
probe step-9-show-dprc1-after.txt restool dprc show dprc.1
if grep -qE '(^| )dpbp\.0( |$)' "$RESULTS/step-9-show-dprc1.txt" 2>/dev/null; then
  echo "PASS step 9: the DPL-born dpbp.0 is untouched (present after the residue leg)"
else
  echo "RECORD step 9: dpbp.0 not seen in the root listing — confirm the DPL-born boot pool from the census diff"
fi
pool_capture pool-after-surplus-residue.txt

# step 10 (teardown scenario — the ASSERTED leg): empty intent + prune tears both
# regimes down. Expect the kernel dpni gone (netdev absent), the child dprc gone from
# the root listing, and — root being grow-only — the managed trio AND the dpio seats
# printed as typed reboot-required residue, not reclaimed (ADR-0020 decision 2; ADR-0003 §7).
printf '[intent]\nschema = 1\n' > "$RESULTS/step-10-empty.toml"
run 11 "$DPAA2CTL" --config "$RESULTS/step-10-empty.toml" ensure --prune --allow disruptive --no-link
expect_zero 11 "empty-intent ensure tears both regimes down"
# Grow-only capacity a teardown cannot return renders as typed residue, observed vs required 0
# (ADR-0020 decision 2). A residue fires per family that holds labeled + plugged capacity now;
# the empty-label DPL boot pool stays silent (engine.rs residue() reads labeled_plugged).
expect_out 11 "residue: Dpcon pool:" "dpcon grew to 34 in the step-9 highwater leg, labeled + plugged ⇒ residue"
expect_out 11 "residue: Dpmcp pool:" "the 18 managed dpmcp are labeled + plugged ⇒ residue"
expect_out 11 "residue: Dpbp pool:" "the 2 managed dpbp are labeled + plugged (the bare survivor is unlabeled ⇒ silent) ⇒ residue"
expect_out 11 "dpio seats:" "teardown reports the grow-only dpio seat residue (ADR-0003 §7)"
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
# The managed trio (labeled + plugged) and the grow-only dpio seats are NOT reclaimed at
# runtime (ADR-0020 decision 2); they remain as typed reboot-required residue, witnessed
# offline by the census diff below (a raw row count would fold in the DPL-born rows).
pool_capture pool-post.txt
echo "census diff vs step 0: the managed trio capacity plus the grow-only dpio seats remain as reboot-required residue (ADR-0020 decision 2); the closing reboot restores the DPL baseline (ADR-0003 §7)"

echo "suite V-MVP-1 MVP end-to-end convergence complete"
