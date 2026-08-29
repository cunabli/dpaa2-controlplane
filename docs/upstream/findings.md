# Upstream findings

Divergences and defects found while writing the object baselines
(`docs/baseline/`, phases 3–6 of change `restool-baseline`) and, from
entry 33 on, while running the board sittings of change
`verify-foundation` (`models/board/README.md`, `docs/adr/`). Everything
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
    starving probe silently. Board-confirmed: the MC creates and
    connects the default-shaped switch without complaint and the kernel
    driver refuses it at probe (`Flooding domain is not per FDB`,
    −EOPNOTSUPP). Evidence: dpsw.md, DPSW-I2, suite V-DPSW-4. — candidate
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
    platform limit? Board-answered: `ethtool -T` on a kernel dpni lists
    `onestep-sync` among the hardware transmit timestamp modes on this
    kernel/firmware, so the manual row is stale. Evidence: dprtc.md
    unknown 6, suite V-DPRTC-2 rev 2. — question
28. **`dpni_cfg` documents `num_queues` max 8**; restool accepts 32,
    NXP's own LX2160A DPLs declare 16 and it works. The documented limit
    is stale for WRIOP 3.0.0; the true ceiling is undocumented — a
    bracketing walk of 32, 24, 28, 20 was accepted at create on the
    board. Evidence: dpni.md unknown 2, suite V-DPNI-2. — question
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

## From the board sittings (tasks 5.9–5.11, 2026-08-29)

Observed on LX2160A-class hardware with the pinned pair; each entry
cites the suite and the design record that carry the numbers.

### Linux kernel

33. **The fsl-mc uapi admits one opener per root container — the second
    concurrent `open()` of `/dev/dprc.N` fails `EINVAL` regardless of
    free portals.** `fsl_mc_uapi_dev_open` allocates the extra portal
    with `fsl_mc_portal_allocate(root dprc)`, which records the *root*
    as the consumer of its own child dpmcp; `device_link_add` refuses
    the cycle and returns NULL, and the open fails with `-EINVAL`
    instead of the documented `-ENXIO`-at-exhaustion. Observed as 119
    of 120 held openers refused, 27 of 32 concurrent readers refused,
    with over a hundred portals free. The uapi is upstream code, so
    this applies beyond the LF tree. Evidence: ADR-0006 amendment,
    dpmcp.md DPMCP-I2, suites V-POOL-3 and V-CONC-1. — candidate
34. **Unbinding a dpaa2-eth netdev on a wired port leaves the dpmac
    driverless until reboot** (vendor standalone MAC driver). Binding
    a dpni connected to a dpmac evicts the standalone `fsl_dpaa2_mac`
    driver (`dpaa2_eth_connect_mac` → `dpaa2_mac_driver_detach`); the
    dpni's remove path asks the device core to re-attach it while the
    dpni is still the dpmac's endpoint, so the standalone probe returns
    `-EPROBE_DEFER`, and a later disconnect or destroy of the dpni
    never re-runs the deferred probe (the core retries only after some
    other probe on the bus succeeds). An unbind-then-destroy therefore
    strands the port; the working order is disconnect while bound, then
    unbind. Evidence: ADR-0008 §8, suite V-LINK-4 (snapshots at rev 1
    and rev 2). — candidate
35. **On PHY-typed ports ethtool never touches the firmware's link
    channels.** `dpaa2_eth_{get,set}_pauseparam` hand both the read and
    the write to phylink when the dpmac has a PHY, so `ethtool -a`
    reports phylink's configuration and `ethtool -A` never issues
    `dpni_set_link_cfg`; the dpni's own pause request stays at the
    probe-time default (`dpaa2_eth_set_pause` sets `DPNI_LINK_OPT_PAUSE`
    unconditionally) and `dpni_get_link_state` options are surfaced
    nowhere. Is the dpni request channel meaningful on PHY ports, and
    should the firmware's resolved options be readable? Evidence:
    dpmac.md DPMAC-I4, suite V-LINK-4 rev 2. — question

### restool

