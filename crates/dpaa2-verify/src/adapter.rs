//! The shared MBT adapter: model action ↔ restool command ↔ read-back
//! observation (design D6; ADR-0002 §4).
//!
//! One mapping binds the three faces for every action of the core machine
//! (`models/core/machine.qnt`):
//!
//! - [`drive`] — the restool/sysfs command that makes the board take the
//!   step, or [`Drive::Await`] for transitions the board takes by itself
//!   (kernel probes, pool draws, consumer enables, PHY link changes).
//! - [`readback`] + [`expect`] — the probes whose parsed output is the
//!   *observation* of the resulting state, and the model's expectation
//!   for exactly those probes.
//! - [`judge`] — conformance is decided on the read-back alone; the
//!   driving command's exit status is carried as auxiliary evidence and
//!   never contributes to the verdict. That is the law DPNI-I6 and
//!   DPMAC-I8 made explicit: restool exits 0 on dead options and partial
//!   failures, so exit status is not an observation.
//!
//! Action vocabulary comes from ITF traces frozen with `quint run --mbt`
//! ([`parse_mbt_trace`]): `mbt::actionTaken` names the machine's nondet
//! wrapper and `mbt::nondetPicks` carries the chosen parameters. Directed
//! (`...At`) runs carry no picks and are not generator input.
//!
//! Model object ids are model-assigned; the board assigns its own at
//! create time. [`Binding`] carries the model→board name map: boot-born
//! objects keep their literal names (their model number reuses the object
//! number), runtime creates are bound from the create command's output.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde_json::Value;

/// The 16 MC object families (object-model.md §3).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[allow(missing_docs)]
pub enum Family {
    Dprc,
    Dpni,
    Dpmac,
    Dpbp,
    Dpio,
    Dpcon,
    Dpmcp,
    Dpseci,
    Dpsw,
    Dpdmux,
    Dpaiop,
    Dpci,
    Dpdcei,
    Dpdmai,
    Dprtc,
    Dpdbg,
}

impl Family {
    /// The restool type name (`dpni`, `dprc`, …).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dprc => "dprc",
            Self::Dpni => "dpni",
            Self::Dpmac => "dpmac",
            Self::Dpbp => "dpbp",
            Self::Dpio => "dpio",
            Self::Dpcon => "dpcon",
            Self::Dpmcp => "dpmcp",
            Self::Dpseci => "dpseci",
            Self::Dpsw => "dpsw",
            Self::Dpdmux => "dpdmux",
            Self::Dpaiop => "dpaiop",
            Self::Dpci => "dpci",
            Self::Dpdcei => "dpdcei",
            Self::Dpdmai => "dpdmai",
            Self::Dprtc => "dprtc",
            Self::Dpdbg => "dpdbg",
        }
    }

    /// Parses the ITF constructor tag (`"Dpni"`, `"Dprc"`, …).
    fn from_tag(tag: &str) -> Result<Self, String> {
        Ok(match tag {
            "Dprc" => Self::Dprc,
            "Dpni" => Self::Dpni,
            "Dpmac" => Self::Dpmac,
            "Dpbp" => Self::Dpbp,
            "Dpio" => Self::Dpio,
            "Dpcon" => Self::Dpcon,
            "Dpmcp" => Self::Dpmcp,
            "Dpseci" => Self::Dpseci,
            "Dpsw" => Self::Dpsw,
            "Dpdmux" => Self::Dpdmux,
            "Dpaiop" => Self::Dpaiop,
            "Dpci" => Self::Dpci,
            "Dpdcei" => Self::Dpdcei,
            "Dpdmai" => Self::Dpdmai,
            "Dprtc" => Self::Dprtc,
            "Dpdbg" => Self::Dpdbg,
            other => return Err(format!("unknown family tag `{other}`")),
        })
    }
}

impl std::str::FromStr for Family {
    type Err = String;

    /// Parses the restool type name (`dpni`, `dprc`, …), the inverse of
    /// [`Family::as_str`]. Shared by [`ObjRef`]'s parsing and the
    /// `--create-args` flag parser ([`CreateArgs::parse_flag`]).
    fn from_str(s: &str) -> Result<Self, String> {
        [
            Self::Dprc,
            Self::Dpni,
            Self::Dpmac,
            Self::Dpbp,
            Self::Dpio,
            Self::Dpcon,
            Self::Dpmcp,
            Self::Dpseci,
            Self::Dpsw,
            Self::Dpdmux,
            Self::Dpaiop,
            Self::Dpci,
            Self::Dpdcei,
            Self::Dpdmai,
            Self::Dprtc,
            Self::Dpdbg,
        ]
        .into_iter()
        .find(|f| f.as_str() == s)
        .ok_or_else(|| format!("unknown family `{s}`"))
    }
}

/// A model-space object id (`ObjId` in `core/types.qnt`): the restool id
/// space, distinct from MC hardware ids (law §6.4).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct ObjRef {
    /// The object family.
    pub fam: Family,
    /// The model-assigned object number.
    pub num: u32,
}

impl fmt::Display for ObjRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.fam.as_str(), self.num)
    }
}

impl std::str::FromStr for ObjRef {
    type Err = String;

    /// Parses `dpni.100`-style references (the inverse of `Display`).
    fn from_str(s: &str) -> Result<Self, String> {
        let (kind, num) = s
            .split_once('.')
            .ok_or_else(|| format!("not an object ref: `{s}`"))?;
        Ok(Self {
            fam: kind.parse()?,
            num: num.parse().map_err(|e| format!("bad object number: {e}"))?,
        })
    }
}

/// A model-space connect endpoint (`type.id.port`, object-model.md §2).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct EndpointRef {
    /// The endpoint's object.
    pub obj: ObjRef,
    /// The endpoint port; 0 for single-port objects.
    pub port: u32,
}

impl fmt::Display for EndpointRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.port == 0 {
            write!(f, "{}", self.obj)
        } else {
            write!(f, "{}.{}", self.obj, self.port)
        }
    }
}

/// Driver binding, mirroring the model's tri-state (`BindState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindView {
    /// No driver holds the object.
    Unbound,
    /// A kernel driver is bound.
    Kernel,
    /// VFIO holds the object.
    Vfio,
}

/// The adapter-relevant slice of one model object's state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjView {
    /// Containing DPRC; `None` only for `mc.global`.
    pub parent: Option<ObjRef>,
    /// Plugged state (`assign --plugged`).
    pub plugged: bool,
    /// Linux-bus visibility (distinct from MC existence, §6.3).
    pub bus_visible: bool,
    /// Driver binding.
    pub bind: BindView,
    /// PHY/consumer link state (distinct from the connection edge).
    pub link_up: bool,
}

/// The generic full-state projection of one ITF state: every object and
/// every connection edge, in model-space ids. (The reconciler-specific
/// projection of phase 3 lives in [`crate::itf`].)
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MachineView {
    /// All objects present in the MC, by model id.
    pub objs: BTreeMap<ObjRef, ObjView>,
    /// Connection edges, each stored with its endpoints in sorted order.
    pub edges: BTreeSet<(EndpointRef, EndpointRef)>,
}

impl MachineView {
    /// The peer endpoint connected to any port of `o`, if one exists.
    #[must_use]
    pub fn peer_of(&self, o: ObjRef) -> Option<EndpointRef> {
        self.edges.iter().find_map(|(a, b)| {
            if a.obj == o {
                Some(*b)
            } else if b.obj == o {
                Some(*a)
            } else {
                None
            }
        })
    }
}

/// The object present in `post` but not in `pre` — the id a create step
/// assigned in the model.
#[must_use]
pub fn created_object(pre: &MachineView, post: &MachineView) -> Option<ObjRef> {
    post.objs
        .keys()
        .find(|o| !pre.objs.contains_key(o))
        .copied()
}

/// One core-machine action with its parameters, as taken in a trace.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum ModelAction {
    CreateContainer { parent: ObjRef },
    CreateObject { fam: Family, container: ObjRef },
    PreplugMutate { obj: ObjRef },
    AssignChild { obj: ObjRef, dst: ObjRef },
    Plug { obj: ObjRef },
    Unplug { obj: ObjRef },
    KernelBind { obj: ObjRef },
    VfioBind { obj: ObjRef },
    Unbind { obj: ObjRef },
    ConnectEdge { a: EndpointRef, b: EndpointRef },
    DisconnectEdge { e: EndpointRef },
    Rescan { container: ObjRef },
    ChildIrqRefresh { container: ObjRef },
    Allocate { consumer: ObjRef, pool: ObjRef },
    Free { pool: ObjRef },
    Enable { obj: ObjRef },
    Disable { obj: ObjRef },
    SetLocked { container: ObjRef, locked: bool },
    LinkChange { obj: ObjRef },
    Destroy { obj: ObjRef },
}

impl ModelAction {
    /// Every object the action references through its parameters — the
    /// surface the safety envelope screens at generation time
    /// ([`crate::safety::check_trace`]). Objects the action *creates*
    /// have no id yet and are covered by the execution-side scan once
    /// the board names them.
    #[must_use]
    pub fn refs(&self) -> Vec<ObjRef> {
        match self {
            Self::CreateContainer { parent } => vec![*parent],
            Self::CreateObject { container, .. }
            | Self::Rescan { container }
            | Self::ChildIrqRefresh { container }
            | Self::SetLocked { container, .. } => vec![*container],
            Self::PreplugMutate { obj }
            | Self::Plug { obj }
            | Self::Unplug { obj }
            | Self::KernelBind { obj }
            | Self::VfioBind { obj }
            | Self::Unbind { obj }
            | Self::Enable { obj }
            | Self::Disable { obj }
            | Self::LinkChange { obj }
            | Self::Destroy { obj } => vec![*obj],
            Self::AssignChild { obj, dst } => vec![*obj, *dst],
            Self::ConnectEdge { a, b } => vec![a.obj, b.obj],
            Self::DisconnectEdge { e } => vec![e.obj],
            Self::Allocate { consumer, pool } => vec![*consumer, *pool],
            Self::Free { pool } => vec![*pool],
        }
    }
}

/// One step of an MBT trace: the action taken and the state it produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MbtStep {
    /// The action the simulator took.
    pub action: ModelAction,
    /// The machine state after the action.
    pub post: MachineView,
}

/// A parsed `quint run --mbt` trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MbtTrace {
    /// The initial state (before any action).
    pub init: MachineView,
    /// The actions taken and their post-states, in order.
    pub steps: Vec<MbtStep>,
}

