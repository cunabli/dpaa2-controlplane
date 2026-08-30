// The intent vocabulary and the hardware inventory as types — the two
// inputs of `compile(intent, inventory)` (openspec change `intent-layer`,
// design D1 and D2; ADR-0005 §1; ADR-0012). The operator states capacity
// and who consumes it: a consumer with a regime and a core budget, a port
// with the rate it must deliver, a link between two consumers, a fabric
// steered by one, an accelerator for one. No construct carries a dpio,
// dpbp, dpcon, dpmcp, queue or worker count; every such number is the
// derivation's (intent/derive.qnt). The dpaa2-api Rust types
// transcribe this module once the vocabulary is accepted at the gate
// (design D6, D7).
//
// The inventory is what the hardware offers, observed and never
// operator-written: each dpmac with the attributes `dpmac info` reports
// and an availability from the ADR-0003 safety matrix, the DPL-owned
// objects a plan must never claim (ADR-0001 §4), and one ceiling per
// derived family in ADR-0011's three values. The reference board's value
// is generated from the change-#2 snapshot (intent/inventory.qnt).
//
// Named invariants: none here (intent/invariants.qnt, task 1.4).
// Apalache-marked subset: none.
module intent_types {
  import types.* from "../core/types"           // Family, ObjId
  import companions.* from "../core/companions" // Regime = Kernel | PollMode (ADR-0012)

  // The reserved root-regime consumer (design D1): the kernel's own
  // network driver in dprc.1. A port that names no owner is its port.
  pure val KERNEL: str = "kernel"

  // A consumer of hardware capacity. `maxCores` is the budget the derived
  // thread count must fit under (design D3); `cryptoFlows` is a
  // consumer-visible demand, never an object count (design D1).
  type Consumer = { name: str, regime: Regime, maxCores: int, cryptoFlows: int }

  // A port: the dpmac anchor, the rate the port must deliver in Mbps (the
  // unit `dpmac info` reports its maximum in), and the owning consumer. A
  // port named by a fabric is terminated by the fabric's owner — a port
  // owner that differs from the fabric owner is a refusal (task 1.3).
  type Port = { name: str, dpmac: int, rate: int, owner: str }

  // A link: point-to-point dpni↔dpni pseudo-wire between two consumers
  // (object-model.md §2, DPNI-I9), ends named by consumer.
  type Link = { name: str, a: str, b: str }

  // Who forwards between the members: Hardware — a DPSW (figure 6c), which
  // only the kernel can drive (dpsw.md: DPDK has no claim path); Software —
  // the owner consumer bridges its own dpnis (kernel bridge, a VPP bridge
  // domain, …), which the MC never sees.
  type Switching = Hardware | Software
  // A fabric member: a declared port, a consumer, or another fabric — so a
  // software switch can bridge a hardware-switched domain and a physical
  // port (a chain of switches), stated, not implied.
  type Member = MPort(str) | MConsumer(str) | MFabric(str)
  // A fabric: one switched domain over its members, steered by the consumer
  // that owns it. That a hardware fabric's owner is the kernel is a rule
  // (task 1.3), not a shape. Members are a List: declaration order numbers
  // the dpsw interfaces and the dpni ordinals (same rationale as the other
  // lists).
  type Fabric = { name: str, switching: Switching, owner: str, members: List[Member] }

  // An accelerator for one consumer; its queue count derives from the
  // owner's `cryptoFlows` (dpseci.md, task 1.2).
  type Crypto = { owner: str }

  // A request/limit override (design D5): every derived count is a
  // request, a per-family limit under a consumer raises it; a limit below
  // the request is a refusal (task 1.3).
  type Limit = { consumer: str, family: Family, limit: int }

  // Declaration order is the ordinal source: design D6 keys every derived
  // object by (consumer, family, ordinal), and a TOML `[[port]]` array is
  // ordered. Quint has no string ordering, so a Set would leave the third
  // dpni's ordinal undefined; the lists carry the positions the derivation
  // reads (intent/derive.qnt). crypto and limits stay Sets — unordered,
  // matched by (owner) / (consumer, family), never by position.
  type Intent = {
    consumers: List[Consumer],
    ports: List[Port],
    links: List[Link],
    fabrics: List[Fabric],
    crypto: Set[Crypto],
    limits: Set[Limit],
  }

