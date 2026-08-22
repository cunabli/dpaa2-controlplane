# Upstream findings

Divergences and defects found while writing the object baselines
(`docs/baseline/`, phases 3–6 of change `restool-baseline`). Everything
here is stated in upstream-shareable terms — object types, source
references, and software versions only. Each entry cites the baseline
document that carries the full evidence trail; versions are the pinned
pair unless stated (restool v2.4, MC firmware 10.39.0, Linux 6.6.52
LF tree, DPDK 22.11-qoriq / 26.03).

Status legend: **candidate** = worth reporting, not yet filed;
**question** = needs an upstream answer rather than a fix; **filed** =
issue/patch submitted (link it when it happens).

## restool

1. **Family-wide destroy exit-code bug (non-root containers).** In every
   family's `destroy` handler, the MC destroy error is overwritten by the
   subsequent `dprc_close` result: a failed destroy prints an error and
   exits 0. The guarded pattern exists elsewhere in the same files.
   Evidence: dpseci.md, dpmac.md, dpdmux.md (`dpdmux_commands.c:897-901`),
   dpsw.md, dpbp.md, dpdcei.md, dpaiop.md. — candidate
2. **`generate-dpl` does not round-trip.** Emitters drop or mangle
   create-time state: dpni emits `num_rx_tcs` (no create flag accepts
   it), dpdcei drops the write-only `priority`, dpaiop drops
   `aiop_container_id`, dpdmai writes the priority *count* where the DPL
   grammar wants the values and omits `num_queues`/`options`; objects
   with unknown option bits produce a `/* Unrecognized options found */`
   comment and a silently incomplete DPL. Evidence: per-family
   silent-failure notes; V-GENDPL-1 in traffic-inventory.md. — candidate
3. **dpci `--options` is discarded by every flib in existence.** restool,
   DPDK, and the MC reference flib all marshal only `num_of_priorities`
   in DPCI_CREATE; the options word never reaches the wire, and GET_ATTR
   has no options field to check afterwards. Corollary: restool's
   `dpci_cfg` is not zero-initialized, so the day the marshalling is
   fixed, create without `--options` sends stack garbage. Evidence:
   dpci.md, DPCI-I2. — candidate
4. **`assert(false)` abort paths reachable in release builds** (no
   `-DNDEBUG`): dpdmux `info` on method S_VLAN or unknown manip; dpmac
   `info` on an eth_if/link_type beyond the enum (the 10.40 API already
   grows the space); dpaiop on unknown state flags (states are bit flags,
   so plausible OR-combinations abort); dpdcei on an unknown engine.
   Evidence: dpdmux.md, dpmac.md, dpaiop.md, dpdcei.md. — candidate
5. **Dead-option create-then-fail.** `dpni create` with any of 11
   never-consumed getopt options (create-time `--mac-addr`, the v9-era
   `--max-*` family) creates the object, prints its id, then exits
   non-zero — leaking an object under `set -e` wrappers. The same shape
   exists for `dpseci --options` on pre-x.1 firmware. Evidence: dpni.md,
   dpseci.md. — candidate
6. **`dpni update` exits 0 on three failure paths**, including a
   malformed MAC that still sends a partially-filled stack buffer to
   `dpni_set_primary_mac_addr`. Evidence: dpni.md
   (`dpni_commands.c:1294-1325`). — candidate
7. **`dpsw update` pushes uninitialized taildrop config** when the
   taildrop read fails (return unchecked), and exits 0 on open failure;
   `dpsw create`'s help advertises defaults the code does not send.
   Evidence: dpsw.md (`dpsw_commands.c`). — candidate
8. **`dpdbg destroy` prints success unconditionally** (error branch falls
   through); the create version gate returns 1 instead of a negative
   errno. Evidence: dpdbg.md. — candidate
9. **Rendered-info defects**: dpmac `info` prints an uninitialized
   connection-state value before checking the query error and silently
   skips refused counters (28 of 62 succeed on MC 10.39); dpio prints
   `num-priorities` in hex with no prefix; fetched-then-discarded fields
   across families (dpio `qbman_version`/`clk`, dpseci options mask,
   dpdcei `dce_version`, dprtc `paddr`/`little_endian`, dpsw
   `max_meters_per_if`). Evidence: dpmac.md, dpio.md, dpseci.md,
   dpdcei.md, dprtc.md, dpsw.md. — candidate
