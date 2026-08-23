// The corpus-wide parameter table: all 16 family records keyed by
// Family. Lives in its own module (not main.qnt) because the quint
// flattener that backs `quint verify` loses sum-type constructors whose
// only value use sits in the main module (QNT404 on six of the 16);
// hosting the table here keeps the Apalache rung runnable.
//
// Named invariants: none here — the phase-2 encodings live in
// core/invariants.qnt and models/main.qnt under baseline ids.
module params {
  import types.* from "../core/types"
  import dprc.* from "./dprc"
  import dpni.* from "./dpni"
  import dpmac.* from "./dpmac"
  import dpbp.* from "./dpbp"
  import dpio.* from "./dpio"
  import dpcon.* from "./dpcon"
  import dpmcp.* from "./dpmcp"
  import dpseci.* from "./dpseci"
  import dpsw.* from "./dpsw"
  import dpdmux.* from "./dpdmux"
  import dpaiop.* from "./dpaiop"
  import dpci.* from "./dpci"
  import dpdcei.* from "./dpdcei"
  import dpdmai.* from "./dpdmai"
  import dprtc.* from "./dprtc"
  import dpdbg.* from "./dpdbg"

  pure val ALL_PARAMS: Family -> FamilyParams = Map(
    Dprc -> dprcParams,
    Dpni -> dpniParams,
    Dpmac -> dpmacParams,
    Dpbp -> dpbpParams,
    Dpio -> dpioParams,
    Dpcon -> dpconParams,
    Dpmcp -> dpmcpParams,
    Dpseci -> dpseciParams,
    Dpsw -> dpswParams,
    Dpdmux -> dpdmuxParams,
    Dpaiop -> dpaiopParams,
    Dpci -> dpciParams,
    Dpdcei -> dpdceiParams,
    Dpdmai -> dpdmaiParams,
    Dprtc -> dprtcParams,
    Dpdbg -> dpdbgParams
  )
}
