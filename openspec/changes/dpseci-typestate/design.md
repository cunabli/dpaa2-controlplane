# dpseci-typestate — design

## Context

The dpseci family (`docs/baseline/dpseci.md`) is ADR-0019's P2 configured
object with companion draw `dpmcp: 1`: created from a cfg block whose
wrong values firmware accepts silently or refuses late, with no
order-hazardous verb surface of its own. The intent side is already
delivered (ADR-0013 `[[crypto]]` → `num_queues = flows`, ordinal naming,
`CryptoFlowsOverDevice` refusal), but the compiled object stops at
`num_queues` + `HAS_CG`, the shim has no dpseci dispatch, and
`models/families/dpseci.qnt` is a parameter stub.

Board knowledge is banked and is cited, never re-learned
(`board-knowledge-read-before-theorizing`):

- V-LIFE-DPSECI-1 rev 2 — the crypto algorithm namespace is global and
  claimed by the boot dpseci; a later-created dpseci never kernel-binds
  for the rest of the boot (`hotBindAvailable: false`, ADR-0008).
- V-DPSECI-1 rev 1 — restool's own parser refuses priority 0, a priority
  above 8, and a count ≠ queue number (exit 234) before any MC command;
  MC-layer validation is unreachable through restool.
- The reference pair's ioctl whitelist (`docs/baseline/mc-ioctl-policy.md`)
  is the objective capability frontier for *every* userspace transport:
  generic OPEN/GET_ATTR/GET_API_VERSION/CLOSE and DPSECI_GET_TX_QUEUE
  pass with no capability flag; `set_rx_queue`, congestion, enable/
  disable/reset, SEC attrs/counters, and the 5.4 queue-status verbs are
  `-EACCES` regardless of privilege.
- Both board profiles are live: the kernel 16-queue/priority-1 dpseci and
  the production VPP child 8-queue/priority-2 dpseci (never touched,
  `peer-is-production` discipline applies to the whole live tenant).

The whole surface below was settled in the proposing grilling session;
decisions here record the how and the why, not open debates.

## Goals / Non-Goals

**Goals:**

- The dpseci typed surface, isomorphic model-to-Rust (ADR-0002), with the
  create cfg complete enough that the shim can actually create one.
- `HAS_CG` as a *real* convergence observable: the MC-ioctl read slice
  debuts, fenced to whitelisted reads, and DPSECI-I3 becomes implemented
  law instead of a deferred row.
