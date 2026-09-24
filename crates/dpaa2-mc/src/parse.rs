//! Pure parsers for `restool` v2.4 output.
//!
//! Kept separate from the I/O so they can be exercised against recorded golden
//! fixtures (design D10; restool-baseline). Each function takes captured stdout and returns typed
//! data; none of them perform I/O.

use std::collections::BTreeMap;

use dpaa2_api::core::family::{ALL_FAMILIES, Family};
use dpaa2_api::core::inventory::{DpmacLinkType, EthInterface};
use dpaa2_api::core::model::{DpmacId, DpniId, DprcId, LinkType, MacAddr, ObjectRef};
use dpaa2_api::core::types::ConstructName;
use dpaa2_api::families::dprc;

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

/// Parses the container id `restool dprc create` echoes, e.g. `dprc.3`, into a
/// [`DprcId`] (`docs/baseline/dprc.md` "create details": the create returns the child
/// id). Scans for the first `dprc.<n>` token so a portal-offset suffix on the same
/// line is ignored.
#[must_use]
pub fn parse_dprc_id(stdout: &str) -> Option<DprcId> {
    stdout
        .split_whitespace()
        .find_map(|tok| parse_indexed(tok.trim_end_matches([',', ':']), "dprc."))
}

/// Extracts the MC status byte from a failed `restool` dprc invocation's output.
///
/// restool surfaces an MC firmware refusal with the status in parentheses, e.g.
/// `... (0x6)` (`docs/baseline/dprc.md` unknown-register #3: Configuration error
/// `0x6`, No resources `0x8`, No privilege `0x4`). Returns the parsed byte, or `None`
/// when no such token is present — the signal the shim reads as a client-side guard,
/// which fires before any MC command and so carries no MC status
/// (`docs/baseline/dprc.md` DPRC-I3). The adapter only extracts the raw byte; the
/// core judges its meaning (design D4; ADR-0003).
#[must_use]
pub fn parse_mc_status(output: &str) -> Option<u8> {
    let start = output.find("(0x")? + 3;
    let hex: String = output[start..]
        .chars()
        .take_while(char::is_ascii_hexdigit)
        .collect();
    u8::from_str_radix(&hex, 16).ok()
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

/// One data row of `restool dprc show <container>` (ADR-0010 Consequences: the
/// read-back parser must surface the label column). The family is recovered from the
/// name token, the number is its ordinal, and the label is empty when the row carried
/// none — the identity ADR-0010 §4 leans on to tell our objects from foreign ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DprcRow {
    /// The object family, recovered from the `family.N` name token.
    pub family: Family,
    /// The object ordinal `N`.
    pub num: u32,
    /// The label column, empty when the row set none.
    pub label: ConstructName,
    /// Whether the trailing state token was `plugged` (vs. `unplugged`).
    pub plugged: bool,
}

/// Parses `restool dprc show <container>` into its data rows, surfacing the label
/// column (ADR-0010 Consequences).
///
/// A data row is whitespace-tokenised as `[name, state]` (empty label) or
/// `[name, label, state]` (label set); the trailing token is exactly `plugged` or
/// `unplugged`, so the header (`… plugged-state`) and the `dprc.N contains M
/// objects:` line — whose last tokens are neither — are skipped without a special
/// case. A name token whose family is not in [`ALL_FAMILIES`] is skipped, never an
/// error: another firmware may list families outside this enum.
#[must_use]
pub fn parse_dprc_rows(stdout: &str) -> Vec<DprcRow> {
    let mut rows = Vec::new();
    for line in stdout.lines() {
        let toks: Vec<&str> = line.split_whitespace().collect();
        let Some(&state) = toks.last() else { continue };
        if state != "plugged" && state != "unplugged" {
            continue;
        }
        let Some((family, num)) = parse_family_num(toks[0]) else {
            continue;
        };
        // Label sits between the name and the state; a two-token row has none.
        let label = if toks.len() >= 3 {
            ConstructName::from(toks[1])
        } else {
            ConstructName::empty()
        };
        rows.push(DprcRow {
            family,
            num,
            label,
            plugged: state == "plugged",
        });
    }
    rows
}

