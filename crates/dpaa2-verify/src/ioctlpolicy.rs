//! The fsl-mc ioctl command-id policy as code (openspec task 6.5).
//!
//! The kernel's `fsl-mc-uapi` driver refuses any MC command not on a
//! fixed whitelist with `-EACCES`, regardless of privilege; some accepted
//! commands additionally need `CAP_NET_ADMIN` (ADR-0003 §5). This module
//! is the machine-readable side of `docs/baseline/mc-ioctl-policy.md`: it
//! parses that generated table (§1 whitelist, §2 verb resolution) and
//! re-implements the kernel match rule, so the harness can name every
//! verb it drives against the same policy the board enforces.
//!
//! The table is the single source of truth; the parser reads it and the
//! kernel rule is recomputed here rather than trusting the table's own
//! verdict column, so the two must agree or the tests fail. The verb-key
//! catalogues (`verbs_of`, `ADAPTER_VERBS`, `HARNESS_VERBS`) mirror
//! the restool invocations the model driver, the `dpaa2-mc` shim and the
//! suite harness emit, so a new invocation with no table row fails here
//! rather than surprising an operator mid-sitting.

use std::sync::OnceLock;

use crate::adapter::{Family, ModelAction};

/// The generated policy table, embedded so the resolution below needs no
/// working directory (the tests re-read it from the repo root to prove
/// the on-disk file still parses).
const POLICY_MD: &str = include_str!("../../../docs/baseline/mc-ioctl-policy.md");

/// One kernel whitelist entry (`fsl_mc_accepted_cmds[]`, §1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Whitelisted {
    /// The entry name (`DPRC_CREATE_CONT`, `OPEN`, …).
    pub name: String,
    /// The value a masked command id must equal.
    pub value: u16,
    /// The mask applied before the equality test.
    pub mask: u16,
    /// The command needs `CAP_NET_ADMIN`.
    pub cap_net_admin: bool,
    /// The command id's module field must be in `1..=0x10`.
    pub check_module_id: bool,
}

/// The parsed kernel whitelist, in table order (first match wins).
#[derive(Debug, Clone, Default)]
pub struct Whitelist {
    /// The accepted-command entries, in the order the kernel scans them.
    pub entries: Vec<Whitelisted>,
}

/// The kernel's verdict for one 16-bit MC command id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The kernel forwards the command; `cap_net_admin` gates it on the
    /// capability restool holds as root.
    Accepted {
        /// The matched entry carries the `CAP_NET_ADMIN` flag.
        cap_net_admin: bool,
    },
    /// The kernel refuses the command with `-EACCES`.
    Refused,
}

/// One `§2` verb-resolution row: the `<fam> <verb>` key, the command ids
/// it emits, and the table's own verdict text (recomputed, not trusted).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerbRow {
    /// The restool `<fam> <verb>` key (the first cell, before any `(`).
    pub key: String,
    /// The 16-bit command ids the verb emits, empty when none are listed.
    pub cmdids: Vec<u16>,
    /// The verdict text the table prints for the row.
    pub verdict: String,
}

impl Whitelist {
    /// Parses `§1` of the policy table.
    #[must_use]
    pub fn parse(md: &str) -> Self {
        let mut entries = Vec::new();
        for row in section_rows(md, "## 1.", "## 2") {
            let c = cells(row);
            // A data row starts with the entry index; the header and the
            // `---` separator do not parse as a number.
            if c.len() < 7 || c[0].parse::<u32>().is_err() {
                continue;
            }
            entries.push(Whitelisted {
                name: c[1].to_owned(),
                value: hex16(c[2]),
                mask: hex16(c[3]),
                cap_net_admin: c[6].contains("CAP_NET_ADMIN"),
                check_module_id: c[6].contains("CHECK_MODULE_ID"),
            });
        }
        Self { entries }
    }

    /// Re-implements `fsl_mc_command_check`: the first entry whose masked
    /// value matches wins; a `CHECK_MODULE_ID` entry matches only when the
    /// command id's module field is in `1..=0x10`. No match is a refusal.
    #[must_use]
    pub fn check(&self, cmdid: u16) -> Verdict {
        for e in &self.entries {
            if (cmdid & e.mask) != e.value {
                continue;
            }
            if e.check_module_id && !(1..=0x10).contains(&((cmdid >> 4) & 0x3f)) {
                continue;
            }
            return Verdict::Accepted {
                cap_net_admin: e.cap_net_admin,
            };
        }
        Verdict::Refused
    }

