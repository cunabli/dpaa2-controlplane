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
| V-LINK-5 | `vlink5.qnt` | **ran once** 2026-08-23 (batch 4) and **retired with its answer** — answers dprc.md unknown #9: `dprc assign --plugged=0` on a kernel-bound, link-up, netdev-backed dpni exited 240 (8-bit −EBUSY) with the object still plugged and the driver still bound. A refusal, not a race and not a silent drop, and the second anchor after V-LIFE-DPIO-1 rev 1's teardown refusal on a driver-bound dpmcp. Every other step passed. The model's `unplugAt` now requires an unbound object, which makes the probing step untraceable — the retirement is the finding, and the module header carries the do-not-regenerate note. The committed trace is re-frozen from the amended model, which no longer reaches the unplug step (15 states); the verdict was obtained on the earlier, equivalent 16-step trace, and the suite can simply be rerun |
| V-DPSW-4 | `vdpsw4.qnt` + `refusal.sh` (suite hook) | **passed** 2026-08-29 (rev 1), 7/7 + hook 2/2 — V-DPSW-1's refusal face (DPSW-I1's refusal face, DPSW-I2): the same create+connect trace, but the switch is built the way restool's silent defaults build it (flooding PER_VLAN, broadcast PER_OBJECT, via `--create-args dpsw=--num-ifs=2`, which drops the PER_FDB flags the positive suite carried). The MC created and connected that switch without complaint — `restool dpsw info` reads flooding `DPSW_FLOODING_PER_VLAN`, broadcast `DPSW_BROADCAST_PER_OBJECT` — and the kernel `fsl_dpaa2_switch` driver refused it at probe: dmesg `Flooding domain is not per FDB, cannot probe`, error −95 (EOPNOTSUPP), driver link empty. So the refusal is the kernel's, not the MC's. The trace has no kernelBind step; the hook read the negative outcome. A code-derived prediction in V-DPSW-1, now issued on the board |
| V-DPDMUX-2 | `vdpdmux2.qnt` + `uplink.sh` (suite hook) | **diverged, final** (rev 1–5, 2026-08-29), model guard kept (ADR-0009). DPDMUX-I8's refusal face: the demux uplink (interface 0) may only face a dpmac, model-forbidden in `connect.legalPorts`, so the illegal pairing lives in the hook. MC 10.39 *accepts* `dprc connect` of a dpni onto the uplink and then refuses the disconnect from either end, Configuration error (rev 2) — a pairing it cannot undo. Rev 5, from a fresh boot, settled the downlink cleanly: the connect was accepted with the state agreeing (`dpdmux info` interface 1 = the dpni, `dpni info` endpoint = `dpdmux.N.1`), and no end could disconnect it — dpni end 0x6, demux downlink end 0x6, demux uplink (bare name) 0x8 — so a dpni is un-disconnectable on any interface. No pairing survived a reboot (all interfaces read none before any connect) or a destroy-and-recreate (rev 5), refuting rev 4's ghost hypothesis. Rev 3's refused-connect-that-still-connects and rev 4's ghost stand as observed on their boots; rev 3's shape did not reproduce on rev 5's fresh boot. The model keeps its `legalPorts` guard ahead of the firmware (ADR-0009). **Management-link risk**: the teardown destroys a connected pair — run it last in a sitting, expect a rebind or reboot (ADR-0008 §7). Rev 5's hook additionally ran an in-run phase-4 destroy that tripped the rescan race and took the management interface down; that phase is removed from the committed hook |
| V-DPRC-6 | `vdprc6.qnt` + `moves.sh` (suite hook) | **passed** 2026-08-29 (rev 1), 4/4 + hook 3/3 — V-DPRC-1's plugged-move face (DPRC-I3, a plugged object cannot be moved) plus the sibling-move status the MC status register listed as unknown. The trace stands two scratch containers and a dpbp plugged in the first; nothing is bus-visible (DPRC-I6), so the refusals are the MC's own gate, not a driver's. (a) Pulling the *plugged* dpbp one hop up to the root is refused by restool's own client guard (`cannot be moved because it is currently in plugged state` / `unplug it first`) before any MC command, and the dpbp stays in dprc.2. (b) After unplugging, the single-command sibling move (`dprc assign --child`, siblings) with the exact rendering V-DPRC-1 rev 1 used (which exited 255) is refused by the MC with No privilege (0x4), which fills the register's unknown sibling-move text |
| V-DPAIOP-1 | `probes.json` | **passed** 2026-08-29 (rev 1), 6/6 — DPAIOP-I1/I2 refusal probes: `dpaiop create --aiop-container-id=dprc.1` on this AIOP-less silicon is refused Configuration error (0x6, exit 250), nothing created; issued against the root container, which itself permits object creation, so the refusal confirms container create-permission does not imply create-permission — the platform gate lives in the MC's dpaiop create handler, not the DPRC options. `dprc create --options=DPRC_CFG_OPT_AIOP` was accepted the same sitting (the AIOP-flagged container created, unplugged, destroyed cleanly), so only `dpaiop create` is gated |
| V-DPSECI-1 | `probes.json` | **passed** 2026-08-29 (rev 1), 8/8 — DPSECI-I2 create-validation: `--num-queues=1 --priorities=0`, `--priorities=9`, and `--num-queues=2 --priorities=1` are each refused by restool's own parser (exit 234, `Invalid priority value.` / `Please set 2 priorities`) before any MC command is built, so the MC-layer validation is unreachable through restool — that unreachability is the finding, not an MC refusal. The positive lifecycle face is V-LIFE-DPSECI-1 |
| V-DPNI-2 | `probes.json` | **passed** 2026-08-29 (rev 1), 12/12 — the DPNI-I6 inversion and the num_queues ceiling: `dpni create --max-senders=8` (a dead v9-era option) creates the dpni and prints its id, *then* exits 234, so exit status is no side-effect oracle and convergence rests on read-back (the object stands, reads back present, and is destroyed by the next step); then a bracketing walk of num_queues 32, 24, 28, 20 — all accepted at create. restool caps the option at 32, so the MC ceiling (if any) lies at or above restool's reach (the true WRIOP-3.0.0 ceiling is dpni.md unknown 2); the walk found no refusal, and the closing census read dpni.1 absent |
| V-DPRC-5 | `vdprc5.qnt` + `visibility.sh` (suite hook) | **passed** 2026-08-29 (rev 1), 3/3 + hook 4/4 — DPRC-I6 and DPCI-I3's rescan face. A dpci created in a scratch child never appears under `/sys/bus/fsl-mc/devices`, before or after `dprc sync`, while `dprc show` lists it in the child all along: bus visibility reaches root residents only (DPRC-I6 held). The root-created dpci was on the bus *before* the hook's explicit rescan — `/sys/bus/fsl-mc/autorescan` reads 1 on this BSP, so the dprc driver rescans on the MC's object-added interrupt and a root create becomes bus-visible without the mutator asking. The MC-side law (a create command triggers no rescan, `createTriggersRescan=false`) stands; the sysfs lag DPCI-I3 describes is closed by a kernel setting here, so it is a BSP property to read, never a law to assume |
| V-DPRC-3 | `vdprc3.qnt` + `lock.sh` (suite hook) | **passed** 2026-08-29 (rev 2), steps 2/2 + hook 6/6 — DPRC-I11, dprc.md unknown 4. Rev 1 (2026-08-29, 2/2, hook 6/7) **failed on one prediction; the board did not diverge**: `set-locked --locked=1` from the root is accepted; under the lock a `dprc assign --plugged=1` on the child's dpbp is refused No privilege (0x4) and reads back unplugged; `dprc show` and `dpbp info` keep working; `--locked=0` from the root lifts it and the plug then succeeds. The one FAIL was the register's guess: `dprc set-label` on the locked child's dpbp is *accepted* and the label reads back — the lock strips assign, not labels. The corrected hook (rev 2 asserts the label lands) passes 6/6. restool's `set-locked` always opens the target's parent portal, so a child-portal unlock stays unreachable through restool; the rev 1 verdict stands as recorded |
| V-DPCI-2 | `vdpci2.qnt` + `pair.sh` (suite hook) | **passed** 2026-08-29 (rev 1), 20/20 + hook 1/1 — dpci.md unknowns 3, 4, 5. Sixteen 2-priority dpcis in one scratch child plus two in the root were all created: the bounded ceiling walk found no ceiling (19 dpcis stood at once with the hook's fixture; no pool in the `--resources` walk names dpci). The two root dpcis connect to each other inside the root (unknown 3: same-container connect is legal there). The hook's 1-priority fixture connected to a 2-priority dpci without refusal (unknown 5: accepted, not rejected), but each end's `dpci info` reports **its own** count as `peer's num_of_priorities` — the peer attribute mirrors the local value, so which count the link carries is unobservable from the control plane; recorded as ambivalent (a traffic probe settles it, restool cannot). The teardown of 19 objects including the connected root pair was clean: spaced destroys, no bus incident, zero snapshot deltas |
| V-GENDPL-1 | `vgendpl1.qnt` + `dpl.sh` (suite hook) | **passed** 2026-08-29 (rev 1), 4/4 + hook 1/1 — DPDCEI-I2 and DPDMAI-I4 (DPAIOP-I3 is re-anchored: no dpaiop can exist on this silicon, V-DPAIOP-1). `generate-dpl` of a scratch child holding a dpdcei (`--engine=DPDCEI_ENGINE_DECOMPRESSION --priority=2`), a dpdmai (`--priorities=2,4`) and a dpci (`--num-priorities=2`) is not a round-trip: the dpdcei node carries `engine` only — the priority is write-only, absent from `dpdcei info` as well; the dpdmai node emits `priorities = <0x2>`, the *count* `dpdmai info` reports, where the DPL grammar expects the list `<2 4>`, so a re-applied DPL builds a different object; the dpci round-trips (`num_of_priorities = <0x2>`); and the child container is emitted with `parent = "none"` and the default option string, i.e. as a root. The emitted `.dts` is kept beside the results; the by-eye diff is folded into dpdcei.md/dpdmai.md's owners via COVERAGE |
| V-DPRC-2-NOCREATE-1 | `vdprc2nocreate.qnt` | **passed** 2026-08-29 (rev 2), 2/2 — rev 1 (2026-08-29, 1/2) failed on the predicted refusal: a dpbp create into a child made *without* `DPRC_CFG_OPT_OBJ_CREATE_ALLOWED` succeeds and reads back in the child, because restool issues every create through the root's portal with the child's open token and the bit gates only creates from the child's *own* portal — one restool never opens. Rev 2 carries the observed expectation (no refusal) and passes: OBJ_CREATE_ALLOWED does not constrain a parent creating on the child's behalf, and its refusal face is unreachable through restool. The rev 1 verdict stands as recorded |
| V-DPRC-2-NOSPAWN-1 | `vdprc2nospawn.qnt` | **passed** 2026-08-29 (rev 2), 2/2 — rev 1 (2026-08-29, 1/2) refused as predicted but with the wrong status: a `dprc create` under a child made without `SPAWN_ALLOWED` is refused Configuration error (0x6), not the predicted No privilege, and the child stays empty. Rev 2 carries `--expect-refusal 1="Configuration error"` and passes |
| V-DPRC-2-NOALLOC-1 | `vdprc2noalloc.qnt` | **passed** 2026-08-29 (rev 2), 2/2 — rev 1 (2026-08-29, 1/2) refused as predicted but with the wrong status: a dpbp create into a child made without `ALLOC_ALLOWED` is refused No resources (0x8) — the child cannot draw the buffer-pool id from its parent's pool, so the create fails as a resource shortfall, not a privilege check. Rev 2 carries `--expect-refusal 1="No resources"` and passes |
| V-DPRC-2-TOPO-1 | `vdprc2topo.qnt` + `topo.sh` (suite hook) | **passed** 2026-08-29 (rev 1), 4/4 + hook 1/1 — with `TOPOLOGY_CHANGES_ALLOWED` added, a disconnect and a connect issued *on the child* (`dprc disconnect`/`connect dprc.2 …`) both succeed. The control for V-DPCI-1 rev 1: that No privilege was the missing topology bit, and connects rendered against the root ancestor never needed it |
| V-DPRC-2-PL-1 | `vdprc2pl.qnt` + `pl.sh` (suite hook) | **passed** 2026-08-29 (rev 1), 2/2 + hook 2/2 — `PL_ALLOWED` is accepted at create and reads back in `dprc info --verbose` (options 0xc7: SPAWN, ALLOC, OBJ_CREATE, IRQ_CFG, PL); nothing else in the surface changes and a dpbp create in that child behaves as in a default one. What the bit enables is not observable through restool |
| V-DPMAC-1 | `probes.json` | **passed** 2026-08-29 (rev 1), 5/5 — DPMAC-I7 / dpmac.md unknown 7. `dpmac info --verbose` on the five unwired ports prints exactly 28 counter rows each, out of the 62 restool 2.4 asks for: the 34 counters MC 10.39.0 does not know are refused and skipped silently (`dpmac_commands.c` swallows the error), identical across the 25G (CAUI) and 10G (XFI) ports, so the vocabulary is firmware-wide, not per port, and the row count is the only observable of a refusal. Surface: endpoint state −1 / no object, link type PHY, max rate per interface type |
| V-DPRC-4 | `probes.json` | **passed** 2026-08-29 (rev 1), 9/9 — dprc.md unknowns 5, 7, 10, 11, 12. `dprc show mc.global` lists dprc.1 alone; `dprc info mc.global` and `dump-mem mc.global` are refused by restool itself (`dprc.0 does not exist` / `Invalid MC object name`, exit 234) before any MC command — the alias exists for `show` only; `dprc show mc.global --resources` is accepted and lists the MC-level pools (bp 63, mcp 203, swp 49, fq 1981, cg 253, qd 253, …). `dump-mem dprc.1 --partition_id=MEM_PART_PEB` prints one page holding one free block (offset 0x80000, 1.5 MiB). dpmcp.14 reads back plugged in dprc.1 — a consumed companion, not an id hole (the 5.5 premise corrected). `/dev/dprc.1` is 0600 root:root and the only udev rule naming dprc tags dprc.1 to start the provisioning unit with no mode change, so the portal needs root. `/sys/bus/fsl-mc/autorescan` reads 1 |
| V-POOL-1 | `vpool1.qnt` + `pool1.sh` (suite hook) | rev 2 2026-08-29, steps 16/17 + hook 2/2 (residents; the faces recorded) — **the plug is refused before the firmware is asked**: `restool dprc assign --plugged=1` on a dprc exits with "Cannot change plugged state of dprc", a rule restool applies itself (dprc_commands.c) and one dprc.md had recorded from the tool's help; the design overlooked it. The child stayed unplugged, the bus match refuses unplugged objects and exempts only the root dprc, so faces (c)–(e) were skipped again. Through restool a runtime-created child container is never kernel-driven; a kernel-driven child exists only when the DPL defines it at boot. Whether the firmware would accept the plug from a raw command is the one route left (`mc-portal-backend`, #10). The suite retires at rev 2 with the frozen trace as the evidence. Rev 1 (2026-08-29) **passed**, 16/16 + hook 8/8 — DPBP-I2's child half held (every plugged resident is MC-listed in the scratch child yet absent from the bus), but the hand-bind the notes planned wrote to a device that does not exist: a restool-created child container is *unplugged*, so it has no bus node and nothing to bind. Faces (c)-(e) were skipped as designed and DPBP-I4/DPRC-I8 keep their online-driver anchor. The cut runs deeper than the skipped faces — 5.10's DPRC-I6 evidence was gathered on an unplugged child too — so whether a *plugged* child is kernel-driven (its own bus, pools, residents probed) is the question the corrected rev 2 answers, with the plug added as a trace step |
| V-POOL-2 | `vpool2.qnt` + `pool2.sh` (suite hook) | rev 2 2026-08-29, steps 16/17 + hook 3/3 — the same refused plug as V-POOL-1's; the one judged line held again (the dpbp stayed plugged and MC-listed through the cycle). DPBP-I3/DPCON-I5's free-path cycle stays unobservable here and DPMCP-I3 keeps its `pool-objects` anchor; the suite retires at rev 2. Rev 1 (2026-08-29) **passed**, 16/16 + hook 2/2 — the same scratch child as V-POOL-1, unplugged and unbindable, so the dpni unbind/rebind cycle DPBP-I3/DPCON-I5 need never ran; the only judged line held (the dpbp stayed plugged and MC-listed through the hook). DPBP-I3 and DPCON-I5 stay open — the free path is Linux-side and not restool-observable — and DPMCP-I3 re-anchors to `pool-objects` (#6). Rev 2 plugs the child as a trace step, as V-POOL-1's |
| V-POOL-3 | `vpool3.qnt` + `pool3.sh` (suite hook) | **passed** 2026-08-29 (rev 2), 1/1 + hook 4/4 — the corrected count: one opener held the portal, 119 were refused `EINVAL` (K=120), and the pool answered after the release. Rev 1 (2026-08-29, 1/1 + hook 2/2) found the law — DPMCP-I2 falsified as stated: the second opener of `/dev/dprc.1` fails `open()` with `EINVAL` while the first is held, not `ENXIO` at exhaustion, with over a hundred portals still free. The uapi allocates the extra portal on the root container's behalf and records the root as the consumer of its own child dpmcp — a device-link cycle the kernel refuses (ADR-0006 amendment). The two PASS lines are the pool recovery and the residents rule; the hook counted the refused openers as zero because a failed `exec` ends the subshell before the counter, so the RECORD lines carry the real count — corrected for a rev 2 |
| V-CONC-1 | `vconc1.qnt` + `conc.sh` (suite hook) | rev 1 2026-08-29, steps 1/1, hook 1/4 — **the firmware-concurrency question could not be reached; not a divergence.** `/dev/dprc.1` admits one opener at a time on this kernel (V-POOL-3's `EINVAL`): the second writer of every concurrent face failed `open()` before a command reached the MC — half of a two-writer create race, so 64 objects never listed. ADR-0006's single-writer stance is now enforced by the kernel rather than asked of operators, and whether the firmware serializes two portals stays open (it needs a second portal the uapi does not grant). The one reading that survived: 26 of 32 destroyed ids were minted again, lowest-free (ADR-0010). Not an invariant row — it amends ADR-0006 |
| V-CEIL-1 | `vceil1.qnt` + `ceil.sh` (suite hook) | rev 2 2026-08-29, steps 1/1, hook 8/9 — the same numbers to the object (dpbp 63, five families at the cap of 64, the dpni refused at the 18th, MC portals 200 → 138 → 138) and the answer rev 1 lacked: the checkpoint snapshot taken *after the scratch child was destroyed* still reads 138 portals, and only the reboot restores 203. A destroyed dpmcp's portal is gone for the rest of the boot — a firmware leak, not a container quota (ADR-0011 §3 settled; DPMCP-I6). The FAIL line is that leak, as in rev 1. Rev 1 2026-08-29, steps 1/1, hook 7/8 — **the one FAIL is a finding the hook could not expect, not a leak.** A dpbp is refused exactly at the buffer pool's free count (63, `No resources`) and every unit comes back on destroy — the census predicts the refusal to the object. dpcon, dpmcp, dpci, dpdmai and dpdcei all reach the cap of 64 in a scratch child with their pools restored after the destroys (dpci's ceiling, "above 19" at 5.10, is now "above 64"). A dpni is refused at the 18th with every listed pool showing room — an unlisted resource. And a destroyed dpmcp does not return its portal to the parent's listing (64 created, 62 drawn, none back after 64 clean destroys) — the FAIL line. Both ambivalent findings are ADR-0011; the snapshot now captures the pools so the next diff can settle them |
| V-POOL-4 | `vpool4.qnt` + `pool4.sh` (suite hook) | **passed** 2026-08-30 (rev 2), 1/1 + hook 8/8 — a dpio create draws one `swp` and one `swpch.8wq`; a dpni create draws 4 fq, 2 cg, 2 qd, 4 kp.wr0.ctlui, 3 plcye.wr0.ctlui, 3 qpr, 1 ifp.wr0, 1 prp.wr0.ctlue, 1 prp.wr0.ctlui and 1 plcy.wr0.ctlui; the draw is linear to the unit, `mcp` never moves, every unit returns on destroy, and both snapshots read 0 deltas. ADR-0012 open question 1 settled; DPMCP-I7. Rev 1 (2026-08-30, 1/1 + hook 6/8) read the identical twelve RECORD lines; its two FAIL lines were the hook's own linearity check comparing the `old->new` strings, whose absolute readings differ at every step by construction — corrected to compare signed differences for rev 2 |
| V-DPNI-3 | `vdpni3.qnt` + `dpni3.sh` (suite hook) | rev 1 2026-08-29, steps 9/9, hook 2/4 — **failed on two predictions; the board did not diverge.** The primary MAC survives both an unbind and a rebind: a MAC set from the netdev reached the firmware and the remove-path reset did not clear it (DPNI-I8's predicted clear did not happen), and a second MAC set through restool while unbound was carried by both the firmware and the new netdev after the rebind (DPNI-I2's predicted reset did not happen). Both laws are falsified for the primary MAC — the driver keeps a non-zero firmware MAC and randomizes only a zero one (DPNI-I3). Max frame length read 1536 while unbound. The hook oracle is inverted for a rev 2; the rev 1 verdict stands as recorded |
| V-LINK-4 | `vlink4.qnt` + `link.sh` (suite hook) | **passed** 2026-08-29 (rev 2), 12/12 + hook 3/3, checkpoint snapshot clean — the inverted write separated nothing, and the kernel says why: dpmac.7 is a PHY-typed port, and on those the ethernet driver routes both `ethtool -A` and `ethtool -a` to phylink — the read returns phylink's own configuration (autoneg on, manual rx/tx bits) and the write updates that configuration and the PHY's advertisement; `dpni_set_link_cfg` and `dpni_get_link_state` are never touched. (a) read off/off — phylink's default, not a negotiated reality; (b) wrote on/on and read it straight back; (c) the bounce left it at on/on; (d) restored off/off. Rev 1's reading — the PHY's reality overwriting a probe-time request — was wrong. DPMAC-I4 has no kernel-side observable on this port and stays with `dpmac-typestate` (#7) for the raw `dpni_get_link_state`/`dpmac_get_link_cfg` reads. The regenerated sever-before-unbind teardown handed dpmac.7's driver back: zero deltas at the checkpoint (ADR-0008 §8 holds on the board). Rev 1 (2026-08-29) **passed**, 12/12 + hook 2/2 — DPMAC-I4's two channels could not be separated as designed and the suite still passed (no FAIL line): the netdev came up with pause off on both sides — the PHY negotiated "flow control off" and that reality overwrote the driver's probe-time request — so writing "off" onto "off" observed nothing, and only the restoring write showed the mechanism. `ethtool -a` reports the last request immediately after a write and the firmware's reality after a link event, because the driver caches the request until the next link interrupt. The channels are separable — the rev 2 face inverts the write against the current reading and reads after a bounce. DPMAC-I4 stays open with that sharper reason; the raw `dpmac_get_link_cfg` read stays with `dpmac-typestate` (#7) |
| V-DPDMAI-2 | `vdpdmai2.qnt` (reboot-persistence pair) | **passed** 2026-08-29 (rev 1), 1/1 — dpdmai.md unknown 4's persistence half: a bare unplugged dpdmai created in the root is absent after the reboot, the closing recovery diff clean at the 97-object reference. The shutdown-path half stays unanswerable on this BSP — no qdma driver binds a dpdmai (ADR-0008), so `dpaa2_qdma_shutdown`'s wrong-token destroy never runs |

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
| V-DPNI-4 | raw command via `/dev/dprc.N` | kernel patch or VFIO transport (#10): the command is outside the `/dev/dprc.N` whitelist (`docs/baseline/mc-ioctl-policy.md` §3, task 6.5) |
| V-DPMAC-2 | the model forbids dpmac create (DPMAC-I1) — the probe deliberately tests an unknown against a model law; a board answer amends the model | online driver |
| V-DPSECI-1 | create-validation refusals (priority range, count-vs-num-queues); the positive lifecycle face is V-LIFE-DPSECI-1 | landed as `V-DPSECI-1/probes.json` (task 5.9), ran 2026-08-29 |
| V-DPSECI-2 | raw GET_ATTR attribute read-back | online driver |
| V-DPSW-2..3 | V-DPSW-2 is a raw-reset probe; V-DPSW-3 needs per-scenario endpoint counts. The positive create+connect face landed as V-DPSW-1 | online driver |
| V-DPDMUX-2 | the dpni-uplink refusal is model-forbidden (like V-DPMAC-2) so it cannot be traced; carried in a suite hook instead. The positive uplink connect landed as V-DPDMUX-1 | landed as `V-DPDMUX-2/` (task 5.9), ran 2026-08-29, final (rev 5) |
| V-DPDMUX-3 | cross-regime reset probe | online driver |
| V-DPCI-2's OPR face | options-discard hardware probe (OPR config, dpci.md unknown 6); the generated V-DPCI-2 carries the connect and ceiling faces (unknowns 3–5), not this one | online driver |
| V-LINK-3 | raw `SET_LINK_STATE` commands through `/dev/dprc.N` that no crate code drives; its kernel-path half is already answered by V-LINK-2 (dpmac.md unknown #2) | kernel patch or VFIO transport (#10): the command is outside the `/dev/dprc.N` whitelist (`docs/baseline/mc-ioctl-policy.md` §3, task 6.5) |
| V-DPDCEI-1 probes | GET_API_VERSION / dce_version reads; the create face is V-LIFE-DPDCEI-1 | online driver |

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

Every restool verb a generated suite, a probe plan or the adapter can
issue crosses the kernel's `/dev/dprc.N` command whitelist before the
firmware sees it; `docs/baseline/mc-ioctl-policy.md` is that whitelist
as a table (task 6.5), regenerated by `models/helpers/mc-ioctl-policy.py`
from the reference kernel and restool trees whose commits it records,
together with `models/core/ioctl_policy.qnt`, the same list as a Quint
module. The model is where the law lives: every action records the §2
verb keys it emits (`lastVerbs` in the machine), `IOCTL_OK` requires
each verb's `VERB_OK`, and the ITF traces the suites are generated from
carry those verbs per step, so the harness is checked against the model, not the other
way round. A command the kernel would refuse `-EACCES` therefore
cannot reach a suite unnoticed, and a suite whose steps the kernel
gates on `CAP_NET_ADMIN` says so in its header (`# operator: run as
root — …`). The table's §3
is the list of commands no restool verb reaches: the two raw probes in
the deferral table (V-DPNI-4, V-LINK-3), whose route is a kernel patch
or the VFIO transport, not the online driver.

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

### Sitting 5.11: the riskier set — design notes before generation

This is the last board sitting of the change and the first whose
scenarios touch the kernel from the outside: binding a container by
hand, holding portals open, rewriting a netdev's state, draining MC
pools, racing two writers, and a reboot. Each scenario below therefore
got a design note *before* any suite was generated (task 5.11's
condition), stating what it observes, how, what it can break, and when
it stops. The notes are the plan; the generated suites implement them
and the fold judges against them.

**Rule shared by every hook here.** A hook reads the boot residents'
driver links (`/sys/bus/fsl-mc/devices/<obj>/driver` for every object
the clean-boot reference lists as bound) before its first face and
after its last, and prints one `FAIL` line if any changed. That is the
abort rule: a changed link means a boot resident lost or swapped its
driver (ADR-0008 §4–§5), the sitting stops there, and the board is
rebooted before anything else runs. The read lives in one sourced file
beside the suites (`models/board/residents.sh`) so eight hooks do not
carry eight copies. No hook destroys anything in the root (ADR-0008 §7);
hooks that create do so in a scratch child only, and reclaim it
themselves before the generated teardown runs.

**Run order.** Rising root involvement, riskiest last, with a snapshot
diff as a checkpoint before the two MC-wide scenarios and the reboot at
the very end healing whatever the last two left:

1. rev 2 of the four corrected 5.10 oracles — V-DPRC-2-NOCREATE-1,
   V-DPRC-2-NOSPAWN-1, V-DPRC-2-NOALLOC-1, V-DPRC-3 (scratch children
   only, nothing new);
2. V-POOL-1, V-POOL-2 (a scratch child, bound to the kernel by hand);
3. V-POOL-3 (root uapi, transient);
4. V-DPNI-3 (a root-bound scratch dpni);
5. V-LINK-4 (root-bound scratch dpni on the flagged dpmac.7);
6. snapshot and diff — the checkpoint;
7. V-CONC-1, then V-CEIL-1 (scratch children, MC-wide pools);
8. snapshot and diff;
9. V-DPDMAI-2 pre-half, reboot, post-half, closing snapshot and diff
   (the reboot restoring the 97-object reference is the sitting's
   milestone).

#### V-POOL-1 — pool mechanics in a kernel-bound scratch child

*What it observes.* DPBP-I2's plugged-vs-allocator split, DPBP-I4's
exhaustion-then-top-up cycle, and DPRC-I8's claim that plugging pool
objects and their consumer in one batch lets the consumer probe. All
three need a kernel that probes consumers against a pool that is not
the root's — and 5.10 proved the kernel never scans a scratch child's
residents on its own (DPRC-I6). dprc.md records that the kernel *can*
drive a child container, with its own bus and pools, when the child
dprc is bound to `fsl_mc_dprc`; a restool-created child evidently is
not bound. Binding it by hand is the untested step this scenario
turns on.

*How.* The trace creates a scratch child with two dpmcps, two dpcons,
two dpbps and two unconnected dpnis, and plugs everything except the
second dpbp — that is DPRC-I8's batch, minus one pool object. The hook
then (a) records that the plugged residents are MC-listed but have no
sysfs node (the child half of DPBP-I2); (b) writes the child's name to
`/sys/bus/fsl-mc/drivers/fsl_mc_dprc/bind` and reads back the child's
driver link and whether its residents now appear on the bus; (c)
expects the first dpni bound and the second not, with `No more
resources of type dpbp` in the kernel log — exhaustion is a deferred
probe, not a refusal; (d) plugs the second dpbp and expects the
deferred dpni to bind (DPBP-I4's top-up); (e) unbinds the child dprc
again so the generated teardown finds the child exactly as its
trace left it. The dpio question is recorded, not assumed: dpaa2-eth
draws dpbp/dpcon/dpmcp from its own container (DPRC-I1) but selects a
dpio from a global service, so a child dpni may or may not probe with
no dpio of its own; if neither dpni ever binds, that is the finding.

*Blast radius.* The child and its residents. Binding the child scans
the child only; the root is neither rescanned nor destroyed in, so the
ADR-0008 §4 race is not in play. Pool draws never cross containers
(DPRC-I1), so the root's residents keep theirs.

*Guard and abort.* The shared residents rule. If step (b) leaves the
child unbound, faces (c)–(e) are skipped and DPBP-I4/DPRC-I8 re-anchor
to `pool-objects` (#6) with "a hand-bound scratch dprc does not probe
its residents on this BSP" as the reason; face (a) is still judged.

*Settles or re-anchors.* DPBP-I2 (child half), DPBP-I4 (kernel side),
DPRC-I8.

#### V-POOL-2 — what a pool object carries across owners

*What it observes.* DPBP-I3 and DPCON-I5 say a freed pool object is not
clean: the kernel frees a dpbp as `drain → disable → close` with no
reset, so the next allocator's reset is what cleans it. DPMCP-I3 says
the same of portals, with no reset anywhere. The board can show the
dpbp half through restool — `dpbp info` prints the object's plugged
state and bpid, and the kernel's drain is what decides whether any
buffer state survives — but restool exposes nothing of a dpmcp's
state, so the portal half has no observable here.

*How.* The same module and trace as V-POOL-1 (the group is the same;
only the hook differs), bound the same way. The hook binds the child,
waits for the first dpni to bind, records `dpbp info` of its dpbp,
unbinds that dpni through sysfs (the kernel's drain-and-free path),
records `dpbp info` again, rebinds the dpni and records a third time.
Every line is a `RECORD`; the only judged line is that the dpbp stays
plugged and MC-listed through the cycle, because the free path is
Linux-side and never reaches the MC object (dprc.md, object removal).

*Blast radius and guard.* As V-POOL-1: the child only, the shared
residents rule, the child unbound before the hook returns.

*Settles or re-anchors.* DPBP-I3 and DPCON-I5 settle to "the free path
is not MC-observable; cleanliness is the next allocator's job" if the
read-backs are identical across the cycle, and stay open otherwise.
DPMCP-I3 re-anchors to `pool-objects` (#6) with "restool exposes no
portal state" as the reason, whatever the run shows.

#### V-POOL-3 — the uapi opener law

*What it observes.* DPMCP-I2: N simultaneous openers of `/dev/dprc.1`
need N−1 free dpmcps in the root pool, the first opener rides the
root's own portal, and exhaustion is `open()` failing with `ENXIO`.
The reference boot has 52 dpmcps in the root and a 203-portal MC pool
(V-DPRC-4), so the expected ceiling is roughly the free count plus one.

*How.* This scenario is inherently root-side — the uapi is the root
container's device — and it is the one place the sitting touches the
root's pool. The trace is the smallest there is (one scratch child, so
the suite has a teardown and a results directory); the hook spawns
openers that each open the device and hold it, until one fails,
records N and the errno, releases every opener at once, and then
proves the pool recovered: `restool dprc show dprc.1` answers again
and the residents rule holds. No restool call happens while openers
are held, because restool is itself an opener.

*Blast radius.* The root's *free* dpmcps, for the seconds the openers
are held. Boot residents already hold theirs (allocated at probe) and
lose nothing; the only thing that cannot happen during the hold is a
new kernel probe needing a portal, and none is scheduled.

*Guard and abort.* Hold cap of a few seconds enforced by the openers
themselves (they exit on a timer whatever the hook does); the shared
residents rule after release; if `dprc show` does not answer after
release the hook prints `FAIL` and the sitting stops.

*Settles or re-anchors.* DPMCP-I2.

#### V-DPNI-3 — netdev runtime state across unbind and rebind

*What it observes.* DPNI-I2 (MC state set before a bind does not
survive the bind: the probe resets the object) and DPNI-I8 (a clean
unbind resets the object, but only the read-back proves it), plus
dpni.md unknown 4 (what `dpni_reset` clears) as far as the primary MAC
shows it. MTU is deliberately not the probe: the kernel never sends an
MTU change to the MC (max frame length stays pinned; dpni.md runtime
knob map), so it cannot say anything about MC-side state.

*How.* V-LIFE-DPNI-1's root group — dpmcp, dpbp, dpcon and one
unconnected dpni, plugged and bound — and a hook that walks one
cycle: (a) record the bound netdev's MAC and `dpni info`'s; (b)
`ip link set address` to a locally administered MAC, expect `dpni
info` to show it (the kernel's `dpni_set_primary_mac_addr` path); (c)
unbind through sysfs, expect `dpni info` *not* to show it (DPNI-I8: the
remove path reset the object) and record `max frame length`; (d) while
unbound, `restool dpni update --mac-addr=` a second MAC — MC state set
before a bind; (e) rebind through sysfs and expect neither the netdev
nor `dpni info` to carry the second MAC (DPNI-I2: the probe reset it
and re-derived a random MAC, since the dpni has no dpmac to inherit
from, DPNI-I3).

*Blast radius.* One root-bound scratch dpni and its companions, the
same set the generated teardown already unbinds and destroys. The
hook's own unbind and rebind are sysfs writes to that one object; they
trigger a probe, not a bus rescan, so no root destroy and no ADR-0008
§4 window.

*Guard and abort.* The shared residents rule. A rebind that does not
produce a driver link within the wait is a `FAIL` (the object would be
left for teardown in the state the trace expects anyway).

*Settles or re-anchors.* DPNI-I2, DPNI-I8; unknown 4 narrows to "the
primary MAC is cleared".

#### V-LINK-4 — the peer-request channel

*What it observes.* DPMAC-I4: the MC keeps two directional link
channels — requests flowing down from the dpni (`dpni_set_link_cfg`,
read by the MAC side as `dpmac_get_link_cfg`) and reality flowing up
from the PHY (`dpmac_set_link_state`, read by the dpni as
`dpni_get_link_state`) — and a model must not fold them into one link
variable. `restool dpmac info` never issues `dpmac_get_link_cfg`
(dpmac_commands.c), so the request channel has no restool read-back;
the bead anticipated this and the note records it. What *is*
observable is the kernel's echo: `ethtool -A` writes the request
channel and `ethtool -a` reads the reality channel back
(`dpni_get_link_state` options, dpni.md), so a request that does not
reappear in the read-back is the two channels being distinct.

*How.* V-TRAF-0's module and fourteen-step trace, verbatim — one
kernel-bound dpni on dpmac.7 brought to a confirmed link-up, which
needs the peer port admin-up as V-TRAF-0 did — with a different hook
and no frames (class link-signaling, flagged dpmac.7). The hook: (a)
records `ethtool -a` as bound (the driver forces pause on at probe);
(b) writes `ethtool -A rx off tx off` and immediately reads `ethtool
-a`, `dpni info` link status and the kernel log for dpmac.7; (c) asks
the operator to bounce the peer port, waits for the local carrier and
restool's link read-back to agree again (V-LINK-2's acknowledgment),
and reads `ethtool -a` once more — a bounce is a PHY-reality push, so
this is the read that shows what reality carries; (d) restores
`ethtool -A rx on tx on` and reads it back before returning. A pause
read-back that stays "on" through (b) and only changes, if at all, at
(c) is the two-channel law holding; one that flips at (b) says the MC
mirrors requests into state and the model may use one variable.

*Blast radius.* The scratch dpni and dpmac.7's pause configuration,
restored by the hook. dpmac.7 is the flagged wired port; on the bare
boot it carries no boot connection to sever or restore.

*Guard and abort.* The shared residents rule; the pause restore in (d)
must read back before the hook returns, else `FAIL`.

*Settles or re-anchors.* DPMAC-I4 settles on the kernel-side evidence
if (b)/(c) separate the channels; the restool gap is recorded in
dpmac.md either way and the raw `dpmac_get_link_cfg` read stays with
`dpmac-typestate` (#7).

#### V-DPDMAI-2 — a runtime dpdmai across a reboot

*What it observes.* dpdmai.md unknown 4 asks whether a created dpdmai
survives a kernel shutdown, because `dpaa2_qdma_shutdown` destroys the
object with the wrong token. On this BSP no qdma driver binds a dpdmai
at all (V-LIFE-DPDMAI-1, ADR-0008), so the shutdown path never runs and
the wrong-token question is unanswerable here; what the board can
answer is the persistence half — whether a runtime-created root
resident of this family outlives a reboot — which V-RECOVERY-1 answered
for a container and a dpbp and this suite answers for a dpdmai.

*How.* V-RECOVERY-1's two-script shape. The pre-half creates one bare
dpdmai in the root (unplugged: nothing for the bus to scan) and captures
`dprc show dprc.1` and `dpdmai info`; it has no teardown trap — the
reboot is the teardown. The post-half, run after the reboot with the
same results directory, expects the dpdmai absent from `dprc show` and
`dpdmai info` to say it does not exist, and the closing snapshot diff
against the clean-boot reference reports zero deltas.

*Blast radius.* One unplugged root resident until the reboot, and the
reboot itself, which is why this runs last.

*Guard and abort.* The residents rule before the reboot; after it, the
snapshot diff is the judge — any delta means the reboot did not restore
the reference and the recovery guarantee (ADR-0003 §7) is re-examined
before the marker is trusted again.

*Settles or re-anchors.* dpdmai.md unknown 4 re-anchors to "no qdma
driver on this BSP; persistence face answered".

#### V-CEIL-1 — MC resource ceilings by create-until-refused

*What it observes.* `dprc show mc.global --resources` lists the MC's
pools — bp 63, mcp 203, swp 49, fq 1981, cg 253, qd 253, opr 256
(V-DPRC-4) — and the reference boot's draw on them is known from the
snapshot. Whether a create is refused exactly when its pool runs dry,
with which status, and whether every refused create leaves the census
untouched, is what a scratch child can show without starving anyone:
boot residents already hold their draws, and a child with the ALLOC
bit draws the rest from the same MC pools (5.10, V-DPRC-2-NOALLOC-1).

*How.* A one-step trace (the scratch child) and a hook that, per
family in a fixed order — dpbp, dpcon, dpmcp, dpci, dpdmai, dpdcei,
dpni — creates in the child until restool is refused or a per-family
cap of 64 is reached, records the count and the MC status text of the
refusal, reads `mc.global --resources` after each family, then destroys
what it created (in the child, never the root; ADR-0008 §7 allows it)
and reads the resources again. The prediction per family is the free
count of its gating pool: dpbp is refused at the 63rd (bp 63, one drawn
at boot) with `No resources`; the others either meet a pool or the cap.
dpio, dpseci, dpsw, dpdmux and dprtc are out: dpio seats and the crypto
algorithm claim belong to boot residents (ADR-0008), dpsw/dpdmux need
endpoint counts per create, and dprtc is a singleton already probed.

*Blast radius.* The child's residents (bus-invisible, DPRC-I6, so no
scan and no driver) and the MC pools, drained to zero for the seconds
between the last accepted create and the family's destroys. The root's
free dpmcps are not touched by the child's dpmcp creates (they draw from
the MC pool, not the root's), so restool keeps working throughout.

*Guard and abort.* A refusal with `No memory available` (0x9) or a
timeout (0x7) stops the family and the hook at once — those say the MC
itself, not a pool, is short. `restool dprc show dprc.1` must answer
between families. The residents rule after the destroys. The snapshot
diff that follows is the leak check; a resources read that does not
return to its pre-family value is a `FAIL` line.

*Settles or re-anchors.* dpci.md unknown 4 (the ceiling above 19),
dpni.md's per-object resource cost, and the mc-status register gains
whichever code the refusals carry. A ceiling met below its pool's free
count is an ambivalence for an ADR, not a verdict.

#### V-CONC-1 — two writers and a rapid create/destroy loop

*What it observes.* ADR-0006 assumes one initiating writer during a
pass and calls a second one an operational violation, not a modeled
transition. Whether that assumption is load-bearing at the MC — whether
two portals issuing creates and destroys into one container can lose
an object, corrupt a listing, or hang a command — is what this run
learns. Every restool invocation opens its own portal (V-POOL-3's law),
so two restool loops are two writers.

*How.* A one-step trace (the scratch child) and a hook with three
faces, all inside the child: (a) two loops each create 32 dpbps
concurrently; expect 64 objects listed, 64 distinct ids, no MC error;
(b) one loop creates and destroys a dpbp repeatedly while the other
lists the child repeatedly; expect every listing to succeed and the
final count to match; (c) one loop destroys an object while the other
reads it; expect each read to be either the object or "does not exist",
never a hang. Ids are expected to be reused lowest-free throughout
(ADR-0010), and the hook counts how often a destroyed id came back.
The hook destroys everything it made, in the child, before returning.

*Blast radius.* The child only — bus-invisible residents, no rescan,
so the rescan race ADR-0008 describes (the one real concurrency hazard
the program has met) is not in play. Two portals from the root's free
dpmcps, returned on exit.

*Guard and abort.* A `Device is busy` (0xA) or timeout (0x7) status
stops the hook immediately and is the finding; the residents rule
after. Loops are bounded (32 iterations each).

*Settles or re-anchors.* Not an invariant row: the result amends
ADR-0006 — either "the MC serializes concurrent portal commands; the
single-writer contract is about plan consistency, not command safety"
or, if anything is lost or hangs, "the contract is load-bearing at the
MC and must be enforced". Either way the assumption stops being
unexamined.

#### Running the sitting

On the board, from a clean boot, from the checkout root, in this order.
A hook's `FAIL residents` line or a non-zero snapshot diff stops the
sitting at that point: reboot, report, do not continue.

```sh
# 1. rev 2 of the four corrected 5.10 oracles
for s in V-DPRC-2-NOCREATE-1 V-DPRC-2-NOSPAWN-1 V-DPRC-2-NOALLOC-1 V-DPRC-3; do
  sh models/board/$s/$s.sh results/$s-rev2; done
# 2–5. the kernel-driving suites, rising root involvement
sh models/board/V-POOL-1/V-POOL-1.sh results/V-POOL-1-rev1
sh models/board/V-POOL-2/V-POOL-2.sh results/V-POOL-2-rev1
sh models/board/V-POOL-3/V-POOL-3.sh results/V-POOL-3-rev1
sh models/board/V-DPNI-3/V-DPNI-3.sh results/V-DPNI-3-rev1
sh models/board/V-LINK-4/V-LINK-4.sh results/V-LINK-4-rev1   # peer port facing dpmac.7 admin-up first; the hook asks for one bounce
# 6. checkpoint
sh models/board/baselines/snapshot.sh results/5.11-snapshot-a
# 7. the MC-wide pair
sh models/board/V-CONC-1/V-CONC-1.sh results/V-CONC-1-rev1
sh models/board/V-CEIL-1/V-CEIL-1.sh results/V-CEIL-1-rev1
# 8. checkpoint
sh models/board/baselines/snapshot.sh results/5.11-snapshot-b
# 9. the reboot pair
sh models/board/V-DPDMAI-2/V-DPDMAI-2.sh results/V-DPDMAI-2-rev1
reboot
# after the reboot
sh models/board/V-DPDMAI-2/V-DPDMAI-2-postboot.sh results/V-DPDMAI-2-rev1
sh models/board/baselines/snapshot.sh results/5.11-snapshot-c
```

Back on the workstation, with `results/` copied over: `diff --plan` per
suite (the postboot half indexes under `V-DPDMAI-2-postboot`), then
`snapshot parse` + `snapshot diff` for each of the three captures, as
"After every sitting" describes.

#### What the board answered

The sitting ran 2026-08-29 in one pass, in the order above, and the
reboot restored the 97-object reference: the closing snapshot has zero
deltas. The two mid-sitting snapshots each carry one delta — dpmac.7
without its driver from V-LINK-4 onward — which no hook saw and which
turned out to be a teardown-order law, not a race (ADR-0008 §8). Every
run is indexed in `VERDICTS.json`; the four rev 2 re-runs of the 5.10
oracles all pass, so those corrections stand.

Nine of the twelve runs pass and three carry a FAIL verdict. As in 5.10,
a FAIL here is a prediction being wrong or a finding the hook could not
have known to expect, never a leaked object: the residents rule passed
in every hook and the reboot healed the one loss it could not see.

- **The management device admits one opener** (V-POOL-3, V-CONC-1).
  The uapi law said N openers need N−1 free portals and fail `ENXIO`
  when out. The board never got there: the second opener of
  `/dev/dprc.1` fails `open()` with `EINVAL` while the first is held,
  with over a hundred portals free — 119 of 120 held openers, 27 of 32
  concurrent reads, and half of a two-writer create race, all the same
  errno. The cause is in the kernel: the uapi allocates the extra
  portal on the root container's behalf and records the root as the
  consumer of its own child dpmcp, a dependency cycle the device core
  refuses. DPMCP-I2 is falsified in that form; ADR-0006's single-writer
  stance is now enforced by the kernel rather than asked of operators;
  and the firmware-side concurrency question V-CONC-1 was built for
  stays open, since no second command ever reached the MC. V-CONC-1's
  one real reading survives: 26 of 32 destroyed ids were minted again
  (ADR-0010). V-POOL-3's hook counted the refused openers as zero
  because a failed `exec` ends the subshell before the counter — the
  RECORD lines carry the truth; the hook is corrected for a rev 2.
- **The primary MAC survives both an unbind and a rebind** (V-DPNI-3).
  A MAC set from the netdev reached the firmware; after the unbind the
  firmware still carried it (the remove path's reset does not clear
  it); a second MAC set through restool while unbound was carried by
  both the firmware and the new netdev after the rebind. DPNI-I2
  ("state set before a bind does not survive the bind") and DPNI-I8
  ("a clean unbind resets the object") are both falsified for the
  primary MAC: the driver keeps a non-zero firmware MAC and randomizes
  only a zero one (DPNI-I3). Max frame length read 1536 while unbound.
- **A restool-created child container is unplugged** (V-POOL-1,
  V-POOL-2), so it has no device on the bus and nothing to bind. The
  hand-bind the notes planned wrote to a device that did not exist;
  faces (c)–(e) were skipped as designed and DPBP-I4/DPRC-I8 keep their
  online-driver anchor for now. The finding cuts deeper than the
  skipped faces: 5.10's DPRC-I6 evidence ("a child's residents never
  reach the bus") was gathered on an unplugged child too. Whether a
  *plugged* child is driven by the kernel — its own bus, its own pools,
  its residents probed — is the question a rev 2 with the plug as a
  trace step answers, and it decides whether pool exhaustion can ever
  be observed outside the root. (Rev 2, below: the plug is refused by
  restool itself.)
- **The pause channels could not be separated as designed** (V-LINK-4).
  The netdev came up with pause off on both sides — the PHY negotiated
  "flow control off" and that reality overwrote the driver's probe-time
  request — so writing "off" onto "off" observed nothing, and only the
  restoring write showed the mechanism: `ethtool -a` reports the last
  request immediately after a write and the firmware's reality after a
  link event, because the driver caches the request until the next
  link interrupt. The channels are separable, with the write inverted
  against the current reading and a bounce after it; that is the rev 2
  face. DPMAC-I4 stays open with that sharper reason. (Rev 2, below:
  this reading was wrong — on a PHY-typed port ethtool never touches
  either channel.)
- **Ceilings** (V-CEIL-1). A dpbp is refused exactly at the pool's
  free count (63, `No resources`); dpcon, dpmcp, dpci, dpdmai and
  dpdcei all reach the cap of 64 in a scratch child with their pools
  restored after the destroys; a dpni is refused at the 18th on the
  board with every listed pool showing room. And a destroyed dpmcp does
  not return its portal to the parent's listing — 64 created, 62 drawn,
  none back after 64 clean destroys. Both ambivalent findings, and the
  reconciler's stance until they settle, are ADR-0011; the snapshot
  script now captures the pools so the next diff can answer. (Rev 2,
  below: the portals never come back within a boot.)
- **A runtime dpdmai does not survive a reboot** (V-DPDMAI-2): created
  bare and unplugged in the root, absent after the reboot, recovery
  diff clean. dpdmai.md unknown 4's shutdown-path half is unanswerable
  here (no DMA driver ever binds), its persistence half is answered.

Corrections landed in place and ran as a rev 2 the same day (next
section): V-POOL-1 and
V-POOL-2 plug the child as a trace step, rescan once before waiting for
the kernel to hold it (a plug's own rescan is unproven), and judge the
inverse claim — a kernel-driven child's residents *are* bus-visible;
V-LINK-4 inverts the pause write against its first reading; V-POOL-3
counts refused openers from the subshell's exit; `residents.sh`
compares against the reference's drivers and lets a hook declare the
one resident its own trace evicts (V-LINK-4 names dpmac.7); the
generator severs a connected, bound dpni's edge before unbinding it
(ADR-0008 §8) and V-LINK-4 is regenerated with that teardown;
`snapshot.sh` captures `mc.global --resources`.

#### Rev 2 (2026-08-29): what the corrections answered

The five corrected suites and the resource-carrying snapshot ran the
same day: V-POOL-1, V-POOL-2, V-POOL-3, V-LINK-4, a checkpoint,
V-CEIL-1, a checkpoint, the reboot, a closing checkpoint. All three
snapshots show zero deltas — the sever-before-unbind teardown hands
dpmac.7's driver back, so ADR-0008 §8 holds on the board — and the
residents rule passed in every hook.

- **A child container cannot be plugged through restool** (V-POOL-1,
  V-POOL-2). The plug step fails before the firmware is asked: restool
  refuses `--plugged` on a dprc with "Cannot change plugged state of
  dprc" — a rule it applies itself, and one dprc.md had recorded from
  the tool's help; the design overlooked it. The kernel's bus match
  refuses unplugged objects and exempts only the root dprc, so a
  runtime-created child never gets a driver and its residents never
  reach the bus. That turns 5.10's DPRC-I6 evidence from qualified to
  settled for every child restool can make: pool exhaustion is
  observable in the root only, and a kernel-driven child exists only
  when the DPL defines it at boot. Whether the firmware would accept
  the plug from a raw command is the one route left
  (`mc-portal-backend`, #10); both suites retire at rev 2.
- **The opener law, counted** (V-POOL-3): one held, 119 refused with
  `EINVAL`, the pool answering after the release. The rev 1 finding
  stands with its numbers.
- **Neither link channel is visible through ethtool on a PHY-typed
  port** (V-LINK-4). dpmac.7 is `DPMAC_LINK_TYPE_PHY`, and for such
  ports the ethernet driver routes both `ethtool -A` and `ethtool -a`
  to phylink: the read returns phylink's own configuration — autoneg
  on, manual rx/tx bits — and the write updates that configuration and
  the PHY's advertisement; `dpni_set_link_cfg` and `dpni_get_link_state`
  are never touched. So (a)'s off/off was phylink's default, not a
  negotiated reality; the inverted on/on read straight back, survived
  the bounce, and off/off restored. Rev 1's reading — the PHY's reality
  overwriting a probe-time request — was wrong. DPMAC-I4 has no
  kernel-side observable here; the raw reads stay with
  `dpmac-typestate` (#7).
- **A destroyed dpmcp's portal is gone until the reboot** (V-CEIL-1
  and the snapshots). The family drew the portal count from 200 to 138
  and returned none on destroy, as before; the new reading is the
  checkpoint after the scratch child itself was destroyed: still 138.
  Only the reboot restored 203. Of ADR-0011 §3's two readings, the
  firmware leak is the one that holds, and the reconciler's rule —
  create portals once, never recycle one through destroy — is now
  grounded rather than cautious. The listing's arithmetic stays odd
  (62 drawn for 64 creates; 203 to 200 by the first checkpoint in both
  sittings, after five and six earlier create/destroy pairs) and is
  left as an open question in the ADR.

The reference snapshot now carries the pool counts from the post-reboot
capture, so every later diff compares them.

### Sitting 5.12: the companion draw, measured — design note before generation

Task 5.12 puts one sentence to the board — "one dpmcp per
portal-consuming object, including each dpio" — and ADR-0012's open
question 1 hangs on it: the poll-mode child carries 3 dpmcps as a
script constant, and the rule would derive a different number. The
sentence splits in two before any suite is generated, because "portal"
names two different resources:

- The **kernel's draw is a pool object**: `fsl_mc_portal_allocate`
  takes a dpmcp from the consumer's container when the consumer's driver
  probes (`dpmcp.md` census, `dpio.md`). A restool-created child is
  unplugged and never kernel-driven (V-POOL-1 rev 2), so no scratch-child
  suite can watch this draw; it is source-read and already
  board-anchored for the kernel regime (`DPMCP_I1Test`; V-LIFE-DPIO-1
  rev 1: after `dprc sync` the allocator claimed a plugged dpmcp).
- The **firmware's draw is an MC portal** — the `mcp` line of
  `dprc show mc.global --resources` — and a dpmcp create takes one for
  the rest of the boot (DPMCP-I6). Whether a dpio or a dpni create takes
  one too is the reading the task names, and it decides whether a
  container's dpmcp count is a firmware cost per object or purely the
  consumer's requirement. That is what the board can settle.

The poll-mode half of question 1 is answered by source, not by the
board. The two buses NXP wrote for the MC — the kernel's fsl-mc bus and
DPDK's fslmc bus — are the record of how the firmware is meant to be
used, and where the kernel's draw cannot be observed here, the DPDK bus
stands proxy. That bus maps **one dpmcp per process**: the primary
process maps the first unblocked dpmcp it lists and drops the rest from
its device list ("Ideally there is only a single dpmcp, but in case
multiple exists, looping on remaining devices"); a secondary process
takes the last one, and with a single dpmcp in the container it finds
none and refuses to start. Every driver on that bus — dpio, dpni, dpbp,
dpci, dprc — sends its MC commands through that one mapped portal
(`MC_PORTAL_INDEX` 0). So the poll-mode count is `1 + 1 if a secondary
process attaches`, independent of dpio and dpni counts; the 3 the
board's poll-mode child carries is that number plus one idle portal,
and every idle portal is an `mcp` drawn for the boot (DPMCP-I6).
Source: dpdk-26.03 `drivers/bus/fslmc/fslmc_vfio.c`
(`fslmc_vfio_process_group`) and `portal/dpaa2_hw_pvt.h`.

#### V-POOL-4 — what a dpio and a dpni create draw from the MC pools

*What it observes.* Per create, the delta of every line of
`dprc show mc.global --resources` — the `mcp` line above all — for a
dpio and then for a dpni in a scratch child, and per destroy whether
each delta comes back. V-CEIL-1 read the pools only before a family and
at its ceiling, and left dpio out; the one dpni number it gave is an
aggregate: 17 dpnis moved ten lines (fq 68, cg 34, qd 34, kp 68, plcye
51, qpr 51, ifp 17, prp 17 + 17, plcy 17) and `mcp` not at all. Every
total is a multiple of 17, so the per-dpni prediction is exact: 4 fq,
2 cg, 2 qd, 4 kp.wr0.ctlui, 3 plcye.wr0.ctlui, 3 qpr, 1 ifp.wr0,
1 prp.wr0.ctlue, 1 prp.wr0.ctlui, 1 plcy.wr0.ctlui, and **0 mcp**. For
a dpio the prediction is 1 `swp` (plus one `swpch.8wq` channel for
restool's default local channel) and **0 mcp**; every draw returns on
destroy.

*How.* The same one-step trace as V-CEIL-1 (one scratch child under
dprc.1, restool's default create) and a hook `pool4.sh` that, for dpio
and then dpni: reads the pools, creates three objects one at a time
reading after each, then destroys them one at a time reading after
each. Three is the smallest count that tells a per-object draw from a
one-off one: one object cannot, two cannot separate "the first is
special" from "alternating", three can — the second and third deltas
equal means linear, a different first delta names a one-off cost.
`RECORD` lines carry every changed pool line per create and per destroy
(an awk join of two readings — the board has no jq); the `PASS` lines
are `mcp` unchanged across the family, the second and third deltas
equal to the first (a linear draw), and the pools back at their
pre-family reading after the destroys. All creates and destroys are in
the child (ADR-0008 §7).

*Blast radius.* The child's residents (an unplugged child has no bus
node, so nothing is scanned or bound) and the MC pools a dpio and a dpni
draw from, by three objects for seconds. A dpio created there never
probes, so the root's dpio seats and the kernel's portal service are
untouched. If a dpio's draw never returns (DPMCP-I6 is the precedent),
the loss is three `swp` of the 23 no consumer holds (49 listed; 16
kernel, 10 in the poll-mode child).

*Guard and abort.* An MC-short status (`No memory available` 0x9, a
timeout 0x7) stops the hook; `restool dprc show dprc.1` must answer
between the two families; the residents rule before and after. A draw
that does not return on destroy is a `FAIL` line, and the snapshot diff
after the suite is the standing leak check.

*Settles or re-anchors.* ADR-0012 open question 1, with the source
half above: the poll-mode dpmcp count is `1 + secondaries`, the kernel
count is one per probing consumer including each dpio, and neither is
a firmware cost of the dpio or dpni itself. `dpmcp.md` gains
DPMCP-I7 (board-settled): a dpio or dpni create draws no MC portal —
the dpmcp count of a container is the consumer's requirement, not the
object's. `dpni.md`'s per-object resource cost gets the per-unit draw
of the listed pools (ADR-0011 §2's unlisted resource stays what it
is — the 18th dpni was refused with every listed pool showing room).
`dpio.md` unknown 3 narrows: the bus keeps one MC portal; its per-thread
QBMan portal types stay outside the corpus. If a dpio or dpni create
*does* move `mcp`, the count is a firmware cost per object, the ADR's
rule holds in both regimes, and question 1's answer becomes
`objects + 1 per process` — a number either way.

*Model side.* A pure derivation, `models/core/companions.qnt`:
`companionDraw(regime, cpus, threads, processes, dpnis)` →
`{ dpio, dpbp, dpmcp }`, Kernel: dpio = one per CPU, dpbp = one per
dpni, dpmcp = one per probing consumer including each dpio; PollMode:
dpio = 2 × T, dpbp = 2, dpmcp = processes. Two directed runs in
`main.qnt` (`ADR0012KernelDrawTest`, `ADR0012PollModeDrawTest`) pin the
board's numbers — 16 kernel dpios for 16 CPUs; 10 / 2 / 1 for T = 5 with
one process — under `pnpm model:test`, and ADR-0012's decision section
cites the module as the place the numbers are derived, not stated.

#### Running the sitting

One suite and one checkpoint; the snapshot diff is the leak check the
hook's `PASS` lines are judged against:

```sh
sh models/board/V-POOL-4/V-POOL-4.sh results/V-POOL-4-rev1
sh models/board/baselines/snapshot.sh results/5.12-snapshot-a
```

then, on the workstation, `snapshot parse` + `snapshot diff` for the
capture and `diff --plan` for the suite, as the section below says.

#### What the board answered

A dpio create drew one `swp` and one `swpch.8wq`; a dpni create drew 4
fq, 2 cg, 2 qd, 4 kp.wr0.ctlui, 3 plcye.wr0.ctlui, 3 qpr, 1 ifp.wr0,
1 prp.wr0.ctlue, 1 prp.wr0.ctlui and 1 plcy.wr0.ctlui. The draw is
linear — the second and third objects of each family moved their pools
by the same amount as the first — every unit came back on destroy, and
both snapshots read 0 deltas.

Neither create touched `mcp`. So the dpmcp count of a container is the
consumer's requirement, not a firmware cost of the object: in the kernel
regime one per probing consumer, each dpio included; on the DPDK bus one
per process. With the source reading of the fslmc bus above, that settles
ADR-0012 open question 1 — the poll-mode count is one per process, not
three; the two extra portals the board's poll-mode child carries are
idle MC portals drawn for the boot (DPMCP-I6). `dpmcp.md` gains
DPMCP-I7.

The lesson from the rev 1 → rev 2 correction is to compare draws, not
readings: rev 1 read the identical twelve RECORD lines, but its hook's
linearity check compared the `old->new` strings whose absolute readings
differ at every step by construction, printing two FAIL lines the board
never earned. Rev 2's hook compares signed differences and passes 8/8.

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
