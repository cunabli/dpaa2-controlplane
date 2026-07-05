//! TOML parsing, validation, and conversion into the neutral model.
//!
//! Parses `topology.toml`, validates it, and converts it into the neutral
//! [`dpaa2_api::DesiredTopology`] — the only thing the reconciler sees. The config is
//! keyed by stable DPMAC anchors and refuses any attempt to pin a DPNI index, whose
//! identity is derived from the DPMAC edge (design D1).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use dpaa2_api::{
    ConfigSource, DesiredPort, DesiredTopology, DpmacId, Error, MacAddr, MacMode, Presence,
};

use crate::schema::{RawMacMode, RawPort, RawTopology};

/// A [`ConfigSource`] backed by a `topology.toml` file on disk.
pub struct TomlConfig {
    path: PathBuf,
}

impl TomlConfig {
    /// Points at a TOML file. The file is not read until [`ConfigSource::load`].
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl ConfigSource for TomlConfig {
    fn load(&self) -> Result<DesiredTopology, Error> {
        let text = std::fs::read_to_string(&self.path)?;
        parse_str(&text)
    }
}

/// Parses and validates TOML text into a neutral desired topology.
///
/// # Errors
/// Returns [`Error::Config`] on malformed TOML, DPNI-index pinning, malformed DPMAC
/// or MAC references, or duplicate interface names.
pub fn parse_str(text: &str) -> Result<DesiredTopology, Error> {
    let raw: RawTopology =
        toml::from_str(text).map_err(|e| Error::Config(e.message().to_owned()))?;
    convert(&raw)
}

fn convert(raw: &RawTopology) -> Result<DesiredTopology, Error> {
    let mut names: HashSet<&str> = HashSet::new();
    let mut anchors: HashSet<DpmacId> = HashSet::new();
    let mut topology = DesiredTopology::new();

    for port in &raw.port {
        let desired = convert_port(port)?;

        if !names.insert(port.name.as_str()) {
            return Err(Error::Config(format!(
                "duplicate interface name `{}`",
                port.name
            )));
        }
        if !anchors.insert(desired.dpmac) {
            return Err(Error::Config(format!(
                "duplicate DPMAC anchor `{}`",
                desired.dpmac
            )));
        }
        topology.push(desired);
    }

    Ok(topology)
}

fn convert_port(port: &RawPort) -> Result<DesiredPort, Error> {
    if let Some(dpni) = &port.dpni {
        return Err(Error::Config(format!(
            "port `{}` pins a DPNI index (`dpni = \"{dpni}\"`); DPNI identity is \
             derived from the DPMAC edge and must not be set",
            port.name
        )));
    }

    validate_name(&port.name)?;

    let dpmac = parse_dpmac(&port.dpmac)
        .ok_or_else(|| Error::Config(format!("port `{}` has malformed `dpmac`", port.name)))?;

    let mac =
        match &port.mac {
            Some(s) => Some(s.parse::<MacAddr>().map_err(|_| {
                Error::Config(format!("port `{}` has malformed MAC `{s}`", port.name))
            })?),
            None => None,
        };

    let mac_mode = match port.mac_mode {
        RawMacMode::Assert => MacMode::Assert,
        RawMacMode::Actuate => MacMode::Actuate,
    };

    Ok(DesiredPort {
        dpmac,
        name: port.name.clone(),
        mac,
        mac_mode,
        presence: Presence::Present,
        immutable: std::collections::BTreeMap::new(),
    })
}

/// Validates a port `name` against the constraints it must satisfy to become both a
/// filename component and `.link` file content: 1-15 bytes (the Linux `IFNAMSIZ`
/// limit) of ASCII alphanumeric, `-`, or `_` only. Without this, a name containing
/// `/`, `..`, or a newline would flow unvalidated into a path or config file rather
/// than being rejected here.
fn validate_name(name: &str) -> Result<(), Error> {
    if name.is_empty() || name.len() > 15 {
        return Err(Error::Config(format!(
            "port `{name}` has an invalid `name`: must be 1-15 bytes (IFNAMSIZ limit), got {} bytes",
            name.len()
        )));
    }
    if let Some(c) = name
        .chars()
        .find(|&c| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
    {
        return Err(Error::Config(format!(
            "port `{name}` has an invalid `name`: character `{c}` is not allowed (only ASCII alphanumeric, `-`, `_`)"
        )));
    }
    Ok(())
}

/// Parses a `dpmac.N` reference into a [`DpmacId`].
fn parse_dpmac(s: &str) -> Option<DpmacId> {
    let n = s.strip_prefix("dpmac.")?;
    n.parse::<u32>().ok().map(DpmacId::new)
}

/// Convenience: load and validate a topology file at `path`.
///
/// # Errors
/// See [`parse_str`]; also returns [`Error::Io`] if the file cannot be read.
pub fn load(path: impl AsRef<Path>) -> Result<DesiredTopology, Error> {
    TomlConfig::new(path.as_ref().to_path_buf()).load()
}

#[cfg(test)]
mod tests {
    //! Parsing, conversion, and validation tests for the TOML parse module.

    use dpaa2_api::{DpmacId, MacAddr, MacMode};

    use crate::parse_str;

    #[test]
    fn valid_config_converts_to_neutral_model() {
        let toml = r#"
            [[port]]
            dpmac = "dpmac.3"
            name = "wan0"
            mac = "02:00:00:00:00:03"

            [[port]]
            dpmac = "dpmac.7"
            name = "lan0"
            mac_mode = "actuate"
            mac = "02-00-00-00-00-07"
        "#;
        let topo = parse_str(toml).expect("valid config");
        assert_eq!(topo.ports().len(), 2);

        let wan = &topo.ports()[0];
        assert_eq!(wan.dpmac, DpmacId::new(3));
        assert_eq!(wan.name, "wan0");
        assert_eq!(wan.mac, Some(MacAddr::new([0x02, 0, 0, 0, 0, 0x03])));
        assert_eq!(wan.mac_mode, MacMode::Assert, "default is assert");

        let lan = &topo.ports()[1];
        assert_eq!(lan.dpmac, DpmacId::new(7));
        assert_eq!(lan.mac_mode, MacMode::Actuate);
        assert_eq!(lan.mac, Some(MacAddr::new([0x02, 0, 0, 0, 0, 0x07])));
    }

    #[test]
    fn dpni_index_pinning_is_rejected() {
        let toml = r#"
            [[port]]
            dpmac = "dpmac.3"
            name = "wan0"
            dpni = "dpni.3"
        "#;
        let err = parse_str(toml).unwrap_err().to_string();
        assert!(
            err.contains("dpni"),
            "message names the offending key: {err}"
        );
        assert!(err.contains("DPMAC edge"), "explains why: {err}");
    }

    #[test]
    fn duplicate_interface_name_is_rejected() {
        let toml = r#"
            [[port]]
            dpmac = "dpmac.3"
            name = "wan0"

            [[port]]
            dpmac = "dpmac.7"
            name = "wan0"
        "#;
        let err = parse_str(toml).unwrap_err().to_string();
        assert!(err.contains("duplicate interface name"), "{err}");
    }

    #[test]
    fn malformed_mac_is_rejected_with_port_identity() {
        let toml = r#"
            [[port]]
            dpmac = "dpmac.3"
            name = "wan0"
            mac = "not-a-mac"
        "#;
        let err = parse_str(toml).unwrap_err().to_string();
        assert!(err.contains("wan0"), "identifies the port: {err}");
        assert!(err.contains("malformed MAC"), "{err}");
    }

    #[test]
    fn invalid_name_is_rejected() {
        let over_length = r#"
            [[port]]
            dpmac = "dpmac.3"
            name = "way-too-long-for-ifnamsiz"
        "#;
        let err = parse_str(over_length).unwrap_err().to_string();
        assert!(err.contains("invalid `name`"), "{err}");

        let path_escape = r#"
            [[port]]
            dpmac = "dpmac.3"
            name = "../etc"
        "#;
        let err = parse_str(path_escape).unwrap_err().to_string();
        assert!(err.contains("invalid `name`"), "{err}");
        assert!(err.contains('/'), "names the offending character: {err}");
    }

    #[test]
    fn ordinary_names_are_accepted() {
        let toml = r#"
            [[port]]
            dpmac = "dpmac.3"
            name = "wan0"

            [[port]]
            dpmac = "dpmac.7"
            name = "lan-1"
        "#;
        let topo = parse_str(toml).expect("ordinary names are valid");
        assert_eq!(topo.ports()[0].name, "wan0");
        assert_eq!(topo.ports()[1].name, "lan-1");
    }

    #[test]
    fn malformed_dpmac_is_rejected() {
        let toml = r#"
            [[port]]
            dpmac = "eth3"
            name = "wan0"
        "#;
        let err = parse_str(toml).unwrap_err().to_string();
        assert!(err.contains("malformed `dpmac`"), "{err}");
    }

    #[test]
    fn assert_mode_is_the_default() {
        let toml = r#"
            [[port]]
            dpmac = "dpmac.3"
            name = "wan0"
            mac = "02:00:00:00:00:03"
        "#;
        let topo = parse_str(toml).unwrap();
        assert_eq!(topo.ports()[0].mac_mode, MacMode::Assert);
    }

    #[test]
    fn empty_config_is_valid_and_empty() {
        assert_eq!(parse_str("").unwrap().ports().len(), 0);
    }

    #[test]
    fn shipped_example_topology_is_valid() {
        // The example installed to /etc/dpaa2/topology.toml must always parse.
        let example = include_str!("../../../packaging/dpaa2/topology.toml");
        let topo = parse_str(example).expect("shipped example topology parses");
        assert!(!topo.ports().is_empty());
    }
}
