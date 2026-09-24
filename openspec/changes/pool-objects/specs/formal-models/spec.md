# formal-models delta — pool-objects

## ADDED Requirements

### Requirement: P3 mechanisms are pattern-owned, machines family-owned per the re-amended ADR-0019 Quint module architecture
The model SHALL follow ADR-0019's re-amended Quint module architecture
(2026-09-20, pool-objects phase 1): `models/families/pool_lifecycle.qnt`
(`module pool_lifecycle`) SHALL carry ONLY the mechanisms genuinely
common to P3 — the parameterized custody substrate: the
allocator-custody cycle (create → free-pool membership → draw →
return, with DPBP-I3's dirty return on free) and ceiling-bound refusal
at the census (ADR-0011, DPBP-I7's board-settled floor) — and SHALL
contextualize each member family rather than host its particulars.
Each member file `models/families/{dpmcp,dpbp,dpcon,dpio}.qnt` SHALL
own its stateful modules in the `dprc.qnt`/`dpni.qnt` shape: `module
<f>_lifecycle` (and `module <f>_scenario` when scenario content
exists) — thin for the allocator trio, each instantiating the
pattern substrate contextualized to its family; dpio's standalone
(unpooled, it reuses the shared core transforms rather than the
pooled substrate) and carrying its family-particular depth (seat
record and arithmetic, the probe-time dpmcp draw of
DPIO-I1/DPMCP-I1, the DPIO-I3 surface) — alongside the
family-specific types (dpio's regime-typed seat vocabulary, the
mutable dpcon→dpio notification edge type of DPCON-I4) and the
named-invariant index. Family-particular state records, actions, or
directed runs in the pattern file are a defect in review, as is a
member module that re-derives the shared mechanisms instead of
instantiating them. `models/core/` SHALL gain only corpus-wide census
extensions; `main.qnt` keeps the baseline-id runs. Invariants SHALL be
Apalache-marked per the DoD model gate and the structural-isomorphism
law (ADR-0002 §3) SHALL hold between the Quint sums and the Rust P3
shapes that phase 2 builds against them.

#### Scenario: Model gate runs green before Rust lands
- **WHEN** the CI ladder runs typecheck, simulate, and Apalache on the
  marked pool invariants
- **THEN** all pass before the corresponding Rust P3 surface merges

#### Scenario: Exhaustion refuses at the census, not the board
- **WHEN** the simulator drives creates past a container's family
  ceiling
- **THEN** the create action is disabled by the census predicate and no
  accepted state exceeds the ceiling

### Requirement: Count convergence and prune are modeled as laws
The model SHALL encode the convergence discipline for anonymous
capacity as named invariants: converged means the per-(container,
family) census equals the derived requirement; a shrink step destroys
only free individuals; an intent below the current draw disables the
shrink action and surfaces a refusal; prune removes exactly the
objects that are undeclared, non-DPL-born, and free. Victim selection
among free individuals SHALL be nondeterministic in the model — no law
distinguishes free individuals of one family.

#### Scenario: Convergence is idempotent and level-triggered
- **WHEN** the simulator replays the same derived counts against a
  converged state
- **THEN** no create, destroy, or prune action is enabled

#### Scenario: Shrink below draw is a refusal, not a teardown
- **WHEN** the derived requirement drops below the currently drawn
  count for a family
- **THEN** no destroy of a drawn individual is reachable and the
  refusal state is the only successor

#### Scenario: DPL-born objects survive prune
- **WHEN** prune runs over a state containing boot-baseline objects
  absent from intent
- **THEN** every DPL-born object remains and every undeclared free
  runtime object is removed

### Requirement: Custody carries the plug facet distinct from draw
The model SHALL split the two board custody observables the census
collapsed (pool-objects design D10; 2026-09-24 audit): *plugged* —
allocatable, in the kernel's pool (DPBP-I2: allocatable ⟺ plugged ∧
allocator-bound) — and *drawn* — held by a consumer. `pool_lifecycle`
SHALL model plug/unplug as transitions distinct from draw/return;
reclaim of a managed individual SHALL be the unplug-probe law (an
unplug of a drawn individual is refused and the refusal is the drawn
signal; a plugged-free individual unplugs, then destroys); the
grow-only dpio residue SHALL be a typed reboot-required disposition,
never a silent carve-out. The ITF twins' observation mapping SHALL
carry the plugged facet explicitly so a Rust census that infers drawn
from plugged fails a frozen twin offline, before any board sitting.

#### Scenario: Managed surplus reclaims through the probe
- **WHEN** the derived requirement drops below the managed count and
  the surplus individuals are plugged but not drawn
- **THEN** each unplugs and is destroyed, and the census converges to
  the requirement

#### Scenario: The probe refuses on a drawn individual
- **WHEN** a shrink selects a managed individual a consumer holds
- **THEN** the unplug is refused, no destroy of that individual is
  reachable, and the refusal surfaces as the ShrinkBelowDraw face

#### Scenario: Teardown orders consumers before pools
- **WHEN** an empty intent replays over a converged two-tenant state
- **THEN** consumer and container teardown steps precede every pool
  shrink step, the trio census returns to the DPL baseline, and the
  dpio residue is the reported disposition, not a failure
