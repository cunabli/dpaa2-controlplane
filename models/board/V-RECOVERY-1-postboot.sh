#!/bin/sh
# suite: V-RECOVERY-1 post-boot diff
# Run AFTER the reboot that follows V-RECOVERY-1.sh, with the same results
# directory. Diffs post-boot state against the pre-mutation capture: the
# reboot must have erased the scratch set and restored the DPL boot
# state. Whether the change-#1 DPL baseline capture also matches
# pre-dpl.dts settles the design's open question on the diff reference;
# if that capture is at hand, compare it too.
set -u
RESULTS="${1:?usage: $0 <results-dir>}"
[ -r "$RESULTS/pre-dpl.dts" ] || { echo "refusing: no pre-state capture in $RESULTS" >&2; exit 1; }

# --- reference pair assertion (ADR-0003 §2) ---
# Evidence is only valid against the stamped pair; refuse anything else.
mc="$(restool -m 2>/dev/null || true)"
case "$mc" in *10.39.0*) ;; *) echo "refusing: MC firmware is not 10.39.0: $mc" >&2; exit 1 ;; esac
kernel="$(uname -r)"
case "$kernel" in 6.6.52*) ;; *) echo "refusing: kernel is not 6.6.52: $kernel" >&2; exit 1 ;; esac

restool dprc list                > "$RESULTS/post-dprc-list.txt"
restool dprc show dprc.1         > "$RESULTS/post-dprc1-show.txt"
restool dprc generate-dpl dprc.1 > "$RESULTS/post-dpl.dts"

status=0
diff -u "$RESULTS/pre-dpl.dts" "$RESULTS/post-dpl.dts"               > "$RESULTS/recovery-diff.txt" || status=1
diff -u "$RESULTS/pre-dprc-list.txt" "$RESULTS/post-dprc-list.txt"  >> "$RESULTS/recovery-diff.txt" || status=1
diff -u "$RESULTS/pre-dprc1-show.txt" "$RESULTS/post-dprc1-show.txt" >> "$RESULTS/recovery-diff.txt" || status=1
if [ "$status" = 0 ]; then
  echo "recovery diff clean: the reboot restored the pre-mutation state"
  echo "guarantee verified - commit models/board/RECOVERY-VERIFIED to unblock mutating suites"
else
  echo "RECOVERY DIFF NOT CLEAN - board program stops here (design D7); see $RESULTS/recovery-diff.txt" >&2
fi
exit "$status"
