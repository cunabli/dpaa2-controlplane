# MC status register

Every refusal the board has issued, one row per observed condition, so
a suite can name the status it expects and the harness can score it
(`verify-foundation` task 6.4). A refusal is judged on the status text
restool prints to stderr and on the read-back, never on the exit code
alone: restool has exited 0 on an MC refusal (ADR-0007 §2), and it
refuses some commands itself before the MC is asked.

Claim markers as in every baseline document: **[read]** = derived from
source or manual, **[verified]** = observed on the board against the
pinned reference pair (`reference-environment.md`).

## How a status reaches the operator

The MC writes an 8-bit status into the command header it hands back
(manual §5.5, Table 2, `STATUS` bits 23:16; `restool/mc_v10/fsl_mc_cmd.h`
`enum mc_cmd_status`). The flib turns it into a negative errno, restool
turns the errno back into a name (`restool.c` `flib_error_to_mc_status`,
`mc_status_to_string`) and prints `MC error: <name> (status 0x<code>)`,
then exits with the errno's low eight bits. So the exit code is a lossy
echo of the status: 255 is `-EPERM`, which is what No privilege becomes,
and 240 is `-EBUSY`, which restool's own client guards return without
any MC status behind them. **[read]**

| Code | Status | errno | restool exit |
|---|---|---|---|
| 0x0 | Command completed successfully | 0 | 0 |
| 0x1 | Command ready to be processed | 0 | 0 |
| 0x3 | Authentication error | EACCES (13) | 243 |
| 0x4 | No privilege | EPERM (1) | 255 |
| 0x5 | DMA or I/O error | EIO (5) | 251 |
| 0x6 | Configuration error | ENXIO (6) | 250 |
| 0x7 | Operation timed out | ETIMEDOUT (110) | 146 |
| 0x8 | No resources | ENAVAIL (119) | 137 |
| 0x9 | No memory available | ENOMEM (12) | 244 |
| 0xA | Device is busy | EBUSY (16) | 240 |
| 0xB | Unsupported operation | ENOTSUPP (524) | 244 |
| 0xC | Invalid state | ENODEV (19) | 237 |

The same table lives in code as `crates/dpaa2-verify/src/mcstatus.rs`;
the ledger lint checks every register row below against it.

restool's own argument and usage errors exit 234 (`-EINVAL`) with a
usage dump and no MC status — a refused create leaves that dump in
`created.txt`, which is why the harness treats any non-binding line
there as evidence rather than a parse error. **[read]**

## Register

One row per observed condition. `Raised by` is the layer that refused:
**MC** (the firmware answered with the status), **restool** (a client
guard refused before the MC was asked), **kernel** (the driver, not the
MC). `Evidence` cites the suite and revision whose verdict carries the
refusal; the lint resolves every citation to `models/board/VERDICTS.json`.

