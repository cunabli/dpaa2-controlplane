# dprc baseline

<!-- Instantiated from _template.md; every section mandatory, empty sections
     state so explicitly (spec: object-baseline). -->

Claim markers, used per claim throughout: **[read]** = derived from source or
manual, not yet observed; **[verified]** = observed on the board against the
pinned reference environment (see `reference-environment.md`).

Findings are written to be provable: behavioral claims name their
observables and are distilled into the Invariant candidates section as
propositions a Quint model can carry — invariant-bearing or
invariant-breaking.

Cross-family relationships are mapped in [object-model.md](object-model.md);
the board scenarios that will settle this document's open items are
classified in [traffic-inventory.md](traffic-inventory.md).

The DPRC (Data Path Resource Container) is the container family everything
else lives in: a software context (kernel, VPP, a VM) is associated with one
DPRC holding every object and resource it may use; parents spawn descendant
containers to delegate resources (manual ch. 6, pp. 54–62) [read]. The MC's
own container is `dprc.0`, aliased `mc.global` by restool
(`dprc_commands.c:832` area) [read]; the root container visible to Linux is
`dprc.1` [verified].

## Command surface

restool v2.4 exposes 14 subcommands (`dprc_commands.c:2404–2464`) [read]:

| Command | MC interaction | Notes |
|---|---|---|
| `sync` | **none** — runs `system("echo 1 > /sys/bus/fsl-mc/rescan")` | Pure kernel bus rescan; see kernel-side section |
| `list [--full-path]` | walks containers from the root via open/enumerate | `--full-path` prints `dprc.1/dprc.2` style |
| `show <container> [--resources] [--resource-type=<t>]` | `dprc_get_obj_count`/`dprc_get_obj`; resource variants use `dprc_get_res_count`/`dprc_get_res_ids` | object list w/ label + plugged state; resource options are **absent from the help text** (see silent-failure notes) |
| `info <dprc.N> [--verbose]` | `dprc_get_attributes` | prints container id, icid, portal id, options mask (decoded) |
| `create <parent> [--options=<mask>] [--label=<s>]` | `dprc_create_container` | see below |
| `destroy <container>` | `dprc_destroy_container` | refuses the root container |
| `assign <container> --object=<o> [--child=<c>] [--plugged=0\|1]` | `dprc_assign` | move parent→child and/or set plugged state |
| `unassign <parent> --child=<c> --object=<o>` | `dprc_unassign` | move child→parent |
| `set-label <object> --label=<s>` | `dprc_set_obj_label` | any object except the root container; ≤15 chars |
| `set-locked <child> --locked=0\|1` | `DPRC_SET_LOCKED` | locks the child **and its entire hierarchy** |
| `connect <parent> --endpoint1=<o> --endpoint2=<o> [--committed-rate=<n> --max-rate=<n>]` | `dprc_connect` | link two objects |
| `disconnect <parent> --endpoint=<o>` | `dprc_disconnect` | either endpoint names the link |
| `generate-dpl <container>` | full query walk | reconstructs DPL DTS for container + descendants from **current** state |
| `dump-mem <container> --partition_id=MEM_PART_PEB` | memory-partition query | free blocks of the PEB partition; only `MEM_PART_PEB` accepted in v2.4 |

`create` details [read, `dprc_commands.c:1065–1130, 1190–1200`]:

- `icid` and `portal_id` are always taken from the MC pools
  (`DPRC_GET_ICID_FROM_POOL`, `DPRC_GET_PORTAL_ID_FROM_POOL`); restool
  offers no way to pin them (the DPL can — ch. 24 declares explicit icid and
  portal ids per container [read]).
- Every child DPRC gets its **own MC command portal**;
  `dprc_create_container` returns the child id and the portal offset.
- Default options when `--options` is omitted:
  `SPAWN_ALLOWED | ALLOC_ALLOWED | OBJ_CREATE_ALLOWED | IRQ_CFG_ALLOWED` —
  exactly the option set observed on the board's VPP child container
  [verified, reference-environment.md].
