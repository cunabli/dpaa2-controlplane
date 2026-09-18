# dpni-typestate design

## Context

The baseline (`docs/baseline/dpni.md`) settles the ground this change builds
on: twelve live create options with ranges and MC defaults, eleven dead
options whose acceptance is worse than an error, one never-settable field
(`num_rx_tcs`), an absolute create/runtime mutability split — every
`dpni_cfg` field is create-time-immutable, 36 runtime setters of which
restool exposes exactly one (primary MAC) — and two board-verified consumer
option profiles (PMD and kernel). The intent vocabulary already types the
consumer (`Dataplane`), the matcher already keys identity, and the
api-modularization epic left `dpaa2-api` namespaced (`families/`, ADR-0018)
specifically so this change lands per-family without touching the flat core.
Existing board evidence: V-DPNI-1/2/3 and V-LIFE-DPNI-1 verdicts, and the
V-READBACK-1 default observations.

Scoping decisions were settled at the 2026-09-19 grilling; this design
records them and their rationale.

## Goals / Non-Goals

**Goals:**

- An invalid dpni create configuration is unrepresentable in
  `dpaa2_api::families::dpni` — ranges, option-flag vocabulary, and
  profile-consistency by construct.
- The typestate is shaped for tile #10: `dpni_cfg` as immutable type
  parameter, runtime surface as state within the type, so the portal
  backend adds setters without reshaping the family.
- The intent compiler derives options; the operator never writes one.
- Every restool-reachable unknown-register item gets a board answer or a
  deferral row; the baseline is amended in the same change.

**Non-Goals:**

- No runtime setter beyond primary MAC (no transport until #10; a suite
  restool cannot drive cannot pass the DoD board gate).
- No per-interface option override in intent (unverified combinations would
  need their own refusal story; a third profile is a documented amendment).
- No dpni table-content management (MAC/VLAN/QoS/FS entries) — table sizes
  are create-time here; contents ride the kernel or PMD, not the control
  plane, until a tile needs them.
- No queue/DPCON wiring — companion-object math is tile #6's.

## Decisions

### D1 — dpni_cfg is the immutable type parameter; runtime is state within

