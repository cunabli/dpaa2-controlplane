//! Pure parsers for `restool` v2.4 output.
//!
//! Kept separate from the I/O so they can be exercised against recorded golden
//! fixtures (design D10). Each function takes captured stdout and returns typed
//! data; none of them perform I/O.

use dpaa2_api::{DpmacId, DpniId, LinkType, MacAddr};

/// Strips `prefix` from `tok` and parses the remainder as the numeric index behind
/// an id type, e.g. `parse_indexed::<DpmacId>("dpmac.7", "dpmac.")`.
fn parse_indexed<T: From<u32>>(tok: &str, prefix: &str) -> Option<T> {
    tok.strip_prefix(prefix)?.parse::<u32>().ok().map(T::from)
}

/// Parses a bare object id such as `dpni.7` (the `--script` create output) into a
/// [`DpniId`]. Surrounding whitespace is ignored.
#[must_use]
pub fn parse_dpni_object_id(stdout: &str) -> Option<DpniId> {
    parse_indexed(stdout.trim(), "dpni.")
}

/// Parses the bare object reference produced by any `restool --script <type> create`
/// invocation, e.g. `dpcon.5`, `dpbp.0`, `dpio.3`. Returns the trimmed token when it
/// looks like a `dp<type>.<index>` reference.
#[must_use]
pub fn parse_object_ref(stdout: &str) -> Option<&str> {
    let tok = stdout.split_whitespace().next()?;
    let (kind, index) = tok.split_once('.')?;
    if kind.starts_with("dp") && !index.is_empty() && index.bytes().all(|b| b.is_ascii_digit()) {
        Some(tok)
    } else {
        None
    }
}

/// Counts how many objects of `kind` (e.g. `"dpio"`) appear in `dprc show` output.
#[must_use]
pub fn count_objects(stdout: &str, kind: &str) -> usize {
    let prefix = format!("{kind}.");
    stdout
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .filter(|tok| tok.starts_with(&prefix))
        .count()
}

/// Parses `restool dprc show <container>` and returns the DPNI and DPMAC ids it
/// lists. Lines are expected to begin with the object reference in the first column.
#[must_use]
pub fn parse_dprc_show(stdout: &str) -> (Vec<DpniId>, Vec<DpmacId>) {
    let mut dpnis = Vec::new();
    let mut dpmacs = Vec::new();
    for line in stdout.lines() {
        let Some(tok) = line.split_whitespace().next() else {
            continue;
        };
        if let Some(id) = parse_indexed::<DpniId>(tok, "dpni.") {
            dpnis.push(id);
        } else if let Some(id) = parse_indexed::<DpmacId>(tok, "dpmac.") {
            dpmacs.push(id);
        }
    }
    (dpnis, dpmacs)
}

/// What `restool dpni info dpni.N` tells us about a DPNI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RawDpniInfo {
    /// The DPMAC this DPNI is connected to, from the `endpoint:` line.
    pub endpoint: Option<DpmacId>,
    /// The DPNI primary MAC, from the `mac address:` line.
    pub mac: Option<MacAddr>,
}

/// Parses `restool dpni info dpni.N`.
///
/// The endpoint line looks like `endpoint: dpmac.7, link is up`; only the object
/// reference before the comma is significant (design recipe).
#[must_use]
pub fn parse_dpni_info(stdout: &str) -> RawDpniInfo {
    let mut info = RawDpniInfo::default();
    for line in stdout.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("endpoint:") {
            let obj = rest.split(',').next().unwrap_or("").trim();
            info.endpoint = parse_indexed(obj, "dpmac.");
        } else if let Some(rest) = line.strip_prefix("mac address:") {
            info.mac = rest.trim().parse::<MacAddr>().ok();
        }
    }
    info
}

/// What `restool dpmac info dpmac.N` tells us about a DPMAC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawDpmacInfo {
    /// PHY vs. fixed link (design E1). Defaults to PHY when the field is absent.
    pub link_type: LinkType,
    /// The DPMAC burned-in MAC, if reported.
    pub mac: Option<MacAddr>,
}

impl Default for RawDpmacInfo {
    fn default() -> Self {
        Self {
            link_type: LinkType::Phy,
            mac: None,
        }
    }
}

/// Parses `restool dpmac info dpmac.N`.
///
/// Recognizes a `link type:` line carrying `DPMAC_LINK_TYPE_PHY` or
/// `DPMAC_LINK_TYPE_FIXED`; when absent, defaults to PHY.
#[must_use]
pub fn parse_dpmac_info(stdout: &str) -> RawDpmacInfo {
    let mut info = RawDpmacInfo::default();
    for line in stdout.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("link type:") {
            let v = rest.trim();
            if v.contains("FIXED") {
                info.link_type = LinkType::Fixed;
            } else if v.contains("PHY") {
                info.link_type = LinkType::Phy;
            }
        } else if let Some(rest) = line.strip_prefix("mac address:") {
            info.mac = rest.trim().parse::<MacAddr>().ok();
        }
    }
    info
}