/// One child-DPRC option bit: its `DPRC_CFG_OPT_*` spelling paired with the read and
/// set of the [`dprc::Options`] field it maps. Single-sourced here so the write side
/// (`render_options` in [`RestoolMc`](crate::RestoolMc)) and the read side
/// ([`parse_dprc_info`]) can never spell the four bits differently (review M11; PASS3-F9).
pub(crate) struct OptionBit {
    /// The token restool prints and accepts for this bit.
    pub token: &'static str,
    /// Reads the bit from an [`dprc::Options`] value (drives render).
    pub get: fn(dprc::Options) -> bool,
    /// Sets the bit on an [`dprc::Options`] value (drives decode).
    pub set: fn(&mut dprc::Options),
}

/// The four refusal-gating option bits [`dprc::Options`] carries, in render order.
pub(crate) const OPTION_BITS: &[OptionBit] = &[
    OptionBit {
        token: "DPRC_CFG_OPT_SPAWN_ALLOWED",
        get: |o| o.spawn,
        set: |o| o.spawn = true,
    },
    OptionBit {
        token: "DPRC_CFG_OPT_ALLOC_ALLOWED",
        get: |o| o.alloc,
        set: |o| o.alloc = true,
    },
    OptionBit {
        token: "DPRC_CFG_OPT_OBJ_CREATE_ALLOWED",
        get: |o| o.obj_create,
        set: |o| o.obj_create = true,
    },
    OptionBit {
        token: "DPRC_CFG_OPT_TOPOLOGY_CHANGES_ALLOWED",
        get: |o| o.topology_changes,
        set: |o| o.topology_changes = true,
    },
];

/// Parses `restool dprc info dprc.N` into the lifecycle option mask (DPRC-I4;
/// `docs/baseline/dprc.md` "create details"). restool prints a `dprc options: 0x…`
/// line then one tab-indented `DPRC_CFG_OPT_*` token per set bit; this reads those
/// decoded tokens into the four permission bits [`dprc::Options`] carries (its doc;
/// the write-side inverse is `render_options` in [`RestoolMc`](crate::RestoolMc)).
/// The `IRQ_CFG`/`AIOP`/`PL` tokens fall outside that mask and are ignored. Returns `None`
/// when no `dprc options:` line is present — a malformed or refused read the caller
/// reports, never reads as the default.
#[must_use]
pub fn parse_dprc_info(stdout: &str) -> Option<dprc::Options> {
    if !stdout
        .lines()
        .any(|l| l.trim_start().starts_with("dprc options:"))
    {
        return None;
    }
    let mut options = dprc::Options {
        spawn: false,
        alloc: false,
        obj_create: false,
        topology_changes: false,
    };
    for line in stdout.lines() {
        let tok = line.trim();
        if let Some(bit) = OPTION_BITS.iter().find(|b| b.token == tok) {
            (bit.set)(&mut options);
        }
    }
    Some(options)
}

/// Recovers `(family, ordinal)` from a `family.N` name token by scanning
/// [`ALL_FAMILIES`] (ADR-0010: the name is a projection of the key). Returns `None`
/// when the prefix names no known family or the ordinal is not a number.
fn parse_family_num(tok: &str) -> Option<(Family, u32)> {
    let (kind, index) = tok.split_once('.')?;
    let num = index.parse::<u32>().ok()?;
    let family = ALL_FAMILIES.iter().copied().find(|f| f.as_str() == kind)?;
    Some((family, num))
}

