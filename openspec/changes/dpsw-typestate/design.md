# Design

## Context

See `proposal.md` — Why. Standing substrate this change builds on: the
ADR-0019 pattern catalog (dpsw assigned P1, phase machine, with the
reset facet), the ADR-0022 edge-kind table with two dpsw placeholder
rows, the dpseci-typestate census precedent (dpseci-typestate design
D9: convergence as a multiset census on the observable cfg signature),
the cross-dprc-links edge actuation machinery (#9), and
`docs/baseline/dpsw.md` with V-DPSW-1 / V-DPSW-4 verdicts already
banked. Series constraint: this is the first tile of the locked
#11 → #12 → #10 order (ADR-0004 §8); the dpsw `McControl` slice this
change freezes is part of the portal's eventual differential surface,
and its sittings bank the portal's encode fixtures.

## Goals / Non-Goals

**Goals:**

- The kernel-bindability predicate as construction law in both model
  and Rust — the family's one load-bearing typestate.
- The switched-domain construct compiled with full rule provenance.
- The two dpsw edge kinds live in the ADR-0022 table with planning
  actuation.
- The dpsw unknowns settled or loudly anchored; the capture tap
  standing in the suite machinery for #12 and #10 to reuse.
- The Tier-B shared frame stated (D8) so #12 starts from it.

**Non-Goals:**

- The runtime switchdev surface (VLAN/FDB/ACL/LAG/mirroring,
  ~17 function groups): consumer-owned, ADR-0004 §8; DPSW-I8.
- Any dpdmux modeling or code — #12 inherits the frame, nothing lands
  for it here.
- LAG, S_VLAN, soft-parser, custom-TPID semantics: no in-corpus caller.
- Portal (ioctl) execution of any dpsw verb: #10's work; this change
  only banks fixtures.

## Decisions

- **D1 — `families/dpsw.qnt` instantiates the P1 idiom directly.** The
  switch is created-configured dataplane state like dpni, not a pool
  member; it gets its own family module in the dpni shape, with the
  reset facet marked (`resetOnBind: true`, ADR-0019). Alternative — a
  new shared "switch substrate" for dpsw+dpdmux — rejected: the two
  families are regime mirror images, not lifecycle twins; sharing waits
  for #12 to prove a real common mechanism (ADR-0019's rule that
  pattern files own only shared mechanisms).
- **D2 — the bindability predicate gates construction, not dispatch.**
  The Rust create block is obtainable only through a constructor that
  enforces DPSW-I1, mirroring the Quint type law; the compiler can
  therefore never emit, and the shim never render, an unbindable
  kernel-steered cfg. Alternative — validate at plan dispatch —
  rejected: all twelve cfg fields are create-time immutable, so the
  only repair is destroy-and-recreate; refusing at construction makes
  the invalid state unrepresentable rather than detected.
- **D3 — bindability fields are derivation outputs, never intent
  knobs.** The northbound schema does not carry flooding/broadcast/
  ctrl-if at all (topology-config delta). Alternative — expose them
  with validation — rejected: MC accepts shapes the kernel refuses
  (V-DPSW-4 verdict), and the only in-corpus consumer defines
  bindability; a knob would be a trap with exactly one legal value.
- **D4 — the regime law is a compile-time refusal.** A non-kernel
  steering owner is refused at compile with a typed refusal (DPSW-I6),
  joining the programmatic-parity refusal family (intent-layer
  vocabulary-v2 precedent). Alternative — defer to scan/observation —
  rejected: the DPDK scan discards dpsw *silently*; waiting converts a
  typable intent error into a runtime mystery.
- **D5 — the dpsw edge kinds carry no teardown witness.** The baseline
  records no teardown hazard for either dpsw edge; the rows get
  reification policy and port-bound connects only. Revisit trigger: a
  discovery finding of ordering sensitivity mints the witness in the
  dpni–dpmac idiom (ADR-0019 edge facet).
- **D6 — convergence is census + read-back only.** The dpsw verb
  surface has exit-code truth gaps (DPSW-I7), so the family joins the
  dpseci-typestate design D9 census discipline; no verb outcome is
  trusted without re-observation.
