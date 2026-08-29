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

Online-driver suites (task 5.4 onward) come in a second shape: a
hand-authored **probe plan** (`probes.json`) beside — or instead of — a
driven trace, for the steps a trace cannot express (refusals are
disabled actions, write-only state has no expected observation). Probe
plans run under `dpaa2-verify drive --probes` with mandatory per-step
confirmation and the same safety envelope; their expectations are
human-written oracles, so the standing `board_artifacts` test only
guarantees they parse and clear the envelope, and the verdicts below
carry the judgment.

`RECOVERY-VERIFIED` is the recovery-guarantee marker (ADR-0003 §7):
committed when suite V-RECOVERY-1 passed, its presence is what lets the
generator emit mutating suites. `baselines/` holds the read-only board
snapshot script (`snapshot.sh`) and the committed clean-boot reference
(`reference.json`) a sitting's residue is diffed against.
`VERDICTS.json` is the machine-readable side of the suite ledger below:
one entry per run (suite → run label → pass/fail, steps judged, hook
lines, date, revision, plan hash, archive), written by `dpaa2-verify
diff` and read by the ledger lint, so a status cell here or a "verified
(V-…)" cell in `models/COVERAGE.md` has to match a recorded verdict.

## Suite ledger

| Suite | Module | Status |
|---|---|---|
| V-RECOVERY-1 | `recovery.qnt` | **passed** 2026-08-23 — recovery diff clean, all steps conform |
| V-DPRC-1 | `vdprc1.qnt` | **diverged twice**, model amended each time (ADR-0007). Rev 1 2026-08-23: sibling move refused → single-hop law. Rev 2 2026-08-23: anchored the two-hop route and the `dprc unassign` rendering, but standalone destroy of the moved dpni hit MC "No privilege" (restool exited 0 — read-back caught it) and the container destroy *evicted* its foreign resident instead of cascading → creator-bound destroy authority + release/evict by ownership; rev 2 left an ownerless dpni in dprc.1 that only a reboot clears. Rev 3 (repatriation route) **passed** 2026-08-23, 13/13: both unassign/assign directions exercised twice, repatriation restored destroy authority (ADR-0007 §2's positive anchor), and owned-resident release re-anchored with a dpbp |
| V-DPNI-1 | `vdpni1.qnt` | **passed** 2026-08-23 — destroy-while-plugged of a child-container dpni succeeded, confirming the in_use-blindness law. The suite probes only `dprc show`, so the bare create's defaults were never read back and DPNI-I7 stays open (ledger pass, task 5.6) |
| V-LIFE-DPNI-1 | `vlife_dpni1.qnt` | **passed** 2026-08-23 — kernel bound the dpni through the §5 canonical order; census draw satisfied (positive face of DPBP-I4) |
| V-LIFE-DPIO-1 | `vlife_dpio1.qnt` | **passed** 2026-08-23 (rev 2), 6/6 under ADR-0008 — the kernel binds one dpio per CPU and the boot layout fills every seat, so a dpio created at runtime never binds; the probe fails inside the kernel and leaves nothing drawn. Rev 1 diverged only in the harness: the model expected a bind, and the teardown leaked the plugged dpmcp companion because restool refuses to unplug a driver-bound object and the refusal went to /dev/null. Both fixed; rev 2 left no residue and the companion was reclaimed cleanly |
| V-LIFE-DPSECI-1 | `vlife_dpseci1.qnt` | **passed** 2026-08-23 (rev 2), 6/6 under ADR-0008 — the crypto-API algorithm names are one global namespace and the boot-time dpseci claims them, so every later dpseci is refused its registrations and stays unbound for the rest of that boot. Rev 1 never got that far: `dpseci create` mandates `--num-queues` and `--priorities` together, so the bare create was refused. Regenerated with restool's own example pair, 2 queues at priorities 2,4. Teardown residue fixed with V-LIFE-DPIO-1's; rev 2 clean |
| V-LIFE-DPDMAI-1 | `vlife_dpdmai1.qnt` | **passed** 2026-08-23 (rev 2), 6/6 under ADR-0008 — the reference kernel registers no qdma driver at all, so nothing ever claims a dpdmai; the unbound read-back is the conforming answer, not a gap in how the kernel handles MC defaults. It settles nothing about the defaults themselves: the suite probes only `dprc show`, so DPDMAI-I5's MC-chosen queue count is unread, and DPDMAI-I3's consumer-shape coupling is unfalsifiable with no consumer (ledger pass, task 5.6). Rev 1 diverged only on the model's bind expectation and leaked its dpmcp companion through the same teardown hole; both fixed, rev 2 clean |
| V-LIFE-DPDCEI-1 | `vlife_dpdcei1.qnt` | **passed** 2026-08-23 (rev 2), 5/5 — create, plug and destroy of a dpdcei in a scratch container, no driver to await. Rev 1 failed at the restool layer, not the MC: there is no bare `dpdcei create`, since restool mandates `--engine` and `--priority`. Regenerated with an explicit DPDCEI_ENGINE_DECOMPRESSION at priority 1 |
| V-DPCI-1 | `vdpci1.qnt` | **passed** 2026-08-23 (rev 2), 7/7 — answers dpci.md unknown #2: the MC destroys a connected dpci without demanding a disconnect first, the edge dying with the object as the model assumed, and the connect itself is legal while both endpoints are unplugged. Rev 1 diverged twice over: the connect was issued on the scratch container the pair lives in and the MC refused it with No privilege, which anchored the topology-changes option-bit finding (connects now render against the root ancestor); and the conforming rev-2 connect was then scored wrong by a read-back parser that knew only one family's wording for the peer line |
| V-DPSW-1 | `vdpsw1.qnt` | **passed** 2026-08-23 (batch 3), 9/9 — the switch driver does take a dpsw created at runtime, but only one built in the shape it accepts: control interface on, flooding and broadcast both scoped per FDB. Those are not restool's silent defaults — the driver code refuses the default shape at probe — so the create carries them explicitly; the refusal itself was never issued on the board and stays a code-derived prediction (ledger pass, task 5.6). The census drew the created dpmcp and dpbp companions rather than any boot resident, and the connect read back through the per-interface dialect |
| V-DPDMUX-1 | `vdpdmux1.qnt` | **passed** 2026-08-23 (rev 2, batch 3), 7/7 — the evb driver takes a runtime dpdmux and gates only on the object's API version, so no create-time configuration is at stake; the uplink-to-dpmac connect was clean. Rev 1's own face passed too, but its unspaced teardown reproducibly tripped the ADR-0008 rescan race: three boot residents silently unbound in one scan window — the boot dpni among them, which took the management interface down — plus a boot dpmcp fully removed and re-added. A settle after each destroy removed every marker and every casualty in rev 2 |
| V-LINK-1 | `vlink1.qnt` | **passed** 2026-08-23 (batch 4), 4/4 — answers dpci.md unknown #1 and settles DPCI-I5: a restool-created dpci pair reads `link status: 0 - down` right after the connect, with the peer named and the peer's priorities visible. The edge is up and the link is not; the connect carries no link state and restool, which has no enable verb for this family, can never raise it — link-up is the consumer's to grant |
| V-LINK-2 | `vlink2.qnt` | **passed** 2026-08-24 (rev 3, batch 4), 16/16 — the first suite to flap a wired dpmac. With a real cable pull, `dpni info`'s `link status:` tracked PHY reality down and back up, and since every kernel link push carries `state_valid=0`, that answers dpmac.md unknown #2 on the kernel path: the `up` bit does take effect, with propagation lag. Revs 1 and 2 both read a stale `up` at the flap-down step, for two causes the bench work separated: an admin-down of the peer's interface never drops the light it transmits, so only pulling the cable is a link-down stimulus on this wiring; and the MC-visible link state lags the local carrier flag, so a probe fired the moment the operator acknowledges reads the old answer. Rev 3's acknowledgments require the carrier flag and the restool read-back to agree before continuing; the post-sitting census was clean at the 97-object baseline, so the teardown reclaimed the full scratch set. Evidence probes also caught both endpoint lines' `, link is up/down` text co-varying with the flap while the connection edge itself persisted — DPMAC-I5's law stands, its assumed independence from link state does not |
| V-DPDBG-1 | `vdpdbg1.qnt` + `probes.json` | **passed** 2026-08-24 (rev 2, batch 5): trace 4/4, probes 10/10 — the online driver's first outing and the first probe plan. The lifecycle face conforms once the verbs are rendered bare: restool's `dpdbg create`/`destroy` take no arguments (root container hardcoded, id pinned to 0), which also makes the non-root half of DPDBG-I1 restool-unreachable — recorded, not dropped. The singleton refusal is MC No resources (0x8). `set` takes exactly one module option per invocation (rev 1 tripped on the combined form) and `--level=99` is refused by firmware with Configuration error (0x6), not clamped. Both dumps exit 0 with the artifact only in the MC log — DPDBG-I3's law held on the board |
| V-DPRTC-1 | `probes.json` | **passed** 2026-08-24 (batch 5), 4/4 — the dprtc singleton refuses a second create with the same No resources (0x8) as dpdbg's, hinting at a shared firmware path, and `dprc show` is byte-identical before and after: the 10.37 blast-radius fix observed |
| V-DPRTC-2 | `probes.json` | **passed** 2026-08-24 (batch 5), 3/3 — API version 2.3 (the flib's, not restool's 2.0 header), a verbose surface with no time or frequency anywhere (DPRTC-I3 anchored), kernel ownership confirmed through the registered driver name `fsl_dpaa2_ptp` and its PTP chardev, and the DT ptp-timer window recorded for unknown 5's observable half. Unknown 6 was left open there and answered by **rev 2** (2026-08-29, 5/5): `ethtool -T` on the kernel dpni lists `onestep-sync` among the hardware transmit timestamp modes and the DPAA2 PTP clock advertises 2 alarms, 2 external timestamps, 3 periodic outputs and PPS — one-step 1588 is claimable on this kernel/firmware; the manual's "two-step only" row is stale |
| V-DPRTC-3 | `probes.json` + `postboot.probes.json` | **passed** 2026-08-24 (rev 2, batch 5), 8/8 + postboot 3/3 — answers dprtc.md unknown 3: the DPL-born dprtc.0 *can* be destroyed by GPP software, but only unbound; while the driver holds it, restool's own client guard refuses before the MC is asked. The sysfs unbind takes kernel PTP down with it (chardev gone with the driver link), the destroy reads back absent at a 96-object census, and the closing reboot restores object, driver and chardev at the 97 baseline — the recovery guarantee's restore direction verified for the first time on a deleted DPL-born resident. Rev 1 failed only in the harness: the unbind named the hyphenated module spelling where sysfs carries the registered underscore name |
| V-TRAF-0 | `vtraf0.qnt` + `traffic.sh` (suite hook) | **passed** 2026-08-24 (rev 3, batch 6), 14/14 and both legs: the peer's 16-frame broadcast burst counted 16 on the dpni (`ingress_all_frames` 0 → 16) and 16 on the kernel netdev; 8 pings unicast to the peer port's MAC counted 8 out (`egress_all_frames` +8) and 8 in on the peer, as `ip4` and `drops` — that port's `rx packets` line never ticks, which read as "no rx" in rev 1 until the other two counters were read. The dpni's own statistics pages are an exact frame oracle, so reachability needs no capture on either side. Revs 1 and 2 ran under the online driver and settled the same numbers, but the shape was wrong: a probe plan cannot name what the trace created, so its by-hand teardown left the companions behind twice (six residue objects, removed by hand), and its per-step prose was unreadable at the prompt — hence the suite hook, and the closing `No privilege` in teardown.log is the vacuous boot-edge restore on a bare boot, as in V-LINK-2 |
| V-READBACK-1 | `vreadback1.qnt` + `readback.sh` (suite hook) | **passed** 2026-08-29 (rev 2), steps 6/6 + hook 10/10 — the corrected hook oracle confirms the rev-1 read-back. Five bare creates in one scratch container (dpni, dpdmai, a dpio on DPIO_NO_CHANNEL, dpbp, dpdcei), each read back with `info` from the hook while the set stood. The hook's read-back corrected the prediction rather than the board: a bare dpni carries 16 MAC entries and 0 QoS entries, not the 80/64 the baseline table had taken from restool's maxima (DPNI-I7 settled; the hook now asserts the observed defaults). Recorded: a bare dpdmai is 1 queue × 2 priorities (DPDMAI-I5), a NO_CHANNEL dpio keeps its requested 8 priorities (DPIO-I3's reported half), a runtime dpbp's bpid equals its object id (DPBP-I5 stays unfalsified), dpdcei reports API 2.3 (its version half). Rev 1 (2026-08-25) had the hook oracle wrong on two rows (8/10) and was regenerated; both sittings were judged clean by the structured snapshot, zero deltas against the clean-boot reference. The dpio is the first suite to render a family with `--create-args` |
| V-LINK-5 | `vlink5.qnt` | **ran once** 2026-08-23 (batch 4) and **retired with its answer** — answers dprc.md unknown #9: `dprc assign --plugged=0` on a kernel-bound, link-up, netdev-backed dpni exited 240 (8-bit −EBUSY) with the object still plugged and the driver still bound. A refusal, not a race and not a silent drop, and the second anchor after V-LIFE-DPIO-1 rev 1's teardown refusal on a driver-bound dpmcp. Every other step passed. The model's `unplugAt` now requires an unbound object, which makes the probing step untraceable — the retirement is the finding, and the module header carries the do-not-regenerate note |
| V-DPSW-4 | `vdpsw4.qnt` + `refusal.sh` (suite hook) | **passed** 2026-08-29 (rev 1), 7/7 + hook 2/2 — V-DPSW-1's refusal face (DPSW-I1's refusal face, DPSW-I2): the same create+connect trace, but the switch is built the way restool's silent defaults build it (flooding PER_VLAN, broadcast PER_OBJECT, via `--create-args dpsw=--num-ifs=2`, which drops the PER_FDB flags the positive suite carried). The MC created and connected that switch without complaint — `restool dpsw info` reads flooding `DPSW_FLOODING_PER_VLAN`, broadcast `DPSW_BROADCAST_PER_OBJECT` — and the kernel `fsl_dpaa2_switch` driver refused it at probe: dmesg `Flooding domain is not per FDB, cannot probe`, error −95 (EOPNOTSUPP), driver link empty. So the refusal is the kernel's, not the MC's. The trace has no kernelBind step; the hook read the negative outcome. A code-derived prediction in V-DPSW-1, now issued on the board |
| V-DPDMUX-2 | `vdpdmux2.qnt` + `uplink.sh` (suite hook) | **diverged, final** (rev 1–5, 2026-08-29), model guard kept (ADR-0009). DPDMUX-I8's refusal face: the demux uplink (interface 0) may only face a dpmac, model-forbidden in `connect.legalPorts`, so the illegal pairing lives in the hook. MC 10.39 *accepts* `dprc connect` of a dpni onto the uplink and then refuses the disconnect from either end, Configuration error (rev 2) — a pairing it cannot undo. Rev 5, from a fresh boot, settled the downlink cleanly: the connect was accepted with the state agreeing (`dpdmux info` interface 1 = the dpni, `dpni info` endpoint = `dpdmux.N.1`), and no end could disconnect it — dpni end 0x6, demux downlink end 0x6, demux uplink (bare name) 0x8 — so a dpni is un-disconnectable on any interface. No pairing survived a reboot (all interfaces read none before any connect) or a destroy-and-recreate (rev 5), refuting rev 4's ghost hypothesis. Rev 3's refused-connect-that-still-connects and rev 4's ghost stand as observed on their boots; rev 3's shape did not reproduce on rev 5's fresh boot. The model keeps its `legalPorts` guard ahead of the firmware (ADR-0009). **Management-link risk**: the teardown destroys a connected pair — run it last in a sitting, expect a rebind or reboot (ADR-0008 §7). Rev 5's hook additionally ran an in-run phase-4 destroy that tripped the rescan race and took the management interface down; that phase is removed from the committed hook |
| V-DPRC-6 | `vdprc6.qnt` + `moves.sh` (suite hook) | **passed** 2026-08-29 (rev 1), 4/4 + hook 3/3 — V-DPRC-1's plugged-move face (DPRC-I3, a plugged object cannot be moved) plus the sibling-move status the MC status register listed as unknown. The trace stands two scratch containers and a dpbp plugged in the first; nothing is bus-visible (DPRC-I6), so the refusals are the MC's own gate, not a driver's. (a) Pulling the *plugged* dpbp one hop up to the root is refused by restool's own client guard (`cannot be moved because it is currently in plugged state` / `unplug it first`) before any MC command, and the dpbp stays in dprc.2. (b) After unplugging, the single-command sibling move (`dprc assign --child`, siblings) with the exact rendering V-DPRC-1 rev 1 used (which exited 255) is refused by the MC with No privilege (0x4), which fills the register's unknown sibling-move text |
| V-DPAIOP-1 | `probes.json` | **passed** 2026-08-29 (rev 1), 6/6 — DPAIOP-I1/I2 refusal probes: `dpaiop create --aiop-container-id=dprc.1` on this AIOP-less silicon is refused Configuration error (0x6, exit 250), nothing created; issued against the root container, which itself permits object creation, so the refusal confirms container create-permission does not imply create-permission — the platform gate lives in the MC's dpaiop create handler, not the DPRC options. `dprc create --options=DPRC_CFG_OPT_AIOP` was accepted the same sitting (the AIOP-flagged container created, unplugged, destroyed cleanly), so only `dpaiop create` is gated |
| V-DPSECI-1 | `probes.json` | **passed** 2026-08-29 (rev 1), 8/8 — DPSECI-I2 create-validation: `--num-queues=1 --priorities=0`, `--priorities=9`, and `--num-queues=2 --priorities=1` are each refused by restool's own parser (exit 234, `Invalid priority value.` / `Please set 2 priorities`) before any MC command is built, so the MC-layer validation is unreachable through restool — that unreachability is the finding, not an MC refusal. The positive lifecycle face is V-LIFE-DPSECI-1 |
| V-DPNI-2 | `probes.json` | **passed** 2026-08-29 (rev 1), 12/12 — the DPNI-I6 inversion and the num_queues ceiling: `dpni create --max-senders=8` (a dead v9-era option) creates the dpni and prints its id, *then* exits 234, so exit status is no side-effect oracle and convergence rests on read-back (the object stands, reads back present, and is destroyed by the next step); then a bracketing walk of num_queues 32, 24, 28, 20 — all accepted at create. restool caps the option at 32, so the MC ceiling (if any) lies at or above restool's reach (the true WRIOP-3.0.0 ceiling is dpni.md unknown 2); the walk found no refusal, and the closing census read dpni.1 absent |
| V-DPRC-5 | `vdprc5.qnt` + `visibility.sh` (suite hook) | **passed** 2026-08-29 (rev 1), 3/3 + hook 4/4 — DPRC-I6 and DPCI-I3's rescan face. A dpci created in a scratch child never appears under `/sys/bus/fsl-mc/devices`, before or after `dprc sync`, while `dprc show` lists it in the child all along: bus visibility reaches root residents only (DPRC-I6 held). The root-created dpci was on the bus *before* the hook's explicit rescan — `/sys/bus/fsl-mc/autorescan` reads 1 on this BSP, so the dprc driver rescans on the MC's object-added interrupt and a root create becomes bus-visible without the mutator asking. The MC-side law (a create command triggers no rescan, `createTriggersRescan=false`) stands; the sysfs lag DPCI-I3 describes is closed by a kernel setting here, so it is a BSP property to read, never a law to assume |
| V-DPRC-3 | `vdprc3.qnt` + `lock.sh` (suite hook) | rev 1 2026-08-29, steps 2/2, hook 6/7 — **failed on one prediction; the board did not diverge** (DPRC-I11, dprc.md unknown 4). `set-locked --locked=1` issued from the root is accepted; under the lock a `dprc assign --plugged=1` on the child's dpbp is refused No privilege (0x4) and the dpbp reads back unplugged; `dprc show` and `dpbp info` keep working; `--locked=0` from the root is accepted and the same plug then succeeds. The FAIL line is the register's guess: `dprc set-label` on the locked child's dpbp is *accepted* and the label reads back — the lock strips assign, not labels. restool's `set-locked` always opens the target's parent portal, so "who may unlock" is exercisable only as the root; a child-portal unlock needs a portal restool never opens. The hook oracle is corrected (rev 2 asserts the label lands) and re-runs with the next sitting |
| V-DPCI-2 | `vdpci2.qnt` + `pair.sh` (suite hook) | **passed** 2026-08-29 (rev 1), 20/20 + hook 1/1 — dpci.md unknowns 3, 4, 5. Sixteen 2-priority dpcis in one scratch child plus two in the root were all created: the bounded ceiling walk found no ceiling (19 dpcis stood at once with the hook's fixture; no pool in the `--resources` walk names dpci). The two root dpcis connect to each other inside the root (unknown 3: same-container connect is legal there). The hook's 1-priority fixture connected to a 2-priority dpci without refusal (unknown 5: accepted, not rejected), but each end's `dpci info` reports **its own** count as `peer's num_of_priorities` — the peer attribute mirrors the local value, so which count the link carries is unobservable from the control plane; recorded as ambivalent (a traffic probe settles it, restool cannot). The teardown of 19 objects including the connected root pair was clean: spaced destroys, no bus incident, zero snapshot deltas |
| V-GENDPL-1 | `vgendpl1.qnt` + `dpl.sh` (suite hook) | **passed** 2026-08-29 (rev 1), 4/4 + hook 1/1 — DPDCEI-I2 and DPDMAI-I4 (DPAIOP-I3 is re-anchored: no dpaiop can exist on this silicon, V-DPAIOP-1). `generate-dpl` of a scratch child holding a dpdcei (`--engine=DPDCEI_ENGINE_DECOMPRESSION --priority=2`), a dpdmai (`--priorities=2,4`) and a dpci (`--num-priorities=2`) is not a round-trip: the dpdcei node carries `engine` only — the priority is write-only, absent from `dpdcei info` as well; the dpdmai node emits `priorities = <0x2>`, the *count* `dpdmai info` reports, where the DPL grammar expects the list `<2 4>`, so a re-applied DPL builds a different object; the dpci round-trips (`num_of_priorities = <0x2>`); and the child container is emitted with `parent = "none"` and the default option string, i.e. as a root. The emitted `.dts` is kept beside the results; the by-eye diff is folded into dpdcei.md/dpdmai.md's owners via COVERAGE |
| V-DPRC-2-NOCREATE-1 | `vdprc2nocreate.qnt` | rev 1 2026-08-29, 1/2 — **failed on the predicted refusal; the board accepted.** A dpbp create into a child made *without* `DPRC_CFG_OPT_OBJ_CREATE_ALLOWED` succeeds and reads back in the child. restool issues every create through the root's portal with the child's open token, and the bit gates creates issued from the child's *own* portal — one restool never opens. Matrix row: OBJ_CREATE_ALLOWED does not constrain a parent creating on the child's behalf; its refusal face is unreachable through restool. Rev 2 is regenerated with the observed expectation (no refusal) and re-runs with the next sitting |
| V-DPRC-2-NOSPAWN-1 | `vdprc2nospawn.qnt` | rev 1 2026-08-29, 1/2 — **failed on the predicted status only.** A `dprc create` under a child made without `SPAWN_ALLOWED` is refused, with Configuration error (0x6) rather than the predicted No privilege, and the child stays empty. Rev 2 carries `--expect-refusal 1="Configuration error"` and re-runs with the next sitting |
| V-DPRC-2-NOALLOC-1 | `vdprc2noalloc.qnt` | rev 1 2026-08-29, 1/2 — **failed on the predicted status only.** A dpbp create into a child made without `ALLOC_ALLOWED` is refused No resources (0x8): the child cannot draw the buffer-pool id from its parent's pool, so the create fails as a resource shortfall, not a privilege check. Rev 2 carries `--expect-refusal 1="No resources"` and re-runs with the next sitting |
| V-DPRC-2-TOPO-1 | `vdprc2topo.qnt` + `topo.sh` (suite hook) | **passed** 2026-08-29 (rev 1), 4/4 + hook 1/1 — with `TOPOLOGY_CHANGES_ALLOWED` added, a disconnect and a connect issued *on the child* (`dprc disconnect`/`connect dprc.2 …`) both succeed. The control for V-DPCI-1 rev 1: that No privilege was the missing topology bit, and connects rendered against the root ancestor never needed it |
| V-DPRC-2-PL-1 | `vdprc2pl.qnt` + `pl.sh` (suite hook) | **passed** 2026-08-29 (rev 1), 2/2 + hook 2/2 — `PL_ALLOWED` is accepted at create and reads back in `dprc info --verbose` (options 0xc7: SPAWN, ALLOC, OBJ_CREATE, IRQ_CFG, PL); nothing else in the surface changes and a dpbp create in that child behaves as in a default one. What the bit enables is not observable through restool |
| V-DPMAC-1 | `probes.json` | **passed** 2026-08-29 (rev 1), 5/5 — DPMAC-I7 / dpmac.md unknown 7. `dpmac info --verbose` on the five unwired ports prints exactly 28 counter rows each, out of the 62 restool 2.4 asks for: the 34 counters MC 10.39.0 does not know are refused and skipped silently (`dpmac_commands.c` swallows the error), identical across the 25G (CAUI) and 10G (XFI) ports, so the vocabulary is firmware-wide, not per port, and the row count is the only observable of a refusal. Surface: endpoint state −1 / no object, link type PHY, max rate per interface type |
| V-DPRC-4 | `probes.json` | **passed** 2026-08-29 (rev 1), 9/9 — dprc.md unknowns 5, 7, 10, 11, 12. `dprc show mc.global` lists dprc.1 alone; `dprc info mc.global` and `dump-mem mc.global` are refused by restool itself (`dprc.0 does not exist` / `Invalid MC object name`, exit 234) before any MC command — the alias exists for `show` only; `dprc show mc.global --resources` is accepted and lists the MC-level pools (bp 63, mcp 203, swp 49, fq 1981, cg 253, qd 253, …). `dump-mem dprc.1 --partition_id=MEM_PART_PEB` prints one page holding one free block (offset 0x80000, 1.5 MiB). dpmcp.14 reads back plugged in dprc.1 — a consumed companion, not an id hole (the 5.5 premise corrected). `/dev/dprc.1` is 0600 root:root and the only udev rule naming dprc tags dprc.1 to start the provisioning unit with no mode change, so the portal needs root. `/sys/bus/fsl-mc/autorescan` reads 1 |

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
| V-DPNI-2 | attribute read-back (num_queues ceiling) and the dead-option *exit-shape* probe (DPNI-I6 inversion) | landed as `V-DPNI-2/probes.json` (task 5.9), ran 2026-08-29 |
| V-DPNI-3 | post-bind runtime-state mutation is not restool-drivable; bind/unbind faces covered by V-LIFE-DPNI-1 | online driver |
| V-DPNI-4 | raw command via `/dev/dprc.N` | online driver |
| V-DPMAC-2 | the model forbids dpmac create (DPMAC-I1) — the probe deliberately tests an unknown against a model law; a board answer amends the model | online driver |
| V-POOL-1..3 | exhaustion/defer faces are refusals; kernel-internal draws are `Await`; the positive census face is judged by V-LIFE-DPNI-1 | online driver |
| V-DPSECI-1 | create-validation refusals (priority range, count-vs-num-queues); the positive lifecycle face is V-LIFE-DPSECI-1 | landed as `V-DPSECI-1/probes.json` (task 5.9), ran 2026-08-29 |
| V-DPSECI-2 | raw GET_ATTR attribute read-back | online driver |
| V-DPSW-2..3 | V-DPSW-2 is a raw-reset probe; V-DPSW-3 needs per-scenario endpoint counts. The positive create+connect face landed as V-DPSW-1 | online driver |
| V-DPDMUX-2 | the dpni-uplink refusal is model-forbidden (like V-DPMAC-2) so it cannot be traced; carried in a suite hook instead. The positive uplink connect landed as V-DPDMUX-1 | landed as `V-DPDMUX-2/` (task 5.9), ran 2026-08-29, final (rev 5) |
| V-DPDMUX-3 | cross-regime reset probe | online driver |
| V-DPCI-2's OPR face | options-discard hardware probe (OPR config, dpci.md unknown 6); the generated V-DPCI-2 carries the connect and ceiling faces (unknowns 3–5), not this one | online driver |
| V-LINK-3 | raw `SET_LINK_STATE` commands through `/dev/dprc.N` that no crate code drives; its kernel-path half is already answered by V-LINK-2 (dpmac.md unknown #2) | online driver |
| V-LINK-4 | the peer-request channel (`dpni_set_link_cfg`, reachable as `ethtool -A`) has no restool verb, and the flagged wiring carries no kernel netdev to drive it from | online driver |
| V-DPDCEI-1 probes | GET_API_VERSION / dce_version reads; the create face is V-LIFE-DPDCEI-1 | online driver |
| V-DPDMAI-2 | shutdown/reboot-cycle shaped — the V-RECOVERY-1 two-script pattern, not a plain batch suite | later, recovery-shaped suite |

Which roadmap change owns each deferred scenario is recorded per
invariant in `models/COVERAGE.md` (task 5.6): the family's own change
for its probes, `mc-portal-backend` for the raw-command ones,
`dpl-tape-out` for the `generate-dpl` audit.

Batch 2 (all five passed, ledger above) covers the remaining positive
lifecycle faces that render with the adapter as-is: dpio/dpseci/dpdmai
canonical orders in the root, dpdcei in a scratch container, and the
dpci pair connect. Batch 3 closed the sweep with the dpsw and dpdmux
positive create+connect faces; every remaining scenario in the deferral
table now belongs to the online driver.

The three root-container suites all read back an unbound object where
the model expected a bind, each for its own reason: the dpio seats are
filled at boot, the crypto algorithm names are claimed by the first
dpseci of the boot, and no qdma driver exists for a dpdmai to bind to.
A loaded driver is not the same claim as a driver that takes an object
created after boot, so that became its own per-family property
(ADR-0008) and the suites now assert the negative face. The board never
diverged; the model's expectation did.

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
objects never bind (DPRC-I6), so they skip the unbind. The fix held on
the re-sitting: no residue anywhere, and every dpmcp companion was
reclaimed cleanly.

Batch 3 turned that leftover sitting risk into a fixed one. Destroying
objects in the Linux root races the bus's own rescan, and a bystander
in that container can be silently detached from its driver — no log
entry, device directory still in place (ADR-0008 §4–§6). The dpdmux
suite's first run made it plain: two destroys cost three boot residents
their drivers at once. Teardown now waits after every destroy, which
gives each rescan a container nobody is still changing, and the
re-sitting produced no markers and no casualties across five destroys.
Reading the boot dpseci's driver link is still worth doing as a
post-sitting health check, and any anomaly still means rebooting before
the next sitting.

The five batch-2 scripts on disk predate that settle. They are passed
evidence and were deliberately not rewritten, so anyone re-running one
must regenerate it first — otherwise the old, unspaced teardown is what
executes.

Connecting a switch-family object to a dpmac wakes the mac driver on
the peer: both sittings logged the dpmac configuring its link mode the
moment the edge came up, without anything else touching that dpmac.
Worth knowing before reading a sitting's log as unexplained activity.

Batch 4 closes the batchable half of link signaling: V-LINK-1 and
V-LINK-2 passed, V-LINK-5 answered its unknown and retired, and only
V-LINK-3 and V-LINK-4 remain, both in the deferral table above. These
are the first suites to run explicitly flagged against a wired pair,
and the wiring changed how a suite asks for help: an operator
acknowledgment is no longer a keystroke but an assertion, with both
faces of the state — the kernel's carrier flag and restool's own
read-back — required to agree before the run continues, because the
MC-visible link lags the local one. Teardown gained the matching
repair: a suite that severs a boot connection restores it before
finishing. On the live bare boot that step is vacuous, since the boot
pair the reference capture shows is a provisioned-moment artifact and
no such edge exists to restore (reference-environment.md).

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

Besides the printed report, `diff` writes `results/<dir>/verdict.json`
— suite, revision, plan hash, the pinned reference pair, pass/fail,
every step's conformance with its exit codes, observed read-back and
any MC status text from `step-N-err.txt`, the created ids, and the
hook's `PASS`/`FAIL` lines — and records a one-line summary in
`models/board/VERDICTS.json` under the run label (the results directory
name, e.g. `V-DPRC-1-rev3`). A run passes only when every judged step
conforms **and** its hook printed no `FAIL` line; the process exit
follows that verdict. The revision comes from the `-revN` suffix of the
results directory and the date from its newest file; both take
overrides (`--revision`, `--date`), as does the label (`--label`, used
for `results/results-recovery` → `V-RECOVERY-1`). `--archive <path>`
records where the sitting's tarball went; `--no-index` writes the
verdict file only.

Online runs get the same treatment. `drive` writes
`<transcript stem>.verdict.json` beside its transcript on the board
(one transcript per run, `probes-rev2.jsonl` style), and back on the
workstation

```sh
cargo run -p dpaa2-verify -- diff --transcript results/<ID>/probes-rev2.jsonl
```

re-derives that verdict and indexes it under `<ID>/probes-rev2`; a
trace transcript carries no suite id, so it takes `--id <ID>`. The
postboot halves index under their own suite id (`V-DPRTC-3-postboot`).

The index was back-filled once from every results directory on disk
(task 6.2). Two caveats travel with those entries: a revision that
predates a plan regeneration is judged against the *current* plan, so
an early V-LIFE revision whose divergence was the model's bind
expectation now conforms — the entry's plan hash is the tell, and the
suite-ledger prose stays the authority on why a revision was re-run;
and revisions overwritten in place before the archive rule (V-DPSW-1
rev 1, V-LINK-2 revs 1–2) have no entry at all.

A suite whose face needs the created objects standing (V-TRAF-0's
frames) adds `--hook <file>`: the generated script sources the file
after its last step and before its teardown trap, so the hook sees the
script's variables (`$OBJ_…`, `$RESULTS`) and never has to name or
reclaim anything itself. Hooks are screened by the safety envelope at
generation and by the script's own self-check at run time.

A suite that must create an object or container with non-default
`restool` arguments (e.g. a dpio on `DPIO_NO_CHANNEL`, or a dprc on a
non-default `--options=` permission mask) adds `--create-args
<fam>=<args>`: the generated script renders those arguments for that
family — including the `dprc` container create — and the plan records
them, while the model stays free of the create detail.

A step the board is expected to refuse names the status it must be
refused with (`docs/baseline/mc-status.md`, task 6.4). In a probe plan
that is `"refusal": "No privilege"` beside the step's `cmd`; for a
generated suite it is `--expect-refusal <step>=<status>` at
generation, which records the status on the plan step and drops that
step's model read-back (the refused action never reaches the model's
post-state). `diff` scores the step from its exit file and
`step-N-err.txt`, the verdict keeps the status text observed, and the
index lists every status a run scored, so a refusal without a register
row — or a register row without a verdict — fails `cargo test`.

### Sitting 5.9: the refusal and read-back suites

The sitting ran 2026-08-29 from a clean boot: the four batch/hook suites
first (V-READBACK-1 rev 2, then V-DPSW-4, V-DPDMUX-2, V-DPRC-6 into their
own rev directories), a reboot, then the three online probe plans
(V-DPAIOP-1, V-DPSECI-1, V-DPNI-2), and the closing driver-link and
snapshot census. Every suite is folded into the ledgers above and its
verdict recorded in `VERDICTS.json`. Only V-DPDMUX-2 diverged (rev 1
through rev 5): the model guard is kept (ADR-0009).

The sitting is complete (2026-08-29): V-DPDMUX-2 closed at rev 5, which
settled the downlink from a fresh boot — accepted with the state
agreeing and un-disconnectable from any end — and confirmed no pairing
survives a reboot or a destroy-and-recreate. Rev 5's hook ran an in-run
phase-4 destroy that tripped the ADR-0008 §4 rescan race and took the
management interface down until a rebind; that phase is removed from the
committed hook and the incident's rule is ADR-0008 §7 (a hook never
destroys in the root — only the spaced teardown does).

### Sitting 5.10: what the board reports without being changed

The sitting ran 2026-08-29 in one pass: the three read-only probe
plans first (V-DPMAC-1, V-DPRC-4, V-DPRTC-2 rev 2), then the scratch-
child suites in rising order of root involvement (the five V-DPRC-2
option-bit faces, V-GENDPL-1, V-DPRC-3, V-DPRC-5, and V-DPCI-2 last
since its teardown destroys a connected root pair, ADR-0008 §7), then
the closing snapshot — zero deltas against the clean-boot reference.
Every run is folded into the ledger above and `VERDICTS.json`.

Nine runs pass. Three carry a FAIL verdict that is the register's
prediction being wrong, not the board diverging: V-DPRC-2-NOCREATE-1
(the create the bit was expected to refuse was accepted — restool
creates through the root portal, which the bit does not gate),
V-DPRC-2-NOSPAWN-1 and V-DPRC-2-NOALLOC-1 (refused as predicted, with
Configuration error and No resources rather than No privilege), and
V-DPRC-3's one hook line (`set-label` survives a lock). Those oracles
are corrected in place — the plans regenerated, the hook edited — and
their rev 2 runs ride along with the 5.11 sitting; the rev 1 verdicts
stand as recorded. The option-bit matrix, the id-reuse law and the
ABA hazard it creates are written up in dprc.md, object-model.md §6
and ADR-0010.

### After every sitting: snapshot and diff

Run the read-only census on the board, then parse and diff it against
the committed clean-boot reference:

```sh
sh models/board/baselines/snapshot.sh results/<ID>-snapshot   # on the board
cargo run -p dpaa2-verify -- snapshot parse results/<ID>-snapshot --out results/<ID>-snapshot/snapshot.json
cargo run -p dpaa2-verify -- snapshot diff models/board/baselines/reference.json results/<ID>-snapshot/snapshot.json
```

This replaces the `restool dprc show dprc.1 | head -1` object-count
census: it reads every container and object back, not just the root
count, so a swapped driver or a changed attribute shows up too. A
zero-delta diff is the "no residue" verdict; any line is a leaked or
mutated object to explain before the next sitting.

Then archive the sitting's results outside the checkout and point the
verdict at the archive:

```sh
mkdir -p ~/dpaa2-board-evidence
tar czf ~/dpaa2-board-evidence/<ID>-$(date +%F).tar.gz -C results <ID> <ID>-snapshot
cargo run -p dpaa2-verify -- diff --plan models/board/<ID>/<ID>.plan.json --results results/<ID> --archive ~/dpaa2-board-evidence/<ID>-$(date +%F).tar.gz
```

The second `diff` is idempotent — it rewrites the same index entry with
the archive path filled in. Commit `VERDICTS.json` with the ledger
edits of the sitting; `cargo test` refuses a ledger cell that claims
more than the index holds.

`results/` is gitignored operator material and a revision was lost once
by being overwritten in place (V-DPSW-1 rev 1), so every sitting's
results are archived at `~/dpaa2-board-evidence/` before the next
revision runs. Scripts generated from now on also leave `step-N-err.txt`
per step and `dmesg.txt` from the teardown — committed passed scripts
predate that and are not rewritten, so regenerate before re-running one
(as the rule above already says).
