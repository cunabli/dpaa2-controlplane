# Proposal: cross-dprc-links

## Why

The intent vocabulary has spoken dpni↔dpni links since `intent-layer`
(ADR-0013 §link), but the reconciler has never actuated one: child
populate resolves only port-edges and root reconcile actuates only
dpni↔dpmac, so a declared pseudo-wire derives, dry-runs, and then
silently stays `plan_only`. Roadmap #9 closes that gap end to end —
intent → plan → root-issued `dprc connect` → board-witnessed frames —
replacing the `ls-addni` workflow (directionally: a kernel netdev wired
to a userspace dataplane for routing). This change also owns the
ADR-0017 rebind-drift healing policy deferred from `pool-objects`
(design D11), the first tile where a live consumer container makes that
policy meaningful.

## What Changes

- **Shared connection surface** in `dpaa2-api::plan`: an edge-kind
  table with per-kind reification policy, mirroring the one already in
  `models/core/connect.qnt` (`legalPair`, `edgeDemandsSeveredWitness`).
  The delivered dpni↔dpmac machinery (transitions, executor,
  `SeveredProof`) is *claimed* as the port-edge kind's reification —
  not retyped, not rewritten. Proof: the dpmac family's frozen ITF
  traces replay green, unchanged.
- **dpni↔dpni actuation**, container-agnostic: root↔root, root↔child,
  child↔child all representable; connect issued at a common ancestor
  (root today); patterns the MC refuses surface as typed refusals.
  Loopback stays intent-refused (`LinkSelfLoop`, programmatic parity).
- **Lifecycle partial order**: populate → connect → bind on the
  container typestate (ADR-0017 decision 3 extended to the connection
  face); connect/disconnect of already-visible endpoints additionally
  legal post-bind; disconnect-before-destroy; disconnect-before-
  reconnect (DPRC-I5 promoted from candidate).
- **ADR-0017 healing policy** (discharges PASS4-F8): post-bind create
  becomes representable only with an eager `DeferredVisibility`
  obligation attached; the destroy side carries the lazy stale-node
  mirror residue; the only modeled discharge is a consented
  `Disruptive`-class rebind cycle — never a silent rebind. Declined
  consent reports typed residue. No reboot anywhere on this surface.
- **Model gate**: new `link_lifecycle` Quint model consuming
  `core/connect.qnt`, invariants LINK-I\*, the container-interplay
  obligations, and the V-TRAF-1 scenario module.
- **McControl connection verbs shaped portal-ready** for #10: named 1:1
  with whitelisted MC commands (`DPRC_GET_CONNECTION`,
  `DPRC_CONNECT/DISCONNECT`, `DPNI_GET_LINK_STATE`), typed returns, no
  restool-text leakage through the trait. No portal read-slice
  additions here (ADR-0021: #10 owns every addition).
- **Netlink side stays zero-Rust**: the frame witness (netns + ping)
  rides operator-reviewed suite hook scripts, the proven V-TRAF-0
  pattern. No new dependency; `dpaa2-hal` is untouched on this axis.
- **Additive intent knobs** for the two adopted deferral rows: a dpcon
  priority knob (DPCON-I3) and the dpni `SINGLE_SENDER` option
  (baseline unknown #7) — neither is expressible today.
- **Docs**: one new connection-surface ADR (edge-kind table +
  reification rows including the healing policy); amendments to
  ADR-0017 (PASS4-F8 discharged), ADR-0019 (edge facet gains its second
  inhabitant), ADR-0003 §8 (Mellanox decision point dropped — the
  pseudo-wire needs no external port; revisit trigger reworded to "a
  phase requiring sustained external traffic", no tile attached);
  roadmap row #9 cleaned, row #10 gains the three portal-dependent
  deferral rows (DPCON-I4, dpni unknowns #4/#11) via one line naming
  the dossier bead that carries this scoping session's analysis.
- **Board milestone**: suite V-TRAF-1, one sitting, full teardown per
  protocol. Frames witnessed root↔root (netns hook, exact-count
  oracle, saturation smoke with no rate target); cross-container
  witnessed at connect/convergence level; the create-side heal and the
  unwitnessed destroy-side mirror both get dedicated faces; no PHY
  dpmac appears anywhere (finding 49 stays unpoked).

## Capabilities

### New Capabilities

(none — the link construct lands inside the existing capability map)

### Modified Capabilities

- `reconciler`: the connection surface — edge-kind table with per-kind
  reification, link transitions keyed by ends, the populate→connect→
  bind partial order, the two ADR-0017 obligations and their consented
  discharge, DPRC-I5 and disconnect-before-destroy as plan laws.
- `mc-backend`: dpni↔dpni connect/disconnect/observe actuation,
  ancestor-explicit and portal-ready typed verbs; generalized peer
  resolution in child populate; `DriftRefused` → typed obligation
  plumbing.
- `intent-compiler`: link edges become actuatable plan output (today
  derived but `plan_only`); additive derivation for the dpcon priority
  knob and `SINGLE_SENDER` option.
- `topology-config`: the two additive intent knobs on the config
  surface.
- `provisioning-cli`: plan rendering and consent flow for the
  `Disruptive` rebind cycle; link and obligation rows in
  `status --detail`.
- `formal-models`: `link_lifecycle` model, LINK-I\* invariants,
  COVERAGE rows (including re-pointing the three rows assigned to #10).

## Impact

- **Crates**: `dpaa2-api` (plan/connection surface, intent knobs),
  `dpaa2-mc` (actuation), `dpaa2-config` (knobs), `dpaa2-tools`
  (engine faces, render, status, consent), `dpaa2-verify` (V-TRAF-1
  suite + hooks). `dpaa2-hal`: no change.
- **Models**: `models/families/link_lifecycle.qnt` (new),
  `models/board/V-TRAF-1/`, COVERAGE.md.
- **Docs**: new ADR; amendments to ADR-0003/0017/0019; roadmap rows #9
  and #10; baseline deltas (dprc.md, dpni.md) from whatever the board
  answers (post-bind connect dmesg law, destroy-mirror behavior).
- **Dependencies**: none added. No netlink crate, no VPP coupling —
  VPP use is post-archive product usage.
- **Board**: one operator sitting; pre-run record commit per standing
  rule; board returns to baseline (full teardown).
