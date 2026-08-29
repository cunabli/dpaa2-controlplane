# Shared boot-resident abort rule for the sitting-5.11 suite hooks
# (models/board/README.md, "Rule shared by every hook here"; ADR-0008
# §4–§5, §8). A hook sources this file, calls residents_pre before its
# first face and residents_post after its last: a boot resident whose
# driver link differs from the clean-boot reference means the sitting must
# stop and reboot.
#
# The comparison is against the *reference*, not against a before-image of
# this same hook (ADR-0008 §8). A driver can be lost during a suite's own
# trace steps, before the hook's first read — a self-comparison passes
# that (pre and post both show the loss); a reference comparison fails it
# on the first read. residents_pre therefore fails immediately if any
# resident already sits off the driver the clean-boot reference records.
#
# REF_DRIVERS is every object models/board/baselines/reference.json records
# with a non-null driver (its clean-boot bound set), as "<obj> <driver>"
# pairs, hardcoded here because the board carries no jq/python. The three
# total-deny names (dpmac.3, dpmac.17, dpni.0) are deliberately absent:
# ADR-0003 §4 forbids naming them in any scenario, and dpmac.17 is unbound
# at boot anyway; a driver change on the two bound ones (dpmac.3, dpni.0)
# is caught by the closing snapshot diff instead. Regenerate this list if
# the clean-boot reference changes.
REF_DRIVERS="
  dpbp.0 fsl_mc_allocator
  dpcon.0 fsl_mc_allocator dpcon.1 fsl_mc_allocator dpcon.2 fsl_mc_allocator
  dpcon.3 fsl_mc_allocator dpcon.4 fsl_mc_allocator dpcon.5 fsl_mc_allocator
  dpcon.6 fsl_mc_allocator dpcon.7 fsl_mc_allocator dpcon.8 fsl_mc_allocator
  dpcon.9 fsl_mc_allocator dpcon.10 fsl_mc_allocator dpcon.11 fsl_mc_allocator
  dpcon.12 fsl_mc_allocator dpcon.13 fsl_mc_allocator dpcon.14 fsl_mc_allocator
  dpcon.15 fsl_mc_allocator
  dpio.0 fsl_mc_dpio dpio.1 fsl_mc_dpio dpio.2 fsl_mc_dpio dpio.3 fsl_mc_dpio
  dpio.4 fsl_mc_dpio dpio.5 fsl_mc_dpio dpio.6 fsl_mc_dpio dpio.7 fsl_mc_dpio
  dpio.8 fsl_mc_dpio dpio.9 fsl_mc_dpio dpio.10 fsl_mc_dpio dpio.11 fsl_mc_dpio
  dpio.12 fsl_mc_dpio dpio.13 fsl_mc_dpio dpio.14 fsl_mc_dpio dpio.15 fsl_mc_dpio
  dpmac.4 fsl_dpaa2_mac dpmac.5 fsl_dpaa2_mac dpmac.6 fsl_dpaa2_mac
  dpmac.7 fsl_dpaa2_mac dpmac.8 fsl_dpaa2_mac dpmac.9 fsl_dpaa2_mac
  dpmac.10 fsl_dpaa2_mac
  dpmcp.1 fsl_mc_allocator dpmcp.2 fsl_mc_allocator dpmcp.3 fsl_mc_allocator
  dpmcp.4 fsl_mc_allocator dpmcp.5 fsl_mc_allocator dpmcp.6 fsl_mc_allocator
  dpmcp.7 fsl_mc_allocator dpmcp.8 fsl_mc_allocator dpmcp.9 fsl_mc_allocator
  dpmcp.10 fsl_mc_allocator dpmcp.11 fsl_mc_allocator dpmcp.12 fsl_mc_allocator
  dpmcp.13 fsl_mc_allocator dpmcp.14 fsl_mc_allocator dpmcp.15 fsl_mc_allocator
  dpmcp.16 fsl_mc_allocator dpmcp.17 fsl_mc_allocator dpmcp.18 fsl_mc_allocator
  dpmcp.19 fsl_mc_allocator dpmcp.20 fsl_mc_allocator dpmcp.21 fsl_mc_allocator
  dpmcp.22 fsl_mc_allocator dpmcp.23 fsl_mc_allocator dpmcp.24 fsl_mc_allocator
  dpmcp.25 fsl_mc_allocator dpmcp.26 fsl_mc_allocator dpmcp.27 fsl_mc_allocator
  dpmcp.28 fsl_mc_allocator dpmcp.29 fsl_mc_allocator dpmcp.30 fsl_mc_allocator
  dpmcp.31 fsl_mc_allocator dpmcp.32 fsl_mc_allocator dpmcp.33 fsl_mc_allocator
  dpmcp.34 fsl_mc_allocator dpmcp.35 fsl_mc_allocator dpmcp.36 fsl_mc_allocator
  dpmcp.37 fsl_mc_allocator dpmcp.38 fsl_mc_allocator dpmcp.39 fsl_mc_allocator
  dpmcp.40 fsl_mc_allocator dpmcp.41 fsl_mc_allocator dpmcp.42 fsl_mc_allocator
  dpmcp.43 fsl_mc_allocator dpmcp.44 fsl_mc_allocator dpmcp.45 fsl_mc_allocator
  dpmcp.46 fsl_mc_allocator dpmcp.47 fsl_mc_allocator dpmcp.48 fsl_mc_allocator
  dpmcp.49 fsl_mc_allocator dpmcp.50 fsl_mc_allocator dpmcp.51 fsl_mc_allocator
  dpmcp.52 fsl_mc_allocator
  dprtc.0 fsl_dpaa2_ptp dpseci.0 dpaa2_caam
