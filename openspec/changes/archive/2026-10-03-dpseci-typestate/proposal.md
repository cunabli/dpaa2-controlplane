# dpseci-typestate — the crypto interface becomes a typed, observed surface

## Why

The dpseci is the only Tier-A family still a parameter stub: the intent
layer already sizes it (ADR-0013 `[[crypto]]`) and plans it
(`compiled.rs::dpseci()`), but no typed family surface exists, the shim
cannot create one (restool makes `--priorities` mandatory and the compiled
object carries none), and the family's deciding hazard — the `HAS_CG`
safety bit — is unobservable through restool by print policy alone
(`info` fetches the options mask and discards it; DPSECI-I3). Roadmap #8
is the next tile (deps #2 and #4 delivered) and the last blocker of
`mc-portal-backend` (#10). It closes the deferred-anchor set COVERAGE
points at it: DPSECI-I1/I2(restool face banked)/I4/I9, and the V-DPSECI-2
read-back the suite ledger already names.

## What Changes

- `models/families/dpseci.qnt` grows the P2 configured-object shape
  (ADR-0019; `dpni.qnt` reference): the create cfg block (queue pairs
  1..16, a priorities vector length-coupled to the queue count with each
  entry in 1..8, a closed options vocabulary `HAS_CG`/`HAS_OPR`/
  `OPR_SHARED` plus provenance-carrying raw escape), the restool-layer
  refusal surface (banked V-DPSECI-1 rev 1), DPSECI-I1 immutability by
  construction, and DPSECI-I4 as a birth capability: congestion
  backpressure exists iff `HAS_CG` was set at create, and can never be
  added. The model fence is locked at cfg + consumer-facing consequences
  (sizing, birth capability, `hotBindAvailable: false`, the modeled I5
  dirt law, the I9 block-global counter law); no consumer-owned steering
  state machine — that surface is whitelist-fenced from every userspace
  transport and `quint-is-the-spec` forbids model states Rust cannot
  inhabit.
- `crates/dpaa2-api` gains `families/dpseci.rs`, structurally isomorphic
  (ADR-0002), and the derivation completes the create surface: priorities
  derived as the uniform constant `[2; num_queues]` (the verified deployed
  profile; no intent knob — the SEC scheduler semantics of the value are
  baseline unknown #4) and options derived as `HAS_CG` only (ADR-0013
  unamended; OPR bits stay representable for observation so the
  production child dpseci parses `Known`, never escape).
- **The MC-portal ioctl read slice debuts, tightly fenced** (new ADR):
  `crates/dpaa2-hal` gains a typed, policy-free `/dev/dprc.N` primitive
  that can encode only whitelisted reads — OPEN, GET_ATTR,
  GET_API_VERSION, DPSECI_GET_TX_QUEUE, CLOSE — read-only by
  construction; `crates/dpaa2-mc` drives the dpseci attribute read
  (options mask, API version) through it, making `HAS_CG` a real
  convergence observable (DPSECI-I3 implemented, not deferred). Creates
  and destroys stay on restool; all writes, other families' migrations,
  and the differential gate remain `mc-portal-backend` (#10), which
  inherits the primitive.
- `dpaa2ctl status --detail` gains a read-only dpseci row: queue count,
  per-queue tx priorities, observed options, API version, binding state.
  Display-only; nothing in it gates convergence.
- Board program, one suite, one sitting: Suite A converges a `[[crypto]]`
  intent on a scratch tenant (child dprc + dpseci), reads back through
  `info` and raw GET_ATTR (V-DPSECI-2 rev 1), takes the VFIO RemoteOwned
  leg, and tears down typed; read-only boot-object observation hooks ride
  the same script (API 5.4 → baseline unknown #2, the kernel dpseci's
  options → unknown #3, the boot dpseci driver link). The e2e verdict is
  V-DPSECI-3 rev 1. Banked verdicts are cited, never re-run
  (V-LIFE-DPSECI-1 rev 2: a later dpseci never kernel-binds this boot;
  V-DPSECI-1 rev 1: restool's parser owns the create refusals).
- **Out of scope, re-anchored loud**: MC-layer create validation
  (unknown #1 — CREATE encoding is excluded from the read slice by
  construction → #10); the DPSECI-I5 post-unbind dirt board face
  (`get_rx_queue`/`get_congestion` are on no userspace whitelist — the
  kernel's fence, stated in COVERAGE → #10); SEC attributes/counters and
  the 5.4 queue-status verbs (not whitelisted → #10); consumer-side
  congestion thresholds and in-flight caps (ADR-0005/0007/0008 in
  vpp-dpaa2-support; DPSECI-I6 verified there, out of this series'
  suites).

## Capabilities

### New Capabilities

None — the dpseci surface and the ioctl read slice land inside the
existing capability surfaces.

### Modified Capabilities

- `formal-models`: the dpseci family module grows the P2 cfg shape,
  refusal surface, and birth-capability law; invariants named under their
  baseline ids; COVERAGE rows move deferred→modeled with the unreachable
  faces re-anchored loud.
- `intent-compiler`: the compiled dpseci carries the full create surface
  — derived uniform priorities `[2; num_queues]` and the `HAS_CG`-only
  option set.
- `reconciler`: the dpseci typed cfg joins the plan surface; observed
  options join drift judgment (convergence on `HAS_CG` is observable,
  never trusted-on-write).
- `mc-backend`: dpseci create/destroy dispatch over restool; the
  dpaa2-hal MC-ioctl read primitive (whitelist-read-only) and the dpseci
  attribute read path over it.
- `provisioning-cli`: the read-only crypto detail row in
  `status --detail`.
- `mbt-harness`: MBT conformance twins for the dpseci cfg and refusal
  surface; generation of Suite A inside the safety envelope.
- `system-integration`: the end-to-end witness — one `[[crypto]]` intent
  converges a child-container dpseci, the hooks read the typed surface
  through both transports, the VFIO leg reads back across the container
  boundary, and teardown is typed.

## Impact

- Crates: `dpaa2-api` (new family module, derivation priorities/options),
  `dpaa2-mc` (create dispatch, GET_ATTR read path), `dpaa2-hal` (the
  ioctl read primitive — the workspace's fenced unsafe debut),
  `dpaa2-tools` (detail row), `dpaa2-verify` (twins + Suite A).
- Models: `families/dpseci.qnt`, frozen traces, COVERAGE rows.
- Docs: `docs/baseline/dpseci.md` amendments from the sitting (unknowns
  #2/#3 answered, read-slice observability), a new ADR for the ioctl
  read-slice decision, ADR-0019 note (P2 member landed), ROADMAP row #8,
  CHANGELOG via commits.
- No new dependencies; no persisted state; restool remains the only MC
  *write* transport (ADR-0004 posture unchanged until #10).
