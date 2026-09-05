// The raw-conformance trace source (openspec change `intent-layer`, task 3.3e,
// bead gqf.48; the MBT counterpart of intent/replay.qnt). Freezes one ITF trace
// per directed raw document so dpaa2-verify can serialize each `RawIntent` to a
// TOML fragment on the real `dpaa2-config` surface, feed it through `parse_str`,
// and diff the Rust verdict (and, when accepted, the resulting `Intent`) against
// the model's own `parse` — the mechanism that catches a clause the model has and
// `parse.rs` forgot, or vice versa (the gqf.39/gqf.40 bug class).
//
// It mirrors intent/replay.qnt: named `run`s that
// `quint test --out-itf='{test}.itf.json'` writes one frozen file per. Each run's
// single state carries both the raw surface (`raw`) and the model's own verdict
// (`verdict = parse(raw)`), so the replayer parses the raw, re-parses it with the
// Rust `dpaa2-config`, and asserts agreement against the frozen model verdict.
// The model is the oracle — the Rust harness never recomputes `parse`, it reads
// the frozen `verdict`. Directed runs (not a random draw) make the freeze
// deterministic without a seed, exactly as intent/replay.qnt is.
//
// Coverage: one accepted document plus every near-miss kind the alphabet reaches
// (reserved kernel, duplicate name, self-loop link, unresolved member, unknown
// extra family, dangling tenant reference, both isolation/pool contradictions),
// and one dirty-mix document where several near-misses coincide so the replayer
// exercises the DEVIATION — `parse.rs` short-circuits with one error while the
// model carries the whole set. The Rust harness (crates/dpaa2-verify/tests/
// raw_conformance.rs) asserts near-miss coverage over the corpus, not by hand.
//
// Named invariants: none — conformance is the replayer's diff. Apalache-marked
// subset: none.
module intent_raw_replay {
  import intent_types.* from "./types"           // Dataplane, Isolation, Switching, KERNEL
  import intent_raw.* from "./intent_raw"        // RawIntent, RawTenant, …, Parsed, parse, emptyRaw

  // The raw document and the model's own verdict over it. One state per trace
  // suffices — `parse` is pure, so the replayer needs no transitions.
  var raw: RawIntent
  var verdict: Parsed

  // Binds a raw document into the frozen state alongside the model's own `parse`
  // result. This is the run's single (initial) action; the ITF trace it emits has
  // exactly this one state.
  action feed(r: RawIntent): bool = all {
    raw' = r,
    verdict' = parse(r),
  }

  pure val rtBase: RawTenant = { dataplane: UserspacePoll, maxCores: 16, isolation: Isolated, pool: "" }

  // A clean document exercising every construct, so the accepted-arm Intent
  // comparison covers tenants, ports, links, fabrics, crypto, and extras at once.
  run rawAcceptedTrace = feed(
    emptyRaw
      .with("tenants", Map("router" -> rtBase))
      .with("ports", [{ name: "wan0", dpmac: 7, rate: 10000, tenant: "router" }])
      .with("links", [{ name: "uplink", interfaceA: "router", interfaceB: KERNEL }])
      .with("fabrics", Map("lan" -> { switching: Hardware, forwardedBy: KERNEL, members: ["wan0", "router"] }))
      .with("crypto", [{ tenant: "router", flows: 4 }])
      .with("extras", Map("router" -> Map("dpio" -> 2))))

  // The reserved kernel declared as a tenant — ReservedKernel (parse.rs convert).
  run rawReservedKernelTrace = feed(
    emptyRaw.with("tenants", Map(KERNEL -> { dataplane: KernelNetlink, maxCores: 16, isolation: Isolated, pool: "" })))

  // Two ports of one name — DuplicateName (parse.rs convert; TOML keys cannot
  // dedupe a positional name).
  run rawDuplicatePortTrace = feed(
    emptyRaw.with("ports",
      [{ name: "p1", dpmac: 7, rate: 10000, tenant: "" }, { name: "p1", dpmac: 8, rate: 10000, tenant: "" }]))

  // A link naming one tenant at both ends — LinkSelfLoop (parse.rs convert_link).
  run rawSelfLoopTrace = feed(
    emptyRaw.with("tenants", Map("c1" -> rtBase))
      .with("links", [{ name: "l1", interfaceA: "c1", interfaceB: "c1" }]))

  // A fabric member classifying to nothing — RawMemberUnresolved (classify_member).
  run rawUnresolvedMemberTrace = feed(
    emptyRaw.with("fabrics", Map("f1" -> { switching: Hardware, forwardedBy: KERNEL, members: ["ghost"] })))

  // An [extra.<tenant>] family not one of the 16 — UnknownExtraFamily (parse_family).
  run rawUnknownFamilyTrace = feed(
    emptyRaw.with("tenants", Map("c1" -> rtBase)).with("extras", Map("c1" -> Map("dpwidget" -> 1))))

  // A port naming an undeclared tenant — TenantAbsent (the dangling reference).
  run rawDanglingTenantTrace = feed(
    emptyRaw.with("ports", [{ name: "p1", dpmac: 7, rate: 10000, tenant: "ghost" }]))

  // The two isolation/pool contradictions in one document — RestrictedWithoutPool
  // (restricted, no pool) and PoolWithoutRestricted (a pool on a non-restricted
  // tenant), both reused from the compile vocabulary (parse.rs convert_tenant).
  run rawPoolContradictionTrace = feed(
    emptyRaw.with("tenants", Map(
      "restr" -> rtBase.with("isolation", Restricted),
      "stray" -> rtBase.with("pool", "restr"))))

  // Several near-misses at once: a duplicated dangling port. `parse.rs` reports
  // the first (TenantAbsent), the model carries the whole set — the replayer's
  // any-match over the set is what reconciles the short-circuit DEVIATION.
  run rawDirtyMixTrace = feed(
    emptyRaw.with("ports",
      [{ name: "p1", dpmac: 7, rate: -1, tenant: "ghost" },
       { name: "p1", dpmac: 8, rate: 0, tenant: "ghost" }]))
}
