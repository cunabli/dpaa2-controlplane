//! The command-runner seam.
//!
//! `restool` cannot be cleanly mocked, so the shim never calls `std::process`
//! directly: it goes through [`Runner`]. Production uses [`RestoolRunner`]; tests use
//! a recorded-output double, keeping parsing verifiable against golden fixtures with
//! no board (design D10).

use std::process::Command;

use dpaa2_api::Error;

/// Runs one `restool` sub-invocation and returns captured stdout.
pub trait Runner {
    /// Executes `restool` with `args` and returns stdout on success.
    ///
    /// # Errors
    /// Returns [`Error::Backend`] if the process fails to spawn or exits non-zero.
    fn run(&self, args: &[&str]) -> Result<String, Error>;
}

/// A [`Runner`] that shells out to the real `restool` binary.
pub struct RestoolRunner {
    binary: String,
}

impl RestoolRunner {
    /// Uses `restool` from `PATH`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            binary: "restool".to_owned(),
        }
    }

    /// Uses an explicit `restool` binary path.
    #[must_use]
    pub fn with_binary(binary: impl Into<String>) -> Self {
        Self {
            binary: binary.into(),
        }
    }
}

impl Default for RestoolRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl Runner for RestoolRunner {
    fn run(&self, args: &[&str]) -> Result<String, Error> {
        tracing::debug!(binary = %self.binary, ?args, "invoking restool");
        let output = Command::new(&self.binary)
            .args(args)
            .output()
            .map_err(|e| Error::Backend(format!("failed to spawn {}: {e}", self.binary)))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Backend(format!(
                "restool {} failed ({}): {}",
                args.join(" "),
                output.status,
                stderr.trim()
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}
