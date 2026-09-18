# dpni-typestate

## Why

Roadmap tile #5. The dpni is the object behind every interface construct of the
intent layer (ADR-0005), but the reconciler still carries only the narrow dpni
support inherited from the pre-series provisioning flow: the full create-time
option surface — the twelve live options with their ranges, the two
board-verified consumer profiles, and the silent-failure traps restool never
guards — has no typed representation, so an invalid dpni configuration is
representable today and only the board can refuse it. Tiles #9
(cross-dprc-links) and #10 (mc-portal-backend) both gate on this change, and
the api-modularization epic (yfg) closed 2026-09-17 specifically so this
change proposes against the namespaced `families/` layout.

## What Changes

- The reconciler's object graph gains the full dpni create surface as
  typestates in `dpaa2-api::families::dpni`: the twelve live create options
  typed with refined ranges (baseline `dpni.md` option inventory), an options
  flag set typed over the 14-flag MC 10.39 vocabulary plus the raw-mask escape
  (`0x80000000` PFDR_IN_PEB is deployed and working), and the `dpni_cfg` block
  as the immutable type parameter of a DPNI — runtime surface is state within
  that type (baseline attribute-mutability law), pre-shaping tile #10 without
  rewrite.
- The eleven dead options and the never-settable `num_rx_tcs` become
  unrepresentable, each with a programmatic-parity refusal (the vocabulary-v2
  precedent that closed the design-D11 one-sided rows) — passing one through
  restool today is worse than an error (silent-failure notes).
- The runtime surface lands only as far as restool can drive it: the primary
  MAC mutation. The other 35 `dpni_set_*` setters are a named deferral row to
  tile #10 `mc-portal-backend`, where their transport arrives.
- dpni options are purely derived: the intent compiler chooses the option set
  from `Dataplane` + interface construct (the board-verified PMD and kernel
  profiles); intent TOML never names a `DPNI_OPT_*` and no per-interface
  override escape exists. A third profile is a documented amendment when a
  consumer demands it.
- Riders folded in: `observe_container(id)` lands beside the enumerate verb
  (bead z5z — per-candidate re-observation stops rescanning every root child;
  OI-3 dpmcp-budget measurement outcome cited in its disposition), and the two
  recorded typestate hazards in `intent::tenant` close (the zero-value
  `Default` path that skips the `TenantRef::from_name` discipline, and the
  constructible empty `TenantName`).
- `models/families/dpni.qnt` grows the option surface with named invariants;
  MBT twins in `dpaa2-verify` replay the frozen traces; the board milestone is
  a batch suite extending the V-DPNI series plus an online-MBT per-step
  learning session. Probes cover the restool-reachable unknowns only
  (baseline unknown-register #3 sizing-field walks, #8 `HAS_REPLICATION`
  accept/reject, #6 unread option flags); the unreachable ones (#1
  TX_CONFIRMATION_MODE v1 handler, #12 `num_rx_tcs`-via-DPL) are recorded as
  deferral rows to #10/#14.

## Capabilities

### New Capabilities

None — the change lands entirely as deltas to existing capabilities.

### Modified Capabilities

- `reconciler`: the object graph gains the dpni create-surface typestates
  (immutable `dpni_cfg` type parameter, runtime-state-within-type shape), the
  dead-option/`num_rx_tcs` parity refusals, drift handling for the write-only
  `dist_key_size` (never read back — refuse-and-report is impossible, so it is
  treated as write-only by construct), and the `observe_container(id)`
  re-observation seam in planning.
- `formal-models`: `models/families/dpni.qnt` grows the create-option surface,
  the option-profile derivation, and named DPNI invariants, Apalache-marked per
  the DoD model gate.
- `intent-compiler`: option sets derive purely from `Dataplane` + interface
  construct (PMD/kernel profiles); the two `intent::tenant` typestate hazards
  close (no zero-value `Default` intent path, no constructible empty
  `TenantName` outside its sentinel role).
- `mc-backend`: the restool shim grows the dpni create verb at full option
  granularity, the primary-MAC mutation, and `observe_container(id)`;
  attribute read-back maps the `dpni_attr` asymmetries (split rx/tx TCs,
  omitted `dist_key_size`).
- `mbt-harness`: suite generation covers the dpni option walks; the online
  driver gains dpni per-step learning-mode sessions under the existing
  operator-supervised envelope.
- `system-integration`: board milestone suites extending the V-DPNI series —
  option-profile creation walks, sizing-field probes, `HAS_REPLICATION`
  accept/reject, primary-MAC mutation — scratch-first and self-cleaning.

## Impact

- Crates: `dpaa2-api` (`families::dpni` typestates, intent derivation, parity
  refusals, hazard closure, plan seam), `dpaa2-mc` (restool dpni verbs,
  `observe_container`), `dpaa2-verify` (suite generation, online scenarios,
  frozen-trace twins), `dpaa2-config` (no schema change — options never
  surface in TOML).
- Models: `models/families/dpni.qnt`, shared params; new frozen traces under
  `models/traces`.
- Docs: `docs/baseline/dpni.md` amendments for every probe outcome; ADR for
  any decision that solidifies or dies on the board; roadmap row #5; deferral
  rows to #10 (runtime setters, TX_CONF v2) and #14 (`num_rx_tcs` via DPL).
- Board: one operator sitting — batch suite + online session; reference pair
  MC 10.39.0 + Linux 6.6.52 asserted by the scripts.
- Beads: epic + task beads created at propose time; z5z instantiated against
  this change per its acceptance.