- The full option vocabulary: the four defaults plus
  `TOPOLOGY_CHANGES_ALLOWED` (root has it, child does not [verified]),
  `AIOP`, `PL_ALLOWED`.
- On a mid-create failure restool destroys the partially created child
  (compensating cleanup in `create_child_dprc`).

Constraints stated by restool's own help [read]:

- `assign`: DPRCs themselves cannot be assigned; a DPRC's plugged state
  cannot (and need not) be changed; **plugged objects cannot be moved**;
  the operation "may be restricted by the permissions granted in the
  container attributes".
- `connect`: the named parent must be a **common ancestor** of both
  endpoints; endpoints may live in different containers; endpoints must be
  disconnected first; multi-port objects (dpsw, dpdmux) name endpoints as
  `<object>.<id>.<port>`; committed/max rate must be given together, with
  max > committed.
- `set-locked` (manual §6.3.6): the locked hierarchy loses
  lock/unlock, create/destroy, and assign/unassign [read].

## Option inventory: used vs available

**Used** [read, `restool/scripts`]:

| Command/option | Used by | Value passed |
|---|---|---|
| `assign --object --plugged=1` | ls-main (dpio/dpbp/dpcon/dpmcp/dpni/dpsw/dpdmux top-ups), ls-append-dpl | plug after create |
| `assign --child` | ls-append-dpl only | placement per DPL `containers` node |
| `show <c>` | ls-main | parsed with grep to count/probe objects |
| `list` / `list --full-path` | ls-main | container discovery |
| `connect --endpoint1/2` | ls-main, ls-append-dpl | no rate options ever passed |
| `set-label` | ls-main | labels created dpni/dpsw/dpdmux |
| `sync` | ls-main | after object creation batches |
| `create --options=<mask>` | ls-append-dpl only | mask read from the DPL dts |

ls-debug uses no dprc *commands*, but it mutates the family's kernel
machinery: it disables `/sys/bus/fsl-mc/autorescan` bus-wide for its
entire run and restores the prior value on exit (`ls-debug:309-314,75-80`)
— while it runs, child-DPRC hot-plug rescans (the only automatic
visibility path for child containers, see Kernel-side) are suppressed for
every other actor on the system [read].

ls-append-dpl's `create_dprc` accepts exactly three DPL container
properties — `compatible`, `parent`, `options` — and **hard-exits on any
other** (`ls-append-dpl:81-107`): the icid/portal-id pinning the DPL
format can express (ch. 24) is refused by the script, not just absent
from restool. Its `create_connection` issues every connect on a
hardcoded `dprc.1` ancestor [read].

