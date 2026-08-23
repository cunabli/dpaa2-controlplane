# models/board — the board program's scenario modules and suites

Phase-5 artifacts of `verify-foundation` (design D7): each board suite
is generated from a **scenario module** here — a Quint module wrapping
the core machine's actions with picks restricted to the scenario and
guards that leave exactly one action enabled per state, so `quint run
--mbt` freezes the same trace under any seed. Raw `main.qnt` simulation
is not usable suite input: unconstrained picks reach total-deny objects
and restool-unreachable containers, and the generator refuses such
traces (correctly). The frozen trace (`models/traces/`), the generated
`.sh`/`.plan.json`, and this ledger are committed together; result
files stay under `results/` (gitignored — operator material).

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
| V-DPRC-2 | per-option-bit *denial* probes — refusals | online driver |
| V-DPRC-3 | lock/unlock round-trips leave no monotone state, so a deterministic trace cannot phase them; the interesting half is the refusal-under-lock anyway | online driver |
| V-DPRC-4 | read-only `mc.global`/`dump-mem` observation, no adapter probe | online driver |
| V-DPRC-5 | needs a bus-visibility observation the adapter does not take yet; the root-only-reach law itself is already board-exercised (`EVIDENCE`) | adapter extension or online |
| V-DPNI-2 | attribute read-back (num_queues) and dead-option *exit-shape* probes | online driver |
| V-DPNI-3 | post-bind runtime-state mutation is not restool-drivable; bind/unbind faces covered by V-LIFE-DPNI-1 | online driver |
| V-DPNI-4 | raw command via `/dev/dprc.N` | online driver |
| V-DPMAC-1 | read-only info probes, no mutating steps to trace | online driver |
| V-DPMAC-2 | the model forbids dpmac create (DPMAC-I1) — the probe deliberately tests an unknown against a model law; a board answer amends the model | online driver |
| V-POOL-1..3 | exhaustion/defer faces are refusals; kernel-internal draws are `Await`; the positive census face is judged by V-LIFE-DPNI-1 | online driver |

Remaining §1 families (dpio, dpseci, dpsw, dpdmux, dpci, dpdcei,
dpdmai, dprtc, dpdbg, gendpl) follow in later batches, same split:
positive lifecycle faces as batch suites, refusal/raw/read-only faces
to the online driver.

## Regenerating

Each module's header records its freeze command. Then:

```sh
cargo run -p dpaa2-verify -- generate --trace models/traces/<t>.itf.json --id <ID> --out models/board
```

(V-RECOVERY-1 adds `--recovery-verification`.) Offline diff after a
sitting:

```sh
cargo run -p dpaa2-verify -- diff --plan models/board/<ID>.plan.json --results results/<dir>
```
