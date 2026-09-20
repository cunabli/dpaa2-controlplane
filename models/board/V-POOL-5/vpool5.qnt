// Scenario module for board suite V-POOL-5 (pool-objects task 4.1;
// pool-objects design D5 root-bind floor): the dpbp census/ceiling at ROOT
// scope — the pool the
// reconciler manages is dprc.1 (pool_lifecycle.qnt poolContainer), so the
// census the convergence loop reads is the root pool's free count. DPBP-I7's
// floor is board-settled in a scratch child (V-CEIL-1); this suite re-takes it
// against the root's own listing and settles DPBP-I5's read-back (does a
// runtime dpbp's bpid equal its object id) on a root-resident object.
//
// The core machine has no notion of a finite pool (V-CEIL-1 comment), so a
// census refusal is not a trace step: the create-until-refused sweep and the
// DPBP-I5 read-back live in the hook ceiling.sh. The trace is the smallest
// there is — one dpbp in dprc.1 — so the suite has a teardown and a results
// directory. The hook creates and destroys in the ROOT, not a scratch child
// (V-CEIL-1's §7 note is inverted here by pool-objects design D5): it settles
// after every destroy and guards the boot residents (residents.sh) so the
// destroy rescan race (ADR-0008 §4) is caught, not silent — a datum the 4.2
// operator must read before running.
//
// Guards leave exactly one action enabled per state, so any seed freezes the
// same one-step trace:
//
//   pnpm exec quint run models/board/V-POOL-5/vpool5.qnt --main=vpool5 \
//     --mbt --max-steps=1 --n-traces=1 --max-samples=1 \
//     --out-itf=models/board/V-POOL-5/vpool5.itf.json
//
// Named invariants: none — the machine's guards carry the model gate.
// TLA+ escalation: none.
module vpool5 {
  import types.* from "../../core/types"
  import params.* from "../../families/params"

  pure val REF_DPMACS: Set[int] = Set(3, 4, 5, 6, 7, 8, 9, 10, 17)

  import machine(
    PARAMS = ALL_PARAMS,
    BOOT_DPMACS = REF_DPMACS
  ) as m from "../../core/machine"

  action init = m::init

  // One dpbp in the Linux root (dprc.1): the census/ceiling subject and the
  // DPBP-I5 read-back object. Singleton picks (fam, c) so the mbt records them;
  // disabled once it exists, so the trace is one step.
  action createObject = {
    nondet fam = Set(Dpbp).oneOf()
    nondet c = Set(m::dprc1).oneOf()
    all {
      m::s.nextNum.get(Dpbp) == 0,
      m::createObjectAt(fam, c),
    }
  }

  action step = any { createObject }
}
