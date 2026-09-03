//! The [`RestoolMc`] shim: the phase-1 [`McControl`] implementation.
//!
//! [`RestoolMc`] reproduces the exact `restool` v2.4 sequence from the design recipe
//! (`--script` create + plug-assign + connect + sync; observe via
//! `dprc show`/`dpni info`/`dpmac info`) behind the neutral trait, so a future ioctl
//! backend can replace it without touching the core. It introduces **no `unsafe`
//! code** and keeps the workspace `unsafe_code = "forbid"` lint intact (mc-backend
//! spec). netdev observation and driver binding live in [`SysfsKernel`](crate::SysfsKernel).

use std::collections::BTreeMap;

use dpaa2_api::{
    Availability, Ceiling, DERIVED_FAMILIES, DpmacId, DpmacOffer, DpniId, Error, Family, Inventory,
    McControl, ObservedDpmac, ObservedDpni, ObservedTopology,
};

use crate::parse;
use crate::runner::{RestoolRunner, Runner};

/// The default fsl-mc root container.
pub const DEFAULT_CONTAINER: &str = "dprc.1";

/// The MC's own container, aliased `mc.global` by restool; only `dprc show` (and
/// its `--resources` variant) is accepted against it (`dprc.md` mc.global section).
const MC_GLOBAL: &str = "mc.global";

/// The ADR-0003 §3 port safety matrix, transcribed once — the single Rust copy the
/// southbound consults (task 3.5, design D2). A reserved dpmac may never anchor a
/// port; the reason string is the one `REF_INVENTORY` records
/// (`models/intent/inventory.qnt`). Any dpmac not listed here is [`Availability::Free`]
/// unless a DPL object holds it (which this adapter cannot yet observe — see
/// [`RestoolMc::read_inventory`]).
fn reserved_reason(id: DpmacId) -> Option<&'static str> {
    match id.into_inner() {
        3 => Some("ADR-0003 §3: wired to a peer that must never see traffic (total-deny)"),
        17 => Some("ADR-0003 §3: management plane (dpni.0), never touched"),
        _ => None,
    }
}

/// ADR-0011's per-family ceiling policy, transcribed once (task 3.5). It is *not* a
/// plain "listed pool ⇒ [`Ceiling::Counted`]" rule: only dpbp's `bp` pool is a true
/// count (decision 1); dpmcp's `mcp` pool is a per-boot budget the listing reports
/// but destroy never returns, so it is [`Ceiling::Observed`] (decision 3); dpni's
/// ceiling of 18 is measured but *unlisted* (decision 2); every other derived family
/// was never measured and stays [`Ceiling::Unknown`]. This never invents a number —
/// the two `Observed` values are the ones ADR-0011/`REF_INVENTORY` record.
fn ceiling_of(family: Family, pools: &BTreeMap<String, i64>) -> Ceiling {
    match family {
        Family::Dpbp => pools
            .get("bp")
            .map_or(Ceiling::Unknown, |&n| Ceiling::Counted(n)),
        Family::Dpmcp => pools
            .get("mcp")
            .map_or(Ceiling::Unknown, |&n| Ceiling::Observed {
                n,
                provenance: "ADR-0011 decision 3 (V-CEIL-1 rev 2, 2026-08-29): MC portals are a \
                 fixed per-boot budget the listing reports, never returned by destroy"
                    .to_owned(),
            }),
        Family::Dpni => Ceiling::Observed {
            n: 18,
            provenance: "ADR-0011 decision 2 (V-CEIL-1, 2026-08-29): the 18th dpni is refused \
                 with every listed pool showing room; an unlisted resource"
                .to_owned(),
        },
        _ => Ceiling::Unknown,
    }
}

/// One step in a transactional provisioning chain (see
/// [`RestoolMc::provision_chain`]): the object kind (for rollback destroy) and the
/// `restool --script <kind> create ...` arguments.
struct ProvisionStep {
    kind: &'static str,
    args: Vec<String>,
}

