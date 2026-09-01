# Invariant coverage ledger

One row per invariant candidate from the baseline family documents
(`docs/baseline/*.md`). The ledger is the honesty mechanism (design D9
of `openspec/changes/verify-foundation`): a candidate absent from the
model corpus is a decision on record here, never an omission.

Dispositions:

- **modeled** — encoded under its baseline id; the row names the model
  file and the highest CI rung it runs at (`typecheck` / `simulate` /
  `itf-replay` / `apalache`). `typecheck` marks structural encodings:
  the state shape itself carries the law (e.g. a field the model
  deliberately does not have), so the type checker is the rung that
  guards it.
- **deferred** — not encoded in this change; the row names the roadmap
  change that owns it. "ph.4 adapter/generator/driver" rows are owned
  by this change's own `dpaa2-verify` phase (tasks 4.1–4.4): they are
  observation-layer laws (LAW 2 read-back, LAW 6 version carrying,
  recipe distrust) that live in harness code, not in Quint state.
  Scenarios the phase-5 board program did not run are re-anchored to
  the roadmap change that owns the family (task 5.6); raw-command
  probes go to `mc-portal-backend` (#10), the change that gives the
  harness a raw command path, and `generate-dpl` audits to
  `dpl-tape-out` (#14). The Board status cell keeps the scenario id, so
  a re-anchored scenario is never a dropped one.
- **board-settled** — a candidate only the board could settle, settled
  by its suite; the row names the suite and the date.
- **board-pending** — only the board can settle it and no suite has
  yet. None remain after the phase-5 ledger pass (task 5.6): every such
  row is now board-settled or re-anchored to a named change. A Board
  status of `open:` on any row names the scenario still to run and the
  change that owns it.

Two caveats every Board status cell carries, so no cell repeats them:

- **Observations are the MC as restool renders it.** Every read-back
  in this ledger went through `restool … info` / `dprc show`, so a
  "verified" cell settles what restool reports, not what the MC command
  returned; a rendering gap (a counter restool hides, a field it
  reformats) is invisible here until `mc-portal-backend` (#10) gives the
  harness a raw command path.
- **The evidence is one board on one stamped reference pair** (MC
  firmware + kernel of `docs/baseline/reference-environment.md`). A
  cell is a fact about that pair; a different firmware or kernel is a
  re-run, not an inference.

The ledger is lint-checked (`crates/dpaa2-verify/tests/ledger_lint.rs`,
task 6.1): candidate ids match the baseline tables both ways, the tally
line matches the recount, every cited suite has a `models/board/<id>/`
directory and a suite-ledger row, every `→ change (#N)` names roadmap
row N, every `open:` cell names an owning change, and each baseline
status cell agrees with its row here (a baseline `board-pending`
against a verified or board-settled row, or the reverse, is a red
test). Since task 6.2 it also resolves evidence: every dated
`verified YYYY-MM-DD (V-… rev N …)` cell must match a passing entry of
that suite, revision and date in `models/board/VERDICTS.json`, and
every suite-ledger row that says passed must have one — a verdict
written past its evidence is a red test, not a reading exercise. Task
6.4 adds the refusal side: every row of the MC status register
(`docs/baseline/mc-status.md`) that cites a suite resolves to its
verdict, and every status a verdict scored has a register row. Task 6.5 adds
the transport side: every restool verb the model driver, the harness
and the adapter render must resolve inside the kernel's `/dev/dprc.N`
command whitelist (`docs/baseline/mc-ioctl-policy.md`), and the two raw
probes must resolve outside it.

Tally: 53 modeled, 48 deferred, 7 board-settled, 0 board-pending — 108 candidates.

| Candidate | Disposition | Location / owning change / settling scenario | CI rung | Board status |
|-----------|-------------|----------------------------------------------|---------|--------------|
| DPRC-I1 | modeled | `core/invariants.qnt` `DPRC_I1` (also DPMCP-I4's placement face) | apalache | — |
| DPRC-I2 | modeled | `core/invariants.qnt` `DPRC_I2` (`unplugAt` now requires an unbound object) | apalache | verified (V-LINK-5: unplug of a bound netdev-backed dpni refused −EBUSY, not raced) |
| DPRC-I3 | modeled | `main.qnt` `DPRC_I3Test` | simulate | verified 2026-08-29 (V-DPRC-6 rev 1, 4/4 + hook 3/3): the one-hop move of a *plugged* dpbp was refused by restool's own client guard (`cannot be moved because it is currently in plugged state` / `unplug it first`) before any MC command, and the dpbp stayed in its container — the move precondition holds; the refusal is the restool layer, and the MC-layer face stays unreachable through restool (as with the −EBUSY unplug) |
| DPRC-I4 | modeled | `core/invariants.qnt` `DPRC_I4` | apalache | verified (prior work) |
| DPRC-I5 | modeled | `core/connect.qnt` `canConnect` + `main.qnt` `DPNI_I9Test` | simulate | — |
| DPRC-I6 | modeled | `main.qnt` `DPRC_I6Test`, `DPRC_I6RescanRefusedTest` | simulate | verified 2026-08-29 (V-DPRC-5 rev 1, 3/3 + hook 4/4): a child-container dpci is absent from `/sys/bus/fsl-mc/devices` before and after `dprc sync` while `dprc show` lists it in the child — bus visibility reaches root residents only; the root-only bind half was already board-exercised (V-LIFE-DPNI-1 binds in dprc.1; V-DPNI-1's child-container dpni never bound); settled for runtime children 2026-08-29 (V-POOL-1 rev 2): restool refuses to plug a dprc, so a runtime-created child is never kernel-driven |
| DPRC-I7 | modeled | `main.qnt` `DPRC_I7Test` | simulate | — |
| DPRC-I8 | deferred | `pool-objects` (#6): V-POOL-1 — batch plug→probe ordering; the machine models no scan batching | — | open, no runtime observable through restool (V-POOL-1 rev 2, 2026-08-29): a restool-created child is unplugged and the tool refuses `--plugged` on a dprc, so its residents are never probed; needs a DPL-defined child or the raw command path (#10) → `pool-objects` (#6) |
| DPRC-I9 | board-settled | V-DPRC-1 (`canonicalLifecycleTest` is the model-side witness) | simulate | verified 2026-08-23 (V-DPRC-1 rev 3, 13/13): the scratch container was emptied through both move directions and destroyed, absent in read-back; ADR-0007 §3's release/evict law means a non-empty destroy never blocks teardown either |
| DPRC-I10 | modeled | `main.qnt` `DPRC_I10Test` | simulate | — |
| DPRC-I11 | modeled | `main.qnt` `DPRC_I11Test` + `DPRC_I11SpawnTest`/`DPRC_I11UnlockTest` | simulate | open: rev 1 (V-DPRC-3, 2026-08-29, 2/2 + hook 6/7) observed a lock set from the root refusing assign in the child (No privilege, dpbp reads back unplugged), leaving reads working, and lifted by the root; its one hook FAIL was a wrong prediction — `set-label` is *not* stripped — so the corrected hook settles the row at rev 2. The child-portal unlock face stays unreachable through restool (it always opens the parent portal) → `dprc-encapsulation` (#4) |
| DPNI-I1 | modeled | structural — LAW 1: no cfg block in state, no action mutates one | typecheck | — |
| DPNI-I2 | modeled | `main.qnt` `DPNI_I2Test` | simulate | falsified for the primary MAC 2026-08-29 (V-DPNI-3 rev 1): a second MAC set through restool while unbound was carried by both the firmware and the new netdev after the rebind — the probe did not reset it, because the driver keeps a non-zero firmware MAC and randomizes only a zero one (DPNI-I3). The law holds for other pre-bind state but not the primary MAC → `dpni-typestate` (#5) |
| DPNI-I3 | modeled | `retro/reconciler.qnt` association runs, replayed by `dpaa2-verify` against the reconciler; MAC value semantics → `dpmac-typestate` (#7) | itf-replay | verified (ADR-0001 C2) |
| DPNI-I4 | modeled | `machine.qnt` kernelBind census guard + `main.qnt` `DPNI_I4Test` | simulate | verified (ADR-0001 C1) |
| DPNI-I5 | deferred | `dpni-typestate` (#5) + `pool-objects` (#6): queue/channel counts abstracted to draw=1 at core scope | — | — |
| DPNI-I6 | deferred | this change ph.4 adapter — LAW 2: observation = read-back, never exit status | — | the class law is board-anchored on the dpni family itself 2026-08-29 (V-DPNI-2/probes-rev1, 12/12: `dpni create --max-senders=8` created the object and printed its id, then exited 234 on the dead option — exit status is no side-effect oracle, convergence rests on read-back), and twice before (ADR-0007 §2: restool exited 0 on an MC No privilege, the read-back caught it; V-LIFE-DPIO-1 rev 1: a refused teardown unplug went unseen while its stderr was discarded) |
| DPNI-I7 | board-settled | V-READBACK-1 (`dpni info` of a bare create, from the suite hook) | — | verified 2026-08-29 (V-READBACK-1 rev 2, steps 6/6, hook 10/10): the corrected hook oracle confirms the rev-1 read-back — the MC defaults are 1 queue, 1 TC, 1 CG, 64 FS entries, VLAN filtering off, 16 MAC entries and 0 QoS entries; the 80/64 the baseline table first carried were restool's maxima, and the clean-boot reference's DPL-born dpni reads the same 16/0 |
| DPNI-I8 | modeled | `main.qnt` `DPNI_I8Test` (unbind grants no reset — the no-guarantee form) | simulate | falsified for the primary MAC 2026-08-29 (V-DPNI-3 rev 1): a MAC set from the netdev survived the kernel unbind — the remove-path reset did not clear it — so the clean-unbind reset is not even best-effort on the primary MAC. Max frame length read 1536 while unbound → `dpni-typestate` (#5) |
| DPNI-I9 | modeled | `core/invariants.qnt` `DPNI_I9` + `main.qnt` `DPNI_I9Test` | apalache | verified (kdpni pairs in production) |
| DPNI-I10 | deferred | `dpni-typestate` (#5): tx-ring/thread coupling below core-model scope | — | verified (ADR-0012) |
| DPNI-I11 | modeled | `main.qnt` `IOCTL_OK` / `DPNI_I11Test` + `core/ioctl_policy.qnt` | apalache | open: V-DPNI-4 — its raw command `DPNI_SET_TX_CONFIRMATION_MODE` is outside the kernel's `/dev/dprc.N` whitelist (`docs/baseline/mc-ioctl-policy.md` §3, refused EACCES), so it needs a kernel patch or the VFIO transport → `mc-portal-backend` (#10) |
| DPNI-I12 | deferred | this change ph.4 adapter: write-only field, no drift claim | — | — |
| DPMAC-I1 | modeled | `core/invariants.qnt` `DPMAC_I1` (no-additions + root-pin; destroy is off-nominal) | apalache | verified (ADR-0001 §3); phantom-create face V-DPMAC-2 → `dpmac-typestate` (#7) |
| DPMAC-I2 | deferred | `dpmac-typestate` (#7): MAC values not in core state | — | verified (ADR-0001 C2) |
| DPMAC-I3 | deferred | `dpmac-typestate` (#7): attr surface with the eth_if/IPG exceptions | — | — |
| DPMAC-I4 | deferred | `dpmac-typestate` (#7): V-LINK-4 — directional channels; core carries a single linkUp | — | open, no kernel-side observable (V-LINK-4 rev 2, 2026-08-29): dpmac.7 is a PHY-typed port and the ethernet driver routes `ethtool -A`/`-a` through phylink on those — the read is phylink's configuration, the write never reaches `dpni_set_link_cfg`; rev 1's reading was wrong. Both channels need the raw `dpni_get_link_state`/`dpmac_get_link_cfg` reads → `dpmac-typestate` (#7) |
| DPMAC-I5 | modeled | `main.qnt` `DPMAC_I5Test` (connection ⊥ link state) | simulate | verified (V-LINK-2 rev 3: never read as link state — but on a bound, enabled pair the connection-state text co-varies with the flap, so the two are not independent) |
| DPMAC-I6 | deferred | `dpmac-typestate` (#7): driver arbitration ⟺ peer topology not in core machine | — | verified (production use) |
| DPMAC-I7 | deferred | this change ph.4 adapter: counter vocabulary firmware-versioned, refusals silent | — | verified 2026-08-29 (V-DPMAC-1 rev 1, 5/5): restool asks for 62 counters and MC 10.39.0 answers 28, identical on every port — the 34 unknown ones are refused and skipped silently, so the row count is the only observable → the typestate reads the vocabulary from the firmware version, `dpmac-typestate` (#7) |
| DPMAC-I8 | modeled | `main.qnt` `DPSECI_I8Test` (class witness: model refuses only what restool refuses) + ph.4 adapter law | simulate | — |
| DPMAC-I9 | deferred | this change ph.4 adapter: emitted fields carried per action | — | kernel path verified (V-LINK-2 rev 3: `up` takes effect with `state_valid=0`, with propagation lag); raw probe V-LINK-3 (`DPMAC_SET_LINK_STATE`, outside the `/dev/dprc.N` whitelist per `docs/baseline/mc-ioctl-policy.md` §3 — kernel patch or VFIO transport) → `mc-portal-backend` (#10) |
| DPBP-I1 | modeled | `main.qnt` `DPBP_I5Test` (zero cfg; identity is the hwId) | simulate | — |
| DPBP-I2 | modeled | `core/invariants.qnt` `DPBP_I2` + `main.qnt` `DPBP_I2Test`/`DPBP_I2PlugTest` | apalache | open: the kernel-pool half needs a plugged, bound child → `pool-objects` (#6); the child half is board-anchored 2026-08-29 (V-POOL-1 rev 1: every plugged resident is MC-listed in the scratch child yet absent from the bus, an unplugged child having no bus node), as is the plugged-vs-allocator half (V-LIFE-DPNI-1: the census drew plugged companions; V-LIFE-DPIO-1 rev 1: after `dprc sync` the allocator claimed a free plugged dpmcp) |
| DPBP-I3 | modeled | `main.qnt` `DPBP_I3Test` (dirty return on free) | simulate | open: the free path is Linux-side, not restool-observable — V-POOL-2 rev 1 (2026-08-29) recorded the dpbp plugged and MC-listed but the unplugged child never bound, so the `dpbp_reset` drain was never driven → `pool-objects` (#6) |
| DPBP-I4 | modeled | `main.qnt` `DPBP_I4Test`/`DPBP_I4TopUpTest` | simulate | verified (C1 class, ADR-0001); the kernel-side exhaustion→top-up face (V-POOL-1) needs a plugged, bound child, which restool cannot make (rev 2, 2026-08-29: the tool refuses `--plugged` on a dprc) → `pool-objects` (#6) with a DPL-defined child or the raw command path (#10) |
| DPBP-I5 | modeled | `core/invariants.qnt` `LAW4_twoIdSpaces` + `main.qnt` `DPBP_I5Test` | apalache | open: V-READBACK-1 (2026-08-25) read a runtime dpbp back with bpid equal to its object id, so divergence stays unobserved on this board — the law is kept as a prohibition, nothing relies on equality (dpbp.md unknown 2) → `pool-objects` (#6) |
| DPBP-I6 | deferred | `pool-objects` (#6): per-consumer dpbp count below core scope | — | verified (ADR-0012) |
| DPBP-I7 | board-settled | V-CEIL-1 (the dpbp pool floor of `dprc show mc.global --resources`) → `pool-objects` (#6) | — | board-settled at the pool floor, V-CEIL-1 rev 1 (2026-08-29): a dpbp create is refused with No resources exactly when the buffer-pool free count reaches zero (63 created, the 64th refused, the count read zero at the ceiling), and every destroy returned its unit — the census predicts the refusal to the object (the suite's overall FAIL was the unrelated dpmcp-portal ambivalence, ADR-0011) |
| DPIO-I1 | modeled | `core/invariants.qnt` `DPIO_I1` + `main.qnt` `DPMCP_I1Test` (the dpio→dpmcp arrow) | apalache | — |
| DPIO-I2 | deferred | `pool-objects` (#6): regime-typed dpio counts | — | verified (ADR-0012) |
| DPIO-I3 | deferred | `pool-objects` (#6): V-DPIO-1 — NO_CHANNEL dpio, reported `num_priorities` | — | open: the reported half is settled — V-READBACK-1 (2026-08-25) created a NO_CHANNEL dpio at 8 priorities and `dpio info` reports `0x8`, the mode does not fold them away; the kernel half (what the driver does with a NO_CHANNEL dpio's priorities) → #6, since a runtime dpio cannot bind on this pair (ADR-0008) |
| DPIO-I4 | modeled | structural — the state carries no dpio↔CPU pairing to key on | typecheck | — |
| DPIO-I5 | deferred | this change ph.4 adapter: probe success ≠ full function; per-target read-back | — | — |
| DPCON-I1 | deferred | `pool-objects` (#6): min(CPUs, queues) coupling abstracted to draw=1 | — | verified (C1 + shortfall path) |
| DPCON-I2 | modeled | `core/invariants.qnt` `LAW4_twoIdSpaces` + `main.qnt` `DPCON_I2Test` | apalache | — |
| DPCON-I3 | deferred | `pool-objects` (#6): priority capacity not in core state | — | — |
| DPCON-I4 | deferred | `pool-objects` (#6): the mutable dpcon→dpio notification edge | — | — |
| DPCON-I5 | modeled | `main.qnt` `DPCON_I5Test` (shared with DPBP-I3) | simulate | open: the free path is Linux-side, not restool-observable — V-POOL-2 rev 1 (2026-08-29) recorded the dpbp plugged and MC-listed but the unplugged child never bound, so the free-then-reallocate cycle was never driven → `pool-objects` (#6) |
| DPMCP-I1 | modeled | `main.qnt` `DPMCP_I1Test` (dependency bottom; dpio's probe draws a dpmcp) | simulate | — |
| DPMCP-I2 | modeled | `main.qnt` `DPMCP_I2Test`/`DPMCP_I2ReturnTest` | simulate | falsified as stated 2026-08-29 (V-POOL-3 rev 1, V-CONC-1 rev 1): the uapi admits one opener at a time — the second opener of `/dev/dprc.1` fails `open()` with `EINVAL` while the first is held, not `ENXIO` at exhaustion, with over a hundred portals free; the uapi records the root dprc as the consumer of its own child dpmcp, a device-link cycle the kernel refuses (ADR-0006 amendment). The N-openers-need-N−1-portals law does not hold; the model's `DPMCP_I2Test` is superseded → `pool-objects` (#6); rev 2 counted it (one held, 119 refused) |
| DPMCP-I3 | modeled | `main.qnt` `DPMCP_I3Test` (no reset anywhere in the lifecycle) | simulate | open: restool exposes no portal state, so the statefulness-across-owners half has no observable here — V-POOL-2 rev 1 and rev 2 (2026-08-29) could show only the dpbp side of the cycle → `pool-objects` (#6) |
| DPMCP-I4 | modeled | `core/invariants.qnt` `DPRC_I1` (placement face) | apalache | verified (ls-addmux violation demonstrates) |
| DPMCP-I5 | deferred | this change ph.4 online driver: per-step timeout; no fairness assumption | — | — |
| DPMCP-I6 | board-settled | V-CEIL-1 rev 2 + the rev 2 snapshots (`mcp` in `mc.global --resources`) | — | board-settled 2026-08-29: a destroyed dpmcp's MC portal never returns within a boot — 200 → 138 after 64 creates, 138 after 64 destroys, 138 after the scratch child's destroy, 203 after the reboot; a firmware leak, not a container quota (ADR-0011 §3); the reconciler creates portals once and never recycles one through destroy |
| DPMCP-I7 | board-settled | V-POOL-4 (`mcp` across per-create and per-destroy readings of `mc.global --resources`) | — | board-settled 2026-08-30 (V-POOL-4 rev 2): three dpios drew one `swp` and one `swpch.8wq` each, three dpnis drew 4 fq, 2 cg, 2 qd, 4 kp.wr0.ctlui, 3 plcye.wr0.ctlui, 3 qpr, 1 ifp.wr0, 1 prp.wr0.ctlue, 1 prp.wr0.ctlui and 1 plcy.wr0.ctlui each, `mcp` never moved and every unit returned on destroy; the dpmcp count is the consumer's — one per probing consumer in the kernel, one per process on the DPDK bus (`core/companions.qnt`, ADR-0012) |
| DPSECI-I1 | deferred | `dpseci-typestate` (#8): cfg surface | — | — |
| DPSECI-I2 | deferred | this change ph.4 generator: restool-layer validation coded at generation | — | open: V-DPSECI-1 (MC layer) → `dpseci-typestate` (#8); the restool layer is board-anchored 2026-08-29 (V-DPSECI-1/probes-rev1, 8/8: priority 0, a priority above 8, and a priority-count ≠ num-queues are each refused by restool's own parser, exit 234, before any MC command — so the MC-layer validation stays unreachable through restool) |
| DPSECI-I3 | deferred | this change ph.4 adapter: dpseci convergence reads raw GET_ATTR, never `info` | — | open: V-DPSECI-2 (raw GET_ATTR) → `mc-portal-backend` (#10) |
| DPSECI-I4 | deferred | `dpseci-typestate` (#8): HAS_CG backpressure | — | — |
| DPSECI-I5 | modeled | `main.qnt` `DPSECI_I5Test` (unbind grants no cleanliness) | simulate | open: V-DPSECI-2 (API 5.4 reset path) → `dpseci-typestate` (#8) |
| DPSECI-I6 | deferred | consumer-side stress, out of this series' suites (traffic-inventory §4) | — | verified (ADR-0005, vpp-dpaa2-support) |
| DPSECI-I7 | modeled | structural — LAW 5: single-owner fields are ADR-0006's assumption, not an MC claim | typecheck | — |
| DPSECI-I8 | modeled | `main.qnt` `DPSECI_I8Test` (model refuses only what restool refuses) | simulate | — |
| DPSECI-I9 | deferred | `dpseci-typestate` (#8): counters block-global | — | — |
| DPSW-I1 | deferred | `dpsw-typestate` (#11): create-time bindability predicate over cfg | — | positive face verified 2026-08-23 (V-DPSW-1); refusal face verified 2026-08-29 (V-DPSW-4 rev 1): a switch built with restool's silent defaults (flooding PER_VLAN, broadcast PER_OBJECT) was created and connected by the MC without complaint and then refused by the kernel `fsl_dpaa2_switch` at probe (`Flooding domain is not per FDB, cannot probe`, −95 EOPNOTSUPP), driver link empty — the create-time bindability predicate encoding stays → `dpsw-typestate` (#11) |
| DPSW-I2 | deferred | this change ph.4 generator (vendor recipes are never oracles) + #11 | — | refusal face verified 2026-08-29 (V-DPSW-4 rev 1): the default-built (PER_VLAN/PER_OBJECT) switch the vendor recipe emits was created and connected by the MC but refused by the kernel at probe, so the recipe is no oracle — the shape ls-addsw builds can never carry kernel traffic → `dpsw-typestate` (#11) |
| DPSW-I3 | modeled | `main.qnt` `DPSW_I3Test` (census draw 1 dpmcp + 1 dpbp + 0 dpcon) | simulate | verified 2026-08-23 (V-DPSW-1): the probe drew exactly the created dpmcp and dpbp, no dpcon in the container |
| DPSW-I4 | modeled | `main.qnt` `DPSW_I4Test` (bind-resets, strong form) | simulate | open: V-DPSW-2 (raw reset) → `dpsw-typestate` (#11) |
| DPSW-I5 | modeled | `core/invariants.qnt` `LAW4_twoIdSpaces` (`DPBP_I5Test` is the witness) | apalache | — |
| DPSW-I6 | deferred | `dpsw-typestate` (#11): regime-ownership matrix | — | — |
| DPSW-I7 | deferred | this change ph.4 adapter (LAW 2 family instance) | — | — |
| DPSW-I8 | deferred | `dpsw-typestate` (#11): switchdev-owned runtime surface | — | — |
| DPDMUX-I1 | deferred | `dpdmux-typestate` (#12): method×regime operability matrix | — | — |
| DPDMUX-I2 | deferred | `dpdmux-typestate` (#12) + this change ph.4 adapter | — | — |
| DPDMUX-I3 | modeled | `main.qnt` `DPDMUX_I3Test` (reset core; the mutable resetable mask → #12) | simulate | open: V-DPDMUX-3 → `dpdmux-typestate` (#12) |
| DPDMUX-I4 | deferred | `dpdmux-typestate` (#12): default_if as the sole mutable cfg field | — | — |
| DPDMUX-I5 | modeled | `main.qnt` `DPDMUX_I5Test` (census draw 1 dpmcp) | simulate | verified 2026-08-23 (V-DPDMUX-1 rev 2): `dpaa2_evb` bound with a single created dpmcp as the only companion |
| DPDMUX-I6 | deferred | this change ph.4 generator (ls-addmux is never an oracle) | — | — |
| DPDMUX-I7 | deferred | this change ph.4 adapter (LAW 2 family instance) | — | — |
| DPDMUX-I8 | modeled | `core/connect.qnt` `legalPorts` + `main.qnt` `DPDMUX_I8Test`/`DPDMUX_I8NonZeroUplinkTest` | simulate | uplink→dpmac face verified 2026-08-23 (V-DPDMUX-1: clean connect on `.0`); the uplink→dpni refusal is model-forbidden, so the illegal pairing lives in a suite hook — and on the pinned firmware it diverged, final at rev 5 (V-DPDMUX-2 rev 1–5, 2026-08-29): MC 10.39 *accepts* a dpni on the uplink (rev 2) and on the downlink (rev 5, from a fresh boot, with the state agreeing) and cannot disconnect it from any end (Configuration error 0x6, or No resources 0x8 at the bare-name uplink), so both interfaces are accept-then-un-disconnectable and the model keeps its `legalPorts` guard ahead of the firmware (ADR-0009); the dpmac-uplink disconnect and the downlink-with-populated-uplink connect stay unissued → `dpdmux-typestate` (#12) |
| DPAIOP-I1 | modeled | `families/dpaiop.qnt` creatable=false + `main.qnt` `DPAIOP_I1Test` | simulate | verified 2026-08-29 (V-DPAIOP-1/probes-rev1, 6/6): `dpaiop create` on this AIOP-less board is refused Configuration error (0x6), nothing created — the status the 10.18.0 gate never named is now on record |
| DPAIOP-I2 | modeled | `main.qnt` `DPAIOP_I1Test` (OBJ_CREATE present, refused anyway) | simulate | verified 2026-08-29 (V-DPAIOP-1/probes-rev1): the refusal fires in the MC's dpaiop create handler even though the root container permits OBJ_CREATE — `dprc create --options=…AIOP` was accepted the same sitting — so container create-permission does not imply create-permission |
| DPAIOP-I3 | deferred | this change ph.4 generator: generate-dpl is not a round-trip | — | re-anchored 2026-08-29: no dpaiop can exist on this silicon (V-DPAIOP-1), so the dpaiop node can never be emitted here; the not-a-round-trip law itself is verified on the families that do exist (V-GENDPL-1, see DPDCEI-I2/DPDMAI-I4) → `dpl-tape-out` (#14) |
| DPAIOP-I4 | deferred | `tier-c-families` (#13); unfalsifiable on this platform | — | — |
| DPCI-I1 | modeled | `core/connect.qnt` `legalPair` + `main.qnt` `DPCI_I1Test`/`DPCI_I1CrossFamilyTest` (priority asymmetry → #13) | simulate | — |
| DPCI-I2 | modeled | structural — the model carries no dpci options state | typecheck | open: V-DPCI-2 → `tier-c-families` (#13) |
| DPCI-I3 | modeled | `main.qnt` `DPCI_I3Test` + `families/dpci.qnt` createTriggersRescan=false | simulate | verified 2026-08-29 (V-DPRC-5 rev 1): the create command triggers no rescan, but the BSP's `autorescan=1` makes the dprc driver rescan on the MC's object-added interrupt, so a root-created dpci was bus-visible before the hook's explicit `dprc sync`; the MC law holds and the sysfs lag is a kernel setting to read, not assume → `tier-c-families` (#13) |
| DPCI-I4 | board-settled | V-DPCI-1 | — | verified 2026-08-23 (V-DPCI-1 rev 2, 7/7): a bare GPP↔GPP pair created in a scratch container connected (issued against the root ancestor) and read back peered at 1 priority each — no platform gate |
| DPCI-I5 | modeled | `main.qnt` `DPCI_I5Test` (connect sets no link state) | simulate | verified (V-LINK-1: pair reads link-down after connect; consumer enable required) |
| DPDCEI-I1 | deferred | `intent-layer` (#3): the `TenantAbsent` refusal — a compile-time rule of `models/intent/` (task 1.3), not a board probe | — | open: V-DPDCEI-1 probes → `tier-c-families` (#13); the create face landed (V-LIFE-DPDCEI-1) and the API-version half is read: dpdcei reports 2.3, the module is linked into this firmware (V-READBACK-1, 2026-08-25) |
| DPDCEI-I2 | deferred | this change ph.4 generator | — | verified 2026-08-29 (V-GENDPL-1 rev 1, 4/4 + hook 1/1): the emitted dpdcei node carries `engine` only — the create-time priority is write-only, absent from `dpdcei info` and the DPL alike → `dpl-tape-out` (#14) |
| DPDCEI-I3 | deferred | this change ph.4 adapter (LAW 2; `DPSECI_I8Test` is the class witness) | — | — |
| DPDCEI-I4 | deferred | `tier-c-families` (#13) | — | — |
| DPDMAI-I1 | deferred | `tier-c-families` (#13) | — | — |
| DPDMAI-I2 | deferred | this change ph.4 adapter (LAW 6) | — | — |
| DPDMAI-I3 | deferred | `tier-c-families` (#13): V-DPDMAI-1 — the reference kernel registers no qdma driver (ADR-0008), so the consumer-shape coupling is unfalsifiable on this pair | — | unfalsifiable on the reference pair (V-LIFE-DPDMAI-1 rev 2: a bare dpdmai stays unbound with nothing to claim it); re-anchored to #13 with a qdma-capable kernel as its precondition |
| DPDMAI-I4 | deferred | this change ph.4 generator | — | verified 2026-08-29 (V-GENDPL-1 rev 1): a dpdmai created `--priorities=2,4` is emitted as `priorities = <0x2>` — the priority *count* where the DPL grammar expects the list — so a re-applied DPL builds a different object; the dpci in the same container round-trips → `dpl-tape-out` (#14) |
| DPDMAI-I5 | board-settled | V-READBACK-1 (`dpdmai info` of a bare create, from the suite hook) | — | verified 2026-08-29 (V-READBACK-1 rev 2, steps 6/6, hook 10/10): the corrected hook confirms a bare dpdmai is 1 queue and 2 priorities on MC 10.39.0 (API 3.4); the model keeps the count unspecified, the number is on record |
| DPRTC-I1 | modeled | `families/dprtc.qnt` singleton + `main.qnt` `DPRTC_I1Test` | simulate | verified 2026-08-24 (V-DPRTC-1): refused No resources (0x8), `dprc show` byte-identical before/after |
| DPRTC-I2 | modeled | structural — LAW 5: both-stacks-configured is unrepresentable (single bind field) | typecheck | — |
| DPRTC-I3 | deferred | this change ph.4 adapter: no clock state readable via restool | — | — |
| DPRTC-I4 | deferred | `dpmac-typestate` (#7): timestamping path outside object lifecycle | — | verified (reference DPC + 10.36 changelog) |
| DPRTC-I5 | modeled | structural — no create-config state carried (dpbp class) | typecheck | — |
| DPDBG-I1 | modeled | `families/dpdbg.qnt` RootOnly+singleton + `main.qnt` `DPDBG_I1Test`/`DPDBG_I1SingletonTest` | simulate | singleton half verified 2026-08-24 (V-DPDBG-1): No resources (0x8); non-root half restool-unreachable (create hardcodes the root) → raw-command probe under `mc-portal-backend` (#10) |
| DPDBG-I2 | modeled | structural — no debug-state observable exists in the model (formal-models spec scenario) | typecheck | — |
| DPDBG-I3 | deferred | this change ph.4 adapter (LAW 2: dump verified by artifact, never exit) | — | anchored 2026-08-24 (V-DPDBG-1): both dumps exit 0 with the artifact only in the MC log |
| DPDBG-I4 | modeled | `main.qnt` `DPDBG_I4Test` (bus-visible, driver-less, never kernel-bindable) | simulate | `dprc show` face verified 2026-08-24 (V-DPDBG-1 trace 4/4); sysfs face unprobed — needs V-DPRC-5's bus-visibility observation → `dprc-encapsulation` (#4) |

## Intent alphabet coverage (task 2.4)

The `intent-layer` random simulation counts how much of the refusal/warning
alphabet it reaches, the same honesty mechanism the ledger applies to the
board: a variant the alphabet cannot reach is a decision on record here, not
an omission. `pnpm model:coverage` runs `models/intent/alphabet.qnt` under
every invariant with one witness per outcome and structure dimension; the
counted run is seed 20260831, 12 steps, 3000 samples, dated 2026-09-01,
deterministic and reproducible. No invariant violated (the deep hunt found no
counterexample). Three widenings this counting drove are stated in the model:
`DPMACS` gained id 99 (absent from the inventory) so an Unanchored port is
drawable, `RATES` gained 40000 (no worker row) so UnknownRateClass fires, and
`FLOWS` gained 17 (past one dpseci's 16-queue-pair ceiling) so
CryptoFlowsOverDevice fires (task 2.6e).

- **Reached by the random alphabet** (traces of 3000): every anchor refusal
  (Unanchored 824, ReservedAnchor 1488, OverRate 1722), every fabric refusal
  (MemberUnresolved 1629, SelfMember 689, FabricNotKernelForwarded 375,
  PortTenantMismatch 625, UnsupportedEdge 177), DoubleClaimed 706, the sizing
  refusals (UnknownRateClass 339, CoreBudgetExceeded 256), the extra refusals
  (ExtraNotCompanion 979, ExtraNotPositive 1129), both crypto-flows refusals
  (CryptoFlowsOverDevice 1199, CryptoFlowsNotPositive 1172), UnpricedDataplane
  2075, every pool/isolation refusal (PoolWithoutRestricted 2087,
  PoolDataplaneMismatch 654, PoolChain 647, HolderNotPublic 590,
  RestrictedWithoutPool 400), TenantAbsent 235 (a Restricted tenant may name a
  pool holder never declared — construct "pool", reachable through
  `addTenant`), and the UnknownCeiling warning 3000; Accepted 3000, Refused
  3000.
- **Alphabet-unreachable, covered elsewhere** (0 traces): `ForeignAnchor` —
  the inventory marks no dpmac Foreign, covered by `unanchoredForeignTest`
  (`intent/main.qnt`, `invWithForeignDpmac7`); `Infeasible` — intents this
  small never sum past a REF_INVENTORY ceiling, covered by the vfabric
  overdrawn-pool twin (`scenarios/vfabric.qnt` `twinInfeasibleTest`,
  `Counted(5)`) and `infeasibleTest`. The `UnmeasuredCombination` warning is
  reachable but unhit in 3000 samples (a clean accepted cross-class mix is a
  narrow draw — the sole Free 25G dpmac is 4); its shape precursor is counted
  (`wMixedRates` 444) and the warning is covered by `mixedRateClassWarnsTest`.