    /// The verdict text `§2` should print for a verb emitting `cmdids`,
    /// recomputed from the whitelist so it can be compared with the cell.
    #[must_use]
    pub fn verdict_str(&self, cmdids: &[u16]) -> &'static str {
        if cmdids.iter().any(|&c| self.check(c) == Verdict::Refused) {
            "refused EACCES"
        } else if cmdids.iter().any(|&c| {
            matches!(
                self.check(c),
                Verdict::Accepted {
                    cap_net_admin: true
                }
            )
        }) {
            "accepted, CAP_NET_ADMIN"
        } else {
            "accepted"
        }
    }
}

/// Parses `§2a`+`§2b` of the policy table into verb rows.
#[must_use]
pub fn parse_verbs(md: &str) -> Vec<VerbRow> {
    let mut out = Vec::new();
    for row in section_rows(md, "## 2", "## 3") {
        let c = cells(row);
        // Skip the two sub-tables' header and `---` separator rows.
        if c.len() < 6 || c[0] == "verb" || c.iter().all(|x| is_rule(x)) {
            continue;
        }
        out.push(VerbRow {
            key: c[0].split(" (").next().unwrap_or(c[0]).trim().to_owned(),
            cmdids: hex_ids(c[2]),
            verdict: c[4].to_owned(),
        });
    }
    out
}

/// Parses `§3` (the raw probes outside the whitelist) into `(command,
/// cmdid)` pairs, so the tests can prove they are refused rather than
/// asserting the list by hand.
#[must_use]
pub fn parse_outside(md: &str) -> Vec<(String, u16)> {
    let mut out = Vec::new();
    for row in section_rows(md, "## 3", "## 4") {
        let c = cells(row);
        if c.len() < 5 || c[0] == "command" || c.iter().all(|x| is_rule(x)) {
            continue;
        }
        for id in hex_ids(c[1]) {
            out.push((
                c[0].split(" (").next().unwrap_or(c[0]).trim().to_owned(),
                id,
            ));
        }
    }
    out
}

// --- markdown-table helpers -----------------------------------------

/// The `|`-delimited rows of the table section between the line starting
/// `start` and the next line starting `end`.
fn section_rows<'a>(md: &'a str, start: &str, end: &str) -> Vec<&'a str> {
    let mut in_sec = false;
    let mut out = Vec::new();
    for line in md.lines() {
        if line.starts_with(start) {
            in_sec = true;
        } else if in_sec && line.starts_with(end) {
            break;
        } else if in_sec && line.trim_start().starts_with('|') {
            out.push(line);
        }
    }
    out
}

/// One row's trimmed cells, without the outer `|` fences.
fn cells(row: &str) -> Vec<&str> {
    row.trim()
        .trim_start_matches('|')
        .trim_end_matches('|')
        .split('|')
        .map(str::trim)
        .collect()
}

/// A markdown separator cell (`---`) or an empty one.
fn is_rule(cell: &str) -> bool {
    cell.chars().all(|c| c == '-')
}

/// The `u16` a `0x….`-prefixed cell names.
fn hex16(cell: &str) -> u16 {
    let s = cell.trim().trim_start_matches("0x");
    u16::from_str_radix(s, 16).unwrap_or(0)
}

/// Every `0x….` command id in a cell, in order.
fn hex_ids(cell: &str) -> Vec<u16> {
    let mut ids = Vec::new();
    let mut rest = cell;
    while let Some(p) = rest.find("0x") {
        let after = &rest[p + 2..];
        let hex: String = after.chars().take_while(char::is_ascii_hexdigit).collect();
        if let Ok(v) = u16::from_str_radix(&hex, 16) {
            ids.push(v);
        }
        rest = &after[hex.len()..];
    }
    ids
}

// --- verb catalogues ------------------------------------------------