- An end-to-end board witness: `[[crypto]]` intent → converge → child
  dpseci → dual-transport read-back → VFIO leg → typed teardown, plus
  the read-only boot-object faces (baseline unknowns #2 and #3).
- Every unreachable face re-anchored loudly in COVERAGE with the fence
  named, so #10 starts from an honest ledger.

**Non-Goals:**

- Any MC *write* over the ioctl path — creates/destroys stay restool;
  the differential gate and per-family migration are `mc-portal-backend`
  (#10). CREATE/DESTROY command ids are not encodable in the primitive.
- MC-layer create validation (baseline unknown #1) — needs ioctl CREATE,
  excluded by construction → #10.
- The DPSECI-I5 post-unbind dirt board face — `get_rx_queue`/
  `get_congestion` are on no userspace whitelist; the kernel's fence,
  not a scheduling choice → #10.
- Consumer-owned runtime state (rx steering, congestion thresholds,
  in-flight caps, enable/disable): the control plane neither issues nor
  can observe those verbs; consumer knowledge stays anchored in the
  vpp-dpaa2-support crypto ADRs (0005 wedge, 0007 FLE isolation, 0008
  raw-DP stub; DPSECI-I6 verified there).
- An intent priority knob, OPR derivation, SEC counter reads, DPL
  concerns (#14).

## Decisions

### D1 — The model is cfg-only P2 plus consumer-facing consequences

`dpseci.qnt` grows the `dpni.qnt`-reference shape: a create cfg block
(queue pairs 1..16 per `DPSECI_MAX_QUEUE_NUM`; a priorities vector whose
length equals the queue count, each entry 1..8; the closed options
vocabulary) and the restool-layer refusal surface replaying what
V-DPSECI-1 banked. DPSECI-I1 holds by construction (no mutating verb
exists on the surface). DPSECI-I4 is a birth capability: the congestion
backstop exists iff `HAS_CG` is in the create options, and no later
transition can mint it. The consumer appears only as consequences our
cfg determines: sizing (ADR-0012/0013), the birth capability, the
hot-bind refusal already in `dpseciParams`, the modeled I5 dirt law
(`main.qnt DPSECI_I5Test`, unchanged), and the I9 block-global counter
law. No consumer steering machine: those states would demand Rust types
nothing can inhabit (`quint-is-the-spec` isomorphism), typing away a
hazard that does not exist on our surface (ADR-0019 P2 rationale).
Alternative — modeling the consumer runtime surface — was rejected in
the grilling session on exactly that ground.

### D2 — The MC-ioctl read slice debuts here, fenced by the kernel's own whitelist

`dpaa2-hal` gains the `/dev/dprc.N` MC-command primitive the
architecture promised would "join with the change that consumes it";
dpseci is the first family that cannot witness its own deciding hazard
without it (restool fetches the options mask and discards it at print —
DPSECI-I3). The fence is structural, not procedural:

- The primitive's command vocabulary is a closed Rust sum over exactly
  the whitelisted reads this change consumes — OPEN, GET_ATTR,
  GET_API_VERSION, DPSECI_GET_TX_QUEUE, CLOSE. A write command id is
  unrepresentable, not forbidden by review.
- hal stays policy-free (ADR-0018): it encodes the 64-byte portal
  command, performs the ioctl, decodes the response header, and types
  the three outcomes (response, MC status ≠ OK, transport refusal —
  `-EACCES`/`ENOENT`/permission). Retry, tolerance, and error mapping
  live in `dpaa2-mc`, as with the restool shim.
- This is the workspace's unsafe debut; the unsafe block is confined to
  the ioctl call and struct transmutes, with layout asserted against
  `fsl_dpseci.h`/`fsl-mc-uapi.c` constants and covered by fixture tests.
- #10 inherits the primitive as the proven read core of the full portal
  backend; nothing in its shape pre-decides #10's write or differential
  machinery.

A new ADR records the decision: userspace MC reads ride the kernel
whitelist; the read slice precedes #10; the whitelist table is the
single source of what the primitive may ever encode.
Alternatives rejected: restool-pure (the safety bit stays write-trusted
for a tile — rejected as hollow e2e), and reads-plus-validation-probe
(first mutation through a hand-rolled transport; unknown #1 is not worth
that debut risk).

### D3 — Priorities derive as the uniform constant `[2; num_queues]`

restool makes `--priorities` mandatory, so the compiled object must
carry them. The value is the verified deployed profile (the production
child dpseci runs all-2); the kernel DPL convention (all-1) and the SEC
scheduler semantics of the difference are baseline unknown #4 — a knob
no board reading can yet discriminate, so none is exposed
(`justify-every-cap`). The typed range stays the full 1..8 so the kernel
profile's all-1 reads back as `Known`, never escape.

### D4 — Options derive `HAS_CG` only; the vocabulary observes all three

ADR-0013 already decided the derivation: the safety bit, nothing else.
The deployed script's `HAS_OPR,OPR_SHARED` mirror a vendor default no
consumer in the corpus inspects (baseline unknown #6) — deriving them
would bake in bits whose cost is invisible. The typed vocabulary is the
dpni `OptionMask` idiom: a closed enum over the three verified bits plus
a provenance-carrying raw escape, so observing the production child
object parses `Known` and an unknown firmware bit is attributed, never
silently merged.

### D5 — Convergence on options is observed, and unobservability is typed

The reconciler judges `HAS_CG` drift from the GET_ATTR read, never from
`info` (DPSECI-I3 as adapter law: the restool parse type for dpseci
carries no options field at all, so the wrong observable does not
typecheck). When the read path is unavailable (no `/dev/dprc.N` access —
unprivileged `status` runs), the observation is a typed
`Unobservable`-this-run outcome: display shows it as unknown, and
convergence treats it as absence of evidence, never as drift. The
immutability consequence stands in the plan layer: an options or
queue-shape mismatch on an existing object is immutable-cfg repair
(destroy+create disruption class), never a live mutation — no setter
exists (DPSECI-I1).

### D6 — One suite, one sitting, two verdicts

Suite A converges one `[[crypto]]` intent on a scratch tenant and reads
everything back: child dprc + dpseci present, `info` shows
queues = flows and all-2 priorities, raw GET_ATTR shows `HAS_CG`
(V-DPSECI-2 rev 1 — the read-back the suite ledger already names), the
VFIO RemoteOwned leg, typed teardown, census clean (V-DPSECI-3 rev 1,
the e2e verdict). Read-only boot-object hooks ride the same script:
GET_API_VERSION on a dpseci (expected 5.4 — unknown #2), GET_ATTR on
the kernel dpseci (unknown #3: does it carry `HAS_CG`), and the boot
dpseci driver-link read the ledger flags as worth taking. No second
suite: nothing left is a separate mutating experiment (dpmac's Suite B
existed for one). The production VPP child container is never touched;
the boot dpseci is read, never unbound — the unbind would risk kernel
crypto for the boot to witness dirt the whitelist hides anyway.

### D7 — COVERAGE re-anchors are part of the model phase, not an afterthought

The scope-creep failure mode this change guards against is discovering
unreachable observables mid-apply. Phase 1 lands the full disposition
sweep with the fence named per row: DPSECI-I1/I4 deferred→modeled;
I2's MC layer → #10 (CREATE unreachable from the read slice by
construction); I5's board face → #10 with the whitelist fence stated
(`get_rx_queue`/`get_congestion` on no userspace list); I9 → the
block-global law row (SEC counters unreadable by any userspace
transport; per-object accounting is typed impossible); I3 →
implemented (D5). Ledger lint green is acceptance for the phase.

### D8 — Execution: seven parcels, main loop stays thin

Model and suite-generator parcels go to `opus48-developer`; crate
parcels to `rust-developer` (no `model:` override — the pin is in the
agent frontmatter). Every parcel spec carries the settled-decisions
block (D1–D7 verbatim where relevant; no new dependencies; naming reads
as full words). Anticipated sizes: model parcel medium; family-module
and hal-primitive parcels medium; derivation, shim-dispatch, MBT-twins,
CLI parcels small. The main loop gathers context, writes parcel specs,
reviews, gates acceptance, commits — one bead at a time through
acceptance, close-then-commit per the repo hook.

## Risks / Trade-offs

- **[Unsafe debut: wrong struct layout or ioctl misuse]** → the encoded
  command set is five reads; layouts are asserted against the pinned
  kernel/restool sources with fixture tests (captured restool traffic
  crosses the same ioctl); the board's first GET_ATTR face runs before
  anything depends on it, and a decode mismatch is a typed observation
  failure, never a panic.
- **[Read path needs root; tooling runs unprivileged]** → typed
  `Unobservable` outcome (D5); suites run sudo and always observe;
  `status` degrades visibly, convergence never misjudges absence of
  evidence as drift.
- **[GET_ATTR response drift between dpseci API 5.3/5.4]** → the baseline
  records the 10.32→10.39 delta as additive-commands-only (no struct
  changes to anything pre-existing); the suite's GET_API_VERSION face
  (unknown #2) witnesses the actual version in the same sitting.
- **[Seeding #10's transport early fragments its design]** → the
  primitive is read-only by construction and trait-seamed behind
  `dpaa2-mc` policy exactly where #10's backend lands; the new ADR
  states that #10 owns all growth of the command vocabulary.
- **[Derived all-2 priorities later prove sub-optimal]** → the value is
  one constant in the derivation with the unknown-#4 citation; changing
  it is a one-line amendment once a board reading can discriminate, and
  no intent surface has to move.

## Open Questions

None blocking. Baseline unknowns #2 and #3 are answered by this
change's sitting; #1, #5–#8 are re-anchored loud (D7) with their fences
named.
