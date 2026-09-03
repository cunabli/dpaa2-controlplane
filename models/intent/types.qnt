// The intent vocabulary and the hardware inventory as types — the two
// inputs of `compile(intent, inventory)` (openspec change `intent-layer`,
// design D1 and D2; ADR-0005 §1; ADR-0012). The operator states capacity
// and who consumes it: a tenant with a dataplane and a core budget, a port
// with the rate it must deliver, a link between two tenants, a fabric one
// tenant forwards, an accelerator for one. No construct carries a dpio,
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

  // The reserved kernel tenant (design D1): the kernel's own network driver
  // in dprc.1. A port that names no tenant is the kernel's port.
  pure val KERNEL: str = "kernel"

  // The dataplane a tenant runs (design D1; ADR-0012 pricing): KernelNetlink
  // — the kernel's own driver, configured over netlink; UserspacePoll — a
  // userspace poll-mode process (VPP, DPDK); UserspaceEvent — a userspace
  // event-driven process, which ADR-0012 does not price, so compile refuses
  // it (intent/refuse.qnt). The value names the ownership mechanism, not just
  // "kernel/userspace", so a future kernel dataplane (XDP/BPF) has room beside
  // KernelNetlink; UserspaceEvent replaces the earlier interrupt-naming. The
  // intent KernelNetlink maps to core's Kernel draws and UserspacePoll to
  // core's PollMode draws (intent/derive.qnt); the core Regime (comp::Kernel /
  // comp::PollMode) stays the internal pricing key.
  type Dataplane = KernelNetlink | UserspacePoll | UserspaceEvent

  // How a tenant sits in the MC container tree (task 2.6c; the private-VLAN
  // shape the tree already enforces). Isolated — its own child dprc,
  // MC-isolated from its siblings (the default the TOML applies when the field
  // is absent, so every prior intent keeps its shape): both a userspace-poll
  // process and a declared kernel-netlink network namespace land here, the
  // latter with a kernel-bound child dprc (drivers/bus/fsl-mc/dprc-driver.c
  // match_id_table binds any obj_type "dprc" l.882-885, dprc_scan_container
  // rescans l.746, dprc_add_new_devices adds discovered child dprcs as bus
  // devices l.202-224). Restricted — community co-residency: the tenant's
  // objects are created in its `pool` holder's dprc, the concrete case a DPDK
  // secondary process pooling a userspace-poll primary. Public — a holder that
  // accepts legal drawers into its own dprc; the reserved kernel is implicitly
  // public, and a userspace-poll primary may declare it.
  type Isolation = Public | Restricted | Isolated

  // A tenant of hardware capacity. `maxCores` is the budget the derived
  // thread count must fit under (design D3). Crypto demand is not a tenant
  // field: each `Crypto` construct carries its own `flows` (design D1).
  // `isolation` places the tenant in the container tree (default Isolated);
  // `pool` names the public holder a Restricted tenant draws inside ("" when
  // absent). The pool legality rules are intent/refuse.qnt (task 2.6c).
  type Tenant = { name: str, dataplane: Dataplane, maxCores: int, isolation: Isolation, pool: str }

  // A port: the dpmac anchor, the rate the port must deliver in Mbps (the
  // unit `dpmac info` reports its maximum in), and the tenant that
  // terminates it. A port named by a fabric is terminated by the fabric's
  // forwarding tenant — a port tenant that differs is a refusal (task 1.3).
  type Port = { name: str, dpmac: int, rate: int, tenant: str }

  // A link: point-to-point dpni↔dpni pseudo-wire between two tenants
  // (object-model.md §2, DPNI-I9); each end names the tenant whose interface
  // terminates the wire (interfaces, not ports — leaves room for tunnels).
  type Link = { name: str, interfaceA: str, interfaceB: str }

  // Who forwards between the members: Hardware — a DPSW (figure 6c), which
  // only the kernel can drive (dpsw.md: DPDK has no claim path); Software —
  // the forwarding tenant bridges its own dpnis (kernel bridge, a VPP bridge
  // domain, …), which the MC never sees.
  type Switching = Hardware | Software
  // A fabric member: a declared port, a tenant, or another fabric — so a
  // software switch can bridge a hardware-switched domain and a physical
  // port (a chain of switches), stated, not implied.
  type Member = MPort(str) | MTenant(str) | MFabric(str)
  // A fabric: one switched domain over its members; `forwardedBy` names the
  // tenant that runs its forwarding plane (dpsw for Hardware, own bridging
  // for Software). That a hardware fabric is forwarded by the kernel is a
  // rule (task 1.3), not a shape. Members are a List: declaration order numbers
  // the dpsw interfaces and the dpni ordinals (same rationale as the other
  // lists).
  type Fabric = { name: str, switching: Switching, forwardedBy: str, members: List[Member] }

  // An accelerator for one tenant; its dpseci queue count derives from this
  // construct's own `flows` — a tenant-visible demand, never an object count
  // (design D1; dpseci.md, task 1.2). A tenant may declare several `Crypto`
  // blocks: each is self-contained and sizes its own dpseci, and declaration
  // order numbers a tenant's dpseci ordinals (task 2.6e, the same positional
  // convention the fabric members and the other lists use). No max-fold ceiling
  // collapses a tenant's blocks any longer.
  type Crypto = { tenant: str, flows: int }

  // An additive extra (design D5): every derived count is a request; a
  // per-(tenant, family) extra adds its `count` on top, so the effective
  // count is request + count — raise-only by construction. Only the four
  // companion families dpio/dpbp/dpmcp/dpcon accept an extra; any other
  // family is refused (task 1.3), and `count` must be >= 1.
  type Extra = { tenant: str, family: Family, count: int }

  // Declaration order is the ordinal source: design D6 keys every derived
  // object by (tenant, family, ordinal), and a TOML `[[port]]` array is
  // ordered. Quint has no string ordering, so a Set would leave the third
  // dpni's ordinal undefined; the lists carry the positions the derivation
  // reads (intent/derive.qnt). crypto is a List for the same reason (task
  // 2.6e): a `[[crypto]]` array-of-tables is ordered, and a tenant's block
  // positions number its dpseci ordinals — each block sizes its own dpseci.
  // Only extras stay a Set — unordered, matched by (tenant, family), additive,
  // never by position.
  type Intent = {
    tenants: List[Tenant],
    ports: List[Port],
    links: List[Link],
    fabrics: List[Fabric],
    crypto: List[Crypto],
    extras: Set[Extra],
  }

  // The reserved kernel is implicitly public (task 2.6c): a Restricted tenant
  // may pool it, and it never itself draws inside another holder.
  pure def kernelTenant(maxCores: int): Tenant =
    { name: KERNEL, dataplane: KernelNetlink, maxCores: maxCores, isolation: Public, pool: "" }

  // ---- the inventory: the observed hardware offer (design D2) ----

  // dpmac attributes, immutable for the object's life (DPMAC-I3,
  // dpmac.md). Only the values the reference board reports are
  // enumerated; a new value enters with a board that has one.
  type EthInterface = XFI | CAUI | RGMII
  type LinkType = LinkNone | LinkFixed | LinkPhy | LinkBackplane

  // Free to anchor a port; Reserved with the reason (the ADR-0003 §3
  // safety matrix: dpmac.3 total-deny, dpmac.17 management plane);
  // Foreign with its owner (a DPL-owned object, ADR-0001 §4).
  type Availability = Free | Reserved(str) | Foreign(str)

  type DpmacOffer = {
    id: int,
    maxRate: int,        // Mbps, "maximum supported rate N Mbps" of `dpmac info`
    ethIf: EthInterface,
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
    // online CPUs of the kernel container: the kernel dataplane draws one
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
  // domain and the request/extra families.
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
  pure val ETH_IFS: Set[EthInterface] = Set(XFI, CAUI, RGMII)
  pure val DATAPLANES: Set[Dataplane] = Set(KernelNetlink, UserspacePoll, UserspaceEvent)
  pure val ISOLATIONS: Set[Isolation] = Set(Public, Restricted, Isolated)
  pure val SWITCHINGS: Set[Switching] = Set(Hardware, Software)
  pure val MEMBERS: Set[Member] = Set(MPort("p"), MTenant("c"), MFabric("f"))
  pure val LINK_TYPES: Set[LinkType] = Set(LinkNone, LinkFixed, LinkPhy, LinkBackplane)
  pure val AVAILABILITIES: Set[Availability] = Set(Free, Reserved("why"), Foreign("owner"))
  pure val CEILINGS: Set[Ceiling] =
    Set(Counted(0), Observed({ n: 0, provenance: "anchor" }), Unknown)
}
