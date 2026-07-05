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

    /// An I/O error occurred.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
