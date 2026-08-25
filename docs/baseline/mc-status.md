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
| — | unknown | dprc · assign `--child` | A sibling-to-sibling move (source and destination are not parent and child). Exit 255 says `-EPERM`, which restool's mapping makes No privilege, but rev 1 predates stderr capture so the status text was never recorded — task 5.9 fills it | MC | V-DPRC-1 rev 1 |

## What the register does not hold yet

- Refusals the models predict but no suite has issued: the kernel's
  probe refusal of a default-built dpsw (DPSW-I1's refusal face), the
  dpdmux uplink→dpni connect (DPDMUX-I8), `dpaiop create` on this
  platform (DPAIOP-I1/I2), dpseci priority validation at the MC layer
  (DPSECI-I2), the dpni dead-option create (DPNI-I6). Task 5.9 issues
  each with an expected-refusal step and adds its row here; a status
  that contradicts a model guard amends the model in the same change.
- Kernel-side refusals (`-ENXIO "No more resources"` at probe, DPRC-I1)
  are dmesg text, not an MC status; they land here once a suite scores
  them from `dmesg.txt`.
- Statuses 0x3, 0x5, 0x7, 0x9–0xC have never been seen on this board.

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
