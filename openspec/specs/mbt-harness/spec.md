# mbt-harness Specification

## Purpose
Turn the formal models into board evidence: the `dpaa2-verify` crate
binds each model action to its restool command and each observable to
read-back state through one shared adapter, generates reviewable batch
suites with expectations inline, drives operator-launched online sessions,
replays frozen ITF traces in `cargo test`, and enforces the port safety
envelope at generation and execution — so every typestate contract is
board-confirmed, on a recovery guarantee verified first, before any
mutating suite runs.
## Requirements
### Requirement: One shared adapter binds model, restool, and observed state
The `dpaa2-verify` crate SHALL provide a single adapter mapping each
model action to its restool command and each model observable to board
state obtained by read-back. Exit status SHALL NOT be used as an
observation of resulting state.

#### Scenario: Observation is read-back, not exit status
- **WHEN** a suite step's command returns exit 0 but the read-back
  differs from the model's expected state (or returns nonzero with the
  expected state present)
- **THEN** the diff reports the read-back result as the observation and
  the exit status only as auxiliary evidence

### Requirement: Batch suites are reviewable scripts with expectations inline
The batch-suite generator SHALL consume Quint simulator traces and emit
scripts in which every restool command is visible, the model's expected
outcome sits as a comment beside each step, the pinned reference pair
(MC 10.39.0 + Linux 6.6.52) is asserted before any action, and results
are captured to files for offline diffing.

#### Scenario: A generated suite is auditable before it runs
- **WHEN** the operator reviews an emitted suite
- **THEN** every board-touching command and its expected post-state are
  readable in the file, and the script refuses to proceed on a
  reference-pair mismatch

#### Scenario: Suites execute serially and return files
- **WHEN** the operator runs the board program
- **THEN** suites run one at a time in their declared order and each
  produces a result file the harness diffs offline against the model's
  expected states

### Requirement: The port safety envelope is enforced at generation and execution
The harness SHALL encode the ADR-0003 port matrix and traffic classes
as data, refuse to emit or execute any step whose declared traffic
class exceeds what its named ports allow, and make dpmac.3,
dpmac.17, and dpni.0 unreferenceable in any scenario class.

#### Scenario: A forbidden port never reaches a script
- **WHEN** a trace or scenario names dpmac.3, dpmac.17, or dpni.0
- **THEN** generation fails with the violation named, and the execution
  wrapper independently refuses such a step if one appears in a script

#### Scenario: Class must match ports
- **WHEN** a trace declared object-lifecycle-only names dpmac.7 or
  dpmac.9, or an unflagged run declares link-signaling or
  traffic-bearing class
- **THEN** the harness refuses it

### Requirement: Mutating suites are gated on the verified recovery guarantee
The harness SHALL treat the recovery guarantee — a reboot restores the
DPL baseline — as unverified until a recovery-verification run has
passed, and SHALL refuse to emit mutating suites while it is
unverified. The recovery verification itself SHALL mutate only a
scratch-DPRC set that the reboot is expected to erase.

#### Scenario: Recovery check runs first
- **WHEN** the board program begins
- **THEN** the first suite captures pre-state, applies the scratch
  mutation set, has the operator reboot, and diffs post-boot state
  against the DPL baseline; only a clean diff marks the guarantee
  verified

#### Scenario: Root-container teardown stays gated
- **WHEN** a scenario destroys a root-container resident (dprtc.0)
- **THEN** it is emitted only after the recovery guarantee is verified
  and runs under per-step operator confirmation

### Requirement: The online driver is operator-launched and supervised
The crate SHALL provide an online-MBT driver the operator starts, with
step, pause, and abort controls, a full transcript of every action and
observation, and per-step confirmation while a family is in learning
mode; promotion to per-block execution follows a family's model
surviving a complete batch suite and is recorded in the owning change.

#### Scenario: Per-step confirmation in learning mode
- **WHEN** the driver runs a family that has not survived a full batch
  suite, or any root-container-resident scenario regardless of
  promotion
- **THEN** each action requires explicit operator confirmation before
  it executes, and the transcript records the action, the observation,
  and the model's expectation

