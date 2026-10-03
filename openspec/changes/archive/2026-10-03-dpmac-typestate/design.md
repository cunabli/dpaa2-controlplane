# dpmac-typestate — design

## Context

The dpmac is P4, the boot-born offer (ADR-0019): `creatable: false`,
RootOnly, no constructor — the board offers ports, the control plane
observes, judges, and connects. The family's recorded knowledge is deep:
V-DPMAC-1 (counter refusals silent, firmware-wide), V-LINK-2 rev 3 (the
`up` bit effective with `state_valid=0`, propagation lag, cable pull as
the only link-down stimulus), V-LINK-4 (no kernel-side observable for the
directional channels on PHY ports), the 5.11 sitting's sever-then-unbind
law anchored in driver source (ADR-0008 §8), and the fixed cabling roles
(ADR-0003 matrix). The `/dev/dprc.N` whitelist
(`docs/baseline/mc-ioctl-policy.md`) carries DPMAC
OPEN/CLOSE/GET_ATTR/GET_API_VERSION/GET_COUNTER/GET_MAC_ADDR only:
`DPMAC_GET_LINK_CFG` and `DPMAC_SET_LINK_STATE` are unreachable on the
restool transport, which bounds what this change can witness.

## Goals / Non-Goals

**Goals:**

- The full product path for the port surface: intent dpmac anchor → P4
  observation types → inventory → reconciler judgment → `dpaa2ctl`
  witness on the board.
- DPMAC-I2, I3 (observe-only), I6, I7, DPNI-I3's MAC value semantics,
  DPMAC-I4's reachable half, and the phantom-create face V-DPMAC-2.
- The dpni–dpmac edge teardown law typed so the driverless-port sequence
  does not typecheck, for any library consumer, not only the planner.

**Non-Goals (each re-anchored, not dropped):**

