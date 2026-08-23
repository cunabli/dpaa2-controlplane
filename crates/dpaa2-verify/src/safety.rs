//! The coded port safety envelope (ADR-0003 §4–§5, design D6).
//!
//! The port matrix and traffic classes are data here, and every path to
//! the board passes through them twice, independently:
//!
//! - **generation** — [`check_trace`] walks a model trace's referenced
//!   objects before any script is emitted (boot-born model ids name the
//!   real objects; runtime creates are board-named later and covered by
//!   the second layer);
//! - **execution** — [`check_cmd`] scans every rendered command (restool
//!   argv and sysfs paths alike) immediately before it runs, so a
//!   forbidden reference refuses even if it appears in a hand-edited or
//!   foreign script.
//!
//! Port safety survives model bugs and prompt mistakes alike because it
//! is enforced where scripts are generated *and* where they execute —
//! never by care (ADR-0003 §4).

use std::fmt;

use crate::adapter::{Cmd, MbtTrace, ObjRef};

/// ADR-0003 §5: every scenario declares exactly one traffic class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrafficClass {
    /// MC-bus mutations and queries only, no link semantics.
    ObjectLifecycleOnly,
    /// Asserts or observes link state, no frames.
    LinkSignaling,
    /// Frames emitted; explicitly flagged, allowed ports only.
    TrafficBearing,
}

impl fmt::Display for TrafficClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ObjectLifecycleOnly => "object-lifecycle-only",
            Self::LinkSignaling => "link-signaling",
            Self::TrafficBearing => "traffic-bearing",
        })
    }
}

/// The declared class of one run, with its explicit per-run flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunClass {
    /// The scenario's declared traffic class.
    pub class: TrafficClass,
    /// The explicit per-run flag ADR-0003 §4 requires for any use of the
    /// wired 10G pair.
    pub flagged: bool,
}

/// A named safety violation. Refusal messages carry the offending
/// reference so generation "fails with the violation named"
/// (mbt-harness spec).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation(String);

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Violation {}

/// ADR-0003 §4 total-deny rows: the peer that must never see traffic
/// (dpmac.3) and the management plane (dpmac.17, dpni.0). Unreferenceable
/// in any scenario class, including read-only probes.
const TOTAL_DENY: [&str; 3] = ["dpmac.3", "dpmac.17", "dpni.0"];

/// The wired 10G pair: link-signaling and traffic-bearing run here only,
/// each run explicitly flagged. Every other dpmac is lifecycle-only —
/// the named unwired set (4–6, 8, 10) and any id outside the matrix
/// (phantom creates) alike.
const FLAGGED_DPMACS: [u32; 2] = [7, 9];

/// Object-reference tokens of a text: maximal runs of `[a-z0-9_.]`,
/// stripped of surrounding dots — so `--endpoint2=dpmac.7,` yields
/// `dpmac.7` and `dpmac.31` never matches `dpmac.3`.
fn tokens(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '_'))
        .map(|t| t.trim_matches('.'))
        .filter(|t| !t.is_empty())
}

/// Whether `token` references `obj` — exactly, or as an endpoint with a
/// port suffix (`dpmac.3.0`).
fn references(token: &str, obj: &str) -> bool {
    token == obj
        || token
            .strip_prefix(obj)
            .and_then(|rest| rest.strip_prefix('.'))
            .is_some_and(|port| port.bytes().all(|b| b.is_ascii_digit()))
}

/// The dpmac id a token references, if any (`dpmac.7` / `dpmac.7.0`).
fn dpmac_id(token: &str) -> Option<u32> {
    let rest = token.strip_prefix("dpmac.")?;
    let id = rest.split('.').next()?;
    id.parse().ok()
}

/// Scans free text for total-deny references (spec: dpmac.3, dpmac.17,
/// dpni.0 are unreferenceable in any scenario class).
///
/// # Errors
///
/// Returns the named violation on the first forbidden reference.
pub fn scan_text(text: &str) -> Result<(), Violation> {
    for token in tokens(text) {
        for deny in TOTAL_DENY {
            if references(token, deny) {
                return Err(Violation(format!(
                    "`{deny}` is total-deny (ADR-0003 §4) and may not be referenced in any scenario class"
                )));
            }
        }
    }
    Ok(())
}

/// Checks free text against the full envelope: total-deny references and
/// the class-vs-ports ceiling (a lifecycle run must stay off the wired
/// pair; a link/traffic run may name dpmac.7/9 only and must be flagged).
///
/// # Errors
///
/// Returns the named violation on the first breach.
pub fn check_text(run: RunClass, text: &str) -> Result<(), Violation> {
    check_declared(run)?;
    scan_text(text)?;
    for token in tokens(text) {
        let Some(id) = dpmac_id(token) else { continue };
        match run.class {
            TrafficClass::ObjectLifecycleOnly => {
                if FLAGGED_DPMACS.contains(&id) {
                    return Err(Violation(format!(
                        "object-lifecycle-only run names dpmac.{id}: the wired pair carries link-signaling and traffic-bearing runs only (ADR-0003 §4)"
                    )));
                }
            }
            TrafficClass::LinkSignaling | TrafficClass::TrafficBearing => {
                if !FLAGGED_DPMACS.contains(&id) {
                    return Err(Violation(format!(
                        "{} run names dpmac.{id}: physical-port instances run on the flagged wired pair only (ADR-0003 §5)",
                        run.class
                    )));
                }
            }
        }
    }
    Ok(())
}

