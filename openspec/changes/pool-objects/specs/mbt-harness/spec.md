# mbt-harness delta — pool-objects

## ADDED Requirements

### Requirement: Suite generation renders the pool walks from the model
The `dpaa2-verify` batch-suite generator SHALL render pool suites from
the model's actions: census/ceiling walks (create to the floor, the
refusal at zero free, every destroy returning its unit — the DPBP-I7
shape), free/drain observation walks (DPBP-I3/DPCON-I5's Linux-side
free path, observed through the kernel face rather than driven),
convergence walks (deficit grow, surplus shrink through free
individuals, prune of undeclared objects, idempotence re-run), and the
dpio seat/priority probes (DPIO-I3's kernel half). Suites SHALL be
scratch-first and self-cleaning inside the ADR-0003 envelope, asserting
the reference pair before running, and frozen ITF traces SHALL replay
in `cargo test` as the families' conformance twins.

#### Scenario: The ceiling walk predicts the refusal to the object
- **WHEN** the suite creates a pool family toward its census-predicted
  floor
- **THEN** the first refused create is exactly the one the census
  predicted, and cleanup returns every created unit

#### Scenario: A refusal is an answer
- **WHEN** the MC or kernel refuses a probe step
- **THEN** the suite records the refusal as the observed verdict and
  continues cleanup; the run is not failed by the refusal itself

### Requirement: Kernel-face coverage runs at root scope with a gated DPL-child escape
Suites needing a kernel consumer SHALL use the proven root-container
dpaa2-eth bind. The DPL-defined-child mechanism (a boot-configuration
edit touching the ADR-0003 recovery baseline) SHALL NOT run unless a
COVERAGE row routed to this change is demonstrated unreachable at root
scope; taking it is a separately gated task naming the invariant (bead
dpaa2-controlplane-5y7), operator-approved before the first boot-config
write.

#### Scenario: Root scope exhausts before the escape fires
- **WHEN** a routed invariant's suite can express its faces through
  root-bind walks
- **THEN** the suite lands at root scope and the DPL-child task stays
  untaken

#### Scenario: The escape is loud, not silent
- **WHEN** a routed invariant proves unreachable at root scope
- **THEN** the change records the blocked invariant by name and the
  DPL-child task is proposed for operator approval rather than
  narrowing coverage silently