"

# resident_driver OBJ: the basename of the object's sysfs driver link, or
# "unbound" when it holds none.
resident_driver() {
  drv="$(readlink "/sys/bus/fsl-mc/devices/$1/driver" 2>/dev/null)"
  name="${drv##*/}"
  [ -n "$name" ] || name=unbound
  echo "$name"
}

# residents_snapshot FILE: one "<obj> <ref-driver> <observed-driver>" line
# per resident, the observed driver read from its sysfs link.
residents_snapshot() {
  out="$1"
  : > "$out"
  set -- $REF_DRIVERS
  while [ "$#" -ge 2 ]; do
    o="$1"; ref="$2"; shift 2
    echo "$o $ref $(resident_driver "$o")" >> "$out"
  done
}

# residents_check LABEL FILE: compare every resident's observed driver
# against the reference driver; one "FAIL residents: ..." line per
# mismatch, or one "PASS residents <label>" line if all match. Returns
# nonzero if any resident is off its reference driver.
# A hook whose own trace evicts a resident's driver on purpose (a dpni
# bound on a wired dpmac evicts fsl_dpaa2_mac, ADR-0008 §8) names it in
# RESIDENTS_EXPECT_UNBOUND before residents_pre: that object is expected
# unbound for the run and recorded, not failed; the snapshot after the
# teardown is what proves the driver came back.
RESIDENTS_EXPECT_UNBOUND="${RESIDENTS_EXPECT_UNBOUND:-}"
expected_unbound() { case " $RESIDENTS_EXPECT_UNBOUND " in *" $1 "*) return 0;; *) return 1;; esac; }

residents_check() {
  label="$1"; file="$2"
  residents_snapshot "$file"
  mismatched=0
  while read -r o ref got; do
    [ -n "$o" ] || continue
    if expected_unbound "$o"; then
      echo "RECORD residents: $o expected unbound for this run ($ref -> $got)" | tee -a "$RESULTS/residents.txt"
      [ "$got" = unbound ] || { echo "FAIL residents: $o expected unbound, holds $got" | tee -a "$RESULTS/residents.txt"; mismatched=1; }
      continue
    fi
    if [ "$ref" != "$got" ]; then
      echo "FAIL residents: $o off its reference driver ($ref -> $got)" | tee -a "$RESULTS/residents.txt"
      mismatched=1
    fi
  done < "$file"
  [ "$mismatched" = 0 ] && echo "PASS residents $label" | tee -a "$RESULTS/residents.txt"
  return "$mismatched"
}

# residents_pre: the before-image, checked against the reference. Fails on
# the first read: a resident already off its reference driver when the
# hook starts is a loss the suite's own trace steps caused (ADR-0008 §8).
residents_pre() { residents_check pre "$RESULTS/residents-pre.txt"; }

# residents_post: the after-image, checked against the same reference.
residents_post() { residents_check post "$RESULTS/residents-post.txt"; }