- `dpmac_get_link_cfg` + `SET_LINK_STATE` — whitelist-refused;
  → `mc-portal-backend` (#10), beside V-LINK-3 / DPMAC-I9.
- The MC-view dpni link read in the product — deferred by decision D6
  below; recorded on the #10 anchor as a restool-absence ledger row.
- Bulk `dpmac_get_statistics` — not whitelisted and never called by
  restool 2.4: unexercisable on every current transport, so wiring it
  would be unwitnessable dead code; → #10, where its differential gate
  lives. The 10.40 vocabulary rows are named in the model so the day a
  transport can read them the type change is additive.
- `set_protocol` / `set_params` mutation and MDIO — out of intent scope
  until a concrete need appears (baseline intent-mapping stance).
- DPRTC-I4 (timestamping) — lifecycle-foreign to a port-surface change;
  its COVERAGE anchor moves off #7 in the close-out sync.

## Decisions

### D1 — Single-family P4 reference, no offer substrate

`models/families/dpmac.qnt` grows the shape and
`crates/dpaa2-api/src/families/dpmac.rs` mirrors it isomorphically
(ADR-0002 structural-isomorphism law). No shared P4 substrate is built:
dpaiop shares nothing with dpmac beyond `creatable: false`, and ADR-0019
already names dpmac the P4 reference shape. Promotion to a substrate is
a documented ADR-0019 amendment when dpaiop's turn comes — the named
extension point, not a rewrite. The core machine is untouched
(DPMAC_I1/I5 already live there).

*Alternative rejected*: a P4 `offer_lifecycle` module mirroring P3's
`pool_lifecycle` — speculative abstraction over one rich member and one
near-empty one.

### D2 — The typestate axis is driver arbitration, as phase markers

The arbitration sum **Offered / KernelOwned / RemoteOwned** (DPMAC-I6,
board-verified) with typed transitions, judged as a plain observation
enum rather than family-internal phase-marker typestates (the ADR-0019
promotion trigger is met at the edge facet, without family-internal
markers).
The promotion trigger ADR-0019 names has fired: the library surface is
usable outside the planner, so the transitions are order-sensitive verbs
on a typed surface, and ordering must hold by construct for any
consumer, not by planner discipline. States are judged from read-backs
(endpoint query + driver-link/netdev observation), never commanded.

*Alternative rejected*: custody (ADR-0003 matrix) as the axis — custody
is per-board intent configuration, not object state.

### D3 — The teardown law lives on the edge kind, not the family

Sever-then-unbind is typed as the **dpni–dpmac edge kind's** law at the
connection surface (the ADR-0019 edge facet, beside ADR-0009's
`legalPorts`): `sever` consumes KernelOwned, yields Offered plus a
severed witness; the dpni kernel-face unbind demands the witness only
when its edge faces a dpmac. The law does not generalize and must not be
placed higher: a severed-witness demand is unsatisfiable for dpdmux
edges (un-disconnectable on the pinned firmware, ADR-0009 final), has no
driver-handback mechanism on dpni↔dpni (#9's discovery), and has no
recorded hazard on dpsw. Each edge kind carries its own teardown law;
this change types exactly one.

*Alternative rejected*: the guard as a match inside the dpni unbind
transition — buries an edge property in a family and grows a match arm
per future peer.

### D4 — Counter vocabulary is firmware-version-indexed and never zero-defaulted

The model indexes the counter vocabulary by firmware version: 10.39
carries the 28 observed rows (V-DPMAC-1), the 10.40 extension is named
but unread. The Rust read returns `Known(value) | NotInVocabulary` —
absence ≠ zero is unrepresentable, the DPMAC-I7 law by type. Reads go
through one `restool dpmac info` spawn per dpmac with the adapter owning
the skew tolerance (restool prints what succeeds and swallows refusals;
a row count other than the vocabulary's is itself a typed observation).
Counters never enter reconcile — they are traffic-dependent
observability, surfaced read-only.

### D5 — MAC inheritance is a typed judgment, not a new verb

No new intent field: an absent MAC on a dpmac-connected port means
inherit, the universe's default (the consumer driver programs the port
MAC at bind; the MC never does — baseline, V-DPNI-3/V-DPNI-10). The new
piece is the relation classification **Inherited** (dpni primary ==
dpmac burned-in) / **Overridden** (== intent-declared) / **Pending**
(zeros, consumer not yet bound — explicitly not drift) / **Mismatched**
(drift only when intent declared a value; otherwise an observation).
Pending-is-not-drift is the V-MVP-1 rev 1 churn lesson applied: a
judgment that reads a bind-timing transient as drift builds a loop. The
write path is unchanged from dpni-typestate: the reconciler writes a MAC
only when intent declares one.

### D6 — Link observation is sysfs-only; the MC-view read is a named deferral

One policy-free dpaa2-hal carrier primitive covers every port:
KernelOwned reads the peer dpni's netdev carrier, Offered/RemoteOwned
read the standalone driver's `macN` carrier
(`CONFIG_FSL_DPAA2_MAC_NETDEVS=y`, asserted as a reference-pair property
in the ADR-0008 class). `NoObservable` — neither netdev present — is
itself the diagnosis of a driverless port, the D3 hazard's observable.
Link is display-only and never gates convergence (V-LINK-2's propagation
lag).

**The recorded trade-off** (the restool-absence ledger, for #10): the
carrier is the kernel/PHY-local view. The MC-propagated view — what a
consumer like VPP actually sees via `dpni_get_link_state` — is a
distinct signal, and the two never collapse: the MC has no dpmac-side
link read at any transport (the API is `get_link_cfg`/`set_link_state`
only), so for an unconnected port sysfs is the only possible route,
permanently. What sysfs-only forgoes today: seeing PHY-up while the MC
view is stale, which diagnoses a stalled link push (the V-MVP-1
finding-49 crash lived in exactly that path). Adding the MC-view read
later is additive — one `McControl` method on whichever transport is
current, slotted into a type that already names its source. The deferral
and its trigger ride the #10 anchor in COVERAGE.

*Alternative rejected*: both routes now — two different truths in one
status column (lagged MC view for kernel ports, fresh PHY view for the
rest), plus a restool text parse the suites already provide as oracle.

### D7 — The board program: two suites, one sitting

**Suite A** — the end-to-end typestate suite, dpaa2ctl-driven
(V-DPRC-9/V-MVP-1 shape): intent anchored on dpmac.7 converges the
kernel regime; hooks read the arbitration state, MAC immutability and
the inheritance positive face (dpni primary == dpmac MAC post-bind),
attribute constancy across the cycle (DPMAC-I3's observe-only face), the
28-row counter read, and the carrier; the teardown runs the typed
sever-then-unbind path; the RemoteOwned leg reads the child/VFIO
arrangement across the container boundary. **Suite B** — V-DPMAC-2, the
phantom create (`--mac-id` with no DPC port entry), contained in a
scratch child: DPRC-I6 keeps the object off the bus so no kernel driver
can probe it and the standalone driver's NULL-deref hazard
(silent-failure notes) is structurally unreachable. Both outcomes are
findings — refusal settles baseline unknown #1 as DPC-gated; acceptance
yields the phantom's attribute read-back, then an in-child destroy. The
root face (would the kernel bind a phantom?) is recorded as deliberately
untaken — loud, not silent. Suite B runs late in the sitting with its
own teardown and census. Banked verdicts (V-LINK-2, V-LINK-4,
V-DPMAC-1) are cited, never re-run.

## Risks / Trade-offs

- [Counter/attr reads parse restool's rendered text] → the typed
  vocabulary expects an exact row set; any deviation is a typed
  observation, not a parse guess; the parse dies at #10 while the types
  survive. Known shim hazard: `restool dpmac info` aborts via
  `assert(false)` on enum values beyond its vocabulary — the adapter
  treats a dead shim spawn as an observation failure, never inherits it.
- [Sysfs-only link display shows the PHY view, not what consumers see]
  → accepted and recorded (D6); suites keep the restool dpni link line
  as oracle; the MC-view read is a named additive deferral on #10.
- [Phantom create has unknown MC semantics] → scratch-child containment
  makes the kernel structurally unreachable (DPRC-I6 board-held); both
  outcomes are findings; late-sitting placement with own census bounds
  the blast radius.
- [Suite A's sever leg touches dpmac.7, a flagged production-peer port]
  → ADR-0003 matrix governs; the leg manipulates only this change's own
  dpni and the connection edge, never the peer host; V-MVP-1 rev 5's
  finding-49 (one phylink link-up Oops at ordered teardown) is a known,
  recorded hazard — the sitting carries its power-cycle contingency.
- [Carrier route depends on CONFIG_FSL_DPAA2_MAC_NETDEVS] → asserted as
  a reference-pair property before the suite runs; a BSP without it
  reads NoObservable, degrading display, never correctness.

## Open Questions

None blocking — the phantom create's answer and the RemoteOwned carrier
read-back are discovery outcomes the suites are built to record either
way; divergences feed back under the validation-gaps triage order
(implementation first, board state second, characterization last).