  pure def kernelConsumer(maxCores: int): Consumer =
    { name: KERNEL, regime: Kernel, maxCores: maxCores, cryptoFlows: 0 }

  // ---- the inventory: the observed hardware offer (design D2) ----

  // dpmac attributes, immutable for the object's life (DPMAC-I3,
  // dpmac.md). Only the values the reference board reports are
  // enumerated; a new value enters with a board that has one.
  type EthIf = XFI | CAUI | RGMII
  type LinkType = LinkNone | LinkFixed | LinkPhy | LinkBackplane

  // Free to anchor a port; Reserved with the reason (the ADR-0003 §3
  // safety matrix: dpmac.3 total-deny, dpmac.17 management plane);
  // Foreign with its owner (a DPL-owned object, ADR-0001 §4).
  type Availability = Free | Reserved(str) | Foreign(str)

  type DpmacOffer = {
    id: int,
    maxRate: int,        // Mbps, "maximum supported rate N Mbps" of `dpmac info`
    ethIf: EthIf,
    linkType: LinkType,
    avail: Availability,
  }

  // ADR-0011's three-valued ceiling as a type: Counted — the listed pool
  // is the ceiling (dpbp, decision 1); Observed — an unlisted ceiling the
  // board measured, carried with its provenance (dpni at 18, decision 2);
  // Unknown — the cap ended without a refusal, or the family was never
  // measured. Feasibility (task 1.3) refuses against Counted and Observed
  // and warns on Unknown; nothing here invents a number.
  type Ceiling = Counted(int) | Observed({ n: int, provenance: str }) | Unknown

  type Inventory = {
    // online CPUs of the kernel container: the kernel regime draws one
    // dpio per CPU (ADR-0012)
    cpus: int,
    // every DPC-born dpmac, by id (DPMAC-I1)
    dpmacs: int -> DpmacOffer,
    // DPL-owned objects a plan must never claim, with their owner label
    foreign: ObjId -> str,
    // one ceiling per derived family (DERIVED_FAMILIES is the domain)
    ceilings: Family -> Ceiling,
  }

  // The families the derivation emits (design D4): the ceilings map's
  // domain and the request/limit families.
  pure val DERIVED_FAMILIES: Set[Family] =
    Set(Dprc, Dpni, Dpbp, Dpio, Dpcon, Dpmcp, Dpseci, Dpsw)

  // Availability of any object id against the inventory: a dpmac reads
  // its offer, a DPL-owned object reads Foreign, anything else is free to
  // derive.
  pure def availabilityOf(inv: Inventory, id: ObjId): Availability =
    if (id.fam == Dpmac and inv.dpmacs.keys().contains(id.num))
      inv.dpmacs.get(id.num).avail
    else if (inv.foreign.keys().contains(id))
      Foreign(inv.foreign.get(id))
    else
      Free

  // A value use for every constructor inside the module: the verify-path
  // flattener drops a sum-type constructor whose only value use sits in
  // another module (QNT404; see families/params.qnt, core/companions.qnt).
  pure val ETH_IFS: Set[EthIf] = Set(XFI, CAUI, RGMII)
  pure val SWITCHINGS: Set[Switching] = Set(Hardware, Software)
  pure val MEMBERS: Set[Member] = Set(MPort("p"), MConsumer("c"), MFabric("f"))
  pure val LINK_TYPES: Set[LinkType] = Set(LinkNone, LinkFixed, LinkPhy, LinkBackplane)
  pure val AVAILABILITIES: Set[Availability] = Set(Free, Reserved("why"), Foreign("owner"))
  pure val CEILINGS: Set[Ceiling] =
    Set(Counted(0), Observed({ n: 0, provenance: "anchor" }), Unknown)
}