### Requirement: The frozen-trace rung reaches every board-verified dpni refusal face
The ITF parser SHALL decode the full `RawEscape` vocabulary
(`HasReplication` included), and the frozen corpus SHALL carry: a
McClearedFlags create/read-back scenario (projection strips both cleared
escapes), a queue-envelope RefusedTrace twin for the guu.4a intent arm
(`QueueEnvelopeExceeded`), and the `UnpricedDataplane` refusal run. The
parser arm SHALL land before any trace that carries its tag is frozen.
(Review synthesis row 6.)

#### Scenario: A regenerated cleared-escape trace decodes
- **WHEN** `model:freeze-dpni` emits a trace whose option mask carries
  `HasReplication`
- **THEN** the replay decodes it and asserts the projection strips the
  MC-cleared bits, rather than failing at decode

#### Scenario: The intent-side envelope fence is replayed
- **WHEN** the frozen intent corpus is replayed
- **THEN** at least one trace drives the `QueueEnvelopeExceeded` decoder
  arm end-to-end

### Requirement: Online discovery sessions target containment semantics
The online driver SHALL run this change's discovery sessions — the roadmap's
first online-MBT discovery target — against containment/pool semantics on
scratch containers: the DPRC-I1 pool boundary, teardown liveness (DPRC-I9)
under reconciler-generated plans, and the restool-reachable remainder of the
lock face (DPRC-I11). Sessions run under the existing operator-supervised
safety envelope; a model/board divergence amends the model and
`docs/baseline/dprc.md` in the same change.

#### Scenario: Pool boundary session
- **WHEN** a session allocates from a container whose local pool is exhausted while a sibling has surplus
- **THEN** the observed refusal is local (-ENXIO shape) and the trace confirms allocation never crossed the container boundary (DPRC-I1)

#### Scenario: Divergence feeds back
- **WHEN** an observed outcome contradicts the model's predicted transition
- **THEN** the session halts that face, the model and baseline are amended, and the corrected prediction is re-verified before the invariant's disposition advances

#### Scenario: Deferred faces are recorded, not probed
- **WHEN** a session plan would require the child's own portal (I11 unlock face, OBJ_CREATE gate) or the unreachable DPRC-I8 batch-scan ordering
- **THEN** the portal faces are emitted as deferral rows pointing at tile #10 and DPRC-I8 rides bead dpaa2-controlplane-5y7 / tile #10 (`mc-portal-backend`), the `pool-objects` (#6) DPL-child window having closed NOT FIRED (bead dpaa2-controlplane-960.13) — and no probe is attempted


### Requirement: Suite generation renders the dpni option walks from the model
The `dpaa2-verify` batch-suite generator SHALL render dpni suites from
the model's create actions: option-profile creation walks (PMD and
kernel profiles created and read back), sizing-field walks for the
unknown-register probes (`num_cgs`, `num_opr`, `dist_key_size`), the
`HAS_REPLICATION` (0x4000) accept/reject probe, and the unread-flag
probes (`TX_FRM_RELEASE`, `HAS_POLICING`, `SHARED_CONGESTION`), each
with expectations inline and read-back as the only observation.
Suites SHALL be scratch-first and self-cleaning inside the safety
envelope, asserting the reference pair before running.

#### Scenario: A probe refusal is an answer
- **WHEN** the MC refuses a probe step (e.g. rejects 0x4000)
- **THEN** the suite records the refusal as the observed verdict and
  continues cleanup; the run is not marked failed by the refusal itself

### Requirement: dpni enters per-step learning mode for one online session
The online driver SHALL run dpni in per-step learning mode for one
operator-supervised session targeting the restool-reachable
unknown-register items, under the existing envelope; frozen ITF traces
from the session SHALL replay in `cargo test` as the family's
conformance twins.

#### Scenario: Learning-mode divergence feeds the model
- **WHEN** an online step observes behavior diverging from the model
- **THEN** the divergence is recorded against the model and the baseline
  amendment queue, not patched in the harness

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

### Requirement: Conformance twins bind the dpmac typestates to the model