/// `restool`-backed [`McControl`] implementation.
///
/// Generic over [`Runner`] so parsing and command construction are testable with
/// recorded output and no board.
pub struct RestoolMc<R: Runner> {
    runner: R,
    container: String,
    /// Number of CPU cores; sets the DPIO pool size (matches `ls-addni`).
    cores: usize,
    /// DPNI Rx/Tx queues; also the number of private DPCONs to provision.
    queues: usize,
}

/// Best-effort CPU core count for sizing the DPIO/DPCON pools.
fn default_cores() -> usize {
    std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get)
}

impl RestoolMc<RestoolRunner> {
    /// Builds a shim driving the real `restool` against `dprc.1`.
    #[must_use]
    pub fn new() -> Self {
        Self::with_runner(RestoolRunner::new(), DEFAULT_CONTAINER)
    }
}

impl Default for RestoolMc<RestoolRunner> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: Runner> RestoolMc<R> {
    /// Builds a shim over an explicit runner and root container.
    pub fn with_runner(runner: R, container: impl Into<String>) -> Self {
        let cores = default_cores();
        Self {
            runner,
            container: container.into(),
            cores,
            queues: cores,
        }
    }

    /// Overrides the DPNI queue count (and thus the private DPCON count). Clamped to
    /// the core count, mirroring `ls-addni` (`num_dpcons = min(num_queues, nproc)`).
    #[must_use]
    pub fn with_queues(mut self, queues: usize) -> Self {
        self.queues = queues.clamp(1, self.cores);
        self
    }

    /// Overrides the assumed core count (primarily for tests). Also re-clamps queues.
    #[must_use]
    pub fn with_cores(mut self, cores: usize) -> Self {
        self.cores = cores.max(1);
        self.queues = self.queues.clamp(1, self.cores);
        self
    }

    /// The number of private DPCONs a DPNI needs: `min(queues, cores)`.
    fn num_dpcons(&self) -> usize {
        self.queues.clamp(1, self.cores)
    }

    /// `restool --script <type> create …` then plug the result into the container.
    /// Returns the created object reference (e.g. `dpcon.5`).
    fn create_and_plug(&self, create_args: &[&str]) -> Result<String, Error> {
        let out = self.runner.run(create_args)?;
        let obj = parse::parse_object_ref(&out)
            .ok_or_else(|| Error::Parse(format!("no object id in `{}`", out.trim())))?
            .to_owned();
        self.assign_plugged(&obj)?;
        Ok(obj)
    }

    /// `restool dprc assign <container> --object=<obj> --plugged=1`.
    fn assign_plugged(&self, obj: &str) -> Result<(), Error> {
        self.runner.run(&[
            "dprc",
            "assign",
            &self.container,
            &format!("--object={obj}"),
            "--plugged=1",
        ])?;
        Ok(())
    }

    /// Ensures the container holds one DPIO per core (each with its own DPMCP),
    /// topping up idempotently — the DPAA2 datapath needs a per-core DPIO pool
    /// (`ls-addni` `create_dpio`).
    fn ensure_dpio(&self) -> Result<(), Error> {
        let show = self.runner.run(&["dprc", "show", &self.container])?;
        let existing = parse::count_objects(&show, "dpio");
        let container = format!("--container={}", self.container);
        for _ in existing..self.cores {
            let dpio = self.create_and_plug(&[
                "--script",
                "dpio",
                "create",
                "--channel-mode=DPIO_LOCAL_CHANNEL",
                &container,
                "--num-priorities=8",
            ])?;
            // Each DPIO also needs a companion DPMCP.
            self.create_and_plug(&["--script", "dpmcp", "create", &container])?;
            tracing::debug!(%dpio, "provisioned dpio");
        }
        Ok(())
    }

