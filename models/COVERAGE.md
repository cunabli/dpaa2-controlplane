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
- **board-pending** — only the board can settle it; the row names the
  traffic-inventory scenario that settles it. Board results fold back
  into the row as suites complete (task 5.6).

Tally: 52 modeled, 45 deferred, 8 board-pending — 105 candidates.

| Candidate | Disposition | Location / owning change / settling scenario | CI rung | Board status |
|-----------|-------------|----------------------------------------------|---------|--------------|
| DPRC-I1 | modeled | `core/invariants.qnt` `DPRC_I1` (also DPMCP-I4's placement face) | apalache | — |
| DPRC-I2 | modeled | `core/invariants.qnt` `DPRC_I2` (`unplugAt` now requires an unbound object) | apalache | verified (V-LINK-5: unplug of a bound netdev-backed dpni refused −EBUSY, not raced) |
| DPRC-I3 | modeled | `main.qnt` `DPRC_I3Test` | simulate | pending V-DPRC-1 (exact MC error) |
| DPRC-I4 | modeled | `core/invariants.qnt` `DPRC_I4` | apalache | verified (prior work) |
| DPRC-I5 | modeled | `core/connect.qnt` `canConnect` + `main.qnt` `DPNI_I9Test` | simulate | — |
| DPRC-I6 | modeled | `main.qnt` `DPRC_I6Test`, `DPRC_I6RescanRefusedTest` | simulate | pending V-DPRC-5 |
| DPRC-I7 | modeled | `main.qnt` `DPRC_I7Test` | simulate | — |
| DPRC-I8 | board-pending | V-POOL-1 (batch plug→probe ordering; the machine models no scan batching) | — | pending |
| DPRC-I9 | board-pending | V-DPRC-1 (teardown reachability; `canonicalLifecycleTest` is the model-side witness) | simulate | pending |
| DPRC-I10 | modeled | `main.qnt` `DPRC_I10Test` | simulate | — |
| DPRC-I11 | modeled | `main.qnt` `DPRC_I11Test` + `DPRC_I11SpawnTest`/`DPRC_I11UnlockTest` | simulate | pending V-DPRC-3 (who unlocks) |
| DPNI-I1 | modeled | structural — LAW 1: no cfg block in state, no action mutates one | typecheck | — |
| DPNI-I2 | modeled | `main.qnt` `DPNI_I2Test` | simulate | pending V-DPNI-3 |
| DPNI-I3 | modeled | `retro/reconciler.qnt` association runs, replayed by `dpaa2-verify` against the reconciler; MAC value semantics → `dpmac-typestate` (#7) | itf-replay | verified (ADR-0001 C2) |
| DPNI-I4 | modeled | `machine.qnt` kernelBind census guard + `main.qnt` `DPNI_I4Test` | simulate | verified (ADR-0001 C1) |
| DPNI-I5 | deferred | `dpni-typestate` (#5) + `pool-objects` (#6): queue/channel counts abstracted to draw=1 at core scope | — | — |
| DPNI-I6 | deferred | this change ph.4 adapter — LAW 2: observation = read-back, never exit status | — | pending V-DPNI-2 (create-then-fail exit shape) |
| DPNI-I7 | board-pending | V-DPNI-1 | — | pending |
| DPNI-I8 | modeled | `main.qnt` `DPNI_I8Test` (unbind grants no reset — the no-guarantee form) | simulate | pending V-DPNI-3 |
| DPNI-I9 | modeled | `core/invariants.qnt` `DPNI_I9` + `main.qnt` `DPNI_I9Test` | apalache | verified (kdpni pairs in production) |
| DPNI-I10 | deferred | `dpni-typestate` (#5): tx-ring/thread coupling below core-model scope | — | verified (ADR-0012) |
| DPNI-I11 | deferred | this change ph.4 adapter — LAW 6: emitted command version per action | — | pending V-DPNI-4 |
| DPNI-I12 | deferred | this change ph.4 adapter: write-only field, no drift claim | — | — |
| DPMAC-I1 | modeled | `core/invariants.qnt` `DPMAC_I1` (no-additions + root-pin; destroy is off-nominal) | apalache | verified (ADR-0001 §3); pending V-DPMAC-2 (phantom create) |
| DPMAC-I2 | deferred | `dpmac-typestate` (#7): MAC values not in core state | — | verified (ADR-0001 C2) |
| DPMAC-I3 | deferred | `dpmac-typestate` (#7): attr surface with the eth_if/IPG exceptions | — | — |
| DPMAC-I4 | board-pending | V-LINK-4 (directional channels; core carries a single linkUp — refined in #7) | — | pending V-LINK-4, deferred to the online driver (no restool verb for the peer-request channel, no kernel netdev on the flagged wiring) |
| DPMAC-I5 | modeled | `main.qnt` `DPMAC_I5Test` (connection ⊥ link state) | simulate | verified (V-LINK-2 rev 3: never read as link state — but on a bound, enabled pair the connection-state text co-varies with the flap, so the two are not independent) |
| DPMAC-I6 | deferred | `dpmac-typestate` (#7): driver arbitration ⟺ peer topology not in core machine | — | verified (production use) |
| DPMAC-I7 | deferred | this change ph.4 adapter: counter vocabulary firmware-versioned, refusals silent | — | pending V-DPMAC-1 |
| DPMAC-I8 | modeled | `main.qnt` `DPSECI_I8Test` (class witness: model refuses only what restool refuses) + ph.4 adapter law | simulate | — |
| DPMAC-I9 | deferred | this change ph.4 adapter: emitted fields carried per action | — | kernel path verified (V-LINK-2 rev 3: `up` takes effect with `state_valid=0`, with propagation lag); raw probe pending V-LINK-3 (online driver) |
| DPBP-I1 | modeled | `main.qnt` `DPBP_I5Test` (zero cfg; identity is the hwId) | simulate | — |
| DPBP-I2 | modeled | `core/invariants.qnt` `DPBP_I2` + `main.qnt` `DPBP_I2Test`/`DPBP_I2PlugTest` | apalache | pending V-POOL-1 |
| DPBP-I3 | modeled | `main.qnt` `DPBP_I3Test` (dirty return on free) | simulate | pending V-POOL-2 (dpbp_reset drain) |
| DPBP-I4 | modeled | `main.qnt` `DPBP_I4Test`/`DPBP_I4TopUpTest` | simulate | verified (C1 class, ADR-0001) |
| DPBP-I5 | modeled | `core/invariants.qnt` `LAW4_twoIdSpaces` + `main.qnt` `DPBP_I5Test` | apalache | pending (bpid/id divergence on this board — dpbp.md unknown 2) |
| DPBP-I6 | deferred | `pool-objects` (#6): per-consumer dpbp count below core scope | — | verified (ADR-0012) |
| DPIO-I1 | modeled | `core/invariants.qnt` `DPIO_I1` + `main.qnt` `DPMCP_I1Test` (the dpio→dpmcp arrow) | apalache | — |
| DPIO-I2 | deferred | `pool-objects` (#6): regime-typed dpio counts | — | verified (ADR-0012) |
| DPIO-I3 | board-pending | V-DPIO-1 | — | pending |
| DPIO-I4 | modeled | structural — the state carries no dpio↔CPU pairing to key on | typecheck | — |
| DPIO-I5 | deferred | this change ph.4 adapter: probe success ≠ full function; per-target read-back | — | — |
| DPCON-I1 | deferred | `pool-objects` (#6): min(CPUs, queues) coupling abstracted to draw=1 | — | verified (C1 + shortfall path) |
| DPCON-I2 | modeled | `core/invariants.qnt` `LAW4_twoIdSpaces` + `main.qnt` `DPCON_I2Test` | apalache | — |
| DPCON-I3 | deferred | `pool-objects` (#6): priority capacity not in core state | — | — |
| DPCON-I4 | deferred | `pool-objects` (#6): the mutable dpcon→dpio notification edge | — | — |
| DPCON-I5 | modeled | `main.qnt` `DPCON_I5Test` (shared with DPBP-I3) | simulate | pending V-POOL-2 |
| DPMCP-I1 | modeled | `main.qnt` `DPMCP_I1Test` (dependency bottom; dpio's probe draws a dpmcp) | simulate | — |
| DPMCP-I2 | modeled | `main.qnt` `DPMCP_I2Test`/`DPMCP_I2ReturnTest` | simulate | pending V-POOL-3 |
| DPMCP-I3 | modeled | `main.qnt` `DPMCP_I3Test` (no reset anywhere in the lifecycle) | simulate | pending V-POOL-2 (statefulness across owners) |
| DPMCP-I4 | modeled | `core/invariants.qnt` `DPRC_I1` (placement face) | apalache | verified (ls-addmux violation demonstrates) |
| DPMCP-I5 | deferred | this change ph.4 online driver: per-step timeout; no fairness assumption | — | — |
| DPSECI-I1 | deferred | `dpseci-typestate` (#8): cfg surface | — | — |
| DPSECI-I2 | deferred | this change ph.4 generator: restool-layer validation coded at generation | — | pending V-DPSECI-1 (MC layer) |
| DPSECI-I3 | deferred | this change ph.4 adapter: dpseci convergence reads raw GET_ATTR, never `info` | — | pending V-DPSECI-2 |
| DPSECI-I4 | deferred | `dpseci-typestate` (#8): HAS_CG backpressure | — | — |
| DPSECI-I5 | modeled | `main.qnt` `DPSECI_I5Test` (unbind grants no cleanliness) | simulate | pending V-DPSECI-2 (board API 5.4 reset path) |
| DPSECI-I6 | deferred | consumer-side stress, out of this series' suites (traffic-inventory §4) | — | verified (ADR-0005, vpp-dpaa2-support) |
| DPSECI-I7 | modeled | structural — LAW 5: single-owner fields are ADR-0006's assumption, not an MC claim | typecheck | — |
| DPSECI-I8 | modeled | `main.qnt` `DPSECI_I8Test` (model refuses only what restool refuses) | simulate | — |
| DPSECI-I9 | deferred | `dpseci-typestate` (#8): counters block-global | — | — |
| DPSW-I1 | deferred | `dpsw-typestate` (#11): create-time bindability predicate over cfg | — | pending V-DPSW-1 |
| DPSW-I2 | deferred | this change ph.4 generator (vendor recipes are never oracles) + #11 | — | pending V-DPSW-1 |
| DPSW-I3 | modeled | `main.qnt` `DPSW_I3Test` (census draw 1 dpmcp + 1 dpbp + 0 dpcon) | simulate | — |
| DPSW-I4 | modeled | `main.qnt` `DPSW_I4Test` (bind-resets, strong form) | simulate | pending V-DPSW-2 (reset totality) |
| DPSW-I5 | modeled | `core/invariants.qnt` `LAW4_twoIdSpaces` (`DPBP_I5Test` is the witness) | apalache | — |
| DPSW-I6 | deferred | `dpsw-typestate` (#11): regime-ownership matrix | — | — |
| DPSW-I7 | deferred | this change ph.4 adapter (LAW 2 family instance) | — | — |
| DPSW-I8 | deferred | `dpsw-typestate` (#11): switchdev-owned runtime surface | — | — |
| DPDMUX-I1 | deferred | `dpdmux-typestate` (#12): method×regime operability matrix | — | — |
| DPDMUX-I2 | deferred | `dpdmux-typestate` (#12) + this change ph.4 adapter | — | — |
| DPDMUX-I3 | modeled | `main.qnt` `DPDMUX_I3Test` (reset core; the mutable resetable mask → #12) | simulate | pending V-DPDMUX-3 |
| DPDMUX-I4 | deferred | `dpdmux-typestate` (#12): default_if as the sole mutable cfg field | — | — |
| DPDMUX-I5 | modeled | `main.qnt` `DPDMUX_I5Test` (census draw 1 dpmcp) | simulate | — |
| DPDMUX-I6 | deferred | this change ph.4 generator (ls-addmux is never an oracle) | — | — |
| DPDMUX-I7 | deferred | this change ph.4 adapter (LAW 2 family instance) | — | — |
| DPDMUX-I8 | modeled | `core/connect.qnt` `legalPorts` + `main.qnt` `DPDMUX_I8Test`/`DPDMUX_I8NonZeroUplinkTest` | simulate | pending V-DPDMUX-2 |
| DPAIOP-I1 | modeled | `families/dpaiop.qnt` creatable=false + `main.qnt` `DPAIOP_I1Test` | simulate | pending V-DPAIOP-1 (status code) |
| DPAIOP-I2 | modeled | `main.qnt` `DPAIOP_I1Test` (OBJ_CREATE present, refused anyway) | simulate | pending V-DPAIOP-1 |
| DPAIOP-I3 | deferred | this change ph.4 generator: generate-dpl is not a round-trip | — | pending V-GENDPL-1 |
| DPAIOP-I4 | deferred | `tier-c-families` (#13); unfalsifiable on this platform | — | — |
| DPCI-I1 | modeled | `core/connect.qnt` `legalPair` + `main.qnt` `DPCI_I1Test`/`DPCI_I1CrossFamilyTest` (priority asymmetry → #13) | simulate | — |
| DPCI-I2 | modeled | structural — the model carries no dpci options state | typecheck | pending V-DPCI-2 |
| DPCI-I3 | modeled | `main.qnt` `DPCI_I3Test` + `families/dpci.qnt` createTriggersRescan=false | simulate | pending V-DPCI-1 |
| DPCI-I4 | board-pending | V-DPCI-1 | — | pending |
| DPCI-I5 | modeled | `main.qnt` `DPCI_I5Test` (connect sets no link state) | simulate | verified (V-LINK-1: pair reads link-down after connect; consumer enable required) |
| DPDCEI-I1 | deferred | `intent-layer` (#3): consumer-absence refusal is an intent-layer rule | — | pending V-DPDCEI-1 |
| DPDCEI-I2 | deferred | this change ph.4 generator | — | pending V-GENDPL-1 |
| DPDCEI-I3 | deferred | this change ph.4 adapter (LAW 2; `DPSECI_I8Test` is the class witness) | — | — |
| DPDCEI-I4 | deferred | `tier-c-families` (#13) | — | — |
| DPDMAI-I1 | deferred | `tier-c-families` (#13) | — | — |
| DPDMAI-I2 | deferred | this change ph.4 adapter (LAW 6) | — | — |
| DPDMAI-I3 | board-pending | V-DPDMAI-1 | — | pending |
| DPDMAI-I4 | deferred | this change ph.4 generator | — | pending V-GENDPL-1 |
| DPDMAI-I5 | board-pending | V-DPDMAI-1 | — | pending |
| DPRTC-I1 | modeled | `families/dprtc.qnt` singleton + `main.qnt` `DPRTC_I1Test` | simulate | verified 2026-08-24 (V-DPRTC-1): refused No resources (0x8), `dprc show` byte-identical before/after |
| DPRTC-I2 | modeled | structural — LAW 5: both-stacks-configured is unrepresentable (single bind field) | typecheck | — |
| DPRTC-I3 | deferred | this change ph.4 adapter: no clock state readable via restool | — | — |
| DPRTC-I4 | deferred | `dpmac-typestate` (#7): timestamping path outside object lifecycle | — | verified (reference DPC + 10.36 changelog) |
| DPRTC-I5 | modeled | structural — no create-config state carried (dpbp class) | typecheck | — |
| DPDBG-I1 | modeled | `families/dpdbg.qnt` RootOnly+singleton + `main.qnt` `DPDBG_I1Test`/`DPDBG_I1SingletonTest` | simulate | singleton half verified 2026-08-24 (V-DPDBG-1): No resources (0x8); non-root half restool-unreachable (create hardcodes the root), raw-command probe deferred |
| DPDBG-I2 | modeled | structural — no debug-state observable exists in the model (formal-models spec scenario) | typecheck | — |
| DPDBG-I3 | deferred | this change ph.4 adapter (LAW 2: dump verified by artifact, never exit) | — | anchored 2026-08-24 (V-DPDBG-1): both dumps exit 0 with the artifact only in the MC log |
| DPDBG-I4 | modeled | `main.qnt` `DPDBG_I4Test` (bus-visible, driver-less, never kernel-bindable) | simulate | `dprc show` face verified 2026-08-24 (V-DPDBG-1 trace 4/4); sysfs face unprobed — no bus-visibility observation in the adapter yet (V-DPRC-5's gap) |