10. **Top-level dispatch**: `restool dpni help` is advertised but only
    `--help` exists (exact-match dispatch). Evidence: dpni.md
    (`restool.c:1088-1093`). — candidate

## ls-* scripts (shipped with restool)

11. **ls-addmux cannot succeed on MC 10.32–10.39**: it unconditionally
    passes `--custom-key-size=0`, restool sets the option on flag
    *presence*, and the create command version gate refuses below API
    6.11 (MC 10.40). Also: its companion dpmcp is created in the root
    container even when `-c` targets a child (consumer then probes
    against an empty pool), and an unanchored grep prints "EVB creation
    failed!" on success. Evidence: dpdmux.md, dpmcp.md, DPDMUX-I6. —
    candidate
12. **ls-addsw defaults contradict the only in-tree consumer**: it
    hardwires `PER_VLAN`/`PER_OBJECT` flooding/broadcast while
    `dpaa2-switch` hard-requires `PER_FDB` for both — a default
    `ls-addsw` switch can never bind. Its control-interface dpbp
    condition is inverted (`!=` where `=` was meant): the dpbp is
    created exactly when ctrl-if is disabled and skipped when needed,
    starving probe silently. Evidence: dpsw.md, DPSW-I2. — candidate
13. **ls-append-dpl defects**: dpci's DPL property maps to
    `--num-of-priorities` but restool's flag is `--num-priorities`
    (dpcon's matches; dpci is the odd one out); properties valued 0 are
    dropped before reaching restool; multi-cell DPL values splice
    space-separated where restool wants comma-separated; no restool exit
    status is checked anywhere; DPL container properties beyond
    compatible/parent/options hard-exit the script. Evidence: dpci.md,
    dpdmai.md, dpseci.md, dprc.md. — candidate
14. **ls-delete reports success unconditionally**: every per-object
    destroy's output and exit status are discarded
    (`> /dev/null 2>&1`); a failed destroy leaves the object alive while
    the script prints it as deleted. Evidence: dprc.md
    (`ls-main:1194`). — candidate
15. **Unanchored greps throughout ls-main**: `object_exists` and the
    list helpers match substrings (`dpmac.1` matches `dpmac.10`–`.18`) —
    wrong on any LX2160-class board with high port ids; ls-listni
    resolves netdev names from the root-container sysfs path only, so
    child-container/VFIO interfaces list without a name after a
    10000-iteration retry spin. Evidence: dpmac.md, dpni.md. — candidate
16. **ls-addni shell ranges disagree with restool in both directions**
    (`--num-cgs` capped 8 vs 128; `--dist-key-size` allowed 64 vs 56;
    out-of-policy `--num-queues` silently rewritten). Evidence:
    dpni.md. — candidate

## Linux kernel (LF 6.6.52; several apply upstream)

17. **dpaa2-switch uses the DPBP object id as the hardware bpid**
    (`dpaa2-switch.c:3110`) for QBMan seed/refill/drain; dpaa2-eth
    correctly uses `.bpid`. Works only where the two id spaces coincide.
    Related open ABI question: dpaa2-eth passes the DPBP *object id* to
    `dpni_set_pools` while dpaa2-xsk passes the *bpid* for the same wire
    field — one of them is latently wrong. Evidence: dpsw.md DPSW-I5,
    dpbp.md, dpni.md unknown 5. — candidate
18. **dpaa2-eth MACsec deinit zeroes all netdev features**: `features &=
    !NETIF_F_HW_MACSEC` (logical not, vendor tree) on every dpmac
    disconnect — after a hot-unplug the interface silently loses
    csum/SG/TSO. Evidence: dpni.md (`dpaa2-eth-macsec.c:658`). —
    candidate
19. **dpaa2-evb (staging) probe error paths**: double
    `fsl_mc_portal_free` + use-after-free; an `alloc_etherdev` failure
    path can return 0 after tearing down; leaked netdev on portal
    failure. Evidence: dpdmux.md (`evb.c:1328-1333`). — candidate
20. **dpaa2-qdma sizes its queue walk from the priority count** while
    `num_of_queues` is parsed and never used — a 1-queue/2-priority
    dpdmai gets queue_idx 1 requested on a one-queue object; and
    `dpaa2_qdma_shutdown` calls `dpdmai_destroy` after closing the
    token, passing the wrong token type. Evidence: dpdmai.md,
    DPDMAI-I3. — candidate
21. **dpaa2-eth `free_dpbps` leaks every second DPBP** when more than
    one is held (compacts the array while iterating). Evidence:
    dpbp.md. — candidate
22. **Standalone dpmac driver** (vendor): inbound `LINK_CFG_REQ` duplex
    is inverted (half applied as full and vice versa); a FIXED/NONE-type
    dpmac without a DT node installs the MC interrupt handler with no
    phylink guard — a link request dereferences NULL. Evidence:
    dpmac.md. — candidate
23. **dpio `service_select()` on an empty dpio list fabricates a bogus
    non-NULL pointer**, defeating every `-ENODEV` check — the zero-DPIO
    failure mode is memory corruption, not an error. Evidence:
    dpio.md. — candidate

## DPDK (22.11-qoriq and/or 26.03)

24. **dpaa2 IEEE1588 path**: failed dprtc create leaves a dangling
    static pointer (use-after-free on first timesync read); timesync ops
    registered even when no dprtc was found (NULL deref); `adjust_time`
    steps via get/add/set instead of calling the existing
    `dprtc_set_freq_compensation` — a PTP servo steps rather than
    syntonizes; no arbitration against a kernel that drives the same
    clock via MMIO. Evidence: dprtc.md, DPRTC-I2. — candidate
25. **dpaa2_mux flow API**: `rte_pmd_dpaa2_mux_flow_destroy` is declared
    but not implemented (link error); an out-of-range `dest_if` returns
    success without programming anything; one process-global key layout
    across all dpdmux instances. Evidence: dpdmux.md. — candidate
26. **dpaa2_sec raw datapath enqueue is a no-op stub** (both trees): the
    per-op raw enqueue path silently enqueues nothing, so a raw-DP
    consumer starves the SEC with no error. Independently root-caused
    during the VPP port (vpp-dpaa2-support, ADR-0008 there). — candidate

## MC firmware / documentation (questions for NXP)

27. **DPAA2UM Table 2-1 marks DPRTC "Two-step 1588 only"** while the MC
    changelog adds one-step/single-step APIs from 10.22.0 and the kernel
    carries a one-step SYNC path — stale manual comment or a real
    platform limit? Evidence: dprtc.md unknown 6. — question
28. **`dpni_cfg` documents `num_queues` max 8**; restool accepts 32,
    NXP's own LX2160A DPLs declare 16 and it works. The documented limit
    is stale for WRIOP 3.0.0; the true ceiling is undocumented.
    Evidence: dpni.md unknown 2. — question
29. **`dpni_set/get_mtu` are declared in the flib with no implementation
    and no CMDID** at both ends of the 10.32–10.39 span — a permanent
    dangling pair. Evidence: dpni.md. — question
30. **`DPNI_OPT_HAS_REPLICATION`** exists in restool's option map but
    not in the MC 10.39 flib header (nor 10.40's 14-flag list) — real
    firmware option or restool running ahead? Evidence: dpni.md
    unknown 8. — question
31. **Does MC 10.39 retain a v1 handler for
    `DPNI_CMDID_SET/GET_TX_CONFIRMATION_MODE`** (v2 since 10.34, byte 0
    became a live `ceetm_ch_idx`)? A 10.32-built client emits v1;
    client-side sources cannot answer. Evidence: dpni.md unknown 1. —
    question
32. **`dpseci_get_sec_counters` is SEC-block-global, not per-object** —
    two dpsecis read one counter set; worth a documentation note since
    nothing in the flib says so. Evidence: dpseci.md, DPSECI-I9. —
    question