`dpaa2-verify` SHALL carry MBT twins for the dpmac surface: frozen-
trace replay of the arbitration transitions and the dpni–dpmac edge
teardown law against the Rust types, and property twins for the
vocabulary and MAC-relation judgments. The twins SHALL quantify over
the shared model/Rust shape (ADR-0002 isomorphism), not over rendered
strings.

#### Scenario: The edge-law trace replays through the Rust surface

- **WHEN** the frozen sever-then-unbind trace replays against
  `dpaa2-api`
- **THEN** every step conforms, and the mutated trace with the order
  inverted is rejected by construction (it cannot be expressed against
  the typed surface, which the twin asserts as its negative face)

### Requirement: The board program renders Suite A and Suite B inside the envelope

Suite generation SHALL render two suites under the ADR-0003 safety
envelope and the fixed cabling roles. **Suite A** (end-to-end,
dpaa2ctl-driven in the V-DPRC-9/V-MVP-1 shape): intent anchored on
dpmac.7 converges the kernel regime; hooks read arbitration state, MAC
immutability and the inheritance positive face (dpni primary equals
dpmac burned-in after bind), attribute constancy across the cycle
(DPMAC-I3), the 28-row counter read, and the carrier; teardown runs the
typed sever-then-unbind path; a RemoteOwned leg reads the child/VFIO
arrangement. **Suite B** (V-DPMAC-2): the phantom create
(`--mac-id` with no DPC port entry) contained in a scratch child so the
object is never bus-visible (DPRC-I6) and no kernel driver can reach
it; both outcomes are recorded findings; the suite runs late in the
sitting with its own teardown and census; the root-container face is
recorded as deliberately untaken. Banked verdicts (V-LINK-2, V-LINK-4,
V-DPMAC-1) SHALL be cited, never re-run.

#### Scenario: Suite B contains the phantom

- **WHEN** Suite B issues the phantom create
- **THEN** the create targets the scratch child only, a refusal is
  recorded as the DPC-gated answer to baseline unknown #1, an
  acceptance is followed by attribute read-back and in-child destroy,
  and the closing census reads zero deltas

#### Scenario: Suite A witnesses the typed teardown

- **WHEN** Suite A tears down the converged kernel-regime port
- **THEN** the disconnect is issued while the dpni is bound, the unbind
  follows, the dpmac reads back with the standalone driver re-attached
  (no driverless interval in the read-backs), and the post-suite
  census is clean

### Requirement: Conformance twins bind the dpseci surface to the model

`dpaa2-verify` SHALL carry MBT conformance twins for the dpseci cfg and
refusal surface: frozen-trace replay of the model's create/refuse
transitions through the Rust constructors, and property twins for the
length-coupling and range judgments (every model-refused cfg is
constructor-refused, every model-accepted cfg constructs). ITF replay
SHALL be green against the Rust core.

#### Scenario: Refusals match class-for-class

- **WHEN** the frozen refusal traces replay
- **THEN** each model refusal lands on the same Rust refusal class,
  with no Rust-only or model-only refusal in the cfg surface

### Requirement: Suite A is generated inside the safety envelope

The harness SHALL generate one operator suite for the sitting: converge
a `[[crypto]]` intent on a scratch tenant, assert the child dprc and
dpseci read-backs (`info`: queues and all-2 priorities; raw GET_ATTR:
`HAS_CG` — V-DPSECI-2 rev 1), take the VFIO RemoteOwned leg, and tear
down typed with a clean census (V-DPSECI-3 rev 2). Read-only
boot-object hooks ride the same script: GET_API_VERSION (baseline
unknown #2), GET_ATTR on the kernel dpseci (unknown #3), and the boot
dpseci driver-link read. The suite SHALL be self-contained per the
operator-script standing rules, never touch the production VPP child
container, and never unbind the boot dpseci; banked verdicts
(V-LIFE-DPSECI-1 rev 2, V-DPSECI-1 rev 1) are cited, never re-run.

#### Scenario: The sitting mutates only what it created

- **WHEN** Suite A runs end to end
- **THEN** every created object lives under the scratch tenant and is
  destroyed by the suite's own spaced teardown, the boot and
  production dpsecis are only read, and the closing census is clean
