# dpsw-typestate — the switched domain becomes a typed, kernel-steered construct

## Why

The dpsw is the L2 switch the kernel offloads bridging to (switchdev:
STP, VLAN/FDB/MDB, LAG, tc-flower), yet the family today is a 33-line
parameter stub, and the one law that decides whether a created switch can
carry traffic at all — the kernel's bindability predicate — lives only in
`docs/baseline/dpsw.md`. The in-corpus vendor recipe (`ls-addsw`) creates
a kernel-unbindable switch by default (wrong flooding/broadcast config
plus an inverted dpbp condition), so a typestate-correct create is this
tool's pitch made concrete. Roadmap #11 is the next tile (dep #9
delivered), first of the locked #11 → #12 → #10 order (ADR-0004 §8): this
change freezes the dpsw slice of the `McControl` surface before the
portal implements it, and its board sittings bank the portal's encode
fixtures for free.

## What Changes

- `models/families/dpsw.qnt` grows the P1 phase-machine reference shape
  with the reset facet (ADR-0019): created-configured lifecycle whose
  create block carries the **kernel-bindability predicate as type law**
  (DPSW-I1: ctrl-if enabled ∧ flooding PER_FDB ∧ broadcast PER_FDB ∧
  `max_fdbs ≥ num_ifs`) — all twelve `dpsw_cfg` fields are create-time
  immutable, so a wrong switch is refused at compile, never repaired.
  The vendor recipe is explicitly not an oracle (DPSW-I2).
- The ADR-0022 edge-kind table fills its two waiting dpsw rows
  (`dpni ↔ dpsw.N.M`, `dpsw.N.M ↔ dpmac`) with their reification policy;
  switch-port membership joins the connection surface. The
  **interposition transition** (flat dpni↔dpmac rewired into
  dpni↔dpsw↔dpmac — the baseline sharing topology) is planned through
  the dpmac-typestate severed-witness law, so the rewire cannot strand
  a port driverless.
- The intent vocabulary gains the **switched-domain construct**
  (ADR-0013 amendment): N ports (dpmac uplinks + member dpnis) in one
  kernel-steered L2 domain, with the corpus-backed derivation rules —
  `num_ifs` = port count, `max_fdbs ≥ num_ifs` (kernel runs
  FDB-per-port), `max_fdb_entries` scales with ports × expected MACs,
  flooding/broadcast forced PER_FDB, companions 1 dpmcp + 1 dpbp in the
  dpsw's own container (DPSW-I3, verified). Regime law: the steering
  owner is always the kernel — DPDK silently discards a dpsw at scan
  (DPSW-I6); a VPP/DPDK dataplane participates only as a dpni endpoint.
- `crates/dpaa2-api` gains `families/dpsw.rs`, structurally isomorphic
  to the model (ADR-0002), the switched-domain plan surface, and the
  census judgment on the observed cfg signature (design D9 class,
  dpseci-typestate precedent).
- `crates/dpaa2-mc` drives the three restool verbs the plan needs
  (create / destroy / info) and parses `info` with honest-unknown
  handling: the missing-`connection:`-line face is typed, never read as
  "free" (the `has_endpoint` misread in `docs/baseline/dpsw.md`
  "Command surface"); exit-code truth gaps converge by read-back only
  (DPSW-I7).
- `dpaa2ctl status --detail` gains the read-only switch row:
  per-interface endpoint, link, taildrop, counters — display-only,
  never gating convergence.
- Board program settles the open unknowns from `docs/baseline/dpsw.md`:
  what `dpsw_reset` preserves (DPSW-I4), the bpid-vs-object-id law with
  a live consumer (DPSW-I5, extends DPBP-I5), the FDB budget question
  (driver convention vs MC-enforced). V-DPSW-1 and V-DPSW-4 are already
  banked; this change adds one discovery sitting and one witness
  sitting.
- **Board scripts carry the passive ioctl-payload capture tap**
  (ADR-0004 §8): every sitting records the restool-emitted MC command
  bytes, banking `mc-portal-backend` (#10) encode fixtures at zero
  operator cost. The tap is observation-only and lives inside the
  safety envelope (ADR-0003).
- **Out of scope, re-anchored loud**: the runtime switchdev surface
  (VLAN/FDB/ACL/LAG/mirroring tables — consumer-owned, ADR-0004 §8;
  DPSW-I8 class), S_VLAN component type and LAG semantics (no in-corpus
  caller), writable counters (`if_set_counter`, baseline unknown), the
  MC-side semantics of PER_VLAN flooding (no consumer drives it), the
  taildrop **update** verb (no intent knob and no in-corpus caller —
  driving it would be unwitnessable dead code, the dpmac-typestate
  design D6 law; taildrop stays read-only in observation, and the verb's
  uninitialized-cfg hazard in `docs/baseline/dpsw.md` "Silent-failure
  notes" is one more reason not to touch it), and
  any dpdmux work — the Tier-B shared frame lands in this change's
  design as what #12 inherits, nothing more.

## Capabilities

### New Capabilities

None — the dpsw surface lands inside the existing capability surfaces.

### Modified Capabilities

- `formal-models`: the dpsw family module grows the P1 reference shape
  with the reset facet; the bindability predicate and census are stated
  as invariants; the two dpsw edge kinds join `core/connect.qnt`;
  traces frozen.
- `intent-compiler`: the switched-domain construct joins the accepted
  vocabulary (ADR-0013 amendment) with its derivation and refusal rules
  (kernel-unbindable shapes unrepresentable; non-kernel steering owner
  refused).
- `topology-config`: the northbound schema parses the switched-domain
  construct into the backend-neutral model.
- `reconciler`: dpsw typestates, the switched-domain plan surface, the
  two edge kinds with their reification policy, companion placement,
  and the census judgment join the plan; disruption classing covers
  destroy-and-recreate as the only repair.
- `mc-backend`: the restool shim drives and parses the three dpsw verbs
  with typed honest-unknown observation faces.
- `provisioning-cli`: the read-only switch detail row.
- `mbt-harness`: MBT conformance twins for the dpsw typestates and edge
  laws; discovery + witness suite generation inside the safety
  envelope; the capture tap in the suite preamble.
- `system-integration`: the end-to-end witness — one switched-domain
  intent converges to a kernel-bound, traffic-capable switch with
  companions and port edges, read back typed.

## Impact

- Crates: `dpaa2-api` (family module, plan surface, census),
  `dpaa2-config` (construct parse), `dpaa2-mc` (verb drive + parse),
  `dpaa2-tools` (detail row), `dpaa2-verify` (twins, suites, capture
  tap).
- Models: `families/dpsw.qnt`, `core/connect.qnt` edge rows, frozen
  traces, COVERAGE rows (DPSW-I1…I8 dispositions).
- Docs: `docs/baseline/dpsw.md` amendments from the sittings (unknowns
  #2/#5/#6 answers), ADR-0013 vocabulary amendment, ADR-0022 table
  rows, ADR-0019 P1-instance note, ROADMAP row #11.
- No new dependencies; no persisted state; restool remains the executor
  and oracle (ADR-0004 §1); no kernel change (ADR-0004 §8).
