//! The kernel root-bind face: a root dpni's `dpaa2-eth` bind observed per-target and
//! judged into a typed [`BindProbe`] (mc-backend spec requirement 2; pool-objects task 3.2).
//!
//! This module composes read-only observations exactly as [`pool`](crate::pool) composes
//! the delta→id dispatch: it gathers the facts the pure core needs — the bound driver name,
//! the netdev, and each companion family's kernel-poolable census — and hands them to
//! [`judge_bind_probe`], which owns every rule (the core judges, the adapter reports;
//! design D5, ADR-0003). The bind write is a tolerant nudge elsewhere
//! ([`SysfsKernel::bind`](crate::SysfsKernel)); probe success is judged from read-back, never
//! inferred from the write's exit (`docs/baseline/dpio.md` DPIO-I5).
//!
//! A probe deferral for want of companions is observed and reported, never healed here: this
//! face issues no create or destroy verb (the spec's "no object is destroyed or created in
//! response"; the C1 silent `-EPROBE_DEFER` exhaustion class, DPNI-I4).

use dpaa2_api::contract::{KernelControl, McControl};
use dpaa2_api::core::error::Error;
use dpaa2_api::core::family::Family;
use dpaa2_api::core::model::{BindProbe, DpniId, DprcId, judge_bind_probe};
use dpaa2_api::families::pool_lifecycle::{RawDriver, poolable};

/// The four families a root dpni's `dpaa2-eth` probe draws, in the fixed traversal order the
/// [`BindProbe::Deferred`] shortfall is reported in: dpmcp→dpbp→dpcon→dpio. The dpni needs
/// ≥1 dpbp, ≥1 dpmcp, ≥1 dpcon with an affine dpio (DPNI-I4), and the dpio probe itself
/// draws a dpmcp (DPIO-I1), so the portal leads the order.
const PROBE_FAMILIES: [Family; 4] = [Family::Dpmcp, Family::Dpbp, Family::Dpcon, Family::Dpio];

/// Observes a root dpni's `dpaa2-eth` bind through per-target read-back and judges it into a
/// typed [`BindProbe`] (mc-backend spec requirement 2). Read-only: it reads the bound driver
/// name, the netdev, and each companion family's kernel-poolable census (via
/// [`poolable`]), then returns [`judge_bind_probe`]'s verdict. It issues no create or destroy
/// verb ever — a deferral for want of companions is observed and reported, never healed here
/// (the spec's "no object is destroyed or created in response"; DPNI-I4).
///
/// The bind write is not consulted: probe success is judged from the driver link and the
/// netdev, never inferred from the write's exit (`docs/baseline/dpio.md` DPIO-I5). The
/// families are observed in the fixed dpmcp→dpbp→dpcon→dpio order, so the reported shortfall
/// keeps that traversal.
///
/// # Errors
/// Returns the first [`Error`] any observation raises (`dpni_driver`, `netdev_of`, or an
/// `observe_pool` of a companion family).
pub fn observe_bind_probe<M: McControl, K: KernelControl>(
    mc: &M,
    kernel: &K,
    container: Option<DprcId>,
    dpni: DpniId,
) -> Result<BindProbe, Error> {
    let driver = kernel.dpni_driver(dpni)?;
    let netdev = kernel.netdev_of(dpni)?;
    let mut poolable_counts = Vec::with_capacity(PROBE_FAMILIES.len());
    for family in PROBE_FAMILIES {
        let rows = mc.observe_pool(container, family)?;
        poolable_counts.push((family, poolable(&rows)));
    }
    Ok(judge_bind_probe(
        driver.as_ref().map(RawDriver::as_str),
        netdev.as_deref(),
        &poolable_counts,
    ))
}

#[cfg(test)]
mod tests {
    //! The probe judgment driven through [`FakeBackend`] as both the MC and the kernel
    //! seam: satisfied pools + a visible netdev read back [`BindProbe::Live`]; a dry dpcon
    //! pool reads back [`BindProbe::Deferred`] naming exactly `[Dpcon]`; and the deferral
    //! path issues no create/destroy (the pool census is unchanged across the call).

    use dpaa2_api::contract::fake::FakeBackend;
    use dpaa2_api::core::model::{DpmacId, LinkType, MacAddr};
    use dpaa2_api::core::types::ConstructName;
    use dpaa2_api::families::dpio::{ChannelMode, DpioCfg, Priorities};

    use super::*;

    fn cfg() -> DpioCfg {
        DpioCfg {
            mode: ChannelMode::LocalChannel,
            priorities: Priorities::new(8).expect("8 is in 1..=8"),
        }
    }

    fn seed_pools(backend: &FakeBackend, label: &ConstructName, families: &[Family]) {
        for family in families {
            match family {
                Family::Dpbp => {
                    backend.dpbp_create(None, label).unwrap();
                }
                Family::Dpmcp => {
                    backend.dpmcp_create(None, label).unwrap();
                }
                Family::Dpcon => {
                    backend
                        .dpcon_create(None, Priorities::new(2).unwrap(), label)
                        .unwrap();
                }
                Family::Dpio => {
                    backend.dpio_create(None, cfg(), label).unwrap();
                }
                _ => unreachable!("only the four probe families are seeded"),
            }
        }
    }

    // Satisfied pools + a visible netdev ⇒ Live: a PHY-connected dpni reads its driver link
    // and netdev back per-target (DPIO-I5).
    #[test]
    fn satisfied_pools_and_visible_netdev_reads_live() {
        let label = ConstructName::from("wan0");
        let backend = FakeBackend::new()
            .with_dpmac(
                DpmacId::new(3),
                LinkType::Phy,
                MacAddr::new([0, 0, 0, 0, 0, 1]),
            )
            .with_connected_dpni(DpniId::new(7), DpmacId::new(3));
        seed_pools(
            &backend,
            &label,
            &[Family::Dpmcp, Family::Dpbp, Family::Dpcon, Family::Dpio],
        );

        let probe = observe_bind_probe(&backend, &backend, None, DpniId::new(7)).expect("observe");
        assert_eq!(
            probe,
            BindProbe::Live {
                netdev: "eth7".to_owned()
            }
        );
    }

    // A dry dpcon pool (none pushed) with the other three satisfied ⇒ Deferred{[Dpcon]}, and
    // the call creates/destroys nothing (the census is identical before and after).
    #[test]
    fn dry_dpcon_reads_deferred_and_touches_no_object() {
        let label = ConstructName::from("wan0");
        let backend = FakeBackend::new();
        seed_pools(
            &backend,
            &label,
            &[Family::Dpmcp, Family::Dpbp, Family::Dpio],
        );

        let before: Vec<usize> = PROBE_FAMILIES
            .iter()
            .map(|&f| backend.observe_pool(None, f).unwrap().len())
            .collect();

        // DpniId 9 is unconnected, so no driver link and no netdev ⇒ the census decides.
        let probe = observe_bind_probe(&backend, &backend, None, DpniId::new(9)).expect("observe");
        assert_eq!(
            probe,
            BindProbe::Deferred {
                shortfall: vec![Family::Dpcon]
            }
        );

        let after: Vec<usize> = PROBE_FAMILIES
            .iter()
            .map(|&f| backend.observe_pool(None, f).unwrap().len())
            .collect();
        assert_eq!(
            before, after,
            "the deferral path creates or destroys nothing"
        );
        assert!(
            backend
                .observe_pool(None, Family::Dpcon)
                .unwrap()
                .is_empty()
        );
    }
}
