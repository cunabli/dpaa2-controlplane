//! The shared error type for backend and config operations.

use thiserror::Error;

/// Errors surfaced by the southbound and northbound ports.
///
/// The pure reconciler itself performs no I/O and does not fail; this type is for
/// the adapters (`McControl`, `KernelControl`, `ConfigSource`) and the shell.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// An underlying command or backend invocation failed.
    #[error("backend failure: {0}")]
    Backend(String),

    /// The backend produced output that could not be parsed.
    #[error("failed to parse backend output: {0}")]
    Parse(String),

    /// The configuration was malformed or failed validation.
    #[error("invalid configuration: {0}")]
    Config(String),

    /// An MC firmware command was refused with a non-OK status. Carries the raw MC
    /// status byte for the core to judge — the adapter reports it, it does not classify
    /// (design D4). The status→cause mapping lives once, core-side, in
    /// [`dprc::Refusal::mc_status`](crate::dprc::Refusal::mc_status) and
    /// [`dprc_plan::attribute_mc`](crate::dprc_plan::attribute_mc).
    #[error("MC command refused with status {status:#04x}")]
    McStatus {
        /// The raw MC status byte restool surfaced (e.g. `0x4`/`0x6`/`0x8`).
        status: u8,
    },

    /// restool refused an operation with its own client-side guard, before issuing any
    /// MC command (e.g. the plugged-move guard, `docs/baseline/dprc.md` DPRC-I3).
    /// Kept distinct from [`Error::McStatus`] so the core can attribute it to
    /// [`dprc_plan::Attribution::RestoolClientGuard`](crate::dprc_plan::Attribution::RestoolClientGuard)
    /// (design D4).
    #[error("restool client-side refusal: {detail}")]
    RestoolGuard {
        /// The restool guard message, verbatim for the operator.
        detail: String,
    },

    /// An I/O error occurred.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
