//! The MC command mnemonic namespace as a distinct type.
//!
//! The fsl-mc-uapi driver identifies every MC command by a fixed mnemonic —
//! `DPRC_CREATE_CONT`, `OPEN`, … — the whitelist keys the accepted set on
//! (ADR-0003 §5). [`CommandName`] wraps that mnemonic so a command name is
//! never confused with a free string, mirroring the intent vocabulary's name
//! newtypes without depending on `dpaa2-api` (this crate keeps zero
//! dependencies). It is not an interface name, so it grants no `validate`.

/// One MC command mnemonic (`DPRC_CREATE_CONT`, `OPEN`, …), the key the
/// fsl-mc-uapi whitelist matches an accepted command by (ADR-0003 §5). Wraps a
/// [`String`] and derives the ordering/hashing surface so it keys the same
/// ordered collections a bare string did; construction stays terse through
/// `From<&str>`/`From<String>`. No `Deref` and no `validate`: a command name is
/// not a string and not an interface name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommandName(String);

impl CommandName {
    /// The mnemonic as a string slice, for comparing against a bare `&str`
    /// without reopening the type distinction.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Display for CommandName {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for CommandName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for CommandName {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for CommandName {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}
