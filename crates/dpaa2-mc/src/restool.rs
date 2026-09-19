//! The [`RestoolMc`] shim: the phase-1 [`McControl`] implementation.
//!
//! [`RestoolMc`] reproduces the exact `restool` v2.4 sequence from the design recipe
//! (`--script` create + plug-assign + connect + sync; observe via
//! `dprc show`/`dpni info`/`dpmac info`) behind the neutral trait, so a future ioctl
//! backend can replace it without touching the core. It introduces **no `unsafe`
//! code** and keeps the workspace `unsafe_code = "forbid"` lint intact (mc-backend
//! spec). netdev observation and driver binding live in [`SysfsKernel`](crate::SysfsKernel).

use std::collections::BTreeMap;

use dpaa2_api::contract::McControl;
use dpaa2_api::core::error::Error;
use dpaa2_api::core::family::{DERIVED_FAMILIES, Family};
use dpaa2_api::core::inventory::{Availability, Ceiling, DpmacOffer, Inventory};
use dpaa2_api::core::model::{
    DpmacId, DpniId, DprcId, ObjectRef, ObservedDpmac, ObservedDpni, ObservedTopology,
};
use dpaa2_api::core::types::ConstructName;
use dpaa2_api::families::dpni::{
    DpniCfg, DpniObservation, DpniOpt, FsEntries, MacFilterEntries, NumCeetmCh, NumCgs, NumOpr,
    NumQueues, NumTcs, OptionMask, QosEntries, RawEscape, VlanFilterEntries,
};
use dpaa2_api::families::dprc;
use dpaa2_api::intent::compiled::Container;
use dpaa2_api::plan::dprc as dprc_plan;

use crate::parse;
use crate::parse::RawDpniAttr;
use crate::runner::{RestoolRunner, Runner};

/// The default fsl-mc root container.
pub const DEFAULT_CONTAINER: &str = "dprc.1";

/// The MC's own container, aliased `mc.global` by restool; only `dprc show` (and
/// its `--resources` variant) is accepted against it (`dprc.md` mc.global section).
const MC_GLOBAL: &str = "mc.global";

/// The ADR-0003 §3 port safety matrix, transcribed once — the single Rust copy the
/// southbound consults (task 3.5, design D2; ADR-0002). A reserved dpmac may never anchor a
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

// Each named MC 10.39 flag's wire bit, from `restool/mc_v10/fsl_dpni.h` (lines ~75-142).
// Adapter policy stays in the southbound, not on the domain surface (ADR-0018); the
// wildcard-free match forces a new `DpniOpt` variant to state its bit before it compiles.
// The four unnamed header flags (0x20/0x400/0x800/0x1000) have no variant by design.
const fn dpni_opt_bit(opt: DpniOpt) -> u32 {
    match opt {
        DpniOpt::TxFrmRelease => 0x0001,
        DpniOpt::NoMacFilter => 0x0002,
        DpniOpt::HasPolicing => 0x0004,
        DpniOpt::SharedCongestion => 0x0008,
        DpniOpt::HasKeyMasking => 0x0010,
        DpniOpt::HasOpr => 0x0040,
        DpniOpt::OprPerTc => 0x0080,
        DpniOpt::SingleSender => 0x0100,
        DpniOpt::CustomCg => 0x0200,
        DpniOpt::StashingDis => 0x2000,
    }
}

/// Folds the typed option set into the one raw `u32` the shim emits (`--options=0x…`),
/// OR-ing each named flag's bit with every escape's raw value (`PFDR_IN_PEB`
/// `0x80000000`). The arithmetic is `u32` because the escape sets bit 31; restool parses
/// `--options` via `strtoull`, so hex text is accepted. The shim never emits an
/// option-name token — restool's token parsing is loose (dpni-typestate design Risks) —
/// so this computed mask is the single source.
fn dpni_options_mask(mask: &OptionMask) -> u32 {
    let mut raw = 0u32;
    for &flag in mask.flags() {
        raw |= dpni_opt_bit(flag);
    }
    for &escape in mask.escapes() {
        raw |= escape.raw_value();
    }
    raw
}

/// The inverse of [`dpni_options_mask`]: decodes a raw `dpni_attr.options` mask back into
/// the typed set. `0x80000000` maps to the `PFDR_IN_PEB` escape; every other set bit that
/// names no vocabulary flag makes this return `None` — an honest gap, because the typed
/// set cannot faithfully represent an unnamed bit (dpni-typestate design D2/D4). The four
/// unnamed header flags fall in that class by design.
fn decode_dpni_options(raw: u32) -> Option<OptionMask> {
    let mut mask = OptionMask::empty();
    let mut consumed = 0u32;
    for flag in DpniOpt::MC_VOCABULARY {
        let bit = dpni_opt_bit(flag);
        if raw & bit != 0 {
            mask = mask.with_flag(flag);
            consumed |= bit;
        }
    }
    if raw & RawEscape::PfdrInPeb.raw_value() != 0 {
        mask = mask.with_escape(RawEscape::PfdrInPeb);
        consumed |= RawEscape::PfdrInPeb.raw_value();
    }
    if raw & !consumed != 0 {
        return None;
    }
    Some(mask)
}

/// Maps a parsed `dpni_attr` block to the domain [`DpniObservation`]
/// (dpni-typestate task 4.1; dpni-typestate design D4). The asymmetric read-back is
/// mapped to domain names: `num_tx_tcs`
/// is the cfg-settable TC half that becomes `num_tcs` (the baseline's never-settable
/// `num_rx_tcs` law, `docs/baseline/dpni.md` "Never settable"), and `num_channels`
/// becomes `num_ceetm_ch`. Write-only `dist_key_size` has no read-back and is never
/// synthesized; the informational `num_rx_tcs` / `qos_key_size` / `fs_key_size` stay on
/// the raw struct and never enter the observation. Returns `None` on any missing or
/// out-of-envelope field, or an unnamed option bit — the honest gap.
fn map_dpni_observation(attr: &RawDpniAttr) -> Option<DpniObservation> {
    Some(DpniObservation {
        options: decode_dpni_options(attr.options)?,
        num_queues: NumQueues::new(attr.num_queues?).ok()?,
        num_tcs: NumTcs::new(attr.num_tx_tcs?).ok()?,
        mac_filter_entries: MacFilterEntries::new(attr.mac_entries?).ok()?,
        vlan_filter_entries: VlanFilterEntries::new(attr.vlan_entries?).ok()?,
        qos_entries: QosEntries::new(attr.qos_entries?).ok()?,
        fs_entries: FsEntries::new(attr.fs_entries?).ok()?,
        num_cgs: NumCgs::new(attr.num_cgs?).ok()?,
        num_ceetm_ch: NumCeetmCh::new(attr.num_channels?).ok()?,
        num_opr: NumOpr::new(attr.num_opr?).ok()?,
    })
}

/// Appends `flag=<v>` to `args` only when `v` is nonzero — 0 means "omit ⇒ MC default"
/// (DPNI-I7, `docs/baseline/dpni.md` "Option inventory": an omitted flag sends literal 0
/// and the MC applies its own default).
fn push_dpni_flag(args: &mut Vec<String>, flag: &str, v: u16) {
    if v != 0 {
        args.push(format!("{flag}={v}"));
    }
}