Deletion (`ls-main`'s `ls-delete` verb) does not use `dprc destroy` for
targeted removal at all: it walks the **kernel's device-link graph**
(sysfs `consumer:`/`supplier:` links), unbinds the driver, runs the
per-family `restool <type> destroy` on the object, recurses into suppliers
with no remaining consumers, then bus-rescans (`ls-main:1155-1210`) [read].
`restool dprc destroy` is reachable only through `ls-delete all` (every
bus device, container included, goes through the same per-type destroy).
Entry points are restricted to dpni/dpsw ("Object type not supported!"
otherwise).

**Available-but-unused**: `unassign`, `set-locked`, `info` (and
`--verbose`), `generate-dpl`, `dump-mem`,
`show --resources`/`--resource-type`, `connect --committed-rate/--max-rate`,
`create --label`; and `destroy` outside the `ls-delete all` sweep. The
targeted-decommission path NXP ships depends on kernel device links, not on
MC-side dependency knowledge — the port's reconciler must derive teardown
ordering from the object model instead, and validate
`unassign`/`set-locked`/targeted `destroy` from scratch (scratch-DPRC
suites, ADR-0003).

## Attribute mutability

| Attribute | Class | Evidence |
|---|---|---|
| `icid` | create-time-immutable, pool-assigned | no restool/MC update path found [read] |
| `portal_id` | create-time-immutable, pool-assigned | as above [read] |
| `options` mask | create-time-immutable (assumed) | restool has no update command; MC-side mutability unconfirmed → unknown register |
| `label` | mutable (`set-label`) | [read] |
| locked state | mutable (`set-locked 0/1`) | [read] |
| membership (which objects it holds) | mutable (`assign`/`unassign`), only for unplugged objects | [read] |
| contained objects' plugged state | mutable (`assign --plugged`) | [read] |
| connections among descendants | mutable (`connect`/`disconnect`) | [read] |

## MC API notes

- restool drives the v10 API from `mc_v10/fsl_dprc.h`; `dprc_cfg` is
  `{icid, portal_id, options, label[16]}` [read].
- The MC distinguishes **objects** (dpni, dpbp, …) from **resources** —
  pooled primitives (e.g. `mcp`, and the DPL `resources` node's types)
  enumerable per container via `dprc_get_res_count`/`dprc_get_res_ids`
  [read]. The object model doc must carry this split; restool's help hides
  it.
- The DPL (`containers` node, manual ch. 24) can declare per-container
  icid, explicit portal ids, resource sets, and object sets — strictly more
  than restool's create exposes [read]. Relevant to change #14 (tape-out).
- 10.32 → 10.39 command-format delta for DPRC: **none** — `mc-utils/api`
  `dprc.c` is byte-identical between `mc_release_10.32.0` and
  `mc_release_10.39.0` [read]. The firmware-version skew
  (reference-environment.md) is harmless for this family. Families whose
  marshalling *did* change in that span: dpdmux, dpmac, dpni, dpseci, dpsw —
  each of their baseline documents must diff its delta.

## Kernel-side behavior (Linux 6.6.52)

All claims [read] from the pinned tree (lf-6.6.52-2.2.0),
`drivers/bus/fsl-mc/` unless noted.

**Binding.** The `fsl,qoriq-mc` platform driver synthesizes the root DPRC
device (portal address from DT, not from the MC) and `fsl_mc_dprc` binds
every DPRC (`fsl-mc-bus.c:1142-1168`, `dprc-driver.c:882-899`). Probe order:
uapi device file (root only), `dprc_setup` (a *child* DPRC builds its own
`fsl_mc_io` from its region 0; API version gate refuses DPRC < v6), pool
init + first scan, then DPRC IRQ setup (`dprc-driver.c:729-769`).

**Scan and plugged state.** `dprc_scan_objects` adds children in **two
passes: allocatable objects (dpmcp, dpbp, dpcon) first**, so the pools are
full before consumer drivers probe (`dprc-driver.c:202-243`) — the
kernel-side half of ADR-0001 C1. Driver matching refuses unplugged objects;
an existing object flipping unplugged→plugged triggers `device_attach`,
plugged→unplugged triggers `device_release_driver`
(`dprc-driver.c:145-167`, `fsl-mc-bus.c:99-105`). So restool's
`assign --plugged` is the kernel's bind/unbind lever.

**Rescan.** `/sys/bus/fsl-mc/rescan` (the entire mechanism behind
`restool dprc sync`) re-scans **root containers only — it never recurses
into child DPRCs** (`fsl-mc-bus.c:217-248`), passes no IRQ-pool allocation,
and discards scan errors (the write always "succeeds"). Child containers
re-scan only via their own DPRC IRQ (hot-plug events OBJ_ADDED/REMOVED/…,
`dprc-driver.c:400-463`) — gated by `/sys/bus/fsl-mc/autorescan`. There is
no per-device rescan attribute.

**Allocation pools.** Per-DPRC pools for dpmcp/dpbp/dpcon/irq
(`include/linux/fsl/mc.h:67-77`); allocation **never crosses container
boundaries** (`fsl-mc-allocator.c:266-296`). Exhaustion returns `-ENXIO`
with `"No more resources of type %s left"` — the exact ADR-0001 C1 symptom.
DPMCPs are not allocatable through the object API; they back
`fsl_mc_portal_allocate` (`mc-io.c:165-234`). IRQ pool caps at 256 per
container, exhaustion `-ENOSPC`.

**The uapi node.** `/dev/dprc.N` exists for **root** containers only; it
accepts exactly one ioctl, `FSL_MC_SEND_MC_COMMAND` (64-byte command),
validated against a 46-entry whitelist (`fsl-mc-uapi.c:81-450`): mutating
DPRC commands (create/destroy/assign/unassign/set-label/set-locked/
connect/disconnect) and generic create/destroy require `CAP_NET_ADMIN`;
all DPRC queries are unprivileged once the node opens; token *presence* is
checked but its value is not, and there is no per-container scoping — an
opener can address any object it can obtain a token for. Every additional
concurrent opener consumes a DPMCP from the root pool
(`fsl-mc-uapi.c:477-522`) — an exhaustion vector for portal-hungry setups.
This node is the restool transport and the exact surface the Rust MC portal
(ADR-0004) will speak.

**Nested DPRCs.** The kernel binds child DPRCs recursively: each gets its
own `fsl_mc_bus`, own pools, own IRQ handler, own portal — but no
`/dev/dprc.N` node and no reach from bus-level rescan. Object *removal* is
Linux-side only: unbinding/removing devices never destroys MC objects
(`dprc-driver.c:84-113`).

**Kernel-defined (not firmware-defined) semantics.**

- The `type.id` naming (`dprc.1`, `dpni.0`) is a kernel convention
  (`fsl-mc-bus.c:834`).
- A fixed 16-entry type table gates device creation; an object type outside
  it is **invisible to Linux** (`-ENODEV "unknown device type"`,
  `fsl-mc-bus.c:403-434`) — relevant if the MC ever grows new families.
- VFIO: `vfio-fsl-mc` has no match table — reachable only via
  `driver_override` + `bind`; once bound to a DPRC, a bus notifier
  propagates the override to every subsequently added child; the container
  is the IOMMU/VFIO grouping unit (`vfio_fsl_mc.c:423-452, 523-526,
  600-607`). This is the VPP-consumer binding path (change #4 typestates).
- Endpoint lookup: `-ENOTCONN` when unconnected, `-EPERM` when the peer
  exists but in another container (`fsl-mc-bus.c:945-1005`).
- Non-DPRC objects inherit the parent DPRC's ICID and MSI domain; a kernel
  workaround fixes child-DPRC DPMCP base addresses the firmware reports
  as 0 (`fsl-mc-bus.c:722-736, 871-882`).

**NXP deltas vs upstream 6.6.52** (exact, from git history; 7 files,
+93/−69): uapi device-file lifetime moved into probe/remove; a `.shutdown`
op added to the DPRC driver; explicit pool-cleanup walk deleted (left to
devm); **`DPRC_GET_MEM` added to the uapi whitelist, unprivileged** — the
firmware command behind restool's `dump-mem`; **MC command timeout raised
500 ms → 15 s** (affects every command incl. the ioctl path); vfio region
caching improvements. The DPRC command encodings, pool machinery, and
rescan/autorescan ABI are stock upstream.

## Lifecycle ordering and dependencies

- A child DPRC is created *from its parent's handle*; the parent needs
  `SPAWN_ALLOWED` (implied by the option's name; gate unverified → unknown
  register) [read].
- Creation allocates the child's MC portal, icid (IOMMU isolation context),
  and container id atomically; the child is immediately listable from the
  parent [read].
- Population order per ls-append-dpl and ADR-0001 practice: create objects
  (in parent), `assign --child` to place them (only while unplugged), then
  `assign --plugged=1` to hand them to a driver; unplug before any move
  [read].
- `connect` may span containers but is issued on a common ancestor —
  cross-DPRC links (the kernel↔VPP pseudo-wire, change #9) are therefore
  root-issued operations [read].
- `destroy` refuses the root; behavior on a non-empty container
  (recursive vs refuse) is not stated by help text → unknown register.
- The board's own child container (`dprc.2`, VPP) is *unplugged* in the
  parent's listing while fully operational [verified] — plugged state of a
  DPRC does not gate its use as a container (consistent with "not possible
  and unnecessary to change the plugged state of a DPRC").

## Intent mapping

The DPRC realizes the **consumer/runtime** construct (ADR-0005): one child
DPRC per declared consumer, holding the consumer's derived object set;
restool-default options
(`SPAWN|ALLOC|OBJ_CREATE|IRQ_CFG`) match the observed VPP container and are
the derivation default [verified]. The root container is never a consumer;
foreign objects in it are never touched (ADR-0001 §4).

## Silent-failure notes

- `dprc sync` checks only that `system()` itself spawned; a failed sysfs
  write (non-root, path absent) is **silently ignored** — the canonical
  "converged but didn't" trap for any flow that shells out to it [read,
  `dprc_commands.c:cmd_dprc_sync`].
- The kernel side of the same write compounds it: bus rescan **discards
  scan errors** (the sysfs write always succeeds) and **never recurses into
  child DPRCs** — a `sync` after mutating a child container refreshes
  nothing there unless that container's own IRQ path (`autorescan`) is
  live [read, `fsl-mc-bus.c:217-248`]. Loudness invariant candidate for
  the models: "visibility of a mutation is confirmed by re-observation,
  never by issuing sync".
- `generate-dpl` prints `/* Unrecognized options found... */` when an
  object carries option bits newer than restool's tables — the emitted DPL
  is silently incomplete and would not round-trip [verified on this board:
  the VPP DPNIs trigger it; see reference-environment.md open items].
- `show`'s output is designed for humans; ls-main greps it (object
  presence, dpio counts) — a formatting change breaks those scripts
  silently. The port must consume structured queries, never scraped text
  [read].
- `assign` failures due to container-attribute permissions surface only as
  a generic MC error status — which option bit denied the operation is not
  reported [read].
- `ls-delete` discards every destroy's outcome
  (`$restool --script $type destroy $object > /dev/null 2>&1`,
  `ls-main:1194`) and prints the object as deleted regardless — a failed
  destroy leaves the object alive while reporting success [read].

## Invariant candidates

Findings distilled into propositions a Quint model can carry (observables =
how a trace checks it). Status: candidate = corpus-read, board-pending =
needs a suite, verified = observed on the pinned pair, refuted = recorded
false belief.

| Id | Proposition | Observables | Status |
|---|---|---|---|
| DPRC-I1 | Kernel allocation of dpmcp/dpbp/dpcon/irq never crosses container boundaries: `container(consumer) = container(pool)` for every allocation | consumer probe outcome; `dprc show` of both containers; `-ENXIO "No more resources of type %s left"` on local exhaustion regardless of remote surplus | candidate |
| DPRC-I2 | Plug gating: object bound to a kernel driver ⟺ plugged ∧ matching driver present; `assign --plugged=1` ⇒ eventually bound, `--plugged=0` ⇒ released | plugged column of `dprc show`; presence of `driver` symlink under `/sys/bus/fsl-mc/devices/<obj>/` | verified 2026-08-23 (V-LINK-5): the release direction holds by refusal — `assign --plugged=0` on a kernel-bound, netdev-backed dpni came back −EBUSY with the object still plugged and the driver still bound, not a race; the bind direction is V-LIFE-DPNI-1's canonical order |
| DPRC-I3 | Move precondition: `assign --child` is enabled only for unplugged objects; a move of a plugged object fails with an MC error (exact status unrecorded) | command exit + MC status; object's container membership unchanged after refusal | board-pending |
| DPRC-I4 | `dprc create` without `--options` yields exactly {SPAWN, ALLOC, OBJ_CREATE, IRQ_CFG}_ALLOWED | options mask in `dprc info` | verified |
| DPRC-I5 | Connect precondition: `connect(p, e1, e2)` enabled only if p is a common ancestor of e1 and e2 and both are currently unconnected | command exit; `GET_CONNECTION` per endpoint | candidate |
| DPRC-I6 | **Breaking:** the model must NOT assume `sync` ⇒ mutation visible. Bus rescan reaches root containers only and discards errors; visibility of a mutation is established only by re-observation of the affected container | child-container object list unchanged after sync following an out-of-band mutation | candidate |
| DPRC-I7 | **Breaking:** the model must NOT assume Linux device removal destroys MC objects; removal is Linux-side only, objects survive on the bus | object still listed by `dprc show` after driver unbind/device_del | candidate |
| DPRC-I8 | Scan ordering postcondition (ADR-0006 fold): plugging an allocatable (dpmcp/dpbp/dpcon) lands it in its container's kernel pool before any consumer in the same scan probes | consumer probe success when pool objects and consumer are plugged in one batch | candidate |
| DPRC-I9 | Teardown reachability (liveness): from every reachable scratch-container state some finite action sequence empties and destroys the container | suite replay ending in `destroy` success + container absent from `list` | verified 2026-08-23 (V-DPRC-1 rev 3, 13/13): the scratch container was emptied through both move directions and destroyed, absent in read-back; unknown #1 is answered by ADR-0007 §3's release/evict law, so a non-empty destroy never blocks teardown either |
| DPRC-I10 | Immutability: icid, portal_id, and options of a container never change across any post-create action sequence | `dprc info` before/after every suite | candidate |
| DPRC-I11 | `set-locked 1` on a child removes create/destroy/assign/unassign/lock from the entire sub-hierarchy; `set-locked 0` restores it (who may unlock: unknown #4) | denied MC status on each operation class inside the locked hierarchy | board-pending |

## Unknown / unverified register

Board-validation candidates (all runnable as scratch-DPRC,
object-lifecycle-only scenarios except where noted):

1. ~~`destroy` on a non-empty child: recursive, or refused? What error?~~
   **Answered** — board suite V-DPRC-1 rev 1 and rev 2, 2026-08-23:
   neither refused nor an error; restool exited 0 both times and only
   the read-back told the two cases apart. A resident the container
   created dies with it; a resident merely assigned in survives,
   evicted unplugged into the parent (ADR-0007 §3).
2. Are DPRC `options` mutable post-create by any MC command (restool has
   none)?
3. Which option bit gates which operation (`SPAWN` vs `OBJ_CREATE` vs
   `TOPOLOGY_CHANGES` vs `ALLOC`): create-child vs create-object vs
   connect vs assign — the permission matrix is undocumented.
4. `set-locked` semantics: who can unlock (parent only?), and what exactly
   the locked hierarchy still allows (info/show?).
5. What `dprc.0`/`mc.global` reveals via `show` — answered: `dprc.0`
   holds exactly one object, `dprc.1`, listed unplugged [board-observed
   2026-08-25, clean-boot snapshot] — and whether any operation against
   it is accepted (still open, task 5.10).
6. `AIOP` and `PL_ALLOWED` option semantics on a board with no AIOP.
7. dpmcp id hole (14) in the root pool (reference-environment.md) —
   creation-order artifact or consumed companion?
8. ~~MC 10.32 → 10.39 DPRC command deltas~~ — resolved: none (see MC API
   notes).
9. ~~Whether `assign --plugged` on an object bound to a kernel driver is
   refused by MC or races the driver (link-signaling class if tested on a
   netdev-backed object).~~ **Answered** — board suite V-LINK-5,
   2026-08-23: `dprc assign --plugged=0` on a kernel-bound, link-up,
   netdev-backed dpni exits 240 (8-bit −EBUSY) with the object still
   plugged and the driver still bound. It is refused, not raced and not
   silently dropped; releasing such an object requires unbinding the
   driver first.
10. `dump-mem` output semantics and whether partitions beyond PEB exist on
    LX2160A firmware.
11. `/dev/dprc.N` permissions: the kernel sets no mode/owner (default misc
    device); whether this BSP's udev relaxes it is unchecked — matters for
    whether the Rust portal needs root vs CAP_NET_ADMIN only.
12. Whether `autorescan` (child-DPRC IRQ rescan) is enabled in this BSP's
    default configuration — determines if child-container mutations are
    ever observed without an explicit re-scan by the mutator.