// --- ITF parsing -----------------------------------------------------

/// The `#bigint`-encoded integer of an ITF value.
fn num(v: &Value) -> Result<u32, String> {
    v["#bigint"]
        .as_str()
        .ok_or_else(|| format!("not a #bigint: {v}"))?
        .parse()
        .map_err(|e| format!("bad integer: {e}"))
}

/// The constructor tag of an ITF sum-type value.
fn tag(v: &Value) -> Result<&str, String> {
    v["tag"]
        .as_str()
        .ok_or_else(|| format!("not a variant: {v}"))
}

fn obj_ref(v: &Value) -> Result<ObjRef, String> {
    Ok(ObjRef {
        fam: Family::from_tag(tag(&v["fam"])?)?,
        num: num(&v["num"])?,
    })
}

fn endpoint_ref(v: &Value) -> Result<EndpointRef, String> {
    Ok(EndpointRef {
        obj: obj_ref(&v["obj"])?,
        port: num(&v["port"])?,
    })
}

/// The payload of a taken nondet pick, or an error naming the missing one.
fn pick<'a>(picks: &'a Value, name: &str) -> Result<&'a Value, String> {
    let v = &picks[name];
    match v["tag"].as_str() {
        Some("Some") => Ok(&v["value"]),
        Some("None") => Err(format!("nondet pick `{name}` was not taken")),
        _ => Err(format!("no nondet pick `{name}` in trace")),
    }
}

/// Parses `mbt::actionTaken` + `mbt::nondetPicks` into a [`ModelAction`].
/// Pick names are the machine's nondet binder names (`machine.qnt`).
fn action(taken: &str, picks: &Value) -> Result<ModelAction, String> {
    let obj = |name: &str| obj_ref(pick(picks, name)?);
    let ep = |name: &str| endpoint_ref(pick(picks, name)?);
    Ok(match taken {
        "createContainer" => ModelAction::CreateContainer {
            parent: obj("parent")?,
        },
        "createObject" => ModelAction::CreateObject {
            fam: Family::from_tag(tag(pick(picks, "fam")?)?)?,
            container: obj("c")?,
        },
        "preplugMutate" => ModelAction::PreplugMutate { obj: obj("o")? },
        "assignChild" => ModelAction::AssignChild {
            obj: obj("o")?,
            dst: obj("dst")?,
        },
        "plug" => ModelAction::Plug { obj: obj("o")? },
        "unplug" => ModelAction::Unplug { obj: obj("o")? },
        "kernelBind" => ModelAction::KernelBind { obj: obj("o")? },
        "vfioBind" => ModelAction::VfioBind { obj: obj("o")? },
        "unbind" => ModelAction::Unbind { obj: obj("o")? },
        "connectEdge" => ModelAction::ConnectEdge {
            a: ep("a")?,
            b: ep("b")?,
        },
        "disconnectEdge" => ModelAction::DisconnectEdge { e: ep("e")? },
        "rescan" => ModelAction::Rescan {
            container: obj("c")?,
        },
        "childIrqRefresh" => ModelAction::ChildIrqRefresh {
            container: obj("c")?,
        },
        "allocate" => ModelAction::Allocate {
            consumer: obj("consumer")?,
            pool: obj("p")?,
        },
        "free" => ModelAction::Free { pool: obj("p")? },
        "enable" => ModelAction::Enable { obj: obj("o")? },
        "disable" => ModelAction::Disable { obj: obj("o")? },
        "setLocked" => ModelAction::SetLocked {
            container: obj("c")?,
            locked: pick(picks, "v")?
                .as_bool()
                .ok_or("setLocked pick `v` is not a bool")?,
        },
        "linkChange" => ModelAction::LinkChange { obj: obj("o")? },
        "destroy" => ModelAction::Destroy { obj: obj("o")? },
        other => {
            return Err(format!(
                "unknown action `{other}` (mbt traces come from `quint run --mbt`)"
            ));
        }
    })
}

/// Reduces one ITF-encoded `CoreState` to a [`MachineView`].
fn view(s: &Value) -> Result<MachineView, String> {
    let mut out = MachineView::default();
    for entry in s["objs"]["#map"].as_array().ok_or("objs is not a map")? {
        let id = obj_ref(&entry[0])?;
        let st = &entry[1];
        let parent = match tag(&st["parent"])? {
            "Some" => Some(obj_ref(&st["parent"]["value"])?),
            _ => None,
        };
        let bind = match tag(&st["bind"])? {
            "BoundKernel" => BindView::Kernel,
            "BoundVfio" => BindView::Vfio,
            _ => BindView::Unbound,
        };
        out.objs.insert(
            id,
            ObjView {
                parent,
                plugged: st["plugged"].as_bool().ok_or("plugged not a bool")?,
                bus_visible: st["busVisible"].as_bool().ok_or("busVisible not a bool")?,
                bind,
                link_up: st["linkUp"].as_bool().ok_or("linkUp not a bool")?,
            },
        );
    }
    for conn in s["conns"]["#set"].as_array().ok_or("conns is not a set")? {
        let p = conn["#tup"].as_array().ok_or("conn is not a pair")?;
        let (a, b) = (endpoint_ref(&p[0])?, endpoint_ref(&p[1])?);
        out.edges.insert(if a <= b { (a, b) } else { (b, a) });
    }
    Ok(out)
}

/// Parses a `quint run --mbt` ITF trace into its action/state sequence.
///
/// # Errors
///
/// Returns a description of the first structural mismatch — a trace not
/// produced with `--mbt` over the core machine (directed `...At` runs
/// carry no nondet picks and are rejected by name).
pub fn parse_mbt_trace(json: &str) -> Result<MbtTrace, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    // The CoreState variable is the sole non-`mbt::` var — except that a
    // trace frozen after the ioctl-policy change also carries the machine's
    // `lastVerbs` var, which this parser ignores (the `lastverbs_trace` test
    // reads it directly). Exclude it so `s` is picked, not `lastVerbs`.
    let state_var = root["vars"]
        .as_array()
        .ok_or("trace has no vars")?
        .iter()
        .filter_map(Value::as_str)
        .find(|v| !v.starts_with("mbt::") && !v.ends_with("lastVerbs"))
        .ok_or("trace has no machine state variable")?
        .to_owned();
    let states = root["states"].as_array().ok_or("trace has no states")?;
    let mut it = states.iter();
    let init = view(&it.next().ok_or("trace is empty")?[&state_var])?;
    let steps = it
        .map(|s| {
            Ok(MbtStep {
                action: action(
                    s["mbt::actionTaken"]
                        .as_str()
                        .ok_or("state has no mbt::actionTaken")?,
                    &s["mbt::nondetPicks"],
                )?,
                post: view(&s[&state_var])?,
            })
        })
        .collect::<Result<_, String>>()?;
    Ok(MbtTrace { init, steps })
}

// --- model → board name binding --------------------------------------

/// The model→board object-name map.
///
/// Boot-born objects (the DPC/DPL population present in the init state)
/// keep their literal names: their model number reuses the real object
/// number. Runtime creates are bound from the create command's output as
/// the suite executes.
#[derive(Debug, Clone, Default)]
pub struct Binding {
    names: BTreeMap<ObjRef, String>,
}

impl Binding {
    /// Seeds the binding with every object of the initial state.
    #[must_use]
    pub fn seed(init: &MachineView) -> Self {
        Self {
            names: init.objs.keys().map(|o| (*o, o.to_string())).collect(),
        }
    }

    /// Binds a model id to the object named by a `--script` create
    /// output (e.g. `dpni.5`), validating the family prefix.
    ///
    /// # Errors
    ///
    /// Fails when the output carries no object reference or one of the
    /// wrong family.
    pub fn bind_created(&mut self, model: ObjRef, create_stdout: &str) -> Result<String, String> {
        let name = dpaa2_mc::parse::parse_object_ref(create_stdout)
            .ok_or_else(|| format!("no object id in `{}`", create_stdout.trim()))?;
        if !name.starts_with(&format!("{}.", model.fam.as_str())) {
            return Err(format!("created `{name}` does not match model {model}"));
        }
        self.names.insert(model, name.to_owned());
        Ok(name.to_owned())
    }

    /// Binds a model id to an arbitrary name without validation — the
    /// batch generator uses this to render commands over shell variables
    /// (`${OBJ_dpni_100}`) whose real names only exist at run time.
    pub fn bind_symbolic(&mut self, model: ObjRef, name: impl Into<String>) {
        self.names.insert(model, name.into());
    }

    /// The board name bound to a model id.
    ///
    /// # Errors
    ///
    /// Fails when the id was never seeded or bound — a trace referencing
    /// an object before its create step, or a stale binding.
    pub fn name(&self, o: ObjRef) -> Result<&str, String> {
        self.names
            .get(&o)
            .map(String::as_str)
            .ok_or_else(|| format!("model object {o} has no board binding"))
    }

    /// Renders an endpoint in restool syntax (`dpni.5`, `dpsw.0.1` —
    /// port 0 is implied by the bare object reference).
    ///
    /// # Errors
    ///
    /// Fails when the endpoint's object has no binding.
    pub fn endpoint(&self, e: EndpointRef) -> Result<String, String> {
        let name = self.name(e.obj)?;
        Ok(if e.port == 0 {
            name.to_owned()
        } else {
            format!("{name}.{}", e.port)
        })
    }
}

// --- drive side ------------------------------------------------------

/// One board-touching command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cmd {
    /// A `restool` invocation (argv after the binary name).
    Restool(Vec<String>),
    /// A sysfs write (`echo value > path`).
    SysfsWrite {
        /// Absolute sysfs path.
        path: String,
        /// Value to write.
        value: String,
    },
}

/// How a model action reaches the board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Drive {
    /// Commands to run, in order.
    Cmds(Vec<Cmd>),
    /// Not drivable from userspace: the board takes this step by itself
    /// and the suite only observes it. The string says why.
    Await(&'static str),
}

/// Locally-administered scratch MAC for the pre-plug mutation probe —
/// deliberately not any device's real address.
pub const SCRATCH_MAC: &str = "02:00:00:00:00:01";

/// The Linux root container, present in every board trace's boot state.
pub const ROOT_DPRC: ObjRef = ObjRef {
    fam: Family::Dprc,
    num: 1,
};