- **D7 — the capture tap is a suite-driver concern, eBPF-based
  (bpftrace), loud-fail.** The tap records the 64-byte MC command and
  response of every restool `SEND_MC_COMMAND` ioctl passively: a
  bpftrace program on the `sys_enter_ioctl`/`sys_exit_ioctl` syscall
  tracepoints, filtered on `comm == "restool"` and the request
  `_IOWR('R', 0xE0, 64)` (the one constant
  `dpaa2-hal`'s `portal::SEND_MC_COMMAND` pins), dumps the command
  bytes at enter and the kernel-written response from the same user
  buffer at exit; payloads parse offline into the fixture corpus
  committed beside suite results. It lives in `dpaa2-verify`'s suite
  preamble/driver — never in `dpaa2-mc`, keeping the shim policy-free
  and the oracle unmodified. The tap requires the tracing-enabled
  reference kernel (its packaging repo's `tracing.config` fragment:
  `FTRACE_SYSCALLS`/`BPF_EVENTS`; config-only, the `uname` pair and
  `REF_PAIR_ASSERT` unchanged) and the board's bpftrace; the sitting
  preamble proves the whole chain — recorder present, probes attached,
  and a read-only restool self-query yielding a payload record —
  before any mutating step, else it fails loudly. Alternatives —
  strace on the spawn (rejected on evidence: strace has no
  ioctl-buffer dump facility, an undecoded ioctl's third argument
  prints as a bare pointer under every flag; bead
  dpaa2-controlplane-dbf.17), patching restool (rejected: the oracle
  must stay the stock binary, ADR-0004 §1), LD_PRELOAD injection
  (rejected: it alters the oracle's process environment, where eBPF
  observes from outside), and a kernel source patch (rejected:
  ADR-0008 re-pins the pair; the Kconfig fragment does not).
- **D8 — the Tier-B shared frame, stated for #12.** What dpdmux
  inherits from this change: the ownership-matrix regime law idiom
  (dpsw kernel-steered / dpdmux DPDK-steered — mirror images; each
  family types its steering owner and refuses the other), the
  ADR-0022 table-row discipline for sub-indexed interface endpoints
  (`*.N.M`), the vendor-recipe-is-not-an-oracle stance (DPSW-I2 /
  DPDMUX-I2 are the same law), the D6 census discipline, and the D7
  capture tap verbatim. What it does not inherit: D2's predicate
  (dpdmux's bindability story is the kernel *staging* driver's, much
  weaker) and D5's no-witness call (the dpdmux uplink has the ADR-0009
  un-disconnectable hazard — its edge row will carry law).

- **D9 — the board program: two sittings, scoped ports, contained
  probes.** The discovery sitting creates its dpsw and companions in a
  scratch child where containable; the probes that need the kernel to
  bind (reset scope, bpid law, FDB budget) run at root-visible scope on
  objects this change created, never on standing topology. The witness
  sitting's uplink is dpmac.7 under the ADR-0003 matrix (flagged,
  reachability-class traffic only; dpmac.3 stays total-deny and is
  never wired); member dpnis are this change's own. Banked verdicts
  (V-DPSW-1, V-DPSW-4) are cited, never re-run. Divergences feed back
  under the standing triage order: implementation first, board state
  second, characterization last.

**Witness classes** (ROADMAP standing rule 7, declared here per knob):
`num_ifs`, `max_fdbs`, `max_fdb_entries`, and the MAC-budget sizing are
board-witnessable through create + kernel probe + `info` read-back (the
witness suite); the forced bindability fields are board-witnessed
negatively by V-DPSW-4 (already banked) and positively by the probe
succeeding; taildrop and per-port counters are observation-only,
display-face, never reconciled (offline-pin for their parse, board-read
during sittings); `mem_size` and `max_meters_per_if` stay undeclared —
dead baseline fields no knob exposes.

- **D10 — the V-TRAF-1 RCA lessons are preconditions, not findings to
  rediscover.** The two root-caused failure classes from the V-TRAF-1
  revision ledger (`models/board/README.md`) are designed in: (a) the
  dpsw probe is a modeled pool consumer (formal-models delta) so a
  census competitor is representable, not a board surprise; (b) every
  kernel-bind judgment in the suites uses the proven `bind_settle`
  bounded-eventually discipline — no immediate read of an async probe.
  The interposition use case (flat dpni↔dpmac → switched) is in scope
  as the roadmap's "beyond point-to-point" headline, sequenced through
  the dpmac-typestate severed-witness law.

## Risks / Trade-offs

- [dpsw discovery variance — 7 open unknowns could balloon sittings] →
  the program is fixed at one discovery + one witness sitting;
  unanswered faces are anchored as deferral rows (the bead
  dpaa2-controlplane-94k dossier pattern), never chased in-change.
- [DPSW-I4 reset scope could reshape plan ordering if reset proves
  partial] → the discovery sitting runs before the witness sitting;
  a surprising answer amends the model before the end-to-end witness
  freezes.
- [Capture-tap tooling absent or payload format drifts] → preamble
  gate fails the sitting loudly before mutation; the corpus format is
  asserted by a parse round-trip at commit time.
- [Edge-row growth tempts dpdmux scope creep] → D8 names the
  inheritance explicitly; any dpdmux row or code in this change is a
  review refusal.

## Migration Plan

Additive family change; no persisted state, no backend switch. Rollback
of a converged switched-domain is the witnessed teardown path (destroy
to baseline census). ADR-0013 (vocabulary) and ADR-0022 (table rows)
are amended in-change; no existing intent file changes meaning.

## Open Questions

- **[ANSWERED — driver convention; `docs/baseline/dpsw.md` unknown #6]**
  Whether the kernel's per-port FDB split is MC-enforced or driver
  convention: it is driver convention (the FDB-per-interface layout is not
  an MC-enforced budget); the answer lives in the baseline and changes no
  specs.
- Writable counters (`if_set_counter`) semantics — probed only if the
  discovery sitting has slack; otherwise stays a baseline unknown.