/// Parses the `endpoint:` line of `restool dpni info dpni.N` into the peer object it
/// names, or `None` when the dpni is disconnected (`No object associated`). Unlike
/// [`parse_dpni_info`], which keeps only a `dpmac.` peer, this recovers ANY family peer
/// as an [`ObjectRef`], so the cross-container dpni↔dpni case reads back — the
/// idempotence read the child-port converge issues (`docs/baseline/dpni.md` DPNI-I9).
#[must_use]
pub fn parse_dpni_endpoint(stdout: &str) -> Option<ObjectRef> {
    for line in stdout.lines() {
        if let Some(rest) = line.trim().strip_prefix("endpoint:") {
            let obj = rest.split(',').next().unwrap_or("").trim();
            let (family, num) = parse_family_num(obj)?;
            return Some(ObjectRef::new(family, num));
        }
    }
    None
}

/// Parses `restool dprc show <container>` and returns the DPNI and DPMAC ids it
/// lists. Derived from [`parse_dprc_rows`] so both views agree on what a data row is.
#[must_use]
pub fn parse_dprc_show(stdout: &str) -> (Vec<DpniId>, Vec<DpmacId>) {
    let mut dpnis = Vec::new();
    let mut dpmacs = Vec::new();
    for row in parse_dprc_rows(stdout) {
        match row.family {
            Family::Dpni => dpnis.push(DpniId::from(row.num)),
            Family::Dpmac => dpmacs.push(DpmacId::from(row.num)),
            _ => {}
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
    /// The `dpni_attr` read-back block, when `dpni info` printed one (`None` when the
    /// object was not opened or the block was absent — the honest gap,
    /// dpni-typestate task 4.1).
    pub attr: Option<RawDpniAttr>,
}

/// The `dpni_attr` read-back block `restool dpni info` prints, transcribed from
/// `dpni_commands.c` `print_dpni_attr` (lines ~644-662). The `options` field is the
/// authoritative mask from `dpni_attr.options value is: 0x…`; the decoded option-name
/// lines that follow are ignored. Every count is optional so a missing or malformed
/// line leaves an honest gap. There is **no** `dist_key_size` line and none is ever
/// synthesized: that create-time value has no read-back (dpni-typestate design D4;
/// `docs/baseline/dpni.md` "Attribute read-back asymmetry", DPNI-I12).
///
/// The read-back spellings differ from the create flags on purpose: `mac_entries` /
/// `vlan_entries` here vs. `--mac-filter-entries` / `--vlan-filter-entries` on create,
/// and the single create `--num-tcs` splits into read-back `num_rx_tcs` / `num_tx_tcs`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RawDpniAttr {
    /// The authoritative options mask (`dpni_attr.options value is: 0x…`).
    pub options: u32,
    /// `num_queues`.
    pub num_queues: Option<u16>,
    /// `num_cgs`.
    pub num_cgs: Option<u16>,
    /// `num_rx_tcs` — the never-settable read-back half (`docs/baseline/dpni.md`
    /// "Never settable"); informational only, never mapped to the domain observation.
    pub num_rx_tcs: Option<u16>,
    /// `num_tx_tcs` — the cfg-settable TC half; maps to the domain `num_tcs`.
    pub num_tx_tcs: Option<u16>,
    /// `mac_entries` (create flag `--mac-filter-entries`).
    pub mac_entries: Option<u16>,
    /// `vlan_entries` (create flag `--vlan-filter-entries`).
    pub vlan_entries: Option<u16>,
    /// `qos_entries`.
    pub qos_entries: Option<u16>,
    /// `fs_entries`.
    pub fs_entries: Option<u16>,
    /// `qos_key_size` — added read-back; informational only, never in the domain
    /// observation.
    pub qos_key_size: Option<u16>,
    /// `fs_key_size` — added read-back; informational only, never in the domain
    /// observation.
    pub fs_key_size: Option<u16>,
    /// `num_channels` — maps to the domain `num_ceetm_ch`.
    pub num_channels: Option<u16>,
    /// `num_opr`.
    pub num_opr: Option<u16>,
    /// `wriop_version` (0xC00 = WRIOP 3.0.0 = LX2160, `docs/baseline/dpni.md`
    /// "Attribute read-back asymmetry"); informational only, kept off the domain
    /// observation and out of drift — the mc-portal-backend num_queues-ceiling
    /// question anchors on it (#10).
    pub wriop_version: Option<u16>,
}

impl RawDpniAttr {
    /// Absorbs one `family_field: N` count line of the attr block, ignoring any line
    /// that names no attr field (so the statistics pages that follow are dropped).
    fn absorb(&mut self, line: &str) {
        let val = |s: &str| s.trim().parse::<u16>().ok();
        if let Some(r) = line.strip_prefix("num_queues:") {
            self.num_queues = val(r);
        } else if let Some(r) = line.strip_prefix("num_cgs:") {
            self.num_cgs = val(r);
        } else if let Some(r) = line.strip_prefix("num_rx_tcs:") {
            self.num_rx_tcs = val(r);
        } else if let Some(r) = line.strip_prefix("num_tx_tcs:") {
            self.num_tx_tcs = val(r);
        } else if let Some(r) = line.strip_prefix("mac_entries:") {
            self.mac_entries = val(r);
        } else if let Some(r) = line.strip_prefix("vlan_entries:") {
            self.vlan_entries = val(r);
        } else if let Some(r) = line.strip_prefix("qos_entries:") {
            self.qos_entries = val(r);
        } else if let Some(r) = line.strip_prefix("fs_entries:") {
            self.fs_entries = val(r);
        } else if let Some(r) = line.strip_prefix("qos_key_size:") {
            self.qos_key_size = val(r);
        } else if let Some(r) = line.strip_prefix("fs_key_size:") {
            self.fs_key_size = val(r);
        } else if let Some(r) = line.strip_prefix("num_channels:") {
            self.num_channels = val(r);
        } else if let Some(r) = line.strip_prefix("num_opr:") {
            self.num_opr = val(r);
        } else if let Some(r) = line.strip_prefix("wriop_version:") {
            let t = r.trim();
            self.wriop_version = t
                .strip_prefix("0x")
                .and_then(|h| u16::from_str_radix(h, 16).ok())
                .or_else(|| t.parse().ok());
        }
    }
}

/// Parses `restool dpni info dpni.N`.
///
/// The endpoint line looks like `endpoint: dpmac.7, link is up`; only the object
/// reference before the comma is significant (design recipe). The `dpni_attr` block
/// (when present) opens on the `dpni_attr.options value is:` line and its count lines
/// follow in print order (dpni-typestate task 4.1); a `dpni info` that never opened the
/// object prints no block, so [`RawDpniInfo::attr`] stays `None`.
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
        } else if let Some(rest) = line.strip_prefix("dpni_attr.options value is:") {
            // The authoritative mask opens the block; the decoded name lines after are ignored.
            let hex = rest.trim().trim_start_matches("0x");
            if let Ok(options) = u32::from_str_radix(hex, 16) {
                info.attr.get_or_insert_with(RawDpniAttr::default).options = options;
            }
        } else if let Some(attr) = info.attr.as_mut() {
            attr.absorb(line);
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

/// The immutable dpmac attributes the inventory offer needs (task 3.5, design D2 (ADR-0002);
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
    /// The DPNI this dpmac anchors, from the lowercase `endpoint:` line. Only a
    /// `dpni.` peer parses; `No object associated` or a future dpdmux/dpsw peer
    /// leaves `None`. Feeds the ADR-0001 §4 foreign-ownership check in
    /// [`RestoolMc`](crate::RestoolMc)'s `read_inventory`.
    pub endpoint: Option<DpniId>,
}

