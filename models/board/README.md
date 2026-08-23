# models/board — the board program's scenario modules and suites

Phase-5 artifacts of `verify-foundation` (design D7): each board suite
is generated from a **scenario module** here — a Quint module wrapping
the core machine's actions with picks restricted to the scenario and
guards that leave exactly one action enabled per state, so `quint run
--mbt` freezes the same trace under any seed. Raw `main.qnt` simulation
is not usable suite input: unconstrained picks reach total-deny objects
and restool-unreachable containers, and the generator refuses such
traces (correctly). Each suite owns a directory here (`models/board/<ID>/`)
holding its scenario module, frozen trace and generated
`.sh`/`.plan.json`, committed together with this ledger; result files
stay under `results/` (gitignored — operator material).

`RECOVERY-VERIFIED` is the recovery-guarantee marker (ADR-0003 §7):
committed when suite V-RECOVERY-1 passed, its presence is what lets the
generator emit mutating suites.

## Suite ledger

| Suite | Module | Status |
|---|---|---|
| V-RECOVERY-1 | `recovery.qnt` | **passed** 2026-08-23 — recovery diff clean, all steps conform |
| V-DPRC-1 | `vdprc1.qnt` | **diverged twice**, model amended each time (ADR-0007). Rev 1 2026-08-23: sibling move refused → single-hop law. Rev 2 2026-08-23: anchored the two-hop route and the `dprc unassign` rendering, but standalone destroy of the moved dpni hit MC "No privilege" (restool exited 0 — read-back caught it) and the container destroy *evicted* its foreign resident instead of cascading → creator-bound destroy authority + release/evict by ownership; rev 2 left an ownerless dpni in dprc.1 that only a reboot clears. Rev 3 (repatriation route) **passed** 2026-08-23, 13/13: both unassign/assign directions exercised twice, repatriation restored destroy authority (ADR-0007 §2's positive anchor), and owned-resident release re-anchored with a dpbp |
| V-DPNI-1 | `vdpni1.qnt` | **passed** 2026-08-23 — destroy-while-plugged of a child-container dpni succeeded, confirming the in_use-blindness law; bare-create defaults captured for DPNI-I7 |
| V-LIFE-DPNI-1 | `vlife_dpni1.qnt` | **passed** 2026-08-23 — kernel bound the dpni through the §5 canonical order; census draw satisfied (positive face of DPBP-I4) |
| V-LIFE-DPIO-1 | `vlife_dpio1.qnt` | sat 2026-08-23 (batch 2), ran clean end to end on the transcript — dpio canonical order in the root, bind judged on the driver link (DPIO-I3 positive face); verdict pending the offline diff of the captured results. Teardown left the plugged dpmcp companion behind — restool refused the unplug on a driver-bound object and the suppressed stderr hid it; root-caused, cleaned by hand, teardown fixed |
| V-LIFE-DPSECI-1 | `vlife_dpseci1.qnt` | sat 2026-08-23 (batch 2), diverged at the restool layer: `dpseci create` mandates `--num-queues` and `--priorities` together, so the bare create was refused. Regenerated with restool's own example pair — 2 queues at priorities 2,4 — awaiting re-sitting |
| V-LIFE-DPDMAI-1 | `vlife_dpdmai1.qnt` | sat 2026-08-23 (batch 2), ran clean end to end on the transcript — bare-create dpdmai; the bind step is Read evidence (DPDMAI-I3/I5): a missing driver link is a finding about kernel handling of MC defaults, not automatically a model error; verdict pending the offline diff of the captured results. Teardown left the plugged dpmcp companion behind for the same reason as V-LIFE-DPIO-1 — driver-bound unplug refused, stderr suppressed; root-caused, cleaned by hand, teardown fixed |
| V-LIFE-DPDCEI-1 | `vlife_dpdcei1.qnt` | sat 2026-08-23 (batch 2), failed at the restool layer, not the MC: there is no bare `dpdcei create` — restool mandates `--engine` and `--priority`, so it refused, the object never existed, and the later steps rendered against an empty name. Regenerated with an explicit DPDCEI_ENGINE_DECOMPRESSION at priority 1; awaiting re-sitting, no reboot needed because nothing was created and the scratch container was destroyed cleanly |
| V-DPCI-1 | `vdpci1.qnt` | sat 2026-08-23 (batch 2), diverged: both creates landed, but the connect — issued on the scratch container the pair lives in — was refused by the MC with No privilege. A container restool creates without explicit options has no topology-change permission, and `dprc connect` runs on whichever container it is named with. The connect now renders against the root ancestor instead. The destroy-while-connected probe (dpci.md unknown #2) went unanswered, since the pair never got connected; awaiting re-sitting |

V-LIFE-DPNI-1 carries the "per-family lifecycle scenarios" of design
D7 step 2 for the dpni family: the §5 canonical order through the
kernel's own probe, judged on the sysfs driver link. Its objects live
in dprc.1 by necessity (kernel binds happen in the Linux root only,
DPRC-I6); the teardown trap restores the container.

## Batch-suite expressibility — deferred §1 scenarios

A batch suite renders the steps a trace *took*; a refused action is a
disabled action, not a step, so refusal probes cannot be traced. And
the adapter's observation surface today is present / plugged /
endpoint / driver-bound — probes beyond it need the online driver
(ADR-0002 §4) or an adapter extension taken when a scenario demands
it. Dispositions for the §1 rows not yet generated (no silent drops,
design D9):

| Scenario | Why deferred | Where it goes |
|---|---|---|
| V-DPRC-2 | per-option-bit *denial* probes — refusals. The first data point is already board-anchored: a connect issued on a default-created container comes back No privilege, exactly what the omitted topology-changes bit predicts | online driver |
| V-DPRC-3 | lock/unlock round-trips leave no monotone state, so a deterministic trace cannot phase them; the interesting half is the refusal-under-lock anyway | online driver |
| V-DPRC-4 | read-only `mc.global`/`dump-mem` observation, no adapter probe | online driver |
| V-DPRC-5 | needs a bus-visibility observation the adapter does not take yet; the root-only-reach law itself is already board-exercised (`EVIDENCE`) | adapter extension or online |
| V-DPNI-2 | attribute read-back (num_queues) and dead-option *exit-shape* probes | online driver |
| V-DPNI-3 | post-bind runtime-state mutation is not restool-drivable; bind/unbind faces covered by V-LIFE-DPNI-1 | online driver |
| V-DPNI-4 | raw command via `/dev/dprc.N` | online driver |
| V-DPMAC-1 | read-only info probes, no mutating steps to trace | online driver |
| V-DPMAC-2 | the model forbids dpmac create (DPMAC-I1) — the probe deliberately tests an unknown against a model law; a board answer amends the model | online driver |
| V-POOL-1..3 | exhaustion/defer faces are refusals; kernel-internal draws are `Await`; the positive census face is judged by V-LIFE-DPNI-1 | online driver |
| V-DPSECI-1..2 | create-validation refusals and raw GET_ATTR read-back; the positive lifecycle face is V-LIFE-DPSECI-1 | online driver |
| V-DPSW-1..3 | creates need adapter `create_args` rows (PER_VLAN/PER_OBJECT, num_ifs) and per-scenario endpoint counts; V-DPSW-2 is a raw-reset probe | batch 3 (positive create+connect faces) / online (probes) |
| V-DPDMUX-1..3 | same `create_args` gap; V-DPDMUX-2's dpni-uplink refusal is model-forbidden (like V-DPMAC-2) so it cannot be traced; V-DPDMUX-3 is a cross-regime reset probe | batch 3 (positive uplink connect) / online (probes) |
| V-DPCI-2 | options-discard hardware probe (OPR config), attribute read-back | online driver |
| V-DPDCEI-1 probes | GET_API_VERSION / dce_version reads; the create face is V-LIFE-DPDCEI-1 | online driver |
| V-DPDMAI-2 | shutdown/reboot-cycle shaped — the V-RECOVERY-1 two-script pattern, not a plain batch suite | later, recovery-shaped suite |
| V-DPRTC-1..2, V-DPDBG-1 | root-container residents with fixed disposition (traffic-inventory §4, design D7 step 4): online driver, per-step operator confirmation — task 5.4, not 5.2 | online driver (5.4) |
| V-GENDPL-1 | needs a `generate-dpl` emit-and-diff probe no crate code has | online driver or adapter extension |

Batch 2 (authored, ledger above) covers the remaining positive
lifecycle faces that render with the adapter as-is: dpio/dpseci/dpdmai
canonical orders in the root, dpdcei in a scratch container, and the
dpci pair connect. dpsw/dpdmux positive faces are batch 3, gated on the
adapter `create_args` work.

dpdcei and dpseci turned out to need `create_args` rows of their own —
the same gap that gates batch 3, found a batch early. The two refuse
differently worded (dpdcei names each missing flag, dpseci demands its
queue count and priorities as a pair), which is why only the board
sitting caught the second. dpdmai and dpci bare creates did land on the
board, so those two stay bare.

The same sitting exposed a teardown gap. Every root-resident suite left
its plugged dpmcp companion behind: after `dprc sync` the kernel's
allocator claims free companions into its portal pool, and restool
refuses to unplug an object that holds a driver, so the trap's unplug
was rejected and the destroy after it had nothing to remove. The
refusal was invisible because teardown discarded stderr — the same
read-back lesson the step layer already learned, one layer down: an
unchecked command is not a completed one. Teardown now unbinds a
root-resident object through sysfs before unplugging it, and its stderr
goes to `teardown.log` in the results directory. Child-container
objects never bind (DPRC-I6), so they skip the unbind.

## Regenerating

Each module's header records its freeze command. Then:

```sh
cargo run -p dpaa2-verify -- generate --trace models/board/<ID>/<t>.itf.json --id <ID> --out models/board/<ID>
```

(V-RECOVERY-1 adds `--recovery-verification`.) Offline diff after a
sitting:

```sh
cargo run -p dpaa2-verify -- diff --plan models/board/<ID>/<ID>.plan.json --results results/<dir>
```