/// Extra `restool --script <fam> create` arguments per family, beyond
/// the container. Empty means bare create on MC defaults (the V-*-1
/// bare-create probes rely on that). Non-empty rows follow the ls-addni
/// recipe the family doc records as the known-good surface.
fn create_args(fam: Family) -> &'static [&'static str] {
    match fam {
        // dpio.md / ls-addni: local channel, 8 priorities.
        Family::Dpio => &["--channel-mode=DPIO_LOCAL_CHANNEL", "--num-priorities=8"],
        // dpcon.md / ls-addni: 2 priorities.
        Family::Dpcon => &["--num-priorities=2"],
        // restool has no bare dpseci create either: dpseci_commands.c
        // rejects a create that sets neither queue count nor priorities,
        // and the two must agree in length. These are restool's own
        // documented example values.
        Family::Dpseci => &["--num-queues=2", "--priorities=2,4"],
        // restool has no bare dpdcei create: dpdcei_commands.c rejects a
        // create that omits either flag, so these are mandatory, not a
        // recipe. Engine and priority are ours; the rest stays MC default.
        Family::Dpdcei => &["--engine=DPDCEI_ENGINE_DECOMPRESSION", "--priority=1"],
        // restool accepts a bare dpsw create, but the kernel then refuses
        // the object: dpaa2_switch_supports_cpu_traffic (dpaa2-switch.h)
        // demands the control interface enabled and both the flooding and
        // broadcast domains scoped per FDB, and restool's silent defaults
        // are the other value in each case (dpsw_commands.c leaves both
        // configs 0 = PER_VLAN / PER_OBJECT). num-ifs is pinned to the
        // model's endpointPorts rather than restool's undefaulted 4.
        Family::Dpsw => &[
            "--num-ifs=2",
            "--flooding-cfg=DPSW_FLOODING_PER_FDB",
            "--broadcast-cfg=DPSW_BROADCAST_PER_FDB",
        ],
        // restool mandates --num-ifs for a dpdmux (dpdmux_commands.c),
        // and the count excludes the uplink: 1 downlink plus interface 0
        // is the model's 2 endpoint ports. Demux method is left to
        // restool's C_VLAN_MAC default, which the evb driver accepts.
        Family::Dpdmux => &["--num-ifs=1"],
        _ => &[],
    }
}

/// Per-family overrides of the `restool <fam> create` arguments a suite
/// renders, replacing the `create_args` default table for those
/// families. Empty (the [`Default`]) means the table applies unchanged —
/// every suite committed so far was generated this way, so a default
/// `CreateArgs` reproduces them byte for byte. A suite states the
/// arguments it renders (e.g. a dpio on `DPIO_NO_CHANNEL`) so the plan
/// records them; the create arguments never enter the model, because
/// channel mode is not a lifecycle attribute.
///
/// Keyed by [`Family`] directly rather than its `as_str()` string: the
/// family already derives `Ord` (a `BTreeMap` key) and
/// `Serialize`/`Deserialize` (`serde_json` renders a fieldless enum as a
/// string map key, the same `"Dpio"` form the plan already uses for
/// `created`), so keying by the value is the smaller of the two.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct CreateArgs(BTreeMap<Family, Vec<String>>);

impl CreateArgs {
    /// Whether no family carries an override, so the default table
    /// `create_args` applies to every family.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The create arguments a suite renders for `fam`: its stated
    /// override if one exists, else the default table `create_args`.
    #[must_use]
    pub fn args_for(&self, fam: Family) -> Vec<String> {
        self.0
            .get(&fam)
            .cloned()
            .unwrap_or_else(|| create_args(fam).iter().map(|s| (*s).to_owned()).collect())
    }

    /// Parses one `--create-args` value, `<fam>=<args>`, into the family
    /// and the whitespace-separated arguments it overrides the default
    /// table with (`dpio=--channel-mode=DPIO_NO_CHANNEL --num-priorities=8`).
    ///
    /// # Errors
    ///
    /// Fails on a value with no `=`, an unknown family, or empty
    /// arguments.
    pub fn parse_flag(s: &str) -> Result<(Family, Vec<String>), String> {
        let (fam, args) = s
            .split_once('=')
            .ok_or_else(|| format!("create-args `{s}` is not <fam>=<args>"))?;
        let fam: Family = fam.trim().parse()?;
        let args: Vec<String> = args.split_whitespace().map(str::to_owned).collect();
        if args.is_empty() {
            return Err(format!(
                "create-args for {} names no arguments",
                fam.as_str()
            ));
        }
        Ok((fam, args))
    }
}

impl FromIterator<(Family, Vec<String>)> for CreateArgs {
    fn from_iter<I: IntoIterator<Item = (Family, Vec<String>)>>(it: I) -> Self {
        Self(it.into_iter().collect())
    }
}

/// The containing DPRC of `o` in `view`.
fn parent_of(view: &MachineView, o: ObjRef) -> Result<ObjRef, String> {
    view.objs
        .get(&o)
        .ok_or_else(|| format!("{o} not in state"))?
        .parent
        .ok_or_else(|| format!("{o} has no parent container"))
}

fn dev_path(obj: &str, tail: &str) -> String {
    format!("/sys/bus/fsl-mc/devices/{obj}/{tail}")
}

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| (*s).to_owned()).collect()
}

/// Maps a model action to the command(s) that drive it on the board,
/// with the default per-family create-argument table (`create_args`).
/// The thin wrapper over [`drive_with`] keeps every call site that
/// renders on the defaults unchanged.
///
/// `pre` is the machine state the action fires from (parent containers
/// are resolved there); `names` must already bind every referenced id.
///
/// # Errors
///
/// Fails when the action references an object with no binding or not in
/// `pre` — a malformed trace, never a board condition.
pub fn drive(action: &ModelAction, pre: &MachineView, names: &Binding) -> Result<Drive, String> {
    drive_with(action, pre, names, &CreateArgs::default())
}

/// Maps a model action to the command(s) that drive it on the board,
/// rendering create steps with `create`'s per-family overrides where it
/// has them and the default table `create_args` elsewhere.
///
/// `pre` is the machine state the action fires from (parent containers
/// are resolved there); `names` must already bind every referenced id.
///
/// # Errors
///
/// Fails when the action references an object with no binding or not in
/// `pre` — a malformed trace, never a board condition.
#[allow(clippy::too_many_lines)] // one arm per machine action, by design
pub fn drive_with(
    action: &ModelAction,
    pre: &MachineView,
    names: &Binding,
    create: &CreateArgs,
) -> Result<Drive, String> {
    let cmd = |parts: Vec<String>| Ok(Drive::Cmds(vec![Cmd::Restool(parts)]));
    match action {
        ModelAction::CreateContainer { parent } => {
            // dprc.md: identity (id, icid, portal) is pool-assigned;
            // restool cannot pin it — the create output names the child.
            // `--script` keeps that output a bare object name; batch
            // scripts reuse it verbatim as the container reference.
            // A `--create-args dprc=…` override (e.g. `--options=<mask>`)
            // rides on the container create too, the same way a
            // CreateObject renders its family's arguments; the default
            // table is empty for dprc, so committed suites are unchanged.
            let mut v = argv(&["--script", "dprc", "create", names.name(*parent)?]);
            v.extend(create.args_for(Family::Dprc));
            cmd(v)
        }
        // restool's dpdbg create takes no arguments at all: it pins the
        // container to the root and the id to 0 itself
        // (dpdbg_commands.c, cmd_dpdbg_create) and rejects --container
        // as an unrecognized option. `--script` still applies — it is a
        // restool-global flag, and print_new_obj honors it — so the
        // create output stays the bare name the driver binds from.
        ModelAction::CreateObject {
            fam: Family::Dpdbg, ..
        } => cmd(argv(&["--script", "dpdbg", "create"])),
        ModelAction::CreateObject { fam, container } => {
            let mut v = argv(&["--script", fam.as_str(), "create"]);
            v.extend(create.args_for(*fam));
            v.push(format!("--container={}", names.name(*container)?));
            cmd(v)
        }
        ModelAction::PreplugMutate { obj } => {
            if obj.fam == Family::Dpni {
                // dpni.md: `update --mac-addr` is the only attribute
                // restool can mutate post-create.
                cmd(argv(&[
                    "dpni",
                    "update",
                    names.name(*obj)?,
                    &format!("--mac-addr={SCRATCH_MAC}"),
                ]))
            } else {
                Ok(Drive::Await(
                    "no restool-expressible pre-plug mutation for this family",
                ))
            }
        }
        ModelAction::AssignChild { obj, dst } => {
            // The restool/MC primitive moves along one tree edge
            // (ADR-0007): assign pushes down into a direct child,
            // unassign pulls back up to the container's parent. The
            // direction falls out of the pre-state tree.
            let parent = parent_of(pre, *obj)?;
            if parent_of(pre, *dst).is_ok_and(|p| p == parent) {
                cmd(argv(&[
                    "dprc",
                    "assign",
                    names.name(parent)?,
                    &format!("--object={}", names.name(*obj)?),
                    &format!("--child={}", names.name(*dst)?),
                ]))
            } else if parent_of(pre, parent)? == *dst {
                cmd(argv(&[
                    "dprc",
                    "unassign",
                    names.name(*dst)?,
                    &format!("--object={}", names.name(*obj)?),
                    &format!("--child={}", names.name(parent)?),
                ]))
            } else {
                Err(format!(
                    "assignChild {obj} -> {dst} is not a single hop from {parent}"
                ))
            }
        }
        ModelAction::Plug { obj } => {
            let parent = parent_of(pre, *obj)?;
            cmd(argv(&[
                "dprc",
                "assign",
                names.name(parent)?,
                &format!("--object={}", names.name(*obj)?),
                "--plugged=1",
            ]))
        }
        ModelAction::Unplug { obj } => {
            let parent = parent_of(pre, *obj)?;
            cmd(argv(&[
                "dprc",
                "assign",
                names.name(parent)?,
                &format!("--object={}", names.name(*obj)?),
                "--plugged=0",
            ]))
        }
        ModelAction::KernelBind { .. } => Ok(Drive::Await(
            "the kernel probes plugged bus-visible objects on its own; observe the driver link",
        )),
        ModelAction::VfioBind { obj } => {
            let name = names.name(*obj)?;
            Ok(Drive::Cmds(vec![
                Cmd::SysfsWrite {
                    path: dev_path(name, "driver_override"),
                    value: "vfio-fsl-mc".to_owned(),
                },
                Cmd::SysfsWrite {
                    path: "/sys/bus/fsl-mc/drivers/vfio-fsl-mc/bind".to_owned(),
                    value: name.to_owned(),
                },
            ]))
        }
        ModelAction::Unbind { obj } => {
            let name = names.name(*obj)?;
            Ok(Drive::Cmds(vec![Cmd::SysfsWrite {
                path: dev_path(name, "driver/unbind"),
                value: name.to_owned(),
            }]))
        }
        ModelAction::ConnectEdge { a, b } => {
            // Always the root, never the endpoints' own container.
            // `dprc connect` runs on the named container's MC handle, and
            // a container restool created without --options carries no
            // DPRC_CFG_OPT_TOPOLOGY_CHANGES_ALLOWED, so that handle may
            // not connect anything (board: MC No privilege, status 0x4).
            // restool only asks for a common ancestor, and the root is
            // the one ancestor guaranteed to hold the privilege.
            cmd(argv(&[
                "dprc",
                "connect",
                names.name(ROOT_DPRC)?,
                &format!("--endpoint1={}", names.endpoint(*a)?),
                &format!("--endpoint2={}", names.endpoint(*b)?),
            ]))
        }
        ModelAction::DisconnectEdge { e } => {
            // Root, for the same reason connect is: disconnect is the
            // other half of the same topology-change privilege, and a
            // default-created container's handle does not hold it.
            //
            // Singular `--endpoint`, not connect's numbered pair: restool
            // takes either end of the edge and removes the whole link
            // (`dprc_commands.c`, cmd_dprc_disconnect).
            cmd(argv(&[
                "dprc",
                "disconnect",
                names.name(ROOT_DPRC)?,
                &format!("--endpoint={}", names.endpoint(*e)?),
            ]))
        }
        // Design recipe: `dprc sync` after mutations; the model's rescan
        // is root-only bus rescan (DPRC-I6: sync is not visibility).
        ModelAction::Rescan { .. } => cmd(argv(&["dprc", "sync"])),
        ModelAction::ChildIrqRefresh { .. } => Ok(Drive::Await(
            "child containers refresh via their own IRQ path (dprc.md unknown 12)",
        )),
        ModelAction::Allocate { .. } | ModelAction::Free { .. } => Ok(Drive::Await(
            "pool draws are kernel-internal (§3 census); not drivable from userspace",
        )),
        ModelAction::Enable { .. } | ModelAction::Disable { .. } => Ok(Drive::Await(
            "enable is always consumer-side; restool enables nothing (§5 step 7)",
        )),
        ModelAction::SetLocked { container, locked } => cmd(argv(&[
            "dprc",
            "set-locked",
            names.name(*container)?,
            if *locked { "--locked=1" } else { "--locked=0" },
        ])),
        ModelAction::LinkChange { .. } => Ok(Drive::Await(
            "PHY reality; link-signaling scenarios observe it, nothing drives it",
        )),
        // Same restool law on the way out: dpdbg destroy names no
        // object, because restool destroys id 0 by definition and
        // refuses an argument (dpdbg_commands.c, cmd_dpdbg_destroy).
        ModelAction::Destroy { obj } if obj.fam == Family::Dpdbg => {
            cmd(argv(&["dpdbg", "destroy"]))
        }
        ModelAction::Destroy { obj } => {
            cmd(argv(&[obj.fam.as_str(), "destroy", names.name(*obj)?]))
        }
    }
}