The baseline's mutability law is absolute (no `dpni_set_options`; resize =
destroy + create), so the family encodes it structurally: the validated
create block parameterizes the DPNI type, and runtime state (today: primary
MAC; #10: the setter surface) lives inside it. Drift on any cfg field is
refuse-and-report, never repair (ADR-0001 §4). Alternative rejected: typing
the 36 setters now — 35 are unemittable through the restool shim, so the
surface would be untestable speculation #10 would reshape.

### D2 — Live options typed, dead options refused by parity

The twelve live options carry refined range types taken from the restool
ranges (the tightest verified envelope; MC-side ceilings beyond them are
unreachable and stay unknown-register). The options mask is a typed set over
the 14-flag MC 10.39 vocabulary plus a raw-mask escape — `0x80000000`
(PFDR_IN_PEB) is deployed and working but unnamed in any header, so the
escape is a first-class, provenance-carrying constructor, not a backdoor.
The eleven dead options and `num_rx_tcs` follow the vocabulary-v2 parity
precedent: no constructor, and a programmatic refusal names each one so the
design-D11 rows stay two-sided.

### D3 — Options derive purely from Dataplane + construct

The compiler maps (`Dataplane`, interface construct) to the two
board-verified profiles: PMD (`SINGLE_SENDER, CUSTOM_CG, HAS_KEY_MASKING,
HAS_OPR, OPR_PER_TC, 0x80000000`, 16q/16tc shape) and kernel
(`HAS_KEY_MASKING` only, 1q/1tc). Intent TOML never names a `DPNI_OPT_*`.
Rationale: option profiles are consumer-typed, not global (baseline intent
mapping, [verified]); an override escape would admit combinations no board
run validated. Sizing stays where it is: `num_queues` from the ADR-0012 tx
floor, `num_cgs = num_queues + 8` under `CUSTOM_CG` (deployed heuristic;
its rationale is unknown-register #3 — the sizing walks probe it).

### D4 — dist_key_size is write-only by construct

`dpni_attr` omits `dist_key_size`, so the reconciler can never detect drift
on it. Rather than a special-cased runtime check, the type carries it as
write-only: it participates in create, never in observation comparison. The
asymmetric read-back (split rx/tx TCs, added key sizes, `wriop_version`)
maps in the shim's observation type.

### D5 — observe_container(id) lands with the planning seam (z5z)

Per-candidate re-observation replaces the full root rescan (1+2N spawns ×4
per ensure) exactly when this change shapes the family observation surface,
as the bead prescribes. The OI-3 dpmcp-budget measurement outcome (bead
am0.2) is cited in the seam's disposition: if spawns draw the never-returned
budget, the seam is a leak fix, not latency.

### D6 — The tenant.rs typestate hazards close here

This is the first typestate change of the series — the hazards were recorded
against it. (a) The zero-value `Default` path on `Isolation`/`Intent` that
skips the `TenantRef::from_name` discipline is removed. (b) An empty
`TenantName` becomes unconstructible outside its sentinel role. Both are
pure-core changes with frozen-trace parity checks; no intent that was valid
before changes meaning.

### D7 — Board milestone: batch suite plus online-MBT learning session

The online-MBT method debuted clean at dprc; dpni's option semantics carry
real unknowns, so the family enters per-step learning mode for one session.
Probes are limited to what restool can drive: unknown-register #3
(`num_cgs`/`num_opr`/`dist_key_size` create walks + read-back), #8
(`HAS_REPLICATION` 0x4000 accept/reject), #6 (unread option flags — create
and observe). Unreachable unknowns become deferral rows: #1 (TX_CONF v1
handler — needs the portal, #10), #12 (`num_rx_tcs` via DPL — #14), #4's
QoS/FS half and #7/#11 (need table writes or traffic — #9/#10). Suites are
scratch-first and self-cleaning; the reference pair is asserted.

### D8 — Quint model first, MBT twins frozen

`models/families/dpni.qnt` grows the option surface and profile derivation
with named invariants (create-range refusal, profile totality, parity of
unrepresentable options, write-only field law), Apalache-marked per the DoD.
`dpaa2-verify` replays the frozen traces off-board; the batch suite renders
from the same model actions, keeping the structural-isomorphism law
(ADR-0002) intact.

## Risks / Trade-offs

- [Restool option parsing is loose (case-sensitive tokens, silent numeric
  fallback)] → the shim always emits the raw mask it computed itself, never
  operator-supplied tokens; the typed set is the single source.
- [Sizing-walk probes may hit undocumented MC refusals mid-suite] → walks
  are bracketing and scratch-only, each step self-cleaning; a refusal is an
  answer, not a failure (ambivalence goes to the baseline/ADR).
- [Pure-derivation may prove too rigid if a third consumer profile appears
  before #10] → the profile map is one function in the compiler; a new
  profile is a documented amendment with its own board validation, not a
  redesign.
- [Hazard closure (D6) touches shared intent vocabulary mid-series] →
  frozen-trace replay and the public-surface diff gate the parcels; the yfg
  surface archive (296 items) is the comparison base.

## Migration Plan

Land order: model + invariants → pure core typestates and refusals →
compiler derivation + hazard closure → shim verbs + observe_container →
verify twins and suite generation → board sitting → baseline/ADR/roadmap
close-out. Each step keeps the quality floor green; no persisted state, so
rollback is git revert of unsealed commits.

## Open Questions

- The `num_cgs = num_queues + 8` rationale (unknown-register #3): the walks
  may narrow it; if the board stays silent it remains recorded as deployed
  heuristic with a revisit trigger at #10.
- Whether `HAS_REPLICATION` (0x4000) exists on MC 10.39 (#8): probe answers
  accept/reject; semantics, if accepted, stay unknown-register.
