# reconciler — dpsw-typestate delta

## ADDED Requirements

### Requirement: The dpsw lifecycle is typestated and its plan is ordered

`dpaa2-api` SHALL carry `families/dpsw.rs` structurally isomorphic to
the model (ADR-0002): the create block is constructible only through
the bindability predicate (DPSW-I1), so an unbindable kernel-steered
switch is unrepresentable in Rust as in Quint. The plan SHALL order the
working recipe from `docs/baseline/dpsw.md` "Lifecycle ordering":
companions (1 dpmcp + 1 dpbp, own container) → create → port connects →
plug; destroy is the only repair for cfg drift and is disruption-classed
accordingly (ADR-0015 matcher).

#### Scenario: A cfg drift plans destroy-and-recreate

- **WHEN** an observed dpsw matches an intent by anchor but differs in
  any create-time field
- **THEN** the plan is a disruption-classed destroy-and-recreate, never
  an in-place update

#### Scenario: Companion starvation is planned away

- **WHEN** a switched-domain converges on a container missing the dpbp
  or dpmcp
- **THEN** the compiled pool derivation and the grow-pass ordering
  guarantee the companions present co-resident in the dpsw's own
  container before the dpsw create — not this plan emitting the creates
  — so the probe-defer starvation face (`ls-addsw`'s inverted dpbp
  condition, `docs/baseline/dpsw.md` "Option inventory") cannot arise

### Requirement: dpsw convergence judges read-back, never exit codes

Convergence for the dpsw family SHALL be judged only on the observed
census (DPSW-I7): restool's dpsw verbs carry exit-code truth gaps
(destroy-in-child exits 0 on failure; update exits 0 on open failure),
so a verb's success SHALL be established by re-observation. The
observation surface SHALL type the honest-unknown faces: a missing
`connection:` line is `Unobservable`, never "free" (the `has_endpoint`
misread), and an unknown option bit rides the decode as an attributed
raw escape, display-only, with the census projection conservative (the
dpseci-hardening design D1/D2 precedent).

#### Scenario: A silent destroy failure does not converge

- **WHEN** a destroy verb exits 0 but the object survives read-back
- **THEN** the census still counts the object and convergence reports
  the typed residue, not success

#### Scenario: A blanked connection line is not freedom

- **WHEN** `info` yields a port with no `connection:` line
- **THEN** the observation types that port `Unobservable` and no plan
  treats it as unconnected

### Requirement: The dpsw edge kinds carry their reification policy rows

`dpaa2-api::plan` SHALL carry the two dpsw edge-kind rows mirroring the
model table (ADR-0022): switch-port membership (`dpni ↔ dpsw.N.M`) and
switch uplink (`dpsw.N.M ↔ dpmac`), each with its reification policy,
actuated at the connect surface with port indices bounded by the
created `num_ifs`.

#### Scenario: A connect beyond the port count is refused in planning

- **WHEN** an intent wires more ports than the domain's created
  `num_ifs`
- **THEN** planning refuses typed before any verb is dispatched

### Requirement: Interposition sequences the sever before the switch wiring

Converging a switched-domain over an already-faced port SHALL plan the
dpni–dpmac sever (the baseline sharing topology: dpni↔dpmac becomes
dpni↔dpsw↔dpmac) through the typed severed-witness path
first, then the two switch-edge connects — the dpmac-typestate edge law
holds across the rewire, so no plan ordering can strand the port
driverless.

#### Scenario: The flat-to-switched transition types its ordering

- **WHEN** an intent moves a wired port from a flat topology into a
  switched-domain
- **THEN** the plan carries sever (witness-yielding) → connect uplink →
  connect member, and a plan that unbinds before severing does not
  construct