// --- read-back side --------------------------------------------------

/// One observation probe.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Probe {
    /// A read-only `restool` invocation.
    Restool(Vec<String>),
    /// A read-only `restool` invocation whose answer is one block per
    /// interface (dpsw, dpdmux). Same command as [`Probe::Restool`] —
    /// only the parse differs, and it needs to know which interface the
    /// step asked about.
    RestoolIface {
        /// The invocation (argv after the binary name).
        argv: Vec<String>,
        /// Endpoint port whose block carries the answer.
        port: u32,
    },
    /// A sysfs read (symlink or attribute).
    SysfsRead {
        /// Absolute sysfs path.
        path: String,
    },
}

/// The model's expectation for one step's read-back, in model-space ids.
/// Only the fields the step's probes can observe are `Some`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Expected {
    /// The object the expectation is about.
    pub object: ObjRef,
    /// Listed in its container's `dprc show`.
    pub present: Option<bool>,
    /// `dprc show` plugged-state column.
    pub plugged: Option<bool>,
    /// The `endpoint:` peer (`Some(None)` = no peer expected).
    pub endpoint: Option<Option<EndpointRef>>,
    /// A driver claims the device in sysfs.
    pub driver_bound: Option<bool>,
    /// Live link status, as `<fam> info` prints it. Defaulted so plan
    /// files written before this field existed still deserialize.
    #[serde(default)]
    pub link_up: Option<bool>,
}

impl Expected {
    fn about(object: ObjRef) -> Self {
        Self {
            object,
            present: None,
            plugged: None,
            endpoint: None,
            driver_bound: None,
            link_up: None,
        }
    }
}

/// The probes observing a step's post-state, aligned with [`expect`]:
/// the same actions yield probes and expectations, in the same shape.
///
/// # Errors
///
/// Fails on unresolvable bindings or objects absent from the views —
/// malformed traces, never board conditions.
pub fn readback(
    action: &ModelAction,
    pre: &MachineView,
    post: &MachineView,
    names: &Binding,
) -> Result<Vec<Probe>, String> {
    let show = |c: ObjRef| -> Result<Vec<Probe>, String> {
        Ok(vec![Probe::Restool(argv(&[
            "dprc",
            "show",
            names.name(c)?,
        ]))])
    };
    let info = |e: EndpointRef| -> Result<Vec<Probe>, String> {
        let v = argv(&[e.obj.fam.as_str(), "info", names.name(e.obj)?]);
        Ok(vec![if speaks_per_interface(e.obj.fam) {
            Probe::RestoolIface {
                argv: v,
                port: e.port,
            }
        } else {
            Probe::Restool(v)
        }])
    };
    let driver = |o: ObjRef| -> Result<Vec<Probe>, String> {
        Ok(vec![Probe::SysfsRead {
            path: dev_path(names.name(o)?, "driver"),
        }])
    };
    match action {
        ModelAction::CreateContainer { parent } => show(*parent),
        ModelAction::CreateObject { container, .. } => show(*container),
        ModelAction::AssignChild { dst, .. } => show(*dst),
        ModelAction::Plug { obj } => show(parent_of(post, *obj)?),
        // Unplug is also the kernel's unbind trigger (DPRC-I2), so the
        // driver link is part of what it must be read back on.
        ModelAction::Unplug { obj } => {
            let mut probes = show(parent_of(post, *obj)?)?;
            probes.extend(driver(*obj)?);
            Ok(probes)
        }
        ModelAction::Destroy { obj } => show(parent_of(pre, *obj)?),
        ModelAction::ConnectEdge { a, .. } => info(*a),
        ModelAction::DisconnectEdge { e } => info(*e),
        // A link change has no driving command, but it does have a
        // read-back: the object's own `info`, which is where restool
        // renders link status. A connected object gets its peer's `info`
        // captured beside it — the peer's `, link is up` is DPRC
        // *connection* state, and the whole point of DPMAC-I5 is that it
        // does not move with the link. Capturing both in one step is the
        // evidence for that split; only the first probe is judged.
        ModelAction::LinkChange { obj } => {
            let mut probes = info(EndpointRef { obj: *obj, port: 0 })?;
            if let Some(peer) = post.peer_of(*obj) {
                probes.extend(info(peer)?);
            }
            Ok(probes)
        }
        ModelAction::KernelBind { obj }
        | ModelAction::VfioBind { obj }
        | ModelAction::Unbind { obj } => driver(*obj),
        // No restool-visible observable for the remaining actions; their
        // effects surface through later steps' probes (e.g. a rescan
        // enables a kernel bind) or through hand-authored V-* scenarios.
        _ => Ok(vec![]),
    }
}

/// The expectation the step's [`readback`] probes must confirm, or
/// `None` for steps with no restool-visible observable.
///
/// # Errors
///
/// Fails on objects absent from the views — a malformed trace.
pub fn expect(
    action: &ModelAction,
    pre: &MachineView,
    post: &MachineView,
) -> Result<Option<Expected>, String> {
    let plugged_of = |o: ObjRef| -> Result<bool, String> {
        Ok(post
            .objs
            .get(&o)
            .ok_or_else(|| format!("{o} not in post-state"))?
            .plugged)
    };
    // Only asserted for families whose `info` renders link status the
    // model can predict ([`renders_link_status`]); for the rest the
    // expectation stays silent rather than claim an unobservable field.
    let link_of = |o: ObjRef| -> Result<Option<bool>, String> {
        renders_link_status(o.fam)
            .then(|| {
                Ok(post
                    .objs
                    .get(&o)
                    .ok_or_else(|| format!("{o} not in post-state"))?
                    .link_up)
            })
            .transpose()
    };
    let bound_of = |o: ObjRef| -> Result<bool, String> {
        Ok(post
            .objs
            .get(&o)
            .ok_or_else(|| format!("{o} not in post-state"))?
            .bind
            != BindView::Unbound)
    };
    Ok(match action {
        ModelAction::CreateContainer { .. } | ModelAction::CreateObject { .. } => {
            let created =
                created_object(pre, post).ok_or("create step added no object to the state")?;
            Some(Expected {
                present: Some(true),
                plugged: Some(plugged_of(created)?),
                ..Expected::about(created)
            })
        }
        ModelAction::AssignChild { obj, .. } | ModelAction::Plug { obj } => Some(Expected {
            present: Some(true),
            plugged: Some(plugged_of(*obj)?),
            ..Expected::about(*obj)
        }),
        // `assign --plugged=0` is the kernel's unbind trigger (DPRC-I2),
        // and the model's `unplugT` releases the bind with the plug — so
        // the driver must be gone by the read-back. Read from the
        // post-state rather than restated here, as everywhere else.
        ModelAction::Unplug { obj } => Some(Expected {
            present: Some(true),
            plugged: Some(plugged_of(*obj)?),
            driver_bound: Some(bound_of(*obj)?),
            ..Expected::about(*obj)
        }),
        ModelAction::Destroy { obj } => Some(Expected {
            present: Some(false),
            ..Expected::about(*obj)
        }),
        ModelAction::ConnectEdge { a, b } => Some(Expected {
            endpoint: Some(Some(*b)),
            link_up: link_of(a.obj)?,
            ..Expected::about(a.obj)
        }),
        ModelAction::LinkChange { obj } => Some(Expected {
            present: Some(true),
            link_up: link_of(*obj)?,
            ..Expected::about(*obj)
        }),
        ModelAction::DisconnectEdge { e } => Some(Expected {
            endpoint: Some(post.peer_of(e.obj)),
            ..Expected::about(e.obj)
        }),
        ModelAction::KernelBind { obj } | ModelAction::VfioBind { obj } => Some(Expected {
            // Not always true: some families never bind an object
            // created at runtime on the reference pair (ADR-0008), and
            // the probe leaves them unbound. The trace's post-state is
            // where that verdict lives, so read it rather than restate
            // the table here.
            driver_bound: Some(bound_of(*obj)?),
            ..Expected::about(*obj)
        }),
        ModelAction::Unbind { obj } => Some(Expected {
            driver_bound: Some(false),
            ..Expected::about(*obj)
        }),
        _ => None,
    })
}