/// Builds the `restool --script dpni create …` argument vector from the typed create
/// block (dpni-typestate task 4.1). `--num-queues` always rides with the effective count
/// (`queues`, host-derived when the block is unsized); every other sizing field is
/// emitted only when nonzero (DPNI-I7). The flag spellings are verified against restool's
/// create parser (`restool/dpni_commands.c` lines 202-287): the new `--mac-filter-entries`
/// / `--vlan-filter-entries` spellings, and `--num-channels` for `num_ceetm_ch`. Options
/// are one computed raw mask (`--options=0x…`), emitted only when nonzero, never a name
/// token (dpni-typestate design Risks).
fn dpni_create_args(cfg: &DpniCfg, queues: usize) -> Vec<String> {
    let mut args = vec![
        "--script".to_owned(),
        "dpni".to_owned(),
        "create".to_owned(),
        format!("--num-queues={queues}"),
    ];
    push_dpni_flag(&mut args, "--num-tcs", cfg.num_tcs.get());
    push_dpni_flag(
        &mut args,
        "--mac-filter-entries",
        cfg.mac_filter_entries.get(),
    );
    push_dpni_flag(
        &mut args,
        "--vlan-filter-entries",
        cfg.vlan_filter_entries.get(),
    );
    push_dpni_flag(&mut args, "--qos-entries", cfg.qos_entries.get());
    push_dpni_flag(&mut args, "--fs-entries", cfg.fs_entries.get());
    push_dpni_flag(&mut args, "--num-cgs", cfg.num_cgs.get());
    push_dpni_flag(&mut args, "--dist-key-size", cfg.dist_key_size.get());
    push_dpni_flag(&mut args, "--num-channels", cfg.num_ceetm_ch.get());
    push_dpni_flag(&mut args, "--num-opr", cfg.num_opr.get());
    let mask = dpni_options_mask(&cfg.options);
    if mask != 0 {
        args.push(format!("--options=0x{mask:x}"));
    }
    args
}

