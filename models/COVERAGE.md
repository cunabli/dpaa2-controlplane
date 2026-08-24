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

Tally: 52 modeled, 51 deferred, 2 board-settled, 0 board-pending — 105 candidates.

| Candidate | Disposition | Location / owning change / settling scenario | CI rung | Board status |
|-----------|-------------|----------------------------------------------|---------|--------------|
| DPRC-I1 | modeled | `core/invariants.qnt` `DPRC_I1` (also DPMCP-I4's placement face) | apalache | — |
| DPRC-I2 | modeled | `core/invariants.qnt` `DPRC_I2` (`unplugAt` now requires an unbound object) | apalache | verified (V-LINK-5: unplug of a bound netdev-backed dpni refused −EBUSY, not raced) |
| DPRC-I3 | modeled | `main.qnt` `DPRC_I3Test` | simulate | open: no suite has moved a plugged object — V-DPRC-1's three revisions moved only unplugged ones, and V-LINK-5 anchored the neighbouring refusal (unplug of a bound object, −EBUSY); the plugged-move face of V-DPRC-1 → `dprc-encapsulation` (#4) |
| DPRC-I4 | modeled | `core/invariants.qnt` `DPRC_I4` | apalache | verified (prior work) |
| DPRC-I5 | modeled | `core/connect.qnt` `canConnect` + `main.qnt` `DPNI_I9Test` | simulate | — |
| DPRC-I6 | modeled | `main.qnt` `DPRC_I6Test`, `DPRC_I6RescanRefusedTest` | simulate | open: V-DPRC-5 needs a bus-visibility observation the adapter does not take → `dprc-encapsulation` (#4); the root-only bind half is board-exercised (V-LIFE-DPNI-1 binds in dprc.1; V-DPNI-1's child-container dpni never bound) |
| DPRC-I7 | modeled | `main.qnt` `DPRC_I7Test` | simulate | — |
| DPRC-I8 | deferred | `pool-objects` (#6): V-POOL-1 — batch plug→probe ordering; the machine models no scan batching | — | open: V-POOL-1 never ran (its exhaustion faces are refusals, online-driver shaped) → #6 |
| DPRC-I9 | board-settled | V-DPRC-1 (`canonicalLifecycleTest` is the model-side witness) | simulate | verified 2026-08-23 (V-DPRC-1 rev 3, 13/13): the scratch container was emptied through both move directions and destroyed, absent in read-back; ADR-0007 §3's release/evict law means a non-empty destroy never blocks teardown either |
| DPRC-I10 | modeled | `main.qnt` `DPRC_I10Test` | simulate | — |
| DPRC-I11 | modeled | `main.qnt` `DPRC_I11Test` + `DPRC_I11SpawnTest`/`DPRC_I11UnlockTest` | simulate | open: V-DPRC-3 (lock round-trips leave no monotone state to trace) → `dprc-encapsulation` (#4) |
| DPNI-I1 | modeled | structural — LAW 1: no cfg block in state, no action mutates one | typecheck | — |
| DPNI-I2 | modeled | `main.qnt` `DPNI_I2Test` | simulate | open: V-DPNI-3 (post-bind runtime state is not restool-drivable) → `dpni-typestate` (#5) |
| DPNI-I3 | modeled | `retro/reconciler.qnt` association runs, replayed by `dpaa2-verify` against the reconciler; MAC value semantics → `dpmac-typestate` (#7) | itf-replay | verified (ADR-0001 C2) |
| DPNI-I4 | modeled | `machine.qnt` kernelBind census guard + `main.qnt` `DPNI_I4Test` | simulate | verified (ADR-0001 C1) |
| DPNI-I5 | deferred | `dpni-typestate` (#5) + `pool-objects` (#6): queue/channel counts abstracted to draw=1 at core scope | — | — |
| DPNI-I6 | deferred | this change ph.4 adapter — LAW 2: observation = read-back, never exit status | — | open: V-DPNI-2 → `dpni-typestate` (#5); the class law itself is board-anchored twice (ADR-0007 §2: restool exited 0 on an MC No privilege, the read-back caught it; V-LIFE-DPIO-1 rev 1: a refused teardown unplug went unseen while its stderr was discarded) |
| DPNI-I7 | deferred | `dpni-typestate` (#5): V-DPNI-1 — attribute read-back after a bare create | — | open: V-DPNI-1 (2026-08-23) bare-created and destroyed its dpni but probed only `dprc show`, so no default was read back; the suite ledger's "defaults captured" wording was overstated and is corrected → `dpni info` after a bare create under #5 |
| DPNI-I8 | modeled | `main.qnt` `DPNI_I8Test` (unbind grants no reset — the no-guarantee form) | simulate | open: V-DPNI-3 (post-bind runtime state is not restool-drivable) → `dpni-typestate` (#5) |
| DPNI-I9 | modeled | `core/invariants.qnt` `DPNI_I9` + `main.qnt` `DPNI_I9Test` | apalache | verified (kdpni pairs in production) |
| DPNI-I10 | deferred | `dpni-typestate` (#5): tx-ring/thread coupling below core-model scope | — | verified (ADR-0012) |
| DPNI-I11 | deferred | this change ph.4 adapter — LAW 6: emitted command version per action | — | open: V-DPNI-4 (raw command via `/dev/dprc.N`) → `mc-portal-backend` (#10), the change that gives the harness a raw command path |
| DPNI-I12 | deferred | this change ph.4 adapter: write-only field, no drift claim | — | — |
| DPMAC-I1 | modeled | `core/invariants.qnt` `DPMAC_I1` (no-additions + root-pin; destroy is off-nominal) | apalache | verified (ADR-0001 §3); phantom-create face V-DPMAC-2 → `dpmac-typestate` (#7) |
| DPMAC-I2 | deferred | `dpmac-typestate` (#7): MAC values not in core state | — | verified (ADR-0001 C2) |
| DPMAC-I3 | deferred | `dpmac-typestate` (#7): attr surface with the eth_if/IPG exceptions | — | — |
| DPMAC-I4 | deferred | `dpmac-typestate` (#7): V-LINK-4 — directional channels; core carries a single linkUp | — | open: V-LINK-4 → #7 — no restool verb for the peer-request channel and no kernel netdev on the flagged wiring to drive it from |
| DPMAC-I5 | modeled | `main.qnt` `DPMAC_I5Test` (connection ⊥ link state) | simulate | verified (V-LINK-2 rev 3: never read as link state — but on a bound, enabled pair the connection-state text co-varies with the flap, so the two are not independent) |
| DPMAC-I6 | deferred | `dpmac-typestate` (#7): driver arbitration ⟺ peer topology not in core machine | — | verified (production use) |
| DPMAC-I7 | deferred | this change ph.4 adapter: counter vocabulary firmware-versioned, refusals silent | — | open: V-DPMAC-1 (read-only info probes) → `dpmac-typestate` (#7) |
| DPMAC-I8 | modeled | `main.qnt` `DPSECI_I8Test` (class witness: model refuses only what restool refuses) + ph.4 adapter law | simulate | — |
| DPMAC-I9 | deferred | this change ph.4 adapter: emitted fields carried per action | — | kernel path verified (V-LINK-2 rev 3: `up` takes effect with `state_valid=0`, with propagation lag); raw probe V-LINK-3 → `mc-portal-backend` (#10) |
| DPBP-I1 | modeled | `main.qnt` `DPBP_I5Test` (zero cfg; identity is the hwId) | simulate | — |
| DPBP-I2 | modeled | `core/invariants.qnt` `DPBP_I2` + `main.qnt` `DPBP_I2Test`/`DPBP_I2PlugTest` | apalache | open: V-POOL-1 → `pool-objects` (#6); the plugged-vs-allocator half is board-anchored (V-LIFE-DPNI-1: the census drew plugged companions; V-LIFE-DPIO-1 rev 1: after `dprc sync` the allocator claimed a free plugged dpmcp) |
| DPBP-I3 | modeled | `main.qnt` `DPBP_I3Test` (dirty return on free) | simulate | open: V-POOL-2 (`dpbp_reset` drain) → `pool-objects` (#6) |
| DPBP-I4 | modeled | `main.qnt` `DPBP_I4Test`/`DPBP_I4TopUpTest` | simulate | verified (C1 class, ADR-0001) |
| DPBP-I5 | modeled | `core/invariants.qnt` `LAW4_twoIdSpaces` + `main.qnt` `DPBP_I5Test` | apalache | open: no suite took a `dpbp info` read-back, so id-vs-bpid divergence is unobserved (dpbp.md unknown 2) → `pool-objects` (#6) |
| DPBP-I6 | deferred | `pool-objects` (#6): per-consumer dpbp count below core scope | — | verified (ADR-0012) |
| DPIO-I1 | modeled | `core/invariants.qnt` `DPIO_I1` + `main.qnt` `DPMCP_I1Test` (the dpio→dpmcp arrow) | apalache | — |
| DPIO-I2 | deferred | `pool-objects` (#6): regime-typed dpio counts | — | verified (ADR-0012) |
| DPIO-I3 | deferred | `pool-objects` (#6): V-DPIO-1 — NO_CHANNEL dpio, reported `num_priorities` | — | open: V-LIFE-DPIO-1 created a LOCAL_CHANNEL dpio at 8 priorities and read no `dpio info`; the NO_CHANNEL probe → #6 (a runtime dpio cannot bind on this pair anyway, ADR-0008) |
| DPIO-I4 | modeled | structural — the state carries no dpio↔CPU pairing to key on | typecheck | — |
| DPIO-I5 | deferred | this change ph.4 adapter: probe success ≠ full function; per-target read-back | — | — |
| DPCON-I1 | deferred | `pool-objects` (#6): min(CPUs, queues) coupling abstracted to draw=1 | — | verified (C1 + shortfall path) |
| DPCON-I2 | modeled | `core/invariants.qnt` `LAW4_twoIdSpaces` + `main.qnt` `DPCON_I2Test` | apalache | — |
| DPCON-I3 | deferred | `pool-objects` (#6): priority capacity not in core state | — | — |
| DPCON-I4 | deferred | `pool-objects` (#6): the mutable dpcon→dpio notification edge | — | — |
| DPCON-I5 | modeled | `main.qnt` `DPCON_I5Test` (shared with DPBP-I3) | simulate | open: V-POOL-2 → `pool-objects` (#6) |
| DPMCP-I1 | modeled | `main.qnt` `DPMCP_I1Test` (dependency bottom; dpio's probe draws a dpmcp) | simulate | — |
| DPMCP-I2 | modeled | `main.qnt` `DPMCP_I2Test`/`DPMCP_I2ReturnTest` | simulate | open: V-POOL-3 → `pool-objects` (#6) |
| DPMCP-I3 | modeled | `main.qnt` `DPMCP_I3Test` (no reset anywhere in the lifecycle) | simulate | open: V-POOL-2 (statefulness across owners) → `pool-objects` (#6) |
| DPMCP-I4 | modeled | `core/invariants.qnt` `DPRC_I1` (placement face) | apalache | verified (ls-addmux violation demonstrates) |
| DPMCP-I5 | deferred | this change ph.4 online driver: per-step timeout; no fairness assumption | — | — |
| DPSECI-I1 | deferred | `dpseci-typestate` (#8): cfg surface | — | — |
| DPSECI-I2 | deferred | this change ph.4 generator: restool-layer validation coded at generation | — | open: V-DPSECI-1 (MC layer) → `dpseci-typestate` (#8); the restool layer is board-anchored (V-LIFE-DPSECI-1 rev 1: `--num-queues` and `--priorities` demanded as a pair) |
| DPSECI-I3 | deferred | this change ph.4 adapter: dpseci convergence reads raw GET_ATTR, never `info` | — | open: V-DPSECI-2 (raw GET_ATTR) → `mc-portal-backend` (#10) |
| DPSECI-I4 | deferred | `dpseci-typestate` (#8): HAS_CG backpressure | — | — |
| DPSECI-I5 | modeled | `main.qnt` `DPSECI_I5Test` (unbind grants no cleanliness) | simulate | open: V-DPSECI-2 (API 5.4 reset path) → `dpseci-typestate` (#8) |
| DPSECI-I6 | deferred | consumer-side stress, out of this series' suites (traffic-inventory §4) | — | verified (ADR-0005, vpp-dpaa2-support) |
| DPSECI-I7 | modeled | structural — LAW 5: single-owner fields are ADR-0006's assumption, not an MC claim | typecheck | — |
| DPSECI-I8 | modeled | `main.qnt` `DPSECI_I8Test` (model refuses only what restool refuses) | simulate | — |
| DPSECI-I9 | deferred | `dpseci-typestate` (#8): counters block-global | — | — |
| DPSW-I1 | deferred | `dpsw-typestate` (#11): create-time bindability predicate over cfg | — | positive face verified 2026-08-23 (V-DPSW-1): the predicate-satisfying shape — control interface on, PER_FDB flooding and broadcast — bound `fsl_dpaa2_switch`; the refusal face (a default-built switch rejected at probe) was never issued on the board, the suite ledger's wording being a driver-code prediction, now corrected → `dpsw-typestate` (#11) |
| DPSW-I2 | deferred | this change ph.4 generator (vendor recipes are never oracles) + #11 | — | open: no ls-addsw-shaped create was issued on the board → `dpsw-typestate` (#11) |
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
| DPDMUX-I8 | modeled | `core/connect.qnt` `legalPorts` + `main.qnt` `DPDMUX_I8Test`/`DPDMUX_I8NonZeroUplinkTest` | simulate | uplink→dpmac face verified 2026-08-23 (V-DPDMUX-1: clean connect on `.0`); the uplink→dpni refusal is model-forbidden, hence untraceable → V-DPDMUX-2 under `dpdmux-typestate` (#12) |
| DPAIOP-I1 | modeled | `families/dpaiop.qnt` creatable=false + `main.qnt` `DPAIOP_I1Test` | simulate | open: V-DPAIOP-1 (status code) → `tier-c-families` (#13) |
| DPAIOP-I2 | modeled | `main.qnt` `DPAIOP_I1Test` (OBJ_CREATE present, refused anyway) | simulate | open: V-DPAIOP-1 → `tier-c-families` (#13) |
| DPAIOP-I3 | deferred | this change ph.4 generator: generate-dpl is not a round-trip | — | open: V-GENDPL-1 (emit-and-diff of `generate-dpl`) → `dpl-tape-out` (#14), the change that reads DPLs back |
| DPAIOP-I4 | deferred | `tier-c-families` (#13); unfalsifiable on this platform | — | — |
| DPCI-I1 | modeled | `core/connect.qnt` `legalPair` + `main.qnt` `DPCI_I1Test`/`DPCI_I1CrossFamilyTest` (priority asymmetry → #13) | simulate | — |
| DPCI-I2 | modeled | structural — the model carries no dpci options state | typecheck | open: V-DPCI-2 → `tier-c-families` (#13) |
| DPCI-I3 | modeled | `main.qnt` `DPCI_I3Test` + `families/dpci.qnt` createTriggersRescan=false | simulate | open: V-DPCI-1 took no sysfs read around a rescan (its module header parks that face) → `tier-c-families` (#13) |
| DPCI-I4 | board-settled | V-DPCI-1 | — | verified 2026-08-23 (V-DPCI-1 rev 2, 7/7): a bare GPP↔GPP pair created in a scratch container connected (issued against the root ancestor) and read back peered at 1 priority each — no platform gate |
| DPCI-I5 | modeled | `main.qnt` `DPCI_I5Test` (connect sets no link state) | simulate | verified (V-LINK-1: pair reads link-down after connect; consumer enable required) |
| DPDCEI-I1 | deferred | `intent-layer` (#3): consumer-absence refusal is an intent-layer rule | — | open: V-DPDCEI-1 probes → `tier-c-families` (#13); the create face landed (V-LIFE-DPDCEI-1) |
| DPDCEI-I2 | deferred | this change ph.4 generator | — | open: V-GENDPL-1 → `dpl-tape-out` (#14) |
| DPDCEI-I3 | deferred | this change ph.4 adapter (LAW 2; `DPSECI_I8Test` is the class witness) | — | — |
| DPDCEI-I4 | deferred | `tier-c-families` (#13) | — | — |
| DPDMAI-I1 | deferred | `tier-c-families` (#13) | — | — |
| DPDMAI-I2 | deferred | this change ph.4 adapter (LAW 6) | — | — |
| DPDMAI-I3 | deferred | `tier-c-families` (#13): V-DPDMAI-1 — the reference kernel registers no qdma driver (ADR-0008), so the consumer-shape coupling is unfalsifiable on this pair | — | unfalsifiable on the reference pair (V-LIFE-DPDMAI-1 rev 2: a bare dpdmai stays unbound with nothing to claim it); re-anchored to #13 with a qdma-capable kernel as its precondition |
| DPDMAI-I4 | deferred | this change ph.4 generator | — | open: V-GENDPL-1 → `dpl-tape-out` (#14) |
| DPDMAI-I5 | deferred | `tier-c-families` (#13): V-DPDMAI-1 — `info` after a bare create | — | open: V-LIFE-DPDMAI-1 bare-created (`num_queues` omitted) but probed only `dprc show`, so the MC-chosen count is unread; the suite ledger's I3/I5 wording is corrected → #13 |
| DPRTC-I1 | modeled | `families/dprtc.qnt` singleton + `main.qnt` `DPRTC_I1Test` | simulate | verified 2026-08-24 (V-DPRTC-1): refused No resources (0x8), `dprc show` byte-identical before/after |
| DPRTC-I2 | modeled | structural — LAW 5: both-stacks-configured is unrepresentable (single bind field) | typecheck | — |
| DPRTC-I3 | deferred | this change ph.4 adapter: no clock state readable via restool | — | — |
| DPRTC-I4 | deferred | `dpmac-typestate` (#7): timestamping path outside object lifecycle | — | verified (reference DPC + 10.36 changelog) |
| DPRTC-I5 | modeled | structural — no create-config state carried (dpbp class) | typecheck | — |
| DPDBG-I1 | modeled | `families/dpdbg.qnt` RootOnly+singleton + `main.qnt` `DPDBG_I1Test`/`DPDBG_I1SingletonTest` | simulate | singleton half verified 2026-08-24 (V-DPDBG-1): No resources (0x8); non-root half restool-unreachable (create hardcodes the root) → raw-command probe under `mc-portal-backend` (#10) |
| DPDBG-I2 | modeled | structural — no debug-state observable exists in the model (formal-models spec scenario) | typecheck | — |
| DPDBG-I3 | deferred | this change ph.4 adapter (LAW 2: dump verified by artifact, never exit) | — | anchored 2026-08-24 (V-DPDBG-1): both dumps exit 0 with the artifact only in the MC log |
| DPDBG-I4 | modeled | `main.qnt` `DPDBG_I4Test` (bus-visible, driver-less, never kernel-bindable) | simulate | `dprc show` face verified 2026-08-24 (V-DPDBG-1 trace 4/4); sysfs face unprobed — needs V-DPRC-5's bus-visibility observation → `dprc-encapsulation` (#4) |