/// The declaration itself must be coherent: link-signaling and
/// traffic-bearing runs are refused unflagged (mbt-harness spec,
/// "Class must match ports").
fn check_declared(run: RunClass) -> Result<(), Violation> {
    if run.class != TrafficClass::ObjectLifecycleOnly && !run.flagged {
        return Err(Violation(format!(
            "unflagged run declares {}: every such run is explicitly flagged (ADR-0003 §5)",
            run.class
        )));
    }
    Ok(())
}

/// The execution-side wrapper: checks one rendered command — restool
/// argv, sysfs path and value alike — immediately before it runs.
///
/// # Errors
///
/// Returns the named violation on the first breach.
pub fn check_cmd(run: RunClass, cmd: &Cmd) -> Result<(), Violation> {
    match cmd {
        Cmd::Restool(argv) => check_text(run, &argv.join(" ")),
        Cmd::SysfsWrite { path, value } => {
            check_text(run, path)?;
            check_text(run, value)
        }
    }
}

/// The generation-side gate: checks every object a model trace
/// references before any script is emitted. Boot-born model ids name
/// the real board objects, so the total-deny and class ceilings apply
/// directly; runtime-created ids are board-named only at execution,
/// where [`check_cmd`] independently covers them.
///
/// # Errors
///
/// Returns the named violation on the first breaching step.
pub fn check_trace(run: RunClass, trace: &MbtTrace) -> Result<(), Violation> {
    check_declared(run)?;
    for (i, step) in trace.steps.iter().enumerate() {
        let text = step
            .action
            .refs()
            .iter()
            .map(ObjRef::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        check_text(run, &text).map_err(|Violation(v)| {
            Violation(format!("trace step {i} ({:?}): {v}", step.action))
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{EndpointRef, Family, MachineView, MbtStep, ModelAction};

    const LIFECYCLE: RunClass = RunClass {
        class: TrafficClass::ObjectLifecycleOnly,
        flagged: false,
    };
    const LINK_FLAGGED: RunClass = RunClass {
        class: TrafficClass::LinkSignaling,
        flagged: true,
    };

    fn dpmac(num: u32) -> ObjRef {
        ObjRef {
            fam: Family::Dpmac,
            num,
        }
    }

    fn trace_of(actions: &[ModelAction]) -> MbtTrace {
        MbtTrace {
            init: MachineView::default(),
            steps: actions
                .iter()
                .map(|a| MbtStep {
                    action: a.clone(),
                    post: MachineView::default(),
                })
                .collect(),
        }
    }

    #[test]
    fn total_deny_never_reaches_a_script() {
        // Generation: a trace touching dpmac.17 is refused with the
        // violation named.
        let trace = trace_of(&[ModelAction::VfioBind { obj: dpmac(17) }]);
        let err = check_trace(LIFECYCLE, &trace).unwrap_err();
        assert!(err.to_string().contains("dpmac.17"), "{err}");

        // Execution: the wrapper independently refuses such a step even
        // if one appears in a script.
        let cmd = Cmd::Restool(
            [
                "dprc",
                "connect",
                "dprc.1",
                "--endpoint1=dpni.5",
                "--endpoint2=dpmac.3",
            ]
            .map(str::to_owned)
            .to_vec(),
        );
        let err = check_cmd(LIFECYCLE, &cmd).unwrap_err();
        assert!(err.to_string().contains("dpmac.3"), "{err}");

        // dpni.0 is unreferenceable, sysfs paths included.
        let cmd = Cmd::SysfsWrite {
            path: "/sys/bus/fsl-mc/devices/dpni.0/driver_override".to_owned(),
            value: "vfio-fsl-mc".to_owned(),
        };
        assert!(check_cmd(LIFECYCLE, &cmd).is_err());

        // Port-suffixed endpoint references are still caught.
        assert!(scan_text("--endpoint1=dpmac.3.0").is_err());
    }

    #[test]
    fn deny_matching_is_exact_not_prefix() {
        // dpmac.31 is not dpmac.3; dpni.10 is not dpni.0.
        assert!(scan_text("dpmac.31 dpni.10 dpmac.170").is_ok());
    }

    #[test]
    fn class_must_match_ports() {
        // A lifecycle-only trace naming the wired pair is refused.
        let trace = trace_of(&[ModelAction::ConnectEdge {
            a: EndpointRef {
                obj: ObjRef {
                    fam: Family::Dpni,
                    num: 100,
                },
                port: 0,
            },
            b: EndpointRef {
                obj: dpmac(7),
                port: 0,
            },
        }]);
        let err = check_trace(LIFECYCLE, &trace).unwrap_err();
        assert!(err.to_string().contains("dpmac.7"), "{err}");

        // The same trace under a flagged link-signaling run passes.
        assert!(check_trace(LINK_FLAGGED, &trace).is_ok());

        // A link run touching the unwired set exceeds its ports.
        let unwired = trace_of(&[ModelAction::LinkChange { obj: dpmac(4) }]);
        assert!(check_trace(LINK_FLAGGED, &unwired).is_err());
        // The identical actions are fine as lifecycle churn.
        assert!(check_trace(LIFECYCLE, &unwired).is_ok());
    }

    #[test]
    fn unflagged_link_or_traffic_runs_are_refused() {
        let unflagged = RunClass {
            class: TrafficClass::TrafficBearing,
            flagged: false,
        };
        let err = check_trace(unflagged, &trace_of(&[])).unwrap_err();
        assert!(err.to_string().contains("unflagged"), "{err}");
        assert!(check_text(unflagged, "").is_err());
    }
}
