# dpmac-typestate — the physical port becomes a typed, observed surface

## Why

The dpmac is the identity anchor of the whole control plane (ADR-0001 §3):
every port is keyed by its dpmac, yet the family today is a 24-line
parameter stub — no typed arbitration state, no counter vocabulary, no MAC
inheritance judgment — and the one teardown ordering that can strand a
port driverless (sever-then-unbind, ADR-0008 §8) lives only as procedural
plan knowledge. Roadmap #7 is the next tile (deps #2 and #4 delivered),
and it closes the deferred-anchor set COVERAGE has been pointing at it:
DPMAC-I2/I3/I4/I6/I7, DPNI-I3's MAC value semantics, and the
phantom-create face V-DPMAC-2.

## What Changes

- `models/families/dpmac.qnt` grows the P4 boot-born-offer reference
  shape (ADR-0019): the driver-arbitration phase sum
  Offered / KernelOwned / RemoteOwned (DPMAC-I6) with typed transitions,
  both directional MC link channels as distinct types (DPMAC-I4) with the
  requests-down channel typed Unreadable on the current transport, a
  firmware-version-indexed counter vocabulary (DPMAC-I7: 10.39's 28 read,
  the 10.40 extension named but unread), and MAC immutability (DPMAC-I2).
- The dpni–dpmac **edge kind** gains its teardown-order law at the
  connection surface (the ADR-0019 edge facet, beside ADR-0009's
  `legalPorts`): sever consumes KernelOwned and yields Offered plus a
  severed witness; the dpni kernel-face unbind demands that witness only
  for dpmac-facing edges. Other edge kinds carry no new law.
- `crates/dpaa2-api` gains `families/dpmac.rs`, structurally isomorphic
  to the model (ADR-0002), plus the typed MAC relation judgment:
  Inherited / Overridden / Pending / Mismatched — Pending (zeros,
  consumer not yet bound) is explicitly not drift.
- `crates/dpaa2-mc` reads the dpmac surface through the shim (attributes,
  MAC, the 28-counter vocabulary with `Known | NotInVocabulary` so
  absence ≠ zero is unrepresentable); `crates/dpaa2-hal` gains one
  policy-free sysfs carrier primitive (dpni netdev for KernelOwned, macN
  otherwise; NoObservable diagnoses a driverless port).
- `dpaa2ctl` gains a read-only port-detail view: arbitration state, MAC
  relation, link carrier, counters. Link and counters are display-only
  and never gate convergence.
- Board program: Suite A (end-to-end dpaa2ctl typestate suite on
  dpmac.7, V-MVP-1 shape, with the typed sever-then-unbind teardown and
  the RemoteOwned child/VFIO leg) and Suite B (V-DPMAC-2 phantom create
  contained in a scratch child), one sitting.
- **Out of scope, re-anchored loud**: `dpmac_get_link_cfg` +
  `SET_LINK_STATE` and the MC-view dpni link read (→ `mc-portal-backend`
  #10, recorded as restool-absence ledger rows), bulk `get_statistics`
  (unexercisable on the current transport → #10), `set_protocol` /
  `set_params` mutation and MDIO (no intent need), DPRTC-I4
  (lifecycle-foreign here, re-anchored off this change).

## Capabilities

### New Capabilities

None — the dpmac surface lands inside the existing capability surfaces.

### Modified Capabilities

- `formal-models`: the dpmac family module grows the P4 reference shape
  (phase sum, directional link channels, version-indexed counter
  vocabulary, MAC immutability) and the connection surface gains the
  per-edge-kind teardown law; invariants named, traces frozen.
- `reconciler`: dpmac arbitration typestates and the severed-witness
  edge type join the plan surface; the MAC relation judgment
  (Inherited/Overridden/Pending/Mismatched) joins observation, with
  Pending excluded from drift; the write path is unchanged.
- `mc-backend`: shim read surface for dpmac attributes, MAC, and the
  firmware-versioned counter vocabulary; the dpaa2-hal sysfs carrier
  primitive with its reference-pair assertion
  (CONFIG_FSL_DPAA2_MAC_NETDEVS).
- `provisioning-cli`: the read-only port-detail view.
- `mbt-harness`: MBT conformance twins for the dpmac typestates and the
  edge law; generation of Suite A and Suite B inside the safety
  envelope.
- `system-integration`: the end-to-end witness — one intent converges
  the kernel regime on a wired port, the hooks read the typed surface,
  the teardown runs the typed sever-then-unbind path, and the
  RemoteOwned arrangement is read back across the container boundary.

## Impact

- Crates: `dpaa2-api` (new family module, edge type, MAC judgment),
  `dpaa2-mc` (parse/read extensions), `dpaa2-hal` (one sysfs primitive),
  `dpaa2-tools` (port-detail view), `dpaa2-verify` (twins + suites).
- Models: `families/dpmac.qnt`, the connection surface module, frozen
  traces, COVERAGE rows.
- Docs: `docs/baseline/dpmac.md` amendments from the sitting (V-DPMAC-2
  answer, carrier observability), ADR-0019 amendment (P4 reference
  landed; phase-marker promotion trigger fired; edge teardown-law
  facet), COVERAGE #10 anchor rows for the deferred link reads,
  ROADMAP row #7.
- No new dependencies; no persisted state; restool remains the only MC
  transport (ADR-0004 unchanged).