// --- observation and judgement ---------------------------------------

/// What the read-back probes actually reported. `None` = not observed.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct Observed {
    /// Listed in `dprc show`.
    pub present: Option<bool>,
    /// `dprc show` plugged-state column.
    pub plugged: Option<bool>,
    /// The `endpoint:` peer as restool prints it.
    pub endpoint: Option<Option<String>>,
    /// A driver claims the device in sysfs.
    pub driver_bound: Option<bool>,
    /// Live link status as `<fam> info` renders it.
    pub link_up: Option<bool>,
}

/// Parses probe outputs into an [`Observed`], for the object named
/// `object_name` on the board. `outputs[i]` is the captured output of
/// `probes[i]`; a sysfs probe against a missing path captures empty.
///
/// # Errors
///
/// Fails when `outputs` does not align with `probes`.
pub fn observe(
    probes: &[Probe],
    outputs: &[String],
    object_name: &str,
) -> Result<Observed, String> {
    if probes.len() != outputs.len() {
        return Err(format!(
            "{} probes but {} outputs",
            probes.len(),
            outputs.len()
        ));
    }
    let mut obs = Observed::default();
    // A step may carry more than one `info` probe, and only the first is
    // the observation: a link change also probes its peer, whose output
    // is captured as evidence of the DPMAC-I5 split and must not be read
    // as this object's answer. First `info` wins; later ones live in
    // their result file.
    let mut info_read = false;
    for (probe, out) in probes.iter().zip(outputs) {
        match probe {
            Probe::Restool(v)
                if v.first().map(String::as_str) == Some("dprc")
                    && v.get(1).map(String::as_str) == Some("show") =>
            {
                let row = out
                    .lines()
                    .find(|l| l.split_whitespace().next() == Some(object_name));
                obs.present = Some(row.is_some());
                obs.plugged = row.map(|l| !l.split_whitespace().any(|t| t == "unplugged"));
            }
            Probe::Restool(v) if v.get(1).map(String::as_str) == Some("info") => {
                if !info_read {
                    info_read = true;
                    if reports_absent(out) {
                        // Not an info block at all: the object is not
                        // there, and a nonexistent object has no peer.
                        obs.present = Some(false);
                        obs.endpoint = Some(None);
                    } else {
                        obs.endpoint = Some(parse_endpoint_line(
                            v.first().map_or("", String::as_str),
                            out,
                        ));
                        // Nothing on stdout is the object's absence too:
                        // restool sends some of its errors to stderr,
                        // which the probe discards.
                        obs.present = Some(!out.trim().is_empty());
                        obs.link_up = parse_link_status_line(out);
                    }
                }
            }
            Probe::Restool(v) => {
                return Err(format!("unrecognized read-back probe: {v:?}"));
            }
            Probe::RestoolIface { port, .. } => {
                obs.endpoint = Some(parse_interface_block(*port, out));
            }
            Probe::SysfsRead { .. } => {
                obs.driver_bound = Some(!out.trim().is_empty());
            }
        }
    }
    Ok(obs)
}

/// Whether an `<fam> info` capture is restool's "no such object" answer
/// rather than an info block. restool prints `<obj> does not exist` on
/// *stdout*, so the probe captures it verbatim (board V-LINK-2 step 0:
/// `dpni.1 does not exist`, the consumer container absent on a bare
/// boot). Read as an ordinary info it would look like a present object
/// with no endpoint line; it is the opposite — an absent one.
fn reports_absent(info_output: &str) -> bool {
    info_output.trim().ends_with("does not exist")
}

/// The peer object reference an `<fam> info` read-back reports, e.g.
/// `endpoint: dpmac.7, link is up` → `dpmac.7`.
///
/// restool words this per family rather than uniformly — the same trap
/// the dpdcei and dpseci create flags sprang. dpci prints `connected
/// peer: dpci.1`, or `no peer` when it has none (`dpci_commands.c:236`).
/// dpni and dpmac print `endpoint: <type>.<id>` (a dpsw or dpdmux peer
/// adds its interface id), or `No object associated`
/// (`dpni_commands.c:507`, `dpmac_commands.c:177`). Either unconnected
/// wording, and an absent line, mean no peer.
fn parse_endpoint_line(fam: &str, info_output: &str) -> Option<String> {
    let prefix = if fam == Family::Dpci.as_str() {
        "connected peer:"
    } else {
        "endpoint:"
    };
    let line = info_output
        .lines()
        .find_map(|l| l.trim().strip_prefix(prefix))?;
    let token = line.split(',').next()?.trim();
    let (kind, _idx) = token.split_once('.')?;
    kind.starts_with("dp").then(|| token.to_owned())
}

/// The live link status an `<fam> info` read-back reports, or `None`
/// when the output carries no such line.
///
/// restool prints it as `link status: <n> - up` / `- down` / `- error
/// state` (`dpci_commands.c:245`, `dpni_commands.c:621`); the numeric
/// field is the same value spelled twice, so the word is what is read.
///
/// This is deliberately NOT the `, link is up` that `dpmac info` and the
/// `endpoint:` line of `dpni info` append (`dpmac_commands.c:190`,
/// `dpni_commands.c:520`). That text is the DPRC *connection* state that
/// `dprc_get_connection` returned, not MAC link state, and reading it as
/// link state is exactly what DPMAC-I5 forbids — the different wording
/// is what keeps it out of this parse.
fn parse_link_status_line(info_output: &str) -> Option<bool> {
    let line = info_output
        .lines()
        .find_map(|l| l.trim().strip_prefix("link status:"))?;
    match line.rsplit('-').next()?.trim() {
        "up" => Some(true),
        "down" => Some(false),
        // "error state", or wording a later restool invents: unobserved
        // beats guessed.
        _ => None,
    }
}

/// Whether `<fam> info` renders a link status the model can predict, and
/// so whether a step's expectation may assert one.
///
/// dpci and dpni. A dpci link has no PHY behind it — DPCI-I5 holds that
/// it follows the two ends' consumer enables, which the model carries —
/// so the model's `linkUp` is the whole answer. `dpni info` prints the
/// identical `link status:` line (`dpni_commands.c:621`), and the V-LINK
/// scenarios put the cable behind it into the model: an operator drives
/// the physical flap and the trace's `linkChange` steps carry the state
/// it produces, so the field is predicted and therefore asserted.
///
/// dpmac stays out: it prints no link status at all, only the DPRC
/// connection state that DPMAC-I5 forbids reading as one.
fn renders_link_status(fam: Family) -> bool {
    matches!(fam, Family::Dpci | Family::Dpni)
}

/// Whether `<fam> info` reports peers as one block per interface rather
/// than a single line — the multi-port families (§2: `dpsw.N.M`,
/// `dpdmux.N.M`).
fn speaks_per_interface(fam: Family) -> bool {
    matches!(fam, Family::Dpsw | Family::Dpdmux)
}

/// The peer reported for one interface of a multi-port object. dpsw and
/// dpdmux print an `endpoints:` section of per-interface blocks
/// (`dpsw_commands.c:388`, `dpdmux_commands.c:260`):
///
/// ```text
/// endpoints:
/// interface 0:
///     connection: dpmac.4
///     link state: down
/// interface 1:
///     connection: none
///     link state: n/a
/// ```
///
/// so the answer depends on *which* interface was asked about — hence
/// the port. `none` is the unconnected wording here, where dpni says
/// "No object associated" and dpci says "no peer". A switch-family peer
/// always prints three-part (`dpsw.0.1`), any other peer two-part; both
/// are returned verbatim, since that is what the board calls the peer.
/// dpdmux prints one block more than its `--num-ifs`, interface 0 being
/// the uplink.
fn parse_interface_block(port: u32, info_output: &str) -> Option<String> {
    let mut lines = info_output
        .lines()
        .skip_while(|l| l.trim() != format!("interface {port}:"));
    lines.next()?;
    // Stop at the next block: a `connection:` further down belongs to
    // another interface, not this one.
    let line = lines
        .take_while(|l| !l.trim().starts_with("interface "))
        .find_map(|l| l.trim().strip_prefix("connection:"))?;
    let token = line.trim();
    let (kind, _rest) = token.split_once('.')?;
    kind.starts_with("dp").then(|| token.to_owned())
}

/// The driving command's exit status — auxiliary evidence only, never an
/// observation (DPNI-I6, DPMAC-I8: restool exits 0 on dead options and
/// partial failures).
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitEvidence {
    /// Whether the process reported success.
    pub ok: bool,
}

/// The verdict on one executed step.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct StepVerdict {
    /// Read-back conformance: every expected field matched.
    pub pass: bool,
    /// Field-level mismatches; empty iff `pass`.
    pub mismatches: Vec<String>,
    /// The exit status, reported beside the verdict but never part of it.
    pub exit: ExitEvidence,
}

