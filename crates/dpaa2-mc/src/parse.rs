//! Pure parsers for `restool` v2.4 output.
//!
//! Kept separate from the I/O so they can be exercised against recorded golden
//! fixtures (design D10). Each function takes captured stdout and returns typed
//! data; none of them perform I/O.

use std::collections::BTreeMap;

use dpaa2_api::{DpmacId, DpmacLinkType, DpniId, EthInterface, LinkType, MacAddr};

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

/// The immutable dpmac attributes the inventory offer needs (task 3.5, design D2;
/// DPMAC-I3: attributes are read once by `dpmac info`, never written). Every field
/// is optional so an unparsable line leaves an honest gap rather than a guess — the
/// assembling caller decides whether a missing field is fatal.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RawDpmacOffer {
    /// The maximum supported rate in Mbps, from the `maximum supported rate N Mbps`
    /// line (which restool prints with the number embedded and no colon).
    pub max_rate: Option<i64>,
    /// The media type, from `DPMAC ethernet interface: DPMAC_ETH_IF_*`.
    pub eth_if: Option<EthInterface>,
    /// The link type, from `DPMAC link type: DPMAC_LINK_TYPE_*`.
    pub link_type: Option<DpmacLinkType>,
}

/// Parses `restool dpmac info dpmac.N` for the inventory offer (task 3.5, design D2).
///
/// The field spellings mirror the captured baseline in
/// `models/board/baselines/reference.json` (e.g. `DPMAC ethernet interface`,
/// `DPMAC link type`, `maximum supported rate 10000 Mbps`); no raw `dpmac info`
/// text is committed in-repo, so the line shapes are transcribed from that snapshot.
/// Unknown enum values (a media/link type this board never showed) parse to `None`
/// rather than a wrong variant.
#[must_use]
pub fn parse_dpmac_offer(stdout: &str) -> RawDpmacOffer {
    let mut offer = RawDpmacOffer::default();
    for line in stdout.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("DPMAC ethernet interface:") {
            offer.eth_if = match rest.trim() {
                "DPMAC_ETH_IF_XFI" => Some(EthInterface::Xfi),
                "DPMAC_ETH_IF_CAUI" => Some(EthInterface::Caui),
                "DPMAC_ETH_IF_RGMII" => Some(EthInterface::Rgmii),
                _ => None,
            };
        } else if let Some(rest) = line.strip_prefix("DPMAC link type:") {
            offer.link_type = match rest.trim() {
                "DPMAC_LINK_TYPE_NONE" => Some(DpmacLinkType::None),
                "DPMAC_LINK_TYPE_FIXED" => Some(DpmacLinkType::Fixed),
                "DPMAC_LINK_TYPE_PHY" => Some(DpmacLinkType::Phy),
                "DPMAC_LINK_TYPE_BACKPLANE" => Some(DpmacLinkType::Backplane),
                _ => None,
            };
        } else if let Some(rest) = line.strip_prefix("maximum supported rate") {
            // Line has no colon: "maximum supported rate 10000 Mbps".
            offer.max_rate = rest.split_whitespace().find_map(|t| t.parse::<i64>().ok());
        }
    }
    offer
}

/// Parses `restool dprc show <container> --resources` into a pool-name → count map
/// (task 3.5, ADR-0011; anchor `dprc.md` mc.global section: the listing carries the
/// MC-level pools `bp 63, mcp 203, swp 49, …`).
///
/// No raw `--resources` capture is committed in-repo, so the exact column shape
/// (colon vs. space separator) is not pinned; this reads the last integer token on
/// each line keyed by the first token, tolerating either `bp: 63` or `bp 63`.
#[must_use]
pub fn parse_resources(stdout: &str) -> BTreeMap<String, i64> {
    let mut pools = BTreeMap::new();
    for line in stdout.lines() {
        let mut toks = line.split_whitespace();
        let Some(name) = toks.next() else { continue };
        let name = name.trim_end_matches(':');
        if let Some(count) = line
            .split_whitespace()
            .rev()
            .find_map(|t| t.parse::<i64>().ok())
        {
            pools.insert(name.to_owned(), count);
        }
    }
    pools
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

#[cfg(test)]
mod tests {
    use super::*;

    // Canned `dpmac info` text, field spellings from
    // models/board/baselines/reference.json (dpmac.10 entry).
    const DPMAC_INFO_XFI: &str = "\
dpmac version: 4.10
dpmac object id/portal id: 10
DPMAC ethernet interface: DPMAC_ETH_IF_XFI
DPMAC link type: DPMAC_LINK_TYPE_PHY
MAC address: 00:11:22:33:44:55
endpoint: No object associated
endpoint state: -1
maximum supported rate 10000 Mbps
plugged state: plugged
";

    #[test]
    fn dpmac_offer_parses_all_three_attributes() {
        let o = parse_dpmac_offer(DPMAC_INFO_XFI);
        assert_eq!(o.max_rate, Some(10_000));
        assert_eq!(o.eth_if, Some(EthInterface::Xfi));
        assert_eq!(o.link_type, Some(DpmacLinkType::Phy));
    }

    #[test]
    fn dpmac_offer_maps_rgmii_and_caui() {
        let rgmii = parse_dpmac_offer(
            "DPMAC ethernet interface: DPMAC_ETH_IF_RGMII\nmaximum supported rate 1000 Mbps\n",
        );
        assert_eq!(rgmii.eth_if, Some(EthInterface::Rgmii));
        assert_eq!(rgmii.max_rate, Some(1000));
        let caui = parse_dpmac_offer("DPMAC ethernet interface: DPMAC_ETH_IF_CAUI\n");
        assert_eq!(caui.eth_if, Some(EthInterface::Caui));
    }

    #[test]
    fn dpmac_offer_unknown_values_are_none_not_a_wrong_variant() {
        let o = parse_dpmac_offer(
            "DPMAC ethernet interface: DPMAC_ETH_IF_SGMII\nDPMAC link type: DPMAC_LINK_TYPE_MISC\n",
        );
        assert_eq!(o.eth_if, None);
        assert_eq!(o.link_type, None);
        assert_eq!(o.max_rate, None);
    }

    #[test]
    fn resources_parses_space_and_colon_forms() {
        let space = parse_resources("bp 63\nmcp 203\nswp 49\n");
        assert_eq!(space.get("bp"), Some(&63));
        assert_eq!(space.get("mcp"), Some(&203));
        let colon = parse_resources("bp: 63\nswpch.2wq: 112\n");
        assert_eq!(colon.get("bp"), Some(&63));
        assert_eq!(colon.get("swpch.2wq"), Some(&112));
    }
}