36. **A runtime-created child container can never be plugged, so it is
    never driven by the kernel.** `dprc create` leaves the child
    unplugged, and `dprc assign --plugged=1` on a dprc is refused by
    restool itself before any command is issued ("Cannot change plugged
    state of dprc", `dprc_commands.c`); the fsl-mc bus matches only
    plugged objects and exempts the root dprc alone, so the child has
    no bus device, its residents are never probed, and every ls-* flow
    that targets a child container (`-c`) produces objects no driver
    will bind. restool's help says the plugged state of a dprc "need
    not" be changed, which is false for the kernel bus. Whether the
    firmware would accept the plug is untested (the tool never asks).
    Evidence: dprc.md DPRC-I6/I8, suites V-POOL-1 and V-POOL-2 rev 2. —
    candidate

### MC firmware (questions for NXP)

37. **Destroying a dpmcp does not return its MC portal for the rest of
    the boot.** `dprc show mc.global --resources` read 200 `mcp` before
    64 dpmcps were created in a scratch child, 138 at the ceiling, 138
    after all 64 were destroyed without error, 138 after the child
    itself was destroyed, and 203 only after a reboot. Every other
    family (dpbp, dpcon, dpci, dpdmai, dpdcei, dpni) returns its units on
    destroy. The listing's arithmetic is also odd: 62 drawn for 64
    creates, and 203 to 200 by the first checkpoint of two independent
    sittings after five and six create/destroy pairs. Evidence:
    ADR-0011 §3, dpmcp.md DPMCP-I6, suite V-CEIL-1. — question
38. **What refuses the 18th dpni?** A dpni create is refused with
    `No resources` at the 18th object on the board (17 on top of the
    boot dpni) while every pool the firmware lists still shows room
    (frame queues 1913 of 1981, congestion groups and queuing
    destinations 219 of 253, the WRIOP tables above 180); each dpni drew
    a fixed slice and returned all of it on destroy. The gating
    resource is not in the `--resources` listing. Evidence: ADR-0011
    §2, dpni.md, suite V-CEIL-1. — question

### From sittings 5.9 and 5.10, and the batches before them

#### Linux kernel

39. **Destroying objects in the root container silently unbinds
    bystanders.** `dprc_scan_objects` fetches the container's objects one
    index at a time with nothing holding the firmware still; the
    container's interrupt thread rescans on every destroy event, so it
    is scanning while the next destroy lands; a fetch that fails
    mid-scan is marked invalid and the loop continues; and a descriptor
    that comes back with its plugged bit clear for an object Linux has
    plugged reaches `check_plugged_state_change`, which calls
    `device_release_driver` and logs nothing. Observed as the boot dpseci
    losing 62 of its 74 registered algorithms with no log line, and — in
    a two-destroy teardown — three boot residents unbound in one scan
    window, the boot dpni among them (management interface down), plus
    a boot dpmcp removed and re-added. Spacing destroys apart avoids
    it; nothing the caller can serialize against does. Bus code, so
    upstream-relevant. Evidence: ADR-0008 §4 (`dprc-driver.c:245-317`,
    `:145-167`, `:437-443`), suites V-DPDMUX-1 rev 1 and the 5.6
    lifecycle suites. — candidate
40. **A second dpseci never binds within a boot.** The crypto-API
    algorithm names the dpaa2_caam driver registers are one global
    namespace, the boot-time dpseci claims them, and every later dpseci
    is refused its registrations and stays unbound until reboot — a
    runtime dpseci is therefore unusable from the kernel. Design or
    defect? Evidence: dpseci.md, suite V-LIFE-DPSECI-1 rev 2. —
    question

#### restool

41. **Negative errnos surface as wrapped 8-bit exit codes, and the
    `mc.global` alias is partial.** A refusal returns the negative errno
    to the shell: `dprc assign --plugged=0` on a driver-bound dpni exits
    240 (−EBUSY), parser refusals exit 234 (−EINVAL), a sibling move
    exited 255, `dpaiop create` 250 — a script cannot tell a refusal
    class from an exit status, and several teardown flows discard them.
    `dprc show mc.global` works, but `dprc info mc.global` and `dump-mem
    mc.global` are refused before any command with "dprc.0 does not
    exist" / "Invalid MC object name". Evidence: dprc.md, suites
    V-LINK-5, V-DPRC-4, V-DPRC-6. — candidate
42. **`dpci info` prints the local priority count as the peer's.** On an
    asymmetric pair (one end created with 1 priority, the other with 2)
    each end reports *its own* count under `peer's num_of_priorities`,
    so which count the link actually carries is unobservable from the
    tool. Either restool prints the wrong attribute or the firmware's
    peer attributes mirror the local ones. Evidence: dpci.md unknown 5,
    suite V-DPCI-2. — question

#### MC firmware (questions for NXP)

43. **A dpni connected to a dpdmux cannot be disconnected from any
    end.** MC 10.39 accepts `dprc connect` of a dpni onto the demux
    *uplink* (interface 0, documented as dpmac-only) and then refuses
    the disconnect from both ends (Configuration error 0x6; the bare
    demux name answers No resources 0x8); a dpni on a *downlink* is
    equally un-disconnectable. Only destroying an object or rebooting
    undoes the pairing, and no pairing survives either. The control
    plane refuses the uplink pairing ahead of the firmware for that
    reason. Evidence: ADR-0009, suite V-DPDMUX-2 rev 1–5. — question
44. **Container option bits: which portal do they gate, and which
    status do they return?** `DPRC_CFG_OPT_OBJ_CREATE_ALLOWED` absent on
    a child does not refuse a create issued through the parent's portal
    with the child's token (which is every restool create) — the bit
    gates only the child's own portal. `SPAWN_ALLOWED` absent refuses a
    nested `dprc create` with Configuration error (0x6), not No
    privilege; `ALLOC_ALLOWED` absent refuses a dpbp create with No
    resources (0x8). None of this is in the manual's description of
    the bits. Evidence: dprc.md option-bit matrix, suites
    V-DPRC-2-{NOCREATE,NOSPAWN,NOALLOC}-1. — question
45. **What does `set-locked` cover?** A locked child refuses `assign
    --plugged` with No privilege but accepts `set-label` on its
    residents; reads keep working; the root lifts it. The manual says
    the lock removes create/destroy/assign from the sub-hierarchy and
    does not mention labels. Evidence: dprc.md DPRC-I11, suite V-DPRC-3.
    — question
46. **Moves and destroy authority are narrower than documented.** A
    single-command sibling move (`dprc assign --child` between siblings)
    is refused No privilege and only the two-hop route (unassign up,
    assign down) works; a moved object cannot be destroyed where it
    stands (No privilege — the manual says so); and destroying a
    container *evicts* a foreign resident one hop up rather than
    cascading, which nothing documents. Evidence: ADR-0007, suites
    V-DPRC-1 rev 1–3 and V-DPRC-6. — question
