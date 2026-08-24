// Scenario module for board suite V-DPDBG-1 (task 5.4; traffic-inventory
// §1): the driveable half of the dpdbg family — its lifecycle. dpdbg is
// structurally root-only (DPDBG-I1), so unlike every batch suite this one
// mutates the container Linux itself lives in and cannot be scratch-
// contained; it runs under the online driver with the mandatory per-step
// operator confirmation that traffic-inventory §4 fixes for root-container
// residents, promotion state notwithstanding.
//
// Four steps: create the singleton in dprc.1, plug it, rescan the root bus,
// destroy it. That is DPDBG-I1's create-in-root *positive* half — the
// refusals are the other half and are not here — followed by DPDBG-I4: the
// object enumerates on the fsl-mc bus like any other family and stays
// `Unbound`, because the kernel registers the bus type and no driver
// (dpdbg.md, Kernel-side). The model's own DPDBG_I4Test is this same shape.
//
// The destroy needs no unplug first. `destroyAt` refuses a root-container
// object only when a kernel driver holds it or a pool draw does, and
// neither can ever be true of a driver-less, resource-less singleton, so
// the plugged object is destroyed directly and the trace stays at four
// steps. ls-debug's own pattern is create-lazily-never-destroy; the destroy
// here is the suite returning the root container to its boot population.
//
// What this module deliberately does not model: the second-create and
// non-root-create refusals (DPDBG-I1's negative half), the `set` module
// probes and their out-of-range values, and the `dump` probes including the
// unhandled-type one (dpdbg.md unknowns 1, 2, 5). A refused action is a
// disabled action, not a step, so refusals cannot be traced at all; and
// `set` mutates MC-global firmware state that has no GET counterpart
// (DPDBG-I2) while `dump` output is judged by MC-log artifact and never by
// exit status (DPDBG-I3) — neither is object state this machine carries.
// Those probes run as a hand-authored probe plan against the online driver,
// beside this trace rather than inside it. `set --uart` is excluded outright
// (traffic-inventory §4: potentially unrecoverable console loss).
//
// Guards leave exactly one action enabled per state, so any seed freezes
// the same four-step trace:
//
//   pnpm exec quint run models/board/V-DPDBG-1/vdpdbg1.qnt --main=vdpdbg1 \
//     --mbt --max-steps=4 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-DPDBG-1/vdpdbg1.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vdpdbg1 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  // The singleton. restool pins the object id to 0 at every call site
  // (dpdbg.md, Command surface), which is also the number the model mints.
  pure val theDpdbg: ObjId = { fam: Dpdbg, num: 0 }

  action init = m::init

  // 1. The create, in the Linux root because firmware allows nowhere else
  //    ("object creation forbidden - DPDBG must be created in a root
  //    container"). The counter disables it afterwards, as does the
  //    singleton gate in `canCreate`.
  action createObject = {
    nondet fam = Set(Dpdbg).oneOf()
    nondet c = Set(m::dprc1).oneOf()
    all {
      m::s.nextNum.get(Dpdbg) == 0,
      m::createObjectAt(fam, c),
    }
  }

  // 2. Plug it. Bus visibility is still false here — create performs no
  //    rescan — which is what makes the next step's read-back meaningful.
  action plug = {
    nondet o = Set(theDpdbg).oneOf()
    m::plugAt(o)
  }

  // 3. Root bus rescan: the DPDBG-I4 read-back point. The object appears
  //    under /sys/bus/fsl-mc/devices with no driver link, ever.
  action rescan = {
    nondet c = Set(m::dprc1).oneOf()
    all {
      m::s.objs.keys().contains(theDpdbg),
      m::s.objs.get(theDpdbg).plugged,
      not(m::s.objs.get(theDpdbg).busVisible),
      m::rescanAt(c),
    }
  }

  // 4. Destroy, still plugged and still bus-visible. Gated on visibility so
  //    it cannot pre-empt the rescan; the counter keeps the create from
  //    re-firing behind it.
  action destroy = {
    nondet o = Set(theDpdbg).oneOf()
    all {
      m::s.objs.keys().contains(theDpdbg),
      m::s.objs.get(theDpdbg).busVisible,
      m::destroyAt(o),
    }
  }

  action step = any { createObject, plug, rescan, destroy }
}
