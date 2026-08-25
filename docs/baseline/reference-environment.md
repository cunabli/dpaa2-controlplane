# Reference environment

Captured 2026-08-22 by the operator via the read-only capture script
(task 2.2; protocol ADR-0003). Board evidence anywhere in the series is only
valid against this stamped pair. Raw capture output is operator material
(it contains MAC addresses) and is never committed; this summary is the
committed record.

## Pinned pair

Asserted by every emitted board script before running:

- **MC firmware: 10.39.0** [verified]
- **Kernel: Linux 6.6.52** [verified] (not bumped until the port finishes)
- **restool: v2.4** [verified], from the NXP lf-6.6.52-2.2.0 release line

### Finding: the scoping assumption "MC 10.32.0" was wrong

The scoping session pinned MC 10.32.0, taken from restool's banner —
`restool v2.4 … (MC 10.32.0 compatible)`. The firmware itself reports
**10.39.0**. restool's banner states the API version it was built against,
not what the board runs. All series documents now pin 10.39.0; the skew
itself is recorded as an open item below (does any 10.32→10.39 wire-format
delta affect the families we drive?) and belongs in `mc-utils/api` delta
analysis during phase 3.

## DPC/DPL snapshot

Raw boot blobs are **not readable from this system**: `/proc/mtd` exposes
only two flat SPI partitions, none named dpc/dpl — a provenance gap, per
design. The committed snapshot is therefore the `dprc generate-dpl dprc.1`
reconstruction, with the caveat that generate-dpl reconstructs **current MC
state, not the boot DPL**: runtime-created objects and connections appear in
it indistinguishably from boot-time ones.

Topology at capture (identifiers generic by policy):

- **dprc.1 (root)** — options include SPAWN/ALLOC/OBJ_CREATE/
  TOPOLOGY_CHANGES/IRQ_CFG_ALLOWED. Contains: dpni.0 ↔ dpmac.17
  (management pair, foreign, ADR-0001 §4); dpmacs 3–10 and 17; dpbp.0;
  dpseci.0 (16 queue pairs, all priority 1); dprtc.0; 16 dpio; 16 dpcon;
  52 dpmcp (ids 1–53 with 14 absent — hole unexplained, open item).
- **dprc.2 (child, labeled, unplugged in the parent's listing)** — the VPP
  consumer container; options as root minus TOPOLOGY_CHANGES_ALLOWED.
  Contains: dpni.1 ↔ dpmac.7, dpni.2 ↔ dpmac.9, dpni.3 ↔ dpmac.3
  (connection only — dpmac.3 remains total-deny for all series scenarios,
  ADR-0003); **two DPBPs** (dpbp.1/2); **ten DPIOs** (dpio.16–25);
  three DPMCPs (dpmcp.54–56); dpseci.1 (8 queue pairs, all priority 2).

The dprc.2 numbers are live confirmation of the sizing rules the intent
layer codifies (ADR-0005): the two-DPBP rule, and DPIO = 10 = 2·(1+4) for
a main+4-worker consumer. Anchor for the phase-3 pool-object documents.

**This is a provisioned moment, not a bare boot** [board-observed
2026-08-23]. dprc.2 and its dpnis exist only while a consumer has
provisioned them: on a freshly booted board with no consumer, there is
no child container and none of those dpnis, and the wired dpmacs sit
unconnected. The ids are not reserved either — a dpni created at
runtime on such a boot was minted the same id the capture shows on the
boot pair. So a scenario must establish the objects it needs rather
than assume this topology, and must not read an id from this capture as
identifying the same object on the board.

DPNI shapes observed: the management dpni.0 is a plain NIC
(num_queues 16, 1 TC); the VPP DPNIs carry num_queues 16, 8 TCs, 24 CGs,
num_opr 8, and options HAS_KEY_MASKING, HAS_OPR, OPR_PER_TC, SINGLE_SENDER,
CUSTOM_CG.

## Clean-boot snapshot

Captured 2026-08-25 by the operator with `models/board/baselines/snapshot.sh`
on a fresh boot with no consumer started, parsed to
**`models/board/baselines/reference.json`** — the committed clean-boot
reference every sitting is diffed against (`dpaa2-verify snapshot diff`,
task 6.3). Unlike the provisioned-moment capture above, this is the bare
boot: one container, no child, the wired dpmacs unconnected. The head-count
census ("97 objects") is now a property of this file, and a test pins it.

What the bare boot looks like, beyond the count:

- **dprc.1 is the only container**; `restool dprc show mc.global` does
  answer — `dprc.0` holds exactly one object, `dprc.1`, listed
  *unplugged* (dprc.md unknown 5, first half).
- **Driver links**: 69 objects on `fsl_mc_allocator` (dpbp, dpcon, dpmcp),
  16 dpio on `fsl_mc_dpio`, dprtc.0 on `fsl_dpaa2_ptp`, dpseci.0 on
  `dpaa2_caam`, dpni.0 on `fsl_dpaa2_eth`, 8 of the 9 dpmacs on
  `fsl_dpaa2_mac`. The one driverless object is the management dpmac
  behind dpni.0: the eth driver owns it through phylink and the
  standalone mac driver defers, exactly as dpmac.md's arbitration
  paragraph predicts.
- **One DPL connection**: the management pair. Nothing else is wired at
  boot.
- **API versions the firmware reports** per family, against the version
  restool v2.4's headers were built for (the flib side lives in each
  family document):

  | family | firmware (`info`) | restool header |
  |---|---|---|
  | dpbp | 3.4 | 3.3 |
  | dpcon | 3.3 | 3.3 |
  | dpio | 4.3 | 4.2 |
  | dpmac | 4.10 | 4.2 |
  | dpmcp | 4.1 | 4.1 |
  | dpni | 8.5 | 7.18 |
  | dprtc | 2.3 | 2.0 |
  | dpseci | 5.4 | 5.3 |
  | dpdcei | 2.3 | 2.3 |
  | dpdmai | 3.4 | 3.4 |

  The firmware is ahead of restool on six of the eight resident
  families — the same skew the banner finding above records, now per
  family. dpdcei and dpdmai have no boot resident; their rows come from
  V-READBACK-1's runtime creates (2026-08-25). dpsw, dpdmux, dpci and
  dpaiop report theirs when a suite reads one back.

The snapshot deliberately drops counters (`dpni` statistics, the dpmac
`Counters:` block): they move with every management frame, and the diff
must be blind to traffic to be a residue check.

## Open items

- MC 10.32 → 10.39 API delta relevance for the driven families — per
  `mc-utils/api`, marshalling changed only for dpdmux, dpmac, dpni, dpseci,
  dpsw (dprc verified byte-identical, see dprc.md); each of those family
  docs diffs its own delta; feeds the portal pin (ADR-0004).
- restool v2.4's own `generate-dpl` prints `/* Unrecognized options
  found... */` above the VPP DPNIs' options — restool does not fully parse
  option sets current firmware accepts. Upstream-findings candidate
  (task 6.4).
- dpmcp id hole (14) in an otherwise contiguous root pool — a
  provisioned-moment artifact, not a boot fact: on a bare boot the root
  holds dpmcp.1–52 contiguous (the 52 of the 97-object baseline), and a
  runtime-created dpmcp is minted 53 [board-observed 2026-08-24, V-TRAF-0].
  The capture's 53-with-14-absent is what a consumer's create and a
  destroy left behind.
- Boot-DPL provenance: no readable blob; if the true boot DPL is ever
  needed (change #14 tape-out), it must come from the firmware build
  artifacts, not from the running system.
