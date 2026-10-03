//! Plan semantics for the dpseci create block — the pure judgment that decides whether
//! an observed object still carries the desired create cfg, and how a mismatch must be
//! repaired (dpseci-typestate design D5; ADR-0015 decision 12).
//!
//! Filed under `plan/` as `plan/dpseci.rs`, following the family-planner pattern
//! [`crate::plan::dprc`] set (ADR-0018). The reconcile executor that consumes this
//! judgment is dpseci-typestate task 3.2; only the pure judgment and its tests land here,
//! so that executor has the one decision it needs and nothing speculative beyond it.
//!
//! # The immutable-cfg repair law (dpseci-typestate design D5; DPSECI-I1)
//!
//! Every dpseci create-time value — the options mask, the queue count, and the per-queue
//! priorities — is immutable: there is no `dpseci_set_*` for any of them, so no live
//! mutation path exists (`docs/baseline/dpseci.md` "Attribute mutability";
//! [`crate::families::dpseci`]). A desired-vs-observed difference therefore cannot be
//! repaired in place; it is a destroy+create replacement, the [`Class::Disruptive`] class
//! (ADR-0015 decision 12; `dpseci.qnt` `dpseci_lifecycle` `destroy` is the sole resize
//! path). Equal blocks need no repair.

use crate::families::dpseci::DpseciCfg;
use crate::plan::Class;

/// Judge a desired dpseci create block against the observed one, yielding the repair class
/// when they differ and `None` when they already match (dpseci-typestate design D5).
///
/// Any difference — an option bit, the queue count, or a priority entry — is an
/// immutable-cfg mismatch, and no setter exists to repair it in place, so the only repair
/// is a destroy+create replacement at [`Class::Disruptive`] (ADR-0015 decision 12;
/// DPSECI-I1). The judgment is total on the whole block: because [`DpseciCfg`] is immutable
/// and compares by value, a single `!=` captures every field without the caller
/// enumerating them, so no future field can silently escape the mismatch check.
#[must_use]
pub fn classify_cfg_mismatch(desired: &DpseciCfg, observed: &DpseciCfg) -> Option<Class> {
    (desired != observed).then_some(Class::Disruptive)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::dpseci::{DpseciOpt, OptionMask};

    fn cfg(flag: DpseciOpt, queues: usize) -> DpseciCfg {
        DpseciCfg::new(OptionMask::empty().with_flag(flag), queues, vec![2; queues]).unwrap()
    }

    #[test]
    fn equal_blocks_need_no_repair() {
        let a = cfg(DpseciOpt::HasCg, 4);
        let b = cfg(DpseciOpt::HasCg, 4);
        assert_eq!(classify_cfg_mismatch(&a, &b), None);
    }

    #[test]
    fn options_mismatch_is_disruptive_replacement() {
        let desired = cfg(DpseciOpt::HasCg, 4);
        let observed = cfg(DpseciOpt::HasOpr, 4);
        assert_eq!(
            classify_cfg_mismatch(&desired, &observed),
            Some(Class::Disruptive)
        );
    }

    #[test]
    fn queue_shape_mismatch_is_disruptive_replacement() {
        let desired = cfg(DpseciOpt::HasCg, 4);
        let observed = cfg(DpseciOpt::HasCg, 8);
        assert_eq!(
            classify_cfg_mismatch(&desired, &observed),
            Some(Class::Disruptive)
        );
    }

    #[test]
    fn priority_mismatch_is_disruptive_replacement() {
        let desired = cfg(DpseciOpt::HasCg, 2);
        let observed = DpseciCfg::new(
            OptionMask::empty().with_flag(DpseciOpt::HasCg),
            2,
            vec![2, 3],
        )
        .unwrap();
        assert_eq!(
            classify_cfg_mismatch(&desired, &observed),
            Some(Class::Disruptive)
        );
    }
}