/// Parses `restool dpmac info dpmac.N` for the inventory offer (task 3.5, design D2; ADR-0002).
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
        } else if let Some(rest) = line.strip_prefix("endpoint:") {
            // `endpoint:` is lowercase where the dpmac's other fields are
            // capitalized (`DPMAC link type:`, `MAC address:`) — the baseline is the
            // spelling oracle, do not "align" it. Only a `dpni.` peer parses;
            // `No object associated` (and any future dpdmux/dpsw peer) leaves None.
            let obj = rest.split(',').next().unwrap_or("").trim();
            offer.endpoint = parse_indexed(obj, "dpni.");
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
/// The field spellings mirror the captured baseline in
/// `models/board/baselines/reference.json` (`DPMAC link type`, `MAC address`),
/// matching [`parse_dpmac_offer`] above; restool prints these with a `DPMAC`
/// prefix and capitalized `MAC`, so the lowercase forms never appear (DPMAC-I3:
/// attributes are read once by `dpmac info`, the baseline is the spelling oracle).
/// Recognizes `DPMAC_LINK_TYPE_PHY` or `DPMAC_LINK_TYPE_FIXED`; when absent,
/// defaults to PHY.
#[must_use]
pub fn parse_dpmac_info(stdout: &str) -> RawDpmacInfo {
    let mut info = RawDpmacInfo::default();
    for line in stdout.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("DPMAC link type:") {
            let v = rest.trim();
            if v.contains("FIXED") {
                info.link_type = LinkType::Fixed;
            } else if v.contains("PHY") {
                info.link_type = LinkType::Phy;
            }
        } else if let Some(rest) = line.strip_prefix("MAC address:") {
            info.mac = rest.trim().parse::<MacAddr>().ok();
        }
    }
    info
}