/// Renders a child-DPRC option mask into the `--options=` argument for `dprc create`,
/// or `None` when it is the restool default. The default mask
/// (`SPAWN|ALLOC|OBJ_CREATE|IRQ_CFG`) is applied by restool when `--options` is omitted
/// (DPRC-I4; `docs/baseline/dprc.md` "create details"), so the common consumer-container
/// case issues no `--options` at all. A non-default mask renders the set bits as their
/// `DPRC_CFG_OPT_*` tokens; the lifecycle [`dprc::Options`] carries only the four
/// refusal-gating bits, which is exactly what the derivation varies.
fn render_options(options: dprc::Options) -> Option<String> {
    if options == dprc::Options::DEFAULT {
        return None;
    }
    // Same table the decoder reads (review M11; PASS3-F9): render the set bits' tokens.
    let bits: Vec<&str> = parse::OPTION_BITS
        .iter()
        .filter(|b| (b.get)(options))
        .map(|b| b.token)
        .collect();
    Some(format!("--options={}", bits.join(",")))
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

    /// Overrides the host-derived fallback queue count — used only when a `Create`
    /// carries the unsized marker (`num_queues == 0`); a compiled nonzero value is
    /// honored exactly and never overridden here. Clamped to the core count, mirroring
    /// `ls-addni` (`num_dpcons = min(num_queues, nproc)`).
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

    /// The number of private DPCONs a DPNI with `queues` transmit queues needs:
    /// `min(queues, cores)`, mirroring `ls-addni` (`num_dpcons = min(num_queues, nproc)`).
    fn num_dpcons(&self, queues: usize) -> usize {
        queues.clamp(1, self.cores)
    }

    /// `restool --script <type> create …` then plug the result into the container,
    /// stamping it with the owning construct's name. Returns the created object
    /// reference (e.g. `dpcon.5`). Every companion in a dpni's chain wears the same
    /// construct name as the dpni it serves, so the whole chain is readable in bare
    /// restool and — the name being in the declared set — recognized as ours next pass
    /// (ADR-0010 §4 as refined by ADR-0015 decisions 9 + 13). A stamp failure lets the
    /// chain roll back, so a half-labelled object is never left behind.
    fn create_and_plug(
        &self,
        create_args: &[&str],
        label: &ConstructName,
    ) -> Result<String, Error> {
        let out = self.run_verb(create_args)?;
        let obj = parse::parse_object_ref(&out)
            .ok_or_else(|| Error::Parse(format!("no object id in `{}`", out.trim())))?
            .to_owned();
        self.assign_plugged(&obj)?;
        self.stamp_label(&obj, label)?;
        Ok(obj)
    }

    /// `restool dprc set-label <obj> --label=<name>` (ADR-0010 §4; ADR-0015 decision 9):
    /// stamps the owning construct's name on `obj`. Board-verified to land even on a
    /// locked container (V-DPRC-3). The name is bounded to the MC's 15-char cap by
    /// ADR-0015 decision 13, so it is never truncated.
    fn stamp_label(&self, obj: &str, label: &ConstructName) -> Result<(), Error> {
        self.runner
            .run(&["dprc", "set-label", obj, &format!("--label={label}")])?;
        Ok(())
    }

    /// `restool dprc assign <container> --object=<obj> --plugged=1`.
    fn assign_plugged(&self, obj: &str) -> Result<(), Error> {
        self.run_verb(&[
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
    /// (`ls-addni` `create_dpio`). The pool is shared across ports; each object it
    /// tops up is stamped with `label`, the construct name of the port that triggered
    /// the top-up. A shared pool object created under one port's name stays ours for
    /// every port (its name is in the declared set), so ownership recognition never
    /// depends on which port grew the pool (ADR-0010 §4 refined by ADR-0015).
    fn ensure_dpio(&self, label: &ConstructName) -> Result<(), Error> {
        let show = self.run_verb(&["dprc", "show", &self.container])?;
        let existing = parse::count_objects(&show, "dpio");
        let container = format!("--container={}", self.container);
        for _ in existing..self.cores {
            let dpio = self.create_and_plug(
                &[
                    "--script",
                    "dpio",
                    "create",
                    "--channel-mode=DPIO_LOCAL_CHANNEL",
                    &container,
                    "--num-priorities=8",
                ],
                label,
            )?;
            // Each DPIO also needs a companion DPMCP.
            self.create_and_plug(&["--script", "dpmcp", "create", &container], label)?;
            tracing::debug!(%dpio, "provisioned dpio");
        }
        Ok(())
    }

    /// Data for a DPNI's private dependencies (one DPBP, one DPMCP, and
    /// `num_dpcons` DPCONs) — the objects `dpaa2-eth` allocates at probe. Without
    /// them the driver fails with "No more resources of type dpcon left". A plain
    /// data builder; [`Self::provision_chain`] does the actual creation and any
    /// rollback.
    fn dpni_dep_steps(&self, queues: usize) -> Vec<ProvisionStep> {
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
        for _ in 0..self.num_dpcons(queues) {
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
        label: &ConstructName,
        then: impl FnOnce(&Self) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut created: Vec<(&'static str, String)> = Vec::new();
        for step in steps {
            let args: Vec<&str> = step.args.iter().map(String::as_str).collect();
            match self.create_and_plug(&args, label) {
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
            if let Err(e) = self.run_verb(&[kind, "destroy", obj]) {
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
        self.run_verb(&["dprc", "sync"])?;
        Ok(())
    }

    /// The single exit every [`McControl`] verb funnels through: it runs one restool
    /// invocation and classifies a refusal into a typed [`Error`] (design D4, bead dpaa2-controlplane-cd3). This is
    /// the ONLY place the shim reads the raw [`Runner`]; [`Runner::run`] stays raw
    /// transport and no verb calls it directly, so a new verb author inherits one error
    /// path, not a choice between two (review M10; PASS3-F7).
    ///
    /// On a non-zero exit the shim reports the *shape* restool returned, never a bare
    /// string: an MC firmware refusal carries a status token (`(0x…)`,
    /// `docs/baseline/dprc.md` unknown-register #3) and surfaces as [`Error::McStatus`]
    /// with the raw byte for the core to judge; a restool client-side guard fires
    /// before any MC command and so carries no status, surfacing as
    /// [`Error::RestoolGuard`] with the verbatim message (DPRC-I3). Signal death
    /// (`code: None`, [`RunOutcome`](crate::runner::RunOutcome)) is neither refusal — it
    /// is a transport failure, reported [`Error::Backend`] (review M10; PASS3-F8). The
    /// adapter reports; classifying *which* refusal a status means lives core-side
    /// (`dprc::Refusal`/`dprc_plan::attribute_mc`).
    fn run_verb(&self, args: &[&str]) -> Result<String, Error> {
        let out = self.runner.run_capture(args)?;
        if out.code == Some(0) {
            return Ok(out.stdout);
        }
        // No exit code means a signal killed restool: transport failure, not a refusal (review M10; PASS3-F8).
        if out.code.is_none() {
            return Err(Error::Backend(format!(
                "restool {} died by signal: {}",
                args.join(" "),
                out.stderr.trim()
            )));
        }
        // restool prints its diagnostic to stderr; fall back to stdout if empty.
        let diag = if out.stderr.trim().is_empty() {
            out.stdout.as_str()
        } else {
            out.stderr.as_str()
        };
        parse::parse_mc_status(diag).map_or_else(
            || {
                Err(Error::RestoolGuard {
                    detail: diag.trim().to_owned(),
                })
            },
            |status| Err(Error::McStatus { status }),
        )
    }
}

impl<R: Runner> McControl for RestoolMc<R> {
    fn observe(&self) -> Result<ObservedTopology, Error> {
        let show = self.run_verb(&["dprc", "show", &self.container])?;
        let (dpni_ids, dpmac_ids) = parse::parse_dprc_show(&show);

        // The label column is the identity seam the matcher leans on (ADR-0015
        // decisions 9-10, 13). It rides on the `dprc show` row, not `dpni info`, so
        // map dpni id -> label from the rows already read; an empty label is drift.
        let labels: BTreeMap<DpniId, ConstructName> = parse::parse_dprc_rows(&show)
            .into_iter()
            .filter(|r| r.family == Family::Dpni && !r.label.is_empty())
            .map(|r| (DpniId::from(r.num), r.label))
            .collect();

        let mut dpnis = Vec::with_capacity(dpni_ids.len());
        for id in dpni_ids {
            let obj = id.to_string();
            let info = parse::parse_dpni_info(&self.run_verb(&["dpni", "info", &obj])?);
            dpnis.push(ObservedDpni {
                id,
                label: labels.get(&id).cloned(),
                connected_to: info.endpoint,
                mac: info.mac,
                // netdev is a kernel concern; the shell enriches it via KernelControl.
                netdev: None,
                attributes: BTreeMap::new(),
                // `None` (honest gap) when the attr block was absent or carried an unnamed
                // bit (dpni-typestate task 4.1; design D4).
                cfg_observation: info.attr.as_ref().and_then(map_dpni_observation),
            });
        }

        let mut dpmacs = Vec::with_capacity(dpmac_ids.len());
        for id in dpmac_ids {
            let obj = id.to_string();
            let info = parse::parse_dpmac_info(&self.run_verb(&["dpmac", "info", &obj])?);
            dpmacs.push(ObservedDpmac {
                id,
                link_type: info.link_type,
                mac: info.mac,
            });
        }

        Ok(ObservedTopology { dpnis, dpmacs })
    }

    fn observe_containers(&self) -> Result<BTreeMap<DprcId, dprc_plan::ObservedContainer>, Error> {
        // Re-observe the root's child containers from one `dprc show` (DPRC-I6: the
        // convergence verdict re-queries, it never trusts `sync`). Each `dprc.N` child
        // row carries its name-keyed label; the row is a child of the root, so its
        // placement is [`Container::Root`].
        let show = self.run_verb(&["dprc", "show", &self.container])?;
        let mut containers = BTreeMap::new();
        for row in parse::parse_dprc_rows(&show) {
            if row.family != Family::Dprc {
                continue;
            }
            let obj = format!("dprc.{}", row.num);

            // Options are read back from the child's own `dprc info`; a `None` parse
            // (no options line, a refused/malformed read) is an error, never the default.
            let info = self.run_verb(&["dprc", "info", &obj])?;
            let options = parse::parse_dprc_info(&info)
                .ok_or_else(|| Error::Parse(format!("no dprc options in `dprc info {obj}`")))?;

            // Residents are the child's own `dprc show` rows; a grandchild DPRC is a
            // container, not a resident, in the model's World, so it is skipped.
            let child_show = self.run_verb(&["dprc", "show", &obj])?;
            let mut residents = BTreeMap::new();
            for r in parse::parse_dprc_rows(&child_show) {
                if r.family == Family::Dprc {
                    continue;
                }
                // Origin unobservable through restool: reported None, never invented; keyed by ObjectRef so families never collide (review M2/M1; PASS3-F2/F14).
                residents.insert(
                    ObjectRef::new(r.family, r.num),
                    dprc::ObservedResident {
                        origin: None,
                        plugged: r.plugged,
                    },
                );
            }

            // The core judges Created-vs-Populated from the residents the shim read (review M2; PASS3-F1).
            let state = dprc::ContainerState::classify(&residents);

            containers.insert(
                DprcId::from(row.num),
                dprc_plan::ObservedContainer {
                    state,
                    options,
                    label: row.label,
                    placement: Container::Root,
                    residents,
                },
            );
        }
        Ok(containers)
    }

    fn read_inventory(&self) -> Result<Inventory, Error> {
        // One `dprc show`, read as rows so the label column (ADR-0010 Consequences)
        // feeds both the label map and each dpmac's availability.
        let show = self.run_verb(&["dprc", "show", &self.container])?;
        let rows = parse::parse_dprc_rows(&show);

        // The backend reports, it never judges (ADR-0010 §4 as refined by ADR-0015):
        // ownership is decided against the declared-name set on the api side, not by a
        // fixed tag here. Every non-dpmac row reports its raw label verbatim (empty
        // string included) — our own objects are no longer filtered out, because there
        // is no self-tag to recognize them by. Dpmacs are excluded: they are DPC-born
        // hardware anchors with no label of their own, and their availability is
        // derived below from the dpni they anchor.
        let mut labels = BTreeMap::new();
        for row in &rows {
            if row.family == Family::Dpmac {
                continue;
            }
            // `Inventory.labels` (dpaa2-api) keys raw String labels; cross the
            // ConstructName back to its string at this boundary.
            labels.insert((row.family, row.num), row.label.as_str().to_owned());
        }

        let mut dpmacs = BTreeMap::new();
        for row in &rows {
            if row.family != Family::Dpmac {
                continue;
            }
            let id = DpmacId::from(row.num);
            let raw =
                parse::parse_dpmac_offer(&self.run_verb(&["dpmac", "info", &id.to_string()])?);
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
            // Availability precedence: (1) the ADR-0003 §3 safety matrix is strongest
            // — a reserved dpmac is Reserved even when it anchors a labelled dpni;
            // (2) else if it anchors a dpni, it reports that dpni's raw label as the
            // owner verbatim (empty included); the api side judges it — empty ⇒ the DPL
            // resident "dpl", a declared name ⇒ Free (ADR-0010 §4 refined by ADR-0015,
            // Inventory::availability_of); (3) else Free.
            let avail = if let Some(why) = reserved_reason(id) {
                Availability::Reserved(why.to_owned())
            } else if let Some(label) = raw
                .endpoint
                .and_then(|ep| labels.get(&(Family::Dpni, ep.into_inner())))
            {
                Availability::Foreign(label.clone())
            } else {
                Availability::Free
            };
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
            // ADR-0010 §4 closes design D2's GAP: DPL ownership *is* observable — the
            // label column of `dprc show` is the signal. The raw column is reported
            // verbatim; the api side judges each label against the declared-name set
            // (empty ⇒ "dpl", a declared name ⇒ ours, anything else ⇒ that owner).
            labels,
            ceilings,
        })
    }

    fn create_dpni(&self, label: &ConstructName, cfg: &DpniCfg) -> Result<DpniId, Error> {
        // The compiled block is rendered verbatim (dpni-typestate task 4.1); only
        // `num_queues == 0` keeps the host-derived fallback (`self.queues`), which the
        // private DPCON count follows (`ls-addni` min(num_queues, nproc)). `root_container`
        // does not retarget the container here — the shim already operates in its own
        // (dpni-typestate design D1); placement is the assign/move tile's concern.
        let queues = if cfg.num_queues.get() == 0 {
            self.queues
        } else {
            usize::from(cfg.num_queues.get())
        };
        // A DPNI is not usable alone: `dpaa2-eth` allocates a DPBP, a DPMCP, and one
        // DPCON per queue from the container's pool at probe, backed by a per-core
        // DPIO pool. These must exist first (mirrors `ls-addni`'s create_dpni).
        // `ensure_dpio` tops up a shared, container-wide idempotent pool and is left
        // outside the transactional chain (it's always safe to retry from partial
        // state); the per-DPNI deps below are provisioned and, on any failure
        // (including the `dpni create` itself), rolled back together so a failed
        // attempt never leaves orphaned private objects plugged in the container. The
        // whole chain wears `label`, the owning construct's name (ADR-0015 decision 9).
        self.ensure_dpio(label)?;
        self.provision_chain(&self.dpni_dep_steps(queues), label, |this| {
            let create_args = dpni_create_args(cfg, queues);
            let arg_refs: Vec<&str> = create_args.iter().map(String::as_str).collect();
            let out = this.run_verb(&arg_refs)?;
            let id = parse::parse_dpni_object_id(&out).ok_or_else(|| {
                Error::Parse(format!("could not parse created dpni id from `{out}`"))
            })?;
            // Stamp the construct name at create (pre-plug is fine — V-DPRC-3 shows
            // labels land regardless of plug state), so no read-back window ever shows
            // the object unlabelled (ADR-0010 §4 ABA guard). A failure rolls the chain
            // back.
            this.stamp_label(&id.to_string(), label)?;
            Ok(id)
        })
        // The DPNI is plugged (triggering the driver probe) in `connect()`, after
        // any actuate-mode `set_mac` — matching the design recipe's ordering.
    }

    fn connect(&self, dpni: DpniId, dpmac: DpmacId) -> Result<(), Error> {
        // Plug the DPNI in here, not at create time, so actuate-mode `set_mac`
        // always runs against an unplugged DPNI (design recipe: create -> [set-mac]
        // -> plug+connect -> sync).
        self.assign_plugged(&dpni.to_string())?;
        self.run_verb(&[
            "dprc",
            "connect",
            &self.container,
            &format!("--endpoint1={dpni}"),
            &format!("--endpoint2={dpmac}"),
        ])?;
        self.sync()
    }

    fn set_mac(&self, dpni: DpniId, mac: dpaa2_api::core::model::MacAddr) -> Result<(), Error> {
        // MAC actuation uses `dpni update --mac-addr` (as `ls-addni` does). Phase 1
        // defaults to assert mode, so this is reached only when a port opts into
        // actuate; it always runs before the DPNI is plugged, since plugging is now
        // deferred to `connect()`.
        self.run_verb(&[
            "dpni",
            "update",
            &dpni.to_string(),
            &format!("--mac-addr={mac}"),
        ])?;
        self.sync()
    }

    fn set_label(&self, dpni: DpniId, label: &ConstructName) -> Result<(), Error> {
        // `dprc set-label dpni.N --label=<name>`, decision 9's repair verb (ADR-0015):
        // rewrites the construct name as the object's MC label to repair drift or
        // realize a rename on a standing object. The create path already stamps the
        // name via [`Self::stamp_label`]; this is the level-triggered repair.
        self.stamp_label(&dpni.to_string(), label)?;
        self.sync()
    }

    fn disconnect(&self, dpni: DpniId) -> Result<(), Error> {
        self.run_verb(&[
            "dprc",
            "disconnect",
            &self.container,
            &format!("--endpoint1={dpni}"),
        ])?;
        self.sync()
    }

    fn destroy(&self, dpni: DpniId) -> Result<(), Error> {
        self.run_verb(&["dpni", "destroy", &dpni.to_string()])?;
        self.sync()
    }

    fn dprc_create(
        &self,
        parent: DprcId,
        options: dprc::Options,
        label: &ConstructName,
    ) -> Result<DprcId, Error> {
        // `dprc create <parent> [--options] --label=<name>`. The default mask omits
        // `--options` (DPRC-I4). No plug follows: a created DPRC reads back unplugged
        // and restool cannot plug a DPRC (V-POOL-1 rev 2), so the create is the whole
        // mutation. The created id is read straight back from restool's echo, the
        // handle the caller re-observes by (DPRC-I6: re-observe, never trust `sync`).
        let parent = parent.to_string();
        let label = format!("--label={label}");
        let options = render_options(options);
        let mut args: Vec<&str> = vec!["dprc", "create", &parent];
        if let Some(opt) = options.as_deref() {
            args.push(opt);
        }
        args.push(&label);
        let out = self.run_verb(&args)?;
        parse::parse_dprc_id(&out)
            .ok_or_else(|| Error::Parse(format!("no container id in `{}`", out.trim())))
    }

    fn dprc_destroy(&self, container: DprcId) -> Result<(), Error> {
        self.run_verb(&["dprc", "destroy", &container.to_string()])?;
        Ok(())
    }

    fn dprc_assign(
        &self,
        container: DprcId,
        object: ObjectRef,
        child: Option<DprcId>,
        plugged: Option<bool>,
    ) -> Result<(), Error> {
        // One `dprc assign` bears both faces: `--child` re-parents, `--plugged` sets
        // driver-binding. A plugged object cannot be moved — restool refuses that
        // client-side (DPRC-I3), surfaced by `run_verb` as `Error::RestoolGuard`.
        let container = container.to_string();
        let mut args: Vec<String> = vec![
            "dprc".into(),
            "assign".into(),
            container,
            format!("--object={object}"),
        ];
        if let Some(child) = child {
            args.push(format!("--child={child}"));
        }
        if let Some(plugged) = plugged {
            args.push(format!("--plugged={}", u8::from(plugged)));
        }
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        self.run_verb(&args)?;
        Ok(())
    }

    fn dprc_unassign(&self, parent: DprcId, child: DprcId, object: ObjectRef) -> Result<(), Error> {
        self.run_verb(&[
            "dprc",
            "unassign",
            &parent.to_string(),
            &format!("--child={child}"),
            &format!("--object={object}"),
        ])?;
        Ok(())
    }

    fn dprc_set_label(&self, container: DprcId, label: &ConstructName) -> Result<(), Error> {
        self.run_verb(&[
            "dprc",
            "set-label",
            &container.to_string(),
            &format!("--label={label}"),
        ])?;
        Ok(())
    }

    fn dprc_set_locked(&self, child: DprcId, locked: bool) -> Result<(), Error> {
        self.run_verb(&[
            "dprc",
            "set-locked",
            &child.to_string(),
            &format!("--locked={}", u8::from(locked)),
        ])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use dpaa2_api::core::model::MacAddr;
    use dpaa2_api::families::dpni::Profile;

    use crate::runner::RunOutcome;

    use super::*;

    /// A [`Runner`] that replays a canned [`RunOutcome`] keyed by the joined argument
    /// line and records every call, so a verb's exact command and a refusal's typed
    /// shape can both be asserted with no board (the transcript-test idiom).
    struct ScriptedRunner {
        outcomes: HashMap<String, RunOutcome>,
        calls: RefCell<Vec<Vec<String>>>,
    }

    /// A success outcome carrying `stdout`.
    fn ok(stdout: &str) -> RunOutcome {
        RunOutcome {
            stdout: stdout.to_owned(),
            stderr: String::new(),
            code: Some(0),
        }
    }

    /// A refusal outcome: non-zero exit with `stderr` (what restool prints on refusal).
    fn refused(stderr: &str) -> RunOutcome {
        RunOutcome {
            stdout: String::new(),
            stderr: stderr.to_owned(),
            code: Some(1),
        }
    }

    impl ScriptedRunner {
        fn new(pairs: Vec<(&str, RunOutcome)>) -> Self {
            Self {
                outcomes: pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
                calls: RefCell::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.borrow().clone()
        }

        /// A read-only script: each command maps to a success `stdout`. Subsumes the old
        /// `CannedRunner` — a `ScriptedRunner` whose every outcome is `ok()` (review M11; PASS3-F10).
        fn canned(pairs: &[(&str, &str)]) -> Self {
            Self::new(pairs.iter().map(|&(k, v)| (k, ok(v))).collect())
        }
    }

    impl Runner for ScriptedRunner {
        fn run(&self, args: &[&str]) -> Result<String, Error> {
            let out = self.run_capture(args)?;
            if out.code == Some(0) {
                Ok(out.stdout)
            } else {
                Err(Error::Backend(out.stderr))
            }
        }

        fn run_capture(&self, args: &[&str]) -> Result<RunOutcome, Error> {
            self.calls
                .borrow_mut()
                .push(args.iter().map(|s| (*s).to_owned()).collect());
            self.outcomes.get(&args.join(" ")).cloned().ok_or_else(|| {
                Error::Backend(format!("no scripted outcome for `{}`", args.join(" ")))
            })
        }
    }

    // ---- dprc verb surface (dprc-encapsulation task 3.1) ----

    #[test]
    fn dprc_create_returns_child_id_and_reads_back_unplugged() {
        // The create scenario contract: the child's id comes back for re-observation,
        // and the create is the *whole* mutation — no plug follows (a created DPRC is
        // unplugged, V-POOL-1 rev 2). Default options ⇒ no `--options` (DPRC-I4).
        let runner =
            ScriptedRunner::new(vec![("dprc create dprc.1 --label=scratch", ok("dprc.3\n"))]);
        let mc = RestoolMc::with_runner(runner, DEFAULT_CONTAINER);

        let id = mc
            .dprc_create(
                DprcId::new(1),
                dprc::Options::DEFAULT,
                &ConstructName::from("scratch"),
            )
            .expect("create");

        assert_eq!(id, DprcId::new(3));
        // Exactly one command: no plug/assign, so the child reads back unplugged.
        assert_eq!(mc.runner().calls().len(), 1);
    }

    #[test]
    fn dprc_create_renders_nondefault_options() {
        // A non-default mask (topology-changes added) renders an explicit `--options=`.
        let options = dprc::Options {
            topology_changes: true,
            ..dprc::Options::DEFAULT
        };
        let cmd = "dprc create dprc.1 \
            --options=DPRC_CFG_OPT_SPAWN_ALLOWED,DPRC_CFG_OPT_ALLOC_ALLOWED,\
            DPRC_CFG_OPT_OBJ_CREATE_ALLOWED,DPRC_CFG_OPT_TOPOLOGY_CHANGES_ALLOWED \
            --label=scratch"
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let runner = ScriptedRunner::new(vec![(cmd.as_str(), ok("dprc.4\n"))]);
        let mc = RestoolMc::with_runner(runner, DEFAULT_CONTAINER);

        let id = mc
            .dprc_create(DprcId::new(1), options, &ConstructName::from("scratch"))
            .expect("create with options");
        assert_eq!(id, DprcId::new(4));
    }

    #[test]
    fn observe_containers_reads_real_options_and_empty_child_is_created() {
        // Each child DPRC row becomes an ObservedContainer; the info here is the
        // DPRC-I4 default mask and an empty child reads the Created face.
        let root = dprc_show(&[
            "dpni.0                          plugged",
            "dprc.2          router          unplugged",
            "dpmac.7                         plugged",
        ]);
        let info = parse::dprc_info(&[
            "DPRC_CFG_OPT_SPAWN_ALLOWED",
            "DPRC_CFG_OPT_ALLOC_ALLOWED",
            "DPRC_CFG_OPT_OBJ_CREATE_ALLOWED",
            "DPRC_CFG_OPT_IRQ_CFG_ALLOWED",
        ]);
        let child = dprc_show(&[]);
        let runner = ScriptedRunner::canned(&[
            ("dprc show dprc.1", &root),
            ("dprc info dprc.2", &info),
            ("dprc show dprc.2", &child),
        ]);
        let mc = RestoolMc::with_runner(runner, DEFAULT_CONTAINER);

        let seen = mc.observe_containers().expect("observe containers");
        assert_eq!(seen.len(), 1);
        let c = &seen[&DprcId::new(2)];
        assert_eq!(c.label, ConstructName::from("router"));
        assert_eq!(c.placement, Container::Root);
        assert_eq!(c.options, dprc::Options::DEFAULT);
        assert_eq!(c.state.name(), "Created");
        assert!(c.residents.is_empty());
    }

    #[test]
    fn observe_containers_reads_a_nondefault_mask() {
        let root = dprc_show(&["dprc.2          router          unplugged"]);
        let info = parse::dprc_info(&[
            "DPRC_CFG_OPT_SPAWN_ALLOWED",
            "DPRC_CFG_OPT_ALLOC_ALLOWED",
            "DPRC_CFG_OPT_OBJ_CREATE_ALLOWED",
            "DPRC_CFG_OPT_TOPOLOGY_CHANGES_ALLOWED",
        ]);
        let child = dprc_show(&[]);
        let runner = ScriptedRunner::canned(&[
            ("dprc show dprc.1", &root),
            ("dprc info dprc.2", &info),
            ("dprc show dprc.2", &child),
        ]);
        let mc = RestoolMc::with_runner(runner, DEFAULT_CONTAINER);

        let c = mc.observe_containers().expect("observe")[&DprcId::new(2)].clone();
        assert!(c.options.topology_changes);
        assert_ne!(c.options, dprc::Options::DEFAULT);
    }

    #[test]
    fn observe_containers_populated_child_lists_its_residents() {
        let root = dprc_show(&["dprc.2          router          unplugged"]);
        let info = parse::dprc_info(&[
            "DPRC_CFG_OPT_SPAWN_ALLOWED",
            "DPRC_CFG_OPT_ALLOC_ALLOWED",
            "DPRC_CFG_OPT_OBJ_CREATE_ALLOWED",
            "DPRC_CFG_OPT_IRQ_CFG_ALLOWED",
        ]);
        let child = dprc_show(&[
            "dpni.5          wan0            plugged",
            "dpbp.0                          unplugged",
            "dprc.9          grandchild      unplugged",
        ]);
        let runner = ScriptedRunner::canned(&[
            ("dprc show dprc.1", &root),
            ("dprc info dprc.2", &info),
            ("dprc show dprc.2", &child),
        ]);
        let mc = RestoolMc::with_runner(runner, DEFAULT_CONTAINER);

        let c = mc.observe_containers().expect("observe")[&DprcId::new(2)].clone();
        assert_eq!(c.state.name(), "Populated");
        // Only dpni.5 and dpbp.0 are residents (grandchild dprc.9 skipped), keyed by ObjectRef (review M1; PASS3-F14).
        assert_eq!(c.residents.len(), 2);
        assert!(!c.residents.contains_key(&ObjectRef::new(Family::Dprc, 9)));
        let dpni = &c.residents[&ObjectRef::new(Family::Dpni, 5)];
        assert_eq!(dpni.origin, None);
        assert!(dpni.plugged);
        assert!(!c.residents[&ObjectRef::new(Family::Dpbp, 0)].plugged);
    }

    #[test]
    fn observe_containers_same_ordinal_plugged_and_unplugged_both_plan_unplug() {
        // PASS3-F14 producer half (review M1/M2): same-ordinal residents of two families both count; the plugged one plans its unplug.
        let root = dprc_show(&["dprc.2          router          unplugged"]);
        let info = parse::dprc_info(&[
            "DPRC_CFG_OPT_SPAWN_ALLOWED",
            "DPRC_CFG_OPT_ALLOC_ALLOWED",
            "DPRC_CFG_OPT_OBJ_CREATE_ALLOWED",
        ]);
        let child = dprc_show(&[
            "dpbp.0                          plugged",
            "dpmcp.0                         unplugged",
        ]);
        let runner = ScriptedRunner::canned(&[
            ("dprc show dprc.1", &root),
            ("dprc info dprc.2", &info),
            ("dprc show dprc.2", &child),
        ]);
        let mc = RestoolMc::with_runner(runner, DEFAULT_CONTAINER);

        let c = mc.observe_containers().expect("observe")[&DprcId::new(2)].clone();
        assert_eq!(c.residents.len(), 2, "dpbp.0 and dpmcp.0 both count");
        let dpbp0 = ObjectRef::new(Family::Dpbp, 0);
        assert_eq!(c.residents[&dpbp0].origin, None);
        let plan = dprc_plan::plan_teardown(&c);
        assert!(
            plan.steps
                .contains(&dprc_plan::ContainerStep::UnplugResident { object: dpbp0 }),
            "the plugged dpbp.0 is unplugged by its ObjectRef before destroy",
        );
    }

    #[test]
    fn observe_containers_errors_on_a_refused_info_read() {
        let root = dprc_show(&["dprc.2          router          unplugged"]);
        let runner = ScriptedRunner::canned(&[
            ("dprc show dprc.1", &root),
            ("dprc info dprc.2", "container id: 2\nicid: 27\n"),
        ]);
        let mc = RestoolMc::with_runner(runner, DEFAULT_CONTAINER);
        match mc.observe_containers().expect_err("refused info") {
            Error::Parse(msg) => assert!(msg.contains("dprc.2"), "{msg}"),
            other => panic!("expected Parse, got {other:?}"),
        }
    }

    #[test]
    fn dprc_destroy_issues_destroy() {
        let runner = ScriptedRunner::new(vec![("dprc destroy dprc.2", ok(""))]);
        let mc = RestoolMc::with_runner(runner, DEFAULT_CONTAINER);
        mc.dprc_destroy(DprcId::new(2)).expect("destroy");
        assert_eq!(mc.runner().calls()[0], vec!["dprc", "destroy", "dprc.2"]);
    }

    #[test]
    fn dprc_assign_places_child_and_sets_plugged() {
        let runner = ScriptedRunner::new(vec![(
            "dprc assign dprc.1 --object=dpbp.0 --child=dprc.2 --plugged=1",
            ok(""),
        )]);
        let mc = RestoolMc::with_runner(runner, DEFAULT_CONTAINER);
        mc.dprc_assign(
            DprcId::new(1),
            ObjectRef::new(Family::Dpbp, 0),
            Some(DprcId::new(2)),
            Some(true),
        )
        .expect("assign");
        assert_eq!(
            mc.runner().calls()[0],
            vec![
                "dprc",
                "assign",
                "dprc.1",
                "--object=dpbp.0",
                "--child=dprc.2",
                "--plugged=1"
            ]
        );
    }

    #[test]
    fn dprc_unassign_moves_object_up_to_parent() {
        let runner = ScriptedRunner::new(vec![(
            "dprc unassign dprc.1 --child=dprc.2 --object=dpbp.0",
            ok(""),
        )]);
        let mc = RestoolMc::with_runner(runner, DEFAULT_CONTAINER);
        mc.dprc_unassign(
            DprcId::new(1),
            DprcId::new(2),
            ObjectRef::new(Family::Dpbp, 0),
        )
        .expect("unassign");
        assert_eq!(
            mc.runner().calls()[0],
            vec![
                "dprc",
                "unassign",
                "dprc.1",
                "--child=dprc.2",
                "--object=dpbp.0"
            ]
        );
    }

    #[test]
    fn dprc_set_label_rewrites_the_container_label() {
        let runner = ScriptedRunner::new(vec![("dprc set-label dprc.2 --label=scratch", ok(""))]);
        let mc = RestoolMc::with_runner(runner, DEFAULT_CONTAINER);
        mc.dprc_set_label(DprcId::new(2), &ConstructName::from("scratch"))
            .expect("set-label");
        assert_eq!(
            mc.runner().calls()[0],
            vec!["dprc", "set-label", "dprc.2", "--label=scratch"]
        );
    }

    #[test]
    fn dprc_set_locked_locks_the_hierarchy() {
        let runner = ScriptedRunner::new(vec![("dprc set-locked dprc.2 --locked=1", ok(""))]);
        let mc = RestoolMc::with_runner(runner, DEFAULT_CONTAINER);
        mc.dprc_set_locked(DprcId::new(2), true)
            .expect("set-locked");
        assert_eq!(
            mc.runner().calls()[0],
            vec!["dprc", "set-locked", "dprc.2", "--locked=1"]
        );
    }

    #[test]
    fn mc_status_refusal_carries_the_raw_status() {
        // A no-privilege sibling move (0x4): the shim reports the raw status for the
        // core to judge (design D4; ADR-0003), never a scraped string.
        let runner = ScriptedRunner::new(vec![(
            "dprc assign dprc.1 --object=dpbp.0 --child=dprc.2",
            refused("error: dprc_assign() failed: No privilege (0x4)"),
        )]);
        let mc = RestoolMc::with_runner(runner, DEFAULT_CONTAINER);
        let err = mc
            .dprc_assign(
                DprcId::new(1),
                ObjectRef::new(Family::Dpbp, 0),
                Some(DprcId::new(2)),
                None,
            )
            .expect_err("refused");
        assert!(matches!(err, Error::McStatus { status: 4 }), "got {err:?}");
    }

    #[test]
    fn client_guard_refusal_is_a_distinct_typed_error() {
        // The plugged-move guard (DPRC-I3) fires before any MC command, so there is no
        // status token; it must surface as `RestoolGuard`, distinguishable from an MC
        // status refusal (the create-scenario's companion contract).
        let guard =
            "error: cannot be moved because it is currently in plugged state; unplug it first";
        let runner = ScriptedRunner::new(vec![(
            "dprc assign dprc.1 --object=dpbp.0 --child=dprc.2",
            refused(guard),
        )]);
        let mc = RestoolMc::with_runner(runner, DEFAULT_CONTAINER);
        let err = mc
            .dprc_assign(
                DprcId::new(1),
                ObjectRef::new(Family::Dpbp, 0),
                Some(DprcId::new(2)),
                None,
            )
            .expect_err("refused");
        match err {
            Error::RestoolGuard { detail } => assert!(detail.contains("plugged state")),
            other => panic!("expected RestoolGuard, got {other:?}"),
        }
    }

    #[test]
    fn destroy_distinguishes_mc_ebusy_from_client_guard() {
        // Destroying a container with a plugged resident is refused. Two shapes must
        // stay distinct: the MC's own `-EBUSY` (a status token ⇒ `McStatus`) and a
        // restool client-side pre-check (no token ⇒ `RestoolGuard`).
        let mc_ebusy = RestoolMc::with_runner(
            ScriptedRunner::new(vec![(
                "dprc destroy dprc.2",
                refused("error: dprc_destroy() failed: Device is busy (0x10)"),
            )]),
            DEFAULT_CONTAINER,
        );
        assert!(matches!(
            mc_ebusy.dprc_destroy(DprcId::new(2)).expect_err("ebusy"),
            Error::McStatus { status: 0x10 }
        ));

        let client_guard = RestoolMc::with_runner(
            ScriptedRunner::new(vec![(
                "dprc destroy dprc.2",
                refused("error: container still holds plugged objects; unplug them first"),
            )]),
            DEFAULT_CONTAINER,
        );
        assert!(matches!(
            client_guard
                .dprc_destroy(DprcId::new(2))
                .expect_err("guard"),
            Error::RestoolGuard { .. }
        ));
    }

    #[test]
    fn signal_death_is_a_backend_error_not_a_client_guard() {
        // A restool killed by a signal has no exit code; that is transport failure, not a
        // client-side refusal, so it must not collapse to RestoolGuard (review M10; PASS3-F8).
        let killed = RunOutcome {
            stdout: String::new(),
            stderr: String::new(),
            code: None,
        };
        let mc = RestoolMc::with_runner(
            ScriptedRunner::new(vec![("dprc destroy dprc.2", killed)]),
            DEFAULT_CONTAINER,
        );
        assert!(matches!(
            mc.dprc_destroy(DprcId::new(2)).expect_err("signal death"),
            Error::Backend(_)
        ));
    }

    // dpmac info bodies shaped after models/board/baselines/reference.json. `ep` is
    // the raw `endpoint:` value (e.g. "dpni.0, link is up" or "No object associated").
    fn dpmac_info_ep(rate: i64, eth: &str, ep: &str) -> String {
        format!(
            "DPMAC ethernet interface: {eth}\nDPMAC link type: DPMAC_LINK_TYPE_PHY\n\
             MAC address: 00:11:22:33:44:55\nendpoint: {ep}\n\
             maximum supported rate {rate} Mbps\n"
        )
    }

    // An unconnected dpmac (the common case for the ceiling/availability assertions).
    fn dpmac_info(rate: i64, eth: &str) -> String {
        dpmac_info_ep(rate, eth, "No object associated")
    }

    // A realistic `dprc show` body: header, contains-line, then `family.N [label]
    // plugged-state` rows (ADR-0010 Consequences: the label column is surfaced).
    fn dprc_show(rows: &[&str]) -> String {
        let mut s = format!("dprc.1 contains {} objects:\n", rows.len());
        s.push_str("object          label           plugged-state\n");
        for r in rows {
            s.push_str(r);
            s.push('\n');
        }
        s
    }

    const RESOURCES: &str = "bp 63\nmcp 203\nswp 49\nfq 1981\n";

    #[test]
    fn read_inventory_assembles_offers_availability_and_ceilings() {
        // Unlabelled residents (the DPL's own): every dpmac here is unconnected, and
        // dpni.0 carries no label — reported verbatim as an empty label, which the api
        // side reads as the DPL resident.
        let show = dprc_show(&[
            "dpni.0                          plugged",
            "dpmac.3                         unplugged",
            "dpmac.4                         plugged",
            "dpmac.17                        plugged",
        ]);
        let info3 = dpmac_info(25_000, "DPMAC_ETH_IF_CAUI");
        let info4 = dpmac_info(25_000, "DPMAC_ETH_IF_CAUI");
        let info17 = dpmac_info(1000, "DPMAC_ETH_IF_RGMII");
        let runner = ScriptedRunner::canned(&[
            ("dprc show dprc.1", &show),
            ("dpmac info dpmac.3", &info3),
            ("dpmac info dpmac.4", &info4),
            ("dpmac info dpmac.17", &info17),
            ("dprc show mc.global --resources", RESOURCES),
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

        // ADR-0010 §4: the unlabelled dpni.0 reports an empty label verbatim.
        assert_eq!(inv.labels.get(&(Family::Dpni, 0)), Some(&String::new()));
    }

    #[test]
    fn foreign_dpni_makes_the_dpmac_it_anchors_foreign() {
        // The withheld assertion (gqf.41): an unlabelled dpni.0 anchored by a
        // non-reserved dpmac.7 makes that port Foreign — the backend reports the raw
        // empty owner verbatim, and the api side (availability_of) judges empty ⇒ the
        // DPL resident "dpl".
        let show = dprc_show(&[
            "dpni.0                          plugged",
            "dpmac.7                         plugged",
        ]);
        let info7 = dpmac_info_ep(10_000, "DPMAC_ETH_IF_XFI", "dpni.0, link is up");
        let runner = ScriptedRunner::canned(&[
            ("dprc show dprc.1", &show),
            ("dpmac info dpmac.7", &info7),
            ("dprc show mc.global --resources", RESOURCES),
        ]);
        let inv = RestoolMc::with_runner(runner, DEFAULT_CONTAINER)
            .read_inventory()
            .expect("inventory");

        assert_eq!(inv.labels.get(&(Family::Dpni, 0)), Some(&String::new()));
        assert_eq!(
            inv.dpmacs[&DpmacId::new(7)].avail,
            Availability::Foreign(String::new())
        );
    }

    #[test]
    fn a_labelled_dpni_reports_its_label_as_the_dpmac_owner() {
        // The backend never judges (ADR-0010 §4 refined by ADR-0015): a dpni wearing a
        // construct name is reported with that raw label, and the dpmac it anchors is
        // Foreign(that name). Whether the name is ours is the api side's call —
        // Inventory::availability_of demotes it to Free when the name is declared.
        let show = dprc_show(&[
            "dpni.0          wan0            plugged",
            "dpmac.7                         plugged",
        ]);
        let info7 = dpmac_info_ep(10_000, "DPMAC_ETH_IF_XFI", "dpni.0, link is up");
        let runner = ScriptedRunner::canned(&[
            ("dprc show dprc.1", &show),
            ("dpmac info dpmac.7", &info7),
            ("dprc show mc.global --resources", RESOURCES),
        ]);
        let inv = RestoolMc::with_runner(runner, DEFAULT_CONTAINER)
            .read_inventory()
            .expect("inventory");

        assert_eq!(inv.labels.get(&(Family::Dpni, 0)), Some(&"wan0".to_owned()));
        assert_eq!(
            inv.dpmacs[&DpmacId::new(7)].avail,
            Availability::Foreign("wan0".to_owned())
        );
    }

    #[test]
    fn reserved_matrix_wins_over_foreign() {
        // Precedence: dpmac.17 anchors an unlabelled (foreign) dpni.0 yet stays
        // Reserved — the ADR-0003 §3 safety matrix is the strongest rule.
        let show = dprc_show(&[
            "dpni.0                          plugged",
            "dpmac.17                        plugged",
        ]);
        let info17 = dpmac_info_ep(1000, "DPMAC_ETH_IF_RGMII", "dpni.0, link is up");
        let runner = ScriptedRunner::canned(&[
            ("dprc show dprc.1", &show),
            ("dpmac info dpmac.17", &info17),
            ("dprc show mc.global --resources", RESOURCES),
        ]);
        let inv = RestoolMc::with_runner(runner, DEFAULT_CONTAINER)
            .read_inventory()
            .expect("inventory");

        assert!(matches!(
            inv.dpmacs[&DpmacId::new(17)].avail,
            Availability::Reserved(_)
        ));
    }

    #[test]
    fn third_party_label_is_the_owner() {
        // A non-empty label that is not ours names its owner directly.
        let show = dprc_show(&[
            "dpni.5          vendor          plugged",
            "dpmac.7                         plugged",
        ]);
        let info7 = dpmac_info_ep(10_000, "DPMAC_ETH_IF_XFI", "dpni.5, link is up");
        let runner = ScriptedRunner::canned(&[
            ("dprc show dprc.1", &show),
            ("dpmac info dpmac.7", &info7),
            ("dprc show mc.global --resources", RESOURCES),
        ]);
        let inv = RestoolMc::with_runner(runner, DEFAULT_CONTAINER)
            .read_inventory()
            .expect("inventory");

        assert_eq!(
            inv.labels.get(&(Family::Dpni, 5)),
            Some(&"vendor".to_owned())
        );
        assert_eq!(
            inv.dpmacs[&DpmacId::new(7)].avail,
            Availability::Foreign("vendor".to_owned())
        );
    }

    /// The `mac address:` line, built from the domain type so no MAC literal appears in
    /// source text (public-repo leak-scan); `MacAddr` Display renders the canonical form.
    fn mac_line() -> String {
        format!("mac address: {}\n", MacAddr::new([2, 0, 0, 0, 0, 7]))
    }

    /// A faithful `dpni info` body with the full `dpni_attr` block, transcribed from
    /// `restool/dpni_commands.c` `print_dpni_attr` (the dot-spelled options line, the
    /// split `num_rx_tcs`/`num_tx_tcs`, the read-back `mac_entries`/`vlan_entries`
    /// spellings, and the added `qos_key_size`/`fs_key_size` — no `dist_key_size`).
    fn dpni_info_full_attr() -> String {
        format!(
            "dpni version: 8.5\n\
             dpni id: 7\n\
             plugged state: plugged\n\
             endpoint: dpmac.7, link is up\n\
             {}\
             max frame length: 1536\n\
             dpni_attr.options value is: 0x800003d0\n\
             num_queues: 16\n\
             num_cgs: 24\n\
             num_rx_tcs: 8\n\
             num_tx_tcs: 16\n\
             mac_entries: 16\n\
             vlan_entries: 16\n\
             qos_entries: 64\n\
             fs_entries: 1\n\
             qos_key_size: 24\n\
             fs_key_size: 24\n\
             num_channels: 1\n\
             num_opr: 0\n\
             ingress_all_frames: 3150\n\
             egress_all_frames: 42\n",
            mac_line()
        )
    }

    #[test]
    fn options_mask_is_the_computed_pmd_value() {
        // PMD's five flags + PfdrInPeb: 0x100|0x200|0x10|0x40|0x80|0x80000000 = 0x800003d0.
        assert_eq!(dpni_options_mask(&Profile::Pmd.mask()), 0x8000_03d0);
        assert_eq!(dpni_options_mask(&Profile::Kernel.mask()), 0x0010);
        assert_eq!(dpni_options_mask(&OptionMask::empty()), 0);
    }

    #[test]
    fn options_decode_round_trips_and_rejects_unnamed_bits() {
        assert_eq!(decode_dpni_options(0x8000_03d0), Some(Profile::Pmd.mask()));
        // A bit with no vocabulary variant (NO_FS 0x20) is an honest gap, not a guess.
        assert_eq!(decode_dpni_options(0x0020), None);
        assert_eq!(decode_dpni_options(0), Some(OptionMask::empty()));
    }

    #[test]
    fn create_args_render_from_the_typed_block() {
        // Every nonzero sizing flag rides, omitted ones (0 ⇒ MC default) do not, and
        // options is one raw mask — never a name token.
        let args = dpni_create_args(&Profile::Pmd.cfg(), 16);
        assert_eq!(&args[..3], &["--script", "dpni", "create"]);
        assert!(args.contains(&"--options=0x800003d0".to_owned()));
        for present in [
            "--num-queues=16",
            "--num-tcs=16",
            "--vlan-filter-entries=16",
            "--qos-entries=64",
            "--fs-entries=1",
            "--num-cgs=24",
            "--num-channels=1",
        ] {
            assert!(args.iter().any(|a| a == present), "expected {present}");
        }
        for omitted in ["--mac-filter-entries", "--dist-key-size", "--num-opr"] {
            assert!(!args.iter().any(|a| a.starts_with(omitted)), "{omitted}");
        }

        // The bare block is only --num-queues (host fallback).
        assert_eq!(
            dpni_create_args(&DpniCfg::defaults(), 1),
            vec!["--script", "dpni", "create", "--num-queues=1"]
        );
    }

    #[test]
    fn observation_maps_the_readback_asymmetries() {
        // The asymmetric read-back resolves: num_tx_tcs ⇒ num_tcs, num_channels ⇒
        // num_ceetm_ch; DpniObservation has no dist_key_size field (dpni-typestate design D4).
        let attr = parse::parse_dpni_info(&dpni_info_full_attr())
            .attr
            .expect("attr block present");
        let obs = map_dpni_observation(&attr).expect("maps cleanly");

        assert_eq!(obs.options, Profile::Pmd.mask());
        assert_eq!(obs.num_queues.get(), 16);
        assert_eq!(
            obs.num_tcs.get(),
            16,
            "from num_tx_tcs, the cfg-settable half"
        );
        assert_eq!(obs.mac_filter_entries.get(), 16, "from mac_entries");
        assert_eq!(obs.vlan_filter_entries.get(), 16, "from vlan_entries");
        assert_eq!(obs.qos_entries.get(), 64);
        assert_eq!(obs.fs_entries.get(), 1);
        assert_eq!(obs.num_cgs.get(), 24);
        assert_eq!(obs.num_ceetm_ch.get(), 1, "from num_channels");
        assert_eq!(obs.num_opr.get(), 0);

        // Split/informational read-back stays on the raw struct (num_rx_tcs distinct from tx).
        assert_eq!(attr.num_rx_tcs, Some(8));
        assert_eq!(attr.num_tx_tcs, Some(16));
        assert_eq!(attr.qos_key_size, Some(24));
        assert_eq!(attr.fs_key_size, Some(24));
        assert_eq!(attr.num_channels, Some(1));
    }

    #[test]
    fn observe_fills_cfg_observation_from_the_attr_block() {
        let root = dprc_show(&["dpni.7          wan0            plugged"]);
        let runner = ScriptedRunner::canned(&[
            ("dprc show dprc.1", &root),
            ("dpni info dpni.7", &dpni_info_full_attr()),
        ]);
        let mc = RestoolMc::with_runner(runner, DEFAULT_CONTAINER);
        let topo = mc.observe().expect("observe");
        let dpni = &topo.dpnis[0];
        let obs = dpni.cfg_observation.as_ref().expect("cfg observation");
        assert_eq!(obs.num_tcs.get(), 16);
        assert_eq!(obs.num_ceetm_ch.get(), 1);
        assert_eq!(obs.options, Profile::Pmd.mask());
    }

    #[test]
    fn observe_leaves_cfg_observation_none_without_an_attr_block() {
        // No attr block ⇒ cfg_observation None; endpoint/mac parsing unchanged.
        let root = dprc_show(&["dpni.7          wan0            plugged"]);
        let info = format!(
            "dpni version: 8.5\nendpoint: dpmac.7, link is up\n{}",
            mac_line()
        );
        let runner =
            ScriptedRunner::canned(&[("dprc show dprc.1", &root), ("dpni info dpni.7", &info)]);
        let mc = RestoolMc::with_runner(runner, DEFAULT_CONTAINER);
        let topo = mc.observe().expect("observe");
        let dpni = &topo.dpnis[0];
        assert!(dpni.cfg_observation.is_none(), "no attr block ⇒ None");
        assert_eq!(dpni.connected_to, Some(DpmacId::new(7)));
        assert_eq!(dpni.mac, Some(MacAddr::new([2, 0, 0, 0, 0, 7])));
    }

    #[test]
    fn set_mac_round_trips_via_readback() {
        // Re-observation (parsed mac), not the exit status, is the convergence oracle
        // (`docs/baseline/dpni.md` "Silent-failure notes": exit 0 is not convergence).
        let root = dprc_show(&["dpni.7          wan0            plugged"]);
        let mac = MacAddr::new([2, 0, 0, 0, 0, 7]);
        let info = format!(
            "dpni version: 8.5\nendpoint: dpmac.7, link is up\n{}",
            mac_line()
        );
        let update = format!("dpni update dpni.7 --mac-addr={mac}");
        let runner = ScriptedRunner::new(vec![
            (update.as_str(), ok("")),
            ("dprc sync", ok("")),
            ("dprc show dprc.1", ok(&root)),
            ("dpni info dpni.7", ok(&info)),
        ]);
        let mc = RestoolMc::with_runner(runner, DEFAULT_CONTAINER);
        mc.set_mac(DpniId::new(7), mac).expect("set mac");
        let topo = mc.observe().expect("observe");
        assert_eq!(
            topo.dpnis[0].mac,
            Some(mac),
            "read-back reports the new MAC"
        );
    }
}