| Code | Status | Family · verb | Condition | Raised by | Evidence |
|---|---|---|---|---|---|
| 0x4 | No privilege | dprc · destroy | Destroy issued through a portal other than the object's creator (a moved object destroyed from its new home). restool printed the status and **exited 0**; the read-back caught the object still present | MC | V-DPRC-1 rev 2; ADR-0007 §2 |
| 0x4 | No privilege | dprc · connect | Connect issued on a default-created container, which lacks `DPRC_CFG_OPT_TOPOLOGY_CHANGES_ALLOWED`; the same connect succeeds when issued against the root ancestor | MC | V-DPCI-1 rev 1 |
| 0x4 | No privilege | dprc · connect / disconnect | Teardown's restore of a boot edge on a bare boot, where no such edge exists (the reference capture's boot pair is a provisioned-moment artifact) — a vacuous restore, refused | MC | V-LINK-2 rev 3; V-LINK-5; V-TRAF-0 rev 3 (`teardown.log`) |
| 0x6 | Configuration error | dpdbg · set `--level=99` | An out-of-range debug level is rejected by firmware, not clamped; restool does no range validation of its own | MC | V-DPDBG-1 rev 2 |
| 0x8 | No resources | dprtc · create | A second dprtc while the DPL-born dprtc.0 stands (singleton since MC 10.31); `dprc show` byte-identical before and after | MC | V-DPRTC-1 |
| 0x8 | No resources | dpdbg · create | A second dpdbg while dpdbg.0 stands (root-only singleton) — the same status as the dprtc singleton, hinting at a shared firmware path | MC | V-DPDBG-1 rev 2 |
| — | — | dprc · assign `--plugged=0`; dprtc · destroy | The object is bound to a kernel driver: restool's client guard refuses with "unbind it first" and `-EBUSY` (exit 240) before any MC command is sent; the object stays plugged and bound | restool | V-LINK-5; V-DPRTC-3 rev 2 |
| 0x4 | No privilege | dprc · assign `--child` | A sibling-to-sibling move (source and destination are not parent and child), issued with the exact rendering V-DPRC-1 rev 1 used (which exited 255); the object stayed in its source container. This fills the row task 5.9 left as unknown | MC | V-DPRC-6 rev 1 |
| — | — | dprc · assign `--child` | The move of a *plugged* object one hop up is refused by restool's own client guard — "cannot be moved because it is currently in plugged state" / "unplug it first" — before any MC command is sent; the object stays in its container. The MC-layer face is unreachable through restool | restool | V-DPRC-6 rev 1 |
| — | — | dpsw · create + plug | A dpsw built with restool's silent defaults (flooding PER_VLAN, broadcast PER_OBJECT) is created and connected by the MC without complaint, then refused by the kernel `fsl_dpaa2_switch` driver at probe — dmesg "Flooding domain is not per FDB, cannot probe", −95 (EOPNOTSUPP), driver link empty. The refusal is the kernel's, not the MC's | kernel | V-DPSW-4 rev 1 |
| 0x6 | Configuration error | dpaiop · create | `dpaiop create` on this AIOP-less silicon; nothing created. The gate lives in the MC's dpaiop create handler, not the DPRC options — `dprc create --options=…AIOP` was accepted the same sitting | MC | V-DPAIOP-1 rev 1 |
| 0x6 | Configuration error | dprc · connect | A dpni onto a dpdmux downlink (interface 1) while the uplink is empty: refused, yet the read-back shows the dpni on interface 0 — the refused command left a connection behind, and only a destroy removes it | MC | V-DPDMUX-2 rev 3 |
| 0x6 | Configuration error | dprc · disconnect | Tearing down a dpni the MC accepted onto a dpdmux uplink (interface 0), refused from either the demux end or the dpni end; the connect itself was accepted, so the pairing stands until an object is destroyed | MC | V-DPDMUX-2 rev 2; V-DPDMUX-2 rev 3 |
| 0x6 | Configuration error | dprc · disconnect | A dpni on a dpdmux downlink (interface 1), from the dpni end or the demux downlink end; the connect had been accepted on a fresh boot, so the pairing is permanent until a destroy | MC | V-DPDMUX-2 rev 5 |
| 0x8 | No resources | dprc · disconnect | Disconnecting an unconnected dpdmux downlink interface (interface 1) from the demux end | MC | V-DPDMUX-2 rev 2 |
| 0x8 | No resources | dprc · disconnect | The demux uplink end (bare name, interface 0) while nothing is connected there, even though the dpni sits on interface 1 | MC | V-DPDMUX-2 rev 5 |
| — | — | dpseci · create | Priority 0, a priority above 8, or a priority-count that does not equal num-queues: restool's own parser refuses (exit 234, "Invalid priority value." / "Please set N priorities") before any MC command is built. The MC-layer validation is unreachable through restool | restool | V-DPSECI-1 rev 1 |
| — | — | dpni · create `--max-senders` | A dead v9-era option: restool creates the dpni, prints its id, then exits 234 on the unconsumed option — the object stands and read-back is the only side-effect oracle | restool | V-DPNI-2 rev 1 |

## What the register does not hold yet

- MC-layer faces that stay unreachable through restool, because a
  restool client guard refuses first: the move of a *plugged* object
  (the guard answers before the MC is asked), and dpseci create
  priority validation (restool's parser refuses priority range and
  count before any MC command). Both need the ioctl portal to reach the
  MC-layer rule.
- The dpni `num_queues` ceiling above restool's cap of 32: the create
  walk (V-DPNI-2 rev 1) found no MC refusal up to 32, so if the MC caps
  the count at all it caps at or above restool's reach — no status yet.
- The dpdmux→dpni pairing is now on record as accepted, not refused, on
  either interface: MC 10.39 takes the connect and then refuses every
  disconnect (0x6 and 0x8 above), so the model's `legalPorts` guard is
  stricter than the firmware (ADR-0009), and there is no connect-side
  refusal to register. Rev 3 saw a downlink connect refused with a status
  (0x6) that still left the dpni on interface 0, but rev 5 from a fresh
  boot saw the downlink connect accepted cleanly with the state agreeing,
  so that refusal did not reproduce. Two cases remain unissued: a
  dpmac-uplink *disconnect* (never tested — V-DPDMUX-1 only ever destroyed
  its dpmac-uplink pairing, and rev 4's dpmac-uplink connect was refused
  by the rev-3 ghost so the disconnect was never reached) and a
  downlink-with-populated-uplink connect. Both are deferred to
  `dpdmux-typestate` (#12).
- 0x8 No resources, first seen on `dprtc`/`dpdbg` create, has now also
  been seen on `dprc disconnect` of an unconnected dpdmux interface.
- Kernel-side refusals are dmesg text, not an MC status. The dpsw probe
  refusal (−95 EOPNOTSUPP) is now scored from the kernel log and
  registered above (a `kernel` row); DPRC-I1's `-ENXIO "No more
  resources"` at probe still lands here once a suite scores it from
  `dmesg.txt`.
- Statuses 0x3, 0x5, 0x7, 0x9–0xC have never been seen on this board;
  0x4, 0x6 and 0x8 have.

## How a suite expects a refusal

A probe step carries `"refusal": "<Status>"` (a name from the table
above) beside its `cmd`: the command must exit nonzero and print that
status, judged from the captured output; an instruction step cannot
carry one. A generated suite takes `--expect-refusal <step>=<Status>`
at generation: the plan records it on the step, drops the model's
read-back expectation for that step (a refused action leaves the
model's post-state unreached by construction), keeps the probes as
evidence, and `diff` scores the step from `step-N-err.txt` and its
exit file. Either way the verdict records the status observed, and
the index lists every status a run scored, which is what the lint
matches against this register both ways.