/// Builds a `restool dprc info dprc.N` body — the mask line then one tab-indented
/// token per set bit — the single fixture builder the shim and parser transcript
/// tests share (review M11; PASS3-F11).
#[cfg(test)]
pub(crate) fn dprc_info(tokens: &[&str]) -> String {
    let mut s = "container id: 2\nicid: 27\nportal id: 3\ndprc options: 0x603\n".to_owned();
    for t in tokens {
        s.push('\t');
        s.push_str(t);
        s.push('\n');
    }
    s
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
    fn dpmac_info_reads_baseline_mac_and_link_type() {
        let i = parse_dpmac_info(DPMAC_INFO_XFI);
        assert_eq!(i.mac, "00:11:22:33:44:55".parse::<MacAddr>().ok());
        assert_eq!(i.link_type, LinkType::Phy);
    }

    #[test]
    fn dpmac_info_reads_fixed_link_type() {
        // The case the prefix bug hid: a FIXED link must not fall to the PHY default.
        let i = parse_dpmac_info("DPMAC link type: DPMAC_LINK_TYPE_FIXED\n");
        assert_eq!(i.link_type, LinkType::Fixed);
    }

    #[test]
    fn dpmac_info_ignores_lowercase_spellings() {
        // The old assumed spelling is not what restool prints; it must not match.
        let i =
            parse_dpmac_info("link type: DPMAC_LINK_TYPE_FIXED\nmac address: 00:11:22:33:44:55\n");
        assert_eq!(i.mac, None);
        assert_eq!(i.link_type, LinkType::Phy);
    }

    #[test]
    fn dpni_info_reads_lowercase_mac_and_endpoint() {
        // restool spells dpni info fields lowercase, the inverse of dpmac info:
        // reference.json dpni.0 (~line 1410) prints `mac address:` / `endpoint:`,
        // while dpmac blocks print `MAC address:`. Do not "align" dpni to dpmac.
        let i = parse_dpni_info("endpoint: dpmac.7, link is up\nmac address: 00:00:00:00:00:29\n");
        assert_eq!(i.endpoint, Some(DpmacId::from(7)));
        assert_eq!(i.mac, "00:00:00:00:00:29".parse::<MacAddr>().ok());
    }

    #[test]
    fn dpni_endpoint_reads_dpmac_dpni_and_disconnected_peers() {
        // The idempotence read recovers ANY family peer (DPNI-I9 cross-container case).
        let dpmac = parse_dpni_endpoint("endpoint: dpmac.7, link is up\n");
        assert_eq!(dpmac, Some(ObjectRef::new(Family::Dpmac, 7)));
        let dpni = parse_dpni_endpoint("endpoint: dpni.5, link is up\n");
        assert_eq!(dpni, Some(ObjectRef::new(Family::Dpni, 5)));
        assert_eq!(
            parse_dpni_endpoint("endpoint: No object associated\n"),
            None
        );
        assert_eq!(
            parse_dpni_endpoint("mac address: absent\n"),
            None
        );
    }

    #[test]
    fn dpni_info_ignores_capitalized_mac_spelling() {
        // dpmac's capitalized `MAC address:` is not what dpni info prints; it must
        // not match, or a real board's dpni mac would read absent.
        let i = parse_dpni_info("MAC address: 00:00:00:00:00:29\n");
        assert_eq!(i.mac, None);
    }

    #[test]
    fn dpmac_offer_reads_connected_and_unconnected_endpoint() {
        // reference.json dpmac.17: `endpoint: dpni.0, link is up`.
        let connected = parse_dpmac_offer("endpoint: dpni.0, link is up\n");
        assert_eq!(connected.endpoint, Some(DpniId::from(0)));
        // An unconnected dpmac reports `No object associated`.
        assert_eq!(parse_dpmac_offer(DPMAC_INFO_XFI).endpoint, None);
        // A non-dpni peer (a future dpdmux/dpsw) is not an owner signal here.
        assert_eq!(parse_dpmac_offer("endpoint: dpdmux.1\n").endpoint, None);
    }

    #[test]
    fn dprc_rows_surfaces_family_num_and_label() {
        let show = "\
dprc.1 contains 4 objects:
object          label           plugged-state
dpmac.17                        plugged
dpni.0          eth0            plugged
dpbp.0                          unplugged
";
        let rows = parse_dprc_rows(show);
        assert_eq!(
            rows,
            vec![
                DprcRow {
                    family: Family::Dpmac,
                    num: 17,
                    label: ConstructName::empty(),
                    plugged: true,
                },
                DprcRow {
                    family: Family::Dpni,
                    num: 0,
                    label: ConstructName::from("eth0"),
                    plugged: true,
                },
                DprcRow {
                    family: Family::Dpbp,
                    num: 0,
                    label: ConstructName::empty(),
                    plugged: false,
                },
            ]
        );
    }

    #[test]
    fn dprc_rows_skips_header_contains_and_unknown_family() {
        // Header and the `contains` line end in tokens that are neither
        // plugged/unplugged; a `dpfoo.` family is outside ALL_FAMILIES. All skipped.
        let show = "\
dprc.1 contains 2 objects:
object          label           plugged-state
dpfoo.2                         plugged
dpni.7          wan0            plugged
";
        let rows = parse_dprc_rows(show);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].family, Family::Dpni);
        assert_eq!(rows[0].num, 7);
    }

    #[test]
    fn dprc_info_decodes_the_default_mask() {
        // DPRC-I4 default; the IRQ_CFG token is outside the mask and ignored.
        let opts = parse_dprc_info(&dprc_info(&[
            "DPRC_CFG_OPT_SPAWN_ALLOWED",
            "DPRC_CFG_OPT_ALLOC_ALLOWED",
            "DPRC_CFG_OPT_OBJ_CREATE_ALLOWED",
            "DPRC_CFG_OPT_IRQ_CFG_ALLOWED",
        ]))
        .expect("options present");
        assert_eq!(opts, dprc::Options::DEFAULT);
    }

    #[test]
    fn dprc_info_decodes_a_nondefault_mask() {
        let opts = parse_dprc_info(&dprc_info(&[
            "DPRC_CFG_OPT_SPAWN_ALLOWED",
            "DPRC_CFG_OPT_ALLOC_ALLOWED",
            "DPRC_CFG_OPT_OBJ_CREATE_ALLOWED",
            "DPRC_CFG_OPT_TOPOLOGY_CHANGES_ALLOWED",
        ]))
        .expect("options present");
        assert!(opts.topology_changes);
        assert_eq!(
            opts,
            dprc::Options {
                topology_changes: true,
                ..dprc::Options::DEFAULT
            }
        );
    }

    #[test]
    fn dprc_info_absent_options_line_is_none() {
        // No `dprc options:` line (a refused/malformed read) => None, never a default.
        assert_eq!(parse_dprc_info("container id: 2\nicid: 27\n"), None);
    }

    #[test]
    fn dprc_id_parses_created_container() {
        assert_eq!(parse_dprc_id("dprc.3\n"), Some(DprcId::new(3)));
        // A portal-offset suffix on the line is ignored.
        assert_eq!(
            parse_dprc_id("dprc.4, portal offset 0x10\n"),
            Some(DprcId::new(4))
        );
        assert_eq!(parse_dprc_id("no container here"), None);
    }

    #[test]
    fn mc_status_extracts_the_parenthesized_byte() {
        // The three refusal shapes restool prints (unknown-register #3).
        assert_eq!(parse_mc_status("Configuration error (0x6)"), Some(6));
        assert_eq!(parse_mc_status("No resources (0x8)"), Some(8));
        assert_eq!(parse_mc_status("No privilege (0x4)"), Some(4));
    }

    #[test]
    fn mc_status_absent_when_no_status_token() {
        // A restool client-side guard fires before any MC command: no status token.
        assert_eq!(
            parse_mc_status("error: cannot be moved because it is currently in plugged state"),
            None
        );
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

    /// The `mac address:` line, built from the domain type so no MAC literal appears in
    /// source text (public-repo leak-scan); `MacAddr` Display renders the canonical form.
    fn mac_line() -> String {
        format!("mac address: {}\n", MacAddr::new([2, 0, 0, 0, 0, 7]))
    }

    #[test]
    fn dpni_info_parses_the_attr_block_with_split_tcs() {
        // The split read-back lands on the raw struct (num_rx_tcs distinct from num_tx_tcs).
        let body = format!(
            "endpoint: dpmac.7, link is up\n\
             {}\
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
             wriop_version: 0xc00\n",
            mac_line()
        );
        let info = parse_dpni_info(&body);
        assert_eq!(info.endpoint, Some(DpmacId::new(7)));
        assert_eq!(info.mac, Some(MacAddr::new([2, 0, 0, 0, 0, 7])));
        let attr = info.attr.expect("attr block present");
        assert_eq!(attr.options, 0x8000_03d0);
        assert_eq!(attr.num_queues, Some(16));
        assert_eq!(attr.num_rx_tcs, Some(8));
        assert_eq!(attr.num_tx_tcs, Some(16));
        assert_eq!(attr.mac_entries, Some(16));
        assert_eq!(attr.vlan_entries, Some(16));
        assert_eq!(attr.num_channels, Some(1));
        assert_eq!(attr.num_opr, Some(0));
        // Informational read-back kept off the domain observation (baseline: 0xC00 = WRIOP 3.0.0).
        assert_eq!(attr.wriop_version, Some(0xc00));
    }

    #[test]
    fn dpni_info_without_the_attr_block_leaves_attr_none() {
        // Endpoint/mac still parse; the absent attr block is an honest None (no synthesis).
        let body = format!("endpoint: dpmac.7, link is up\n{}", mac_line());
        let info = parse_dpni_info(&body);
        assert_eq!(info.endpoint, Some(DpmacId::new(7)));
        assert_eq!(info.mac, Some(MacAddr::new([2, 0, 0, 0, 0, 7])));
        assert!(info.attr.is_none());
    }
}