/// The `<fam> <verb>` keys the model driver's [`crate::adapter::drive_with`]
/// renders for `action`; empty for the actions the board takes on its own
/// (they issue no restool command).
#[must_use]
pub fn verbs_of(action: &ModelAction) -> Vec<String> {
    match action {
        ModelAction::CreateContainer { .. } => vec!["dprc create".into()],
        ModelAction::CreateObject { fam, .. } => vec![format!("{} create", fam.as_str())],
        ModelAction::PreplugMutate { obj } => {
            // Only a DPNI has a restool-expressible pre-plug mutation.
            if obj.fam == Family::Dpni {
                vec!["dpni update".into()]
            } else {
                vec![]
            }
        }
        // The assign primitive moves one tree edge either way; the
        // pre-state picks assign or unassign, so both are possible.
        ModelAction::AssignChild { .. } => vec!["dprc assign".into(), "dprc unassign".into()],
        ModelAction::Plug { .. } | ModelAction::Unplug { .. } => vec!["dprc assign".into()],
        ModelAction::ConnectEdge { .. } => vec!["dprc connect".into()],
        ModelAction::DisconnectEdge { .. } => vec!["dprc disconnect".into()],
        ModelAction::Rescan { .. } => vec!["dprc sync".into()],
        ModelAction::SetLocked { .. } => vec!["dprc set-locked".into()],
        ModelAction::Destroy { obj } => vec![format!("{} destroy", obj.fam.as_str())],
        ModelAction::KernelBind { .. }
        | ModelAction::VfioBind { .. }
        | ModelAction::Unbind { .. }
        | ModelAction::ChildIrqRefresh { .. }
        | ModelAction::Allocate { .. }
        | ModelAction::Free { .. }
        | ModelAction::Enable { .. }
        | ModelAction::Disable { .. }
        | ModelAction::LinkChange { .. } => vec![],
    }
}

/// Every restool invocation the `dpaa2-mc` shim (`restool.rs`) issues.
pub const ADAPTER_VERBS: &[&str] = &[
    "dprc assign",
    "dprc show",
    "dprc connect",
    "dprc disconnect",
    "dprc sync",
    "dpio create",
    "dpmcp create",
    "dpbp create",
    "dpcon create",
    "dpbp destroy",
    "dpmcp destroy",
    "dpcon destroy",
    "dpni create",
    "dpni info",
    "dpni update",
    "dpni destroy",
    "dpmac info",
];

/// The preamble, post-boot and read-back verbs the suite harness issues
/// (`generate.rs`, `adapter.rs` `readback`/`observe`) beyond a step's own
/// drive command. `<fam> info` is emitted for every observed family; only
/// families with a `§2` info row appear.
pub const HARNESS_VERBS: &[&str] = &[
    "restool -m",
    "dprc list",
    "dprc show",
    "dprc generate-dpl",
    "dprc info",
    "dpni info",
    "dpmac info",
    "dpbp info",
    "dpio info",
    "dpcon info",
    "dpmcp info",
    "dpseci info",
    "dpsw info",
    "dpdmux info",
    "dpaiop info",
    "dpci info",
    "dpdcei info",
    "dpdmai info",
    "dprtc info",
];

// --- suite-header resolution ----------------------------------------

/// The parsed policy, loaded once from the embedded table.
fn policy() -> &'static (Whitelist, Vec<VerbRow>) {
    static P: OnceLock<(Whitelist, Vec<VerbRow>)> = OnceLock::new();
    P.get_or_init(|| (Whitelist::parse(POLICY_MD), parse_verbs(POLICY_MD)))
}

/// The `<fam> <verb>` key a restool argv resolves to (its first two
/// non-flag tokens), or `None` when the argv names no verb.
#[must_use]
pub fn verb_key(argv: &[String]) -> Option<String> {
    let mut it = argv.iter().filter(|a| !a.starts_with("--"));
    Some(format!("{} {}", it.next()?, it.next()?))
}

/// Whether the verb keyed `key` issues an MC command the kernel gates on
/// `CAP_NET_ADMIN`, resolved through the policy table. Reads and unknown
/// keys are `false`.
#[must_use]
pub fn verb_needs_cap_net_admin(key: &str) -> bool {
    let (wl, verbs) = policy();
    verbs
        .iter()
        .filter(|r| r.key == key)
        .flat_map(|r| &r.cmdids)
        .any(|&c| {
            matches!(
                wl.check(c),
                Verdict::Accepted {
                    cap_net_admin: true
                }
            )
        })
}