/// Judges one step: the observation (read-back) against the model's
/// expectation. The exit status is attached as auxiliary evidence and
/// does not influence `pass` in either direction — a clean exit with a
/// diverging read-back fails, a dirty exit with a conforming read-back
/// passes (mbt-harness spec, "Observation is read-back, not exit
/// status").
///
/// # Errors
///
/// Fails when the expected endpoint's object has no binding in `names`.
pub fn judge(
    expected: &Expected,
    observed: &Observed,
    names: &Binding,
    exit: ExitEvidence,
) -> Result<StepVerdict, String> {
    let mut mismatches = Vec::new();
    let mut check = |field: &str, want: String, got: Option<String>| match got {
        Some(got) if got == want => {}
        Some(got) => mismatches.push(format!("{field}: expected {want}, read back {got}")),
        None => mismatches.push(format!("{field}: expected {want}, not observed")),
    };
    if let Some(want) = expected.present {
        check(
            "present",
            want.to_string(),
            observed.present.map(|b| b.to_string()),
        );
    }
    if let Some(want) = expected.plugged {
        check(
            "plugged",
            want.to_string(),
            observed.plugged.map(|b| b.to_string()),
        );
    }
    if let Some(want) = expected.endpoint {
        let want = match want {
            Some(e) => names.endpoint(e)?,
            None => "none".to_owned(),
        };
        let got = observed
            .endpoint
            .as_ref()
            .map(|p| p.clone().unwrap_or_else(|| "none".to_owned()));
        check("endpoint", want, got);
    }
    if let Some(want) = expected.driver_bound {
        check(
            "driver_bound",
            want.to_string(),
            observed.driver_bound.map(|b| b.to_string()),
        );
    }
    if let Some(want) = expected.link_up {
        check(
            "link_up",
            want.to_string(),
            observed.link_up.map(|b| b.to_string()),
        );
    }
    Ok(StepVerdict {
        pass: mismatches.is_empty(),
        mismatches,
        exit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dprc1() -> ObjRef {
        ObjRef {
            fam: Family::Dprc,
            num: 1,
        }
    }

    fn dpni(num: u32) -> ObjRef {
        ObjRef {
            fam: Family::Dpni,
            num,
        }
    }

    fn obj(parent: Option<ObjRef>, plugged: bool, bind: BindView) -> ObjView {
        ObjView {
            parent,
            plugged,
            bus_visible: true,
            bind,
            link_up: false,
        }
    }

    /// A state with dprc.1 holding a dpmac.7 and, optionally, a dpni.
    fn state(with_dpni: Option<(u32, bool)>) -> MachineView {
        let mut v = MachineView::default();
        v.objs.insert(dprc1(), obj(None, true, BindView::Unbound));
        v.objs.insert(
            ObjRef {
                fam: Family::Dpmac,
                num: 7,
            },
            obj(Some(dprc1()), true, BindView::Unbound),
        );
        if let Some((num, plugged)) = with_dpni {
            v.objs
                .insert(dpni(num), obj(Some(dprc1()), plugged, BindView::Unbound));
        }
        v
    }

    #[test]
    fn drive_renders_dpseci_create_with_mandatory_flags() {
        let pre = state(None);
        let names = Binding::seed(&pre);

        let create = ModelAction::CreateObject {
            fam: Family::Dpseci,
            container: dprc1(),
        };
        assert_eq!(
            drive(&create, &pre, &names).unwrap(),
            Drive::Cmds(vec![Cmd::Restool(argv(&[
                "--script",
                "dpseci",
                "create",
                "--num-queues=2",
                "--priorities=2,4",
                "--container=dprc.1",
            ]))])
        );
    }

    /// A create-argument override renders in place of the default table
    /// for that family only, and the default `drive` still renders the
    /// table (byte-compatible with every committed suite).
    #[test]
    fn drive_with_renders_create_argument_overrides() {
        let pre = state(None);
        let names = Binding::seed(&pre);
        let create = ModelAction::CreateObject {
            fam: Family::Dpio,
            container: dprc1(),
        };

        let overrides: CreateArgs = [(
            Family::Dpio,
            vec![
                "--channel-mode=DPIO_NO_CHANNEL".to_owned(),
                "--num-priorities=8".to_owned(),
            ],
        )]
        .into_iter()
        .collect();
        assert_eq!(
            drive_with(&create, &pre, &names, &overrides).unwrap(),
            Drive::Cmds(vec![Cmd::Restool(argv(&[
                "--script",
                "dpio",
                "create",
                "--channel-mode=DPIO_NO_CHANNEL",
                "--num-priorities=8",
                "--container=dprc.1",
            ]))])
        );

        // The default table is untouched: the plain wrapper renders the
        // local-channel dpio every existing suite was generated with.
        assert_eq!(
            drive(&create, &pre, &names).unwrap(),
            Drive::Cmds(vec![Cmd::Restool(argv(&[
                "--script",
                "dpio",
                "create",
                "--channel-mode=DPIO_LOCAL_CHANNEL",
                "--num-priorities=8",
                "--container=dprc.1",
            ]))])
        );
    }

    /// A `--create-args dprc=…` override rides on the container create,
    /// the same way it rides on an object create — the option mask lands
    /// after the parent reference. The V-DPRC-2 option-bit suites depend
    /// on this to set the container's permission mask.
    #[test]
    fn drive_with_renders_container_create_argument_overrides() {
        let pre = state(None);
        let names = Binding::seed(&pre);
        let create = ModelAction::CreateContainer { parent: dprc1() };

        let overrides: CreateArgs = [(
            Family::Dprc,
            vec!["--options=DPRC_CFG_OPT_SPAWN_ALLOWED".to_owned()],
        )]
        .into_iter()
        .collect();
        assert_eq!(
            drive_with(&create, &pre, &names, &overrides).unwrap(),
            Drive::Cmds(vec![Cmd::Restool(argv(&[
                "--script",
                "dprc",
                "create",
                "dprc.1",
                "--options=DPRC_CFG_OPT_SPAWN_ALLOWED",
            ]))])
        );

        // The default table is empty for dprc, so the plain wrapper
        // renders the bare container create every committed suite uses.
        assert_eq!(
            drive(&create, &pre, &names).unwrap(),
            Drive::Cmds(vec![Cmd::Restool(argv(&[
                "--script", "dprc", "create", "dprc.1",
            ]))])
        );
    }

    #[test]
    fn create_args_flag_parses_family_and_arguments() {
        let (fam, args) =
            CreateArgs::parse_flag("dpio=--channel-mode=DPIO_NO_CHANNEL --num-priorities=8")
                .unwrap();
        assert_eq!(fam, Family::Dpio);
        assert_eq!(
            args,
            vec!["--channel-mode=DPIO_NO_CHANNEL", "--num-priorities=8"]
        );

        // Unknown family and empty arguments both fail.
        assert!(CreateArgs::parse_flag("dpnope=--x").is_err());
        assert!(CreateArgs::parse_flag("dpio=   ").is_err());
        // A value with no `=` is not <fam>=<args>.
        assert!(CreateArgs::parse_flag("dpio").is_err());
    }

    /// The endpoints sit in a scratch container, but the connect must
    /// still run on the root: a default-created container's handle has
    /// no topology-change privilege and the MC refuses it there.
    #[test]
    fn drive_connects_child_container_objects_through_the_root() {
        let scratch = ObjRef {
            fam: Family::Dprc,
            num: 2,
        };
        let dpci = |num| ObjRef {
            fam: Family::Dpci,
            num,
        };

        let mut pre = state(None);
        pre.objs
            .insert(scratch, obj(Some(dprc1()), true, BindView::Unbound));
        pre.objs
            .insert(dpci(0), obj(Some(scratch), true, BindView::Unbound));
        pre.objs
            .insert(dpci(1), obj(Some(scratch), true, BindView::Unbound));
        let names = Binding::seed(&pre);

        let connect = ModelAction::ConnectEdge {
            a: EndpointRef {
                obj: dpci(0),
                port: 0,
            },
            b: EndpointRef {
                obj: dpci(1),
                port: 0,
            },
        };
        assert_eq!(
            drive(&connect, &pre, &names).unwrap(),
            Drive::Cmds(vec![Cmd::Restool(argv(&[
                "dprc",
                "connect",
                "dprc.1",
                "--endpoint1=dpci.0",
                "--endpoint2=dpci.1",
            ]))])
        );

        // Disconnect is the same privilege, so the same container — and
        // one `--endpoint`, since either end names the whole edge.
        let disconnect = ModelAction::DisconnectEdge {
            e: EndpointRef {
                obj: dpci(0),
                port: 0,
            },
        };
        assert_eq!(
            drive(&disconnect, &pre, &names).unwrap(),
            Drive::Cmds(vec![Cmd::Restool(argv(&[
                "dprc",
                "disconnect",
                "dprc.1",
                "--endpoint=dpci.0",
            ]))])
        );
    }

    #[test]
    fn drive_renders_dpdcei_create_with_mandatory_flags() {
        let pre = state(None);
        let names = Binding::seed(&pre);

        let create = ModelAction::CreateObject {
            fam: Family::Dpdcei,
            container: dprc1(),
        };
        assert_eq!(
            drive(&create, &pre, &names).unwrap(),
            Drive::Cmds(vec![Cmd::Restool(argv(&[
                "--script",
                "dpdcei",
                "create",
                "--engine=DPDCEI_ENGINE_DECOMPRESSION",
                "--priority=1",
                "--container=dprc.1",
            ]))])
        );
    }

    #[test]
    fn drive_renders_dpdbg_create_and_destroy_bare() {
        // restool's dpdbg verbs take no arguments: create pins the
        // container to the root and the id to 0 itself (--container is
        // an unrecognized option), and destroy refuses an object name
        // outright ("Unexpected argument") because it always destroys
        // id 0. dpdbg needs no binding here for the same reason.
        let mut pre = state(None);
        let rtc = ObjRef {
            fam: Family::Dprtc,
            num: 0,
        };
        pre.objs
            .insert(rtc, obj(Some(dprc1()), true, BindView::Unbound));
        let names = Binding::seed(&pre);

        assert_eq!(
            drive(
                &ModelAction::CreateObject {
                    fam: Family::Dpdbg,
                    container: dprc1(),
                },
                &pre,
                &names
            )
            .unwrap(),
            Drive::Cmds(vec![Cmd::Restool(argv(&["--script", "dpdbg", "create"]))])
        );
        assert_eq!(
            drive(
                &ModelAction::Destroy {
                    obj: ObjRef {
                        fam: Family::Dpdbg,
                        num: 0,
                    },
                },
                &pre,
                &names
            )
            .unwrap(),
            Drive::Cmds(vec![Cmd::Restool(argv(&["dpdbg", "destroy"]))])
        );

        // The exception is dpdbg's alone: the other root-container
        // singleton keeps both arguments (`dprtc create --container`,
        // `dprtc destroy <object>`).
        assert_eq!(
            drive(
                &ModelAction::CreateObject {
                    fam: Family::Dprtc,
                    container: dprc1(),
                },
                &pre,
                &names
            )
            .unwrap(),
            Drive::Cmds(vec![Cmd::Restool(argv(&[
                "--script",
                "dprtc",
                "create",
                "--container=dprc.1",
            ]))])
        );
        assert_eq!(
            drive(&ModelAction::Destroy { obj: rtc }, &pre, &names).unwrap(),
            Drive::Cmds(vec![Cmd::Restool(argv(&["dprtc", "destroy", "dprtc.0"]))])
        );
    }

    #[test]
    fn drive_renders_create_plug_connect_destroy() {
        let pre = state(Some((100, false)));
        let mut names = Binding::seed(&state(None));
        names.bind_created(dpni(100), "dpni.5\n").unwrap();

        let create = ModelAction::CreateObject {
            fam: Family::Dpio,
            container: dprc1(),
        };
        assert_eq!(
            drive(&create, &pre, &names).unwrap(),
            Drive::Cmds(vec![Cmd::Restool(argv(&[
                "--script",
                "dpio",
                "create",
                "--channel-mode=DPIO_LOCAL_CHANNEL",
                "--num-priorities=8",
                "--container=dprc.1",
            ]))])
        );

        let plug = ModelAction::Plug { obj: dpni(100) };
        assert_eq!(
            drive(&plug, &pre, &names).unwrap(),
            Drive::Cmds(vec![Cmd::Restool(argv(&[
                "dprc",
                "assign",
                "dprc.1",
                "--object=dpni.5",
                "--plugged=1",
            ]))])
        );

        let connect = ModelAction::ConnectEdge {
            a: EndpointRef {
                obj: dpni(100),
                port: 0,
            },
            b: EndpointRef {
                obj: ObjRef {
                    fam: Family::Dpmac,
                    num: 7,
                },
                port: 0,
            },
        };
        assert_eq!(
            drive(&connect, &pre, &names).unwrap(),
            Drive::Cmds(vec![Cmd::Restool(argv(&[
                "dprc",
                "connect",
                "dprc.1",
                "--endpoint1=dpni.5",
                "--endpoint2=dpmac.7",
            ]))])
        );

        let destroy = ModelAction::Destroy { obj: dpni(100) };
        assert_eq!(
            drive(&destroy, &pre, &names).unwrap(),
            Drive::Cmds(vec![Cmd::Restool(argv(&["dpni", "destroy", "dpni.5"]))])
        );
    }

    #[test]
    fn moves_render_one_hop_as_assign_or_unassign_and_refuse_the_rest() {
        // dprc.1 ─┬─ dprc.2 ── dpni.100
        //         └─ dprc.3
        let dprc = |num| ObjRef {
            fam: Family::Dprc,
            num,
        };
        let mut pre = MachineView::default();
        pre.objs.insert(dprc(1), obj(None, true, BindView::Unbound));
        pre.objs
            .insert(dprc(2), obj(Some(dprc(1)), false, BindView::Unbound));
        pre.objs
            .insert(dprc(3), obj(Some(dprc(1)), false, BindView::Unbound));
        pre.objs
            .insert(dpni(100), obj(Some(dprc(2)), false, BindView::Unbound));
        let names = Binding::seed(&pre);

        // up-hop: child → its container's parent renders unassign on the
        // destination, --child naming the current holder.
        let up = ModelAction::AssignChild {
            obj: dpni(100),
            dst: dprc(1),
        };
        assert_eq!(
            drive(&up, &pre, &names).unwrap(),
            Drive::Cmds(vec![Cmd::Restool(argv(&[
                "dprc",
                "unassign",
                "dprc.1",
                "--object=dpni.100",
                "--child=dprc.2",
            ]))])
        );

        // sibling: not a tree edge — refused as a malformed trace, the
        // board refused the equivalent command outright (V-DPRC-1).
        let sibling = ModelAction::AssignChild {
            obj: dpni(100),
            dst: dprc(3),
        };
        drive(&sibling, &pre, &names).unwrap_err();

        // down-hop: container → direct child renders assign on the holder.
        let mut pre_down = pre.clone();
        pre_down
            .objs
            .insert(dpni(100), obj(Some(dprc(1)), false, BindView::Unbound));
        let down = ModelAction::AssignChild {
            obj: dpni(100),
            dst: dprc(3),
        };
        assert_eq!(
            drive(&down, &pre_down, &names).unwrap(),
            Drive::Cmds(vec![Cmd::Restool(argv(&[
                "dprc",
                "assign",
                "dprc.1",
                "--object=dpni.100",
                "--child=dprc.3",
            ]))])
        );
    }

    #[test]
    fn board_side_transitions_are_awaited_not_driven() {
        let pre = state(Some((100, true)));
        let names = Binding::seed(&pre);
        for action in [
            ModelAction::KernelBind { obj: dpni(100) },
            ModelAction::Enable { obj: dpni(100) },
            ModelAction::LinkChange { obj: dpni(100) },
            ModelAction::ChildIrqRefresh { container: dprc1() },
        ] {
            assert!(
                matches!(drive(&action, &pre, &names).unwrap(), Drive::Await(_)),
                "{action:?} must be awaited"
            );
        }
    }

    #[test]
    fn judgement_trusts_readback_over_exit_status() {
        let names = Binding::seed(&state(None));
        // A plug's expectation: present and plugged.
        let expected = Expected {
            present: Some(true),
            plugged: Some(true),
            ..Expected::about(dpni(100))
        };

        // Exit 0, but the read-back says unplugged: the step FAILS on the
        // observation; the clean exit is auxiliary.
        let diverging = Observed {
            present: Some(true),
            plugged: Some(false),
            ..Observed::default()
        };
        let v = judge(&expected, &diverging, &names, ExitEvidence { ok: true }).unwrap();
        assert!(!v.pass);
        assert!(v.exit.ok);
        assert_eq!(
            v.mismatches,
            vec!["plugged: expected true, read back false"]
        );

        // Exit nonzero, but the read-back shows the expected state: the
        // step PASSES; the dirty exit is auxiliary.
        let conforming = Observed {
            present: Some(true),
            plugged: Some(true),
            ..Observed::default()
        };
        let v = judge(&expected, &conforming, &names, ExitEvidence { ok: false }).unwrap();
        assert!(v.pass);
        assert!(!v.exit.ok);
    }

    #[test]
    fn observe_parses_show_info_and_sysfs() {
        let show = Probe::Restool(argv(&["dprc", "show", "dprc.1"]));
        let out = "dprc.1 contains 3 objects:\n\
                   object          label           plugged-state\n\
                   dpni.5          scratch         unplugged\n\
                   dpmac.7                         plugged\n"
            .to_owned();
        let obs = observe(std::slice::from_ref(&show), &[out], "dpni.5").unwrap();
        assert_eq!(obs.present, Some(true));
        assert_eq!(obs.plugged, Some(false));

        let obs = observe(
            &[show],
            &["object label plugged-state\n".to_owned()],
            "dpni.5",
        )
        .unwrap();
        assert_eq!(obs.present, Some(false));

        let info = Probe::Restool(argv(&["dpni", "info", "dpni.5"]));
        let out = "dpni version: 7.8\nendpoint: dpmac.7, link is down\n".to_owned();
        let obs = observe(std::slice::from_ref(&info), &[out], "dpni.5").unwrap();
        assert_eq!(obs.endpoint, Some(Some("dpmac.7".to_owned())));
        let obs = observe(
            &[info],
            &["endpoint: No object associated\n".to_owned()],
            "dpni.5",
        )
        .unwrap();
        assert_eq!(obs.endpoint, Some(None));

        // dpci speaks its own dialect: `connected peer:`, and `no peer`
        // where dpni says `No object associated`. Text as the board
        // prints it.
        let info = Probe::Restool(argv(&["dpci", "info", "dpci.0"]));
        let out = "dpci version: 3.4\n\
                   dpci id: 0\n\
                   plugged state: unplugged\n\
                   num_priorities: 1\n\
                   connected peer: dpci.1\n\
                   peer's num_of_priorities: 1\n\
                   link status: 0 - down\n"
            .to_owned();
        let obs = observe(std::slice::from_ref(&info), &[out], "dpci.0").unwrap();
        assert_eq!(obs.endpoint, Some(Some("dpci.1".to_owned())));
        let out = "dpci version: 3.4\n\
                   dpci id: 0\n\
                   plugged state: unplugged\n\
                   num_priorities: 1\n\
                   connected peer: no peer\n\
                   link status: 0 - down\n"
            .to_owned();
        let obs = observe(&[info], &[out], "dpci.0").unwrap();
        assert_eq!(obs.endpoint, Some(None));

        let probe = Probe::SysfsRead {
            path: dev_path("dpni.5", "driver"),
        };
        let obs = observe(
            std::slice::from_ref(&probe),
            &["fsl_dpaa2_eth\n".to_owned()],
            "dpni.5",
        )
        .unwrap();
        assert_eq!(obs.driver_bound, Some(true));
        let obs = observe(&[probe], &[String::new()], "dpni.5").unwrap();
        assert_eq!(obs.driver_bound, Some(false));
    }

    /// restool answers `info` on an object that is not there with
    /// `<obj> does not exist`, on stdout. V-LINK-2 step 0 meets it on a
    /// bare boot: the consumer container and its dpni were captured from
    /// a provisioned moment and do not exist yet, so the auxiliary
    /// disconnect no-ops and reads this back. It must observe an absent
    /// object with no peer — and the step's `endpoint=none` expectation
    /// must keep passing on it.
    #[test]
    fn observe_reads_does_not_exist_as_an_absent_object() {
        let info = Probe::Restool(argv(&["dpni", "info", "dpni.1"]));
        let obs = observe(&[info], &["dpni.1 does not exist\n".to_owned()], "dpni.1").unwrap();
        assert_eq!(obs.present, Some(false));
        assert_eq!(obs.endpoint, Some(None));
        assert_eq!(obs.link_up, None);

        let names = Binding::seed(&state(Some((1, true))));
        let v = judge(
            &Expected {
                endpoint: Some(None),
                ..Expected::about(dpni(1))
            },
            &obs,
            &names,
            ExitEvidence { ok: true },
        )
        .unwrap();
        assert!(v.pass, "{:?}", v.mismatches);

        // Nothing more is claimed than absence: an expectation that the
        // object is present fails against the same read-back.
        let v = judge(
            &Expected {
                present: Some(true),
                ..Expected::about(dpni(1))
            },
            &obs,
            &names,
            ExitEvidence { ok: true },
        )
        .unwrap();
        assert!(!v.pass);
        assert_eq!(
            v.mismatches,
            vec!["present: expected true, read back false"]
        );
    }

    #[test]
    fn binding_rejects_foreign_creates_and_unbound_refs() {
        let mut names = Binding::seed(&state(None));
        assert!(names.bind_created(dpni(100), "dpbp.2\n").is_err());
        assert!(names.name(dpni(100)).is_err());
        names.bind_created(dpni(100), "dpni.5\n").unwrap();
        assert_eq!(names.name(dpni(100)).unwrap(), "dpni.5");
        // Boot-born objects keep their literal names.
        assert_eq!(
            names
                .name(ObjRef {
                    fam: Family::Dpmac,
                    num: 7
                })
                .unwrap(),
            "dpmac.7"
        );
    }

    /// A hand-written two-state `--mbt` trace: init (dprc.1 + dpmac.7),
    /// then `createObject` picking `fam=Dpni, c=dprc.1`.
    #[test]
    fn mbt_trace_parses_actions_and_views() {
        let none = r##"{"tag": "None", "value": {"#tup": []}}"##;
        let obj_state = |parent: &str, plugged: bool| {
            format!(
                r##"{{"parent": {parent}, "plugged": {plugged}, "busVisible": true,
                    "bind": {{"tag": "Unbound", "value": {{"#tup": []}}}},
                    "enabled": false, "pristine": true, "linkUp": false,
                    "allocatedBy": {none}, "hwId": {{"#bigint": "1"}}}}"##
            )
        };
        let id = |fam: &str, num: u32| {
            format!(
                r##"{{"fam": {{"tag": "{fam}", "value": {{"#tup": []}}}}, "num": {{"#bigint": "{num}"}}}}"##
            )
        };
        let in_root = format!(r#"{{"tag": "Some", "value": {}}}"#, id("Dprc", 1));
        let state =
            |objs: &str| format!(r##"{{"objs": {{"#map": [{objs}]}}, "conns": {{"#set": []}}}}"##);
        let init_objs = format!(
            "[{}, {}], [{}, {}]",
            id("Dprc", 1),
            obj_state(none, true),
            id("Dpmac", 7),
            obj_state(&in_root, true)
        );
        let post_objs = format!(
            "{init_objs}, [{}, {}]",
            id("Dpni", 100),
            obj_state(&in_root, false)
        );
        let picks = format!(
            r##"{{"fam": {{"tag": "Some", "value": {{"tag": "Dpni", "value": {{"#tup": []}}}}}},
                "c": {{"tag": "Some", "value": {}}}, "o": {none}}}"##,
            id("Dprc", 1)
        );
        let trace = format!(
            r#"{{"vars": ["mbt::actionTaken", "main::machine::s", "mbt::nondetPicks"],
                "states": [
                  {{"main::machine::s": {init}, "mbt::actionTaken": "init", "mbt::nondetPicks": {{}}}},
                  {{"main::machine::s": {post}, "mbt::actionTaken": "createObject", "mbt::nondetPicks": {picks}}}
                ]}}"#,
            init = state(&init_objs),
            post = state(&post_objs),
        );

        let parsed = parse_mbt_trace(&trace).unwrap();
        assert_eq!(parsed.init.objs.len(), 2);
        assert_eq!(parsed.steps.len(), 1);
        assert_eq!(
            parsed.steps[0].action,
            ModelAction::CreateObject {
                fam: Family::Dpni,
                container: dprc1()
            }
        );
        let created = created_object(&parsed.init, &parsed.steps[0].post).unwrap();
        assert_eq!(created, dpni(100));
        assert!(!parsed.steps[0].post.objs[&created].plugged);

        // The expectation for the create step reads straight off the views.
        let expected = expect(&parsed.steps[0].action, &parsed.init, &parsed.steps[0].post)
            .unwrap()
            .unwrap();
        assert_eq!(expected.object, created);
        assert_eq!(expected.present, Some(true));
        assert_eq!(expected.plugged, Some(false));
    }

    /// dpsw and dpdmux answer per interface, so the same output means
    /// different things depending on which port was asked about.
    #[test]
    fn observe_reads_the_queried_interface_block() {
        // Two switch ports, one connected: a dpmac on interface 0 (a
        // non-switch peer, so two-part) and nothing on interface 1.
        let sw = "dpsw version: 8.9\n\
                  dpsw id: 0\n\
                  plugged state: plugged\n\
                  endpoints:\n\
                  interface 0:\n\
                  \tconnection: dpmac.5\n\
                  \tlink state: down\n\
                  interface 1:\n\
                  \tconnection: none\n\
                  \tlink state: n/a\n"
            .to_owned();
        let at = |port| Probe::RestoolIface {
            argv: argv(&["dpsw", "info", "dpsw.0"]),
            port,
        };
        let obs = observe(&[at(0)], std::slice::from_ref(&sw), "dpsw.0").unwrap();
        assert_eq!(obs.endpoint, Some(Some("dpmac.5".to_owned())));
        // Port 1 must read its own block, not interface 0's line.
        let obs = observe(&[at(1)], &[sw], "dpsw.0").unwrap();
        assert_eq!(obs.endpoint, Some(None));

        // A switch-family peer always prints three-part, port included.
        let sw = "endpoints:\n\
                  interface 0:\n\
                  \tconnection: none\n\
                  \tlink state: n/a\n\
                  interface 1:\n\
                  \tconnection: dpsw.0.1\n\
                  \tlink state: up\n"
            .to_owned();
        let obs = observe(&[at(1)], &[sw], "dpsw.0").unwrap();
        assert_eq!(obs.endpoint, Some(Some("dpsw.0.1".to_owned())));

        // dpdmux prints one block more than its --num-ifs; interface 0
        // is the uplink, which is the one a suite connects to a dpmac.
        let mux = "dpdmux version: 6.9\n\
                   endpoints:\n\
                   interface 0:\n\
                   \tconnection: dpmac.4\n\
                   \tlink state: down\n\
                   interface 1:\n\
                   \tconnection: none\n\
                   \tlink state: n/a\n\
                   num_ifs: 1\n"
            .to_owned();
        let obs = observe(
            &[Probe::RestoolIface {
                argv: argv(&["dpdmux", "info", "dpdmux.0"]),
                port: 0,
            }],
            &[mux],
            "dpdmux.0",
        )
        .unwrap();
        assert_eq!(obs.endpoint, Some(Some("dpmac.4".to_owned())));
    }

    /// The dpci link line, in restool's own wording, and the dpmac line
    /// that must never be read as one (DPMAC-I5).
    #[test]
    fn observe_reads_the_dpci_link_status_line_and_no_other() {
        let dpci_info = |status: &str| {
            format!(
                "dpci version: 3.4\n\
                 dpci id: 0\n\
                 plugged state: unplugged\n\
                 num_priorities: 1\n\
                 connected peer: dpci.1\n\
                 peer's num_of_priorities: 1\n\
                 link status: {status}\n"
            )
        };
        let probe = Probe::Restool(argv(&["dpci", "info", "dpci.0"]));

        let obs = observe(
            std::slice::from_ref(&probe),
            &[dpci_info("0 - down")],
            "dpci.0",
        )
        .unwrap();
        assert_eq!(obs.link_up, Some(false));
        let obs = observe(
            std::slice::from_ref(&probe),
            &[dpci_info("1 - up")],
            "dpci.0",
        )
        .unwrap();
        assert_eq!(obs.link_up, Some(true));
        // Neither up nor down: unobserved beats guessed.
        let obs = observe(&[probe], &[dpci_info("2 - error state")], "dpci.0").unwrap();
        assert_eq!(obs.link_up, None);

        // `dpmac info` says "link is up", and that is the DPRC connection
        // state, not MAC link state (DPMAC-I5): it must not be read here.
        let obs = observe(
            &[Probe::Restool(argv(&["dpmac", "info", "dpmac.4"]))],
            &["dpmac version: 4.5\nendpoint: dpni.5, link is up\n".to_owned()],
            "dpmac.4",
        )
        .unwrap();
        assert_eq!(obs.link_up, None);
    }

    /// The suite's whole point: connecting two dpcis with nothing enabled
    /// must expect the link *down* — and a board that says otherwise must
    /// fail the step, not slip through.
    #[test]
    fn connect_carries_the_link_expectation_for_dpci() {
        let dpci = |num| ObjRef {
            fam: Family::Dpci,
            num,
        };
        let ep = |o| EndpointRef { obj: o, port: 0 };

        let mut pre = state(None);
        pre.objs
            .insert(dpci(0), obj(Some(dprc1()), false, BindView::Unbound));
        pre.objs
            .insert(dpci(1), obj(Some(dprc1()), false, BindView::Unbound));
        let mut post = pre.clone();
        post.edges.insert((ep(dpci(0)), ep(dpci(1))));

        let connect = ModelAction::ConnectEdge {
            a: ep(dpci(0)),
            b: ep(dpci(1)),
        };
        let e = expect(&connect, &pre, &post).unwrap().unwrap();
        assert_eq!(e.link_up, Some(false));

        // A board reporting the link up refutes DPCI-I5; the step fails.
        let names = Binding::seed(&pre);
        let v = judge(
            &e,
            &Observed {
                endpoint: Some(Some("dpci.1".to_owned())),
                link_up: Some(true),
                ..Observed::default()
            },
            &names,
            ExitEvidence { ok: true },
        )
        .unwrap();
        assert!(!v.pass);
        assert_eq!(
            v.mismatches,
            vec!["link_up: expected false, read back true"]
        );

        // A dpmac endpoint has no predictable link status, so the
        // expectation stays silent rather than assert an unobservable.
        let dpmac = ObjRef {
            fam: Family::Dpmac,
            num: 7,
        };
        let connect = ModelAction::ConnectEdge {
            a: ep(dpmac),
            b: ep(dpci(0)),
        };
        let e = expect(&connect, &pre, &post).unwrap().unwrap();
        assert_eq!(e.link_up, None);
    }

    /// A bind step expects whatever the model's post-state says: a
    /// family the reference pair cannot hot-bind (ADR-0008) must be
    /// expected *unbound*, or the suite scores a conforming board wrong.
    #[test]
    fn bind_expectation_follows_the_model_not_the_action() {
        let bind = ModelAction::KernelBind { obj: dpni(100) };
        let pre = state(Some((100, true)));

        let mut post = pre.clone();
        post.objs.get_mut(&dpni(100)).unwrap().bind = BindView::Kernel;
        let e = expect(&bind, &pre, &post).unwrap().unwrap();
        assert_eq!(e.driver_bound, Some(true));

        // The probe ran and did not take: state unchanged.
        let e = expect(&bind, &pre, &pre).unwrap().unwrap();
        assert_eq!(e.driver_bound, Some(false));
    }
}