    /// Data for a DPNI's private dependencies (one DPBP, one DPMCP, and
    /// `num_dpcons` DPCONs) — the objects `dpaa2-eth` allocates at probe. Without
    /// them the driver fails with "No more resources of type dpcon left". A plain
    /// data builder; [`Self::provision_chain`] does the actual creation and any
    /// rollback.
    fn dpni_dep_steps(&self) -> Vec<ProvisionStep> {
        let container = format!("--container={}", self.container);
        let mut steps = vec![
            ProvisionStep {
                kind: "dpbp",
                args: vec![
                    "--script".to_owned(),
                    "dpbp".to_owned(),
                    "create".to_owned(),
                    container.clone(),
                ],
            },
            ProvisionStep {
                kind: "dpmcp",
                args: vec![
                    "--script".to_owned(),
                    "dpmcp".to_owned(),
                    "create".to_owned(),
                    container.clone(),
                ],
            },
        ];
        for _ in 0..self.num_dpcons() {
            steps.push(ProvisionStep {
                kind: "dpcon",
                args: vec![
                    "--script".to_owned(),
                    "dpcon".to_owned(),
                    "create".to_owned(),
                    "--num-priorities=2".to_owned(),
                    container.clone(),
                ],
            });
        }
        steps
    }

    /// Runs `steps` in order via [`Self::create_and_plug`], then `then`. If any
    /// step or `then` fails, destroys every object already created in this chain
    /// (reverse order, best-effort) before returning the original error — so a
    /// failed provisioning attempt never leaves orphaned private objects plugged
    /// in the container (each is otherwise invisible to reconcile, since
    /// ownership is edge-based).
    ///
    /// This is the reusable transactional primitive for any object kind's
    /// dependency chain, not just the DPNI's: a future kind's deps are a new
    /// `Vec<ProvisionStep>` fed to this same helper.
    fn provision_chain<T>(
        &self,
        steps: &[ProvisionStep],
        then: impl FnOnce(&Self) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut created: Vec<(&'static str, String)> = Vec::new();
        for step in steps {
            let args: Vec<&str> = step.args.iter().map(String::as_str).collect();
            match self.create_and_plug(&args) {
                Ok(obj) => created.push((step.kind, obj)),
                Err(e) => {
                    self.rollback_chain(&created);
                    return Err(e);
                }
            }
        }
        match then(self) {
            Ok(v) => Ok(v),
            Err(e) => {
                self.rollback_chain(&created);
                Err(e)
            }
        }
    }

    /// Best-effort teardown of a partially- or fully-created provisioning chain,
    /// in reverse creation order. Destroy failures are logged, not propagated:
    /// the original error is what the caller of [`Self::provision_chain`] needs.
    fn rollback_chain(&self, created: &[(&'static str, String)]) {
        for (kind, obj) in created.iter().rev() {
            if let Err(e) = self.runner.run(&[kind, "destroy", obj]) {
                tracing::warn!(error = %e, %obj, "rollback: failed to destroy");
            }
        }
    }

    /// Borrows the underlying runner (used by tests to inspect issued commands).
    pub fn runner(&self) -> &R {
        &self.runner
    }

    /// Forces a bus rescan; issued after every mutation (design recipe).
    fn sync(&self) -> Result<(), Error> {
        self.runner.run(&["dprc", "sync"])?;
        Ok(())
    }
}

impl<R: Runner> McControl for RestoolMc<R> {
    fn observe(&self) -> Result<ObservedTopology, Error> {
        let show = self.runner.run(&["dprc", "show", &self.container])?;
        let (dpni_ids, dpmac_ids) = parse::parse_dprc_show(&show);

        let mut dpnis = Vec::with_capacity(dpni_ids.len());
        for id in dpni_ids {
            let obj = id.to_string();
            let info = parse::parse_dpni_info(&self.runner.run(&["dpni", "info", &obj])?);
            dpnis.push(ObservedDpni {
                id,
                connected_to: info.endpoint,
                mac: info.mac,
                // netdev is a kernel concern; the shell enriches it via KernelControl.
                netdev: None,
                attributes: BTreeMap::new(),
            });
        }

        let mut dpmacs = Vec::with_capacity(dpmac_ids.len());
        for id in dpmac_ids {
            let obj = id.to_string();
            let info = parse::parse_dpmac_info(&self.runner.run(&["dpmac", "info", &obj])?);
            dpmacs.push(ObservedDpmac {
                id,
                link_type: info.link_type,
                mac: info.mac,
            });
        }

        Ok(ObservedTopology { dpnis, dpmacs })
    }

    fn read_inventory(&self) -> Result<Inventory, Error> {
        // dpmac set: reuse the observe listing rather than re-deriving it.
        let show = self.runner.run(&["dprc", "show", &self.container])?;
        let (_dpnis, dpmac_ids) = parse::parse_dprc_show(&show);

        let mut dpmacs = BTreeMap::new();
        for id in dpmac_ids {
            let raw =
                parse::parse_dpmac_offer(&self.runner.run(&["dpmac", "info", &id.to_string()])?);
            // DPMAC-I3: these attributes are immutable; a missing one is a parse
            // failure, not a default — an invented rate/media would misdescribe the
            // port to `compile`.
            let (Some(max_rate), Some(eth_if), Some(link_type)) =
                (raw.max_rate, raw.eth_if, raw.link_type)
            else {
                return Err(Error::Parse(format!(
                    "dpmac info {id}: missing max_rate/eth_if/link_type"
                )));
            };
            let avail = reserved_reason(id).map_or(Availability::Free, |why| {
                Availability::Reserved(why.to_owned())
            });
            dpmacs.insert(
                id,
                DpmacOffer {
                    id,
                    max_rate,
                    eth_if,
                    link_type,
                    avail,
                },
            );
        }

        // Ceilings: the MC-level resource listing, mapped per ADR-0011's policy.
        let res = self
            .runner
            .run(&["dprc", "show", MC_GLOBAL, "--resources"])?;
        let pools = parse::parse_resources(&res);
        let ceilings = DERIVED_FAMILIES
            .iter()
            .map(|&f| (f, ceiling_of(f, &pools)))
            .collect();

        Ok(Inventory {
            // ADR-0012: the kernel dataplane draws one dpio per online CPU; the crate
            // already sizes its pools by this count.
            cpus: u32::try_from(self.cores).unwrap_or(u32::MAX),
            dpmacs,
            // GAP (design D2, ADR-0001 §4): DPL ownership is not observable through
            // `restool` without persisted state — `dprc show` cannot distinguish a
            // DPL-created object (e.g. dpni.0) from one this control plane made. The
            // honest subset leaves `foreign` empty rather than guess; recovering it
            // needs a DPL/ownership signal Parcel A does not have.
            foreign: BTreeMap::new(),
            ceilings,
        })
    }

    fn create_dpni(&self) -> Result<DpniId, Error> {
        // A DPNI is not usable alone: `dpaa2-eth` allocates a DPBP, a DPMCP, and one
        // DPCON per queue from the container's pool at probe, backed by a per-core
        // DPIO pool. These must exist first (mirrors `ls-addni`'s create_dpni).
        // `ensure_dpio` tops up a shared, container-wide idempotent pool and is left
        // outside the transactional chain (it's always safe to retry from partial
        // state); the per-DPNI deps below are provisioned and, on any failure
        // (including the `dpni create` itself), rolled back together so a failed
        // attempt never leaves orphaned private objects plugged in the container.
        self.ensure_dpio()?;
        self.provision_chain(&self.dpni_dep_steps(), |this| {
            let queues = format!("--num-queues={}", this.queues);
            let out = this.runner.run(&["--script", "dpni", "create", &queues])?;
            parse::parse_dpni_object_id(&out).ok_or_else(|| {
                Error::Parse(format!("could not parse created dpni id from `{out}`"))
            })
        })
        // The DPNI is plugged (triggering the driver probe) in `connect()`, after
        // any actuate-mode `set_mac` — matching the design recipe's ordering.
    }

    fn connect(&self, dpni: DpniId, dpmac: DpmacId) -> Result<(), Error> {
        // Plug the DPNI in here, not at create time, so actuate-mode `set_mac`
        // always runs against an unplugged DPNI (design recipe: create -> [set-mac]
        // -> plug+connect -> sync).
        self.assign_plugged(&dpni.to_string())?;
        self.runner.run(&[
            "dprc",
            "connect",
            &self.container,
            &format!("--endpoint1={dpni}"),
            &format!("--endpoint2={dpmac}"),
        ])?;
        self.sync()
    }

    fn set_mac(&self, dpni: DpniId, mac: dpaa2_api::MacAddr) -> Result<(), Error> {
        // MAC actuation uses `dpni update --mac-addr` (as `ls-addni` does). Phase 1
        // defaults to assert mode, so this is reached only when a port opts into
        // actuate; it always runs before the DPNI is plugged, since plugging is now
        // deferred to `connect()`.
        self.runner.run(&[
            "dpni",
            "update",
            &dpni.to_string(),
            &format!("--mac-addr={mac}"),
        ])?;
        self.sync()
    }

    fn disconnect(&self, dpni: DpniId) -> Result<(), Error> {
        self.runner.run(&[
            "dprc",
            "disconnect",
            &self.container,
            &format!("--endpoint1={dpni}"),
        ])?;
        self.sync()
    }

    fn destroy(&self, dpni: DpniId) -> Result<(), Error> {
        self.runner.run(&["dpni", "destroy", &dpni.to_string()])?;
        self.sync()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A [`Runner`] that replays canned output keyed by the joined argument line.
    struct CannedRunner(std::collections::HashMap<String, String>);

    impl CannedRunner {
        fn new(pairs: &[(&str, &str)]) -> Self {
            Self(
                pairs
                    .iter()
                    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                    .collect(),
            )
        }
    }

    impl Runner for CannedRunner {
        fn run(&self, args: &[&str]) -> Result<String, Error> {
            self.0
                .get(&args.join(" "))
                .cloned()
                .ok_or_else(|| Error::Backend(format!("no canned output for `{}`", args.join(" "))))
        }
    }

    // dpmac info bodies shaped after models/board/baselines/reference.json.
    fn dpmac_info(rate: i64, eth: &str) -> String {
        format!(
            "DPMAC ethernet interface: {eth}\nDPMAC link type: DPMAC_LINK_TYPE_PHY\n\
             MAC address: 00:11:22:33:44:55\nmaximum supported rate {rate} Mbps\n"
        )
    }

    #[test]
    fn read_inventory_assembles_offers_availability_and_ceilings() {
        let show = "dpni.0\ndpmac.3\ndpmac.4\ndpmac.17\n";
        let info3 = dpmac_info(25_000, "DPMAC_ETH_IF_CAUI");
        let info4 = dpmac_info(25_000, "DPMAC_ETH_IF_CAUI");
        let info17 = dpmac_info(1000, "DPMAC_ETH_IF_RGMII");
        let resources = "bp 63\nmcp 203\nswp 49\nfq 1981\n";
        let runner = CannedRunner::new(&[
            ("dprc show dprc.1", show),
            ("dpmac info dpmac.3", &info3),
            ("dpmac info dpmac.4", &info4),
            ("dpmac info dpmac.17", &info17),
            ("dprc show mc.global --resources", resources),
        ]);
        let mc = RestoolMc::with_runner(runner, DEFAULT_CONTAINER).with_cores(16);

        let inv = mc.read_inventory().expect("inventory");

        assert_eq!(inv.cpus, 16);
        // The ADR-0003 matrix: dpmac.3 total-deny, dpmac.17 management, dpmac.4 free.
        assert!(matches!(
            inv.dpmacs[&DpmacId::new(3)].avail,
            Availability::Reserved(_)
        ));
        assert!(matches!(
            inv.dpmacs[&DpmacId::new(17)].avail,
            Availability::Reserved(_)
        ));
        assert_eq!(inv.dpmacs[&DpmacId::new(4)].avail, Availability::Free);
        assert_eq!(inv.dpmacs[&DpmacId::new(4)].max_rate, 25_000);

        // ADR-0011 per-family ceilings: dpbp counted, dpmcp/dpni observed, rest unknown.
        assert_eq!(inv.ceilings[&Family::Dpbp], Ceiling::Counted(63));
        assert!(matches!(
            inv.ceilings[&Family::Dpmcp],
            Ceiling::Observed { n: 203, .. }
        ));
        assert!(matches!(
            inv.ceilings[&Family::Dpni],
            Ceiling::Observed { n: 18, .. }
        ));
        assert_eq!(inv.ceilings[&Family::Dpio], Ceiling::Unknown);
        assert_eq!(inv.ceilings[&Family::Dpcon], Ceiling::Unknown);

        // GAP: DPL ownership is unobservable here, so no foreign objects appear.
        assert!(inv.foreign.is_empty());
    }
}
