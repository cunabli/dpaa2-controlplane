//! The command-runner seam.
//!
//! `restool` cannot be cleanly mocked, so the shim never calls `std::process`
//! directly: it goes through [`Runner`]. Production uses [`RestoolRunner`]; tests use
//! a recorded-output double, keeping parsing verifiable against golden fixtures with
//! no board (design D10).

use std::process::Command;

use dpaa2_api::Error;

/// The full captured result of one `restool` invocation — stdout, stderr, and the
/// process exit code (`None` when the process was killed by a signal). The refusal
/// classifier reads this to tell an MC-status refusal from a restool client-side guard
/// (design D4): a non-zero exit whose output carries an MC status is the former, one
/// without is the latter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome {
    /// Captured stdout.
    pub stdout: String,
    /// Captured stderr — where restool prints its refusal diagnostics.
    pub stderr: String,
    /// The process exit code; `Some(0)` on success.
    pub code: Option<i32>,
}

/// Runs one `restool` sub-invocation and returns captured stdout.
pub trait Runner {
    /// Executes `restool` with `args` and returns stdout on success.
    ///
    /// # Errors
    /// Returns [`Error::Backend`] if the process fails to spawn or exits non-zero.
    fn run(&self, args: &[&str]) -> Result<String, Error>;

    /// Executes `restool` and captures the full [`RunOutcome`] — including a non-zero
    /// exit — so the shim can classify a refusal instead of collapsing it to a string
    /// (design D4). The default adapts [`Runner::run`] for runners that do not model
    /// exit codes: a success yields `code: Some(0)`, and a failure propagates as the
    /// error `run` already built. Only [`RestoolRunner`] (and refusal-transcript test
    /// doubles) override this to carry the exit code and stderr of a refused command.
    ///
    /// # Errors
    /// Returns [`Error::Backend`] if the process fails to spawn.
    fn run_capture(&self, args: &[&str]) -> Result<RunOutcome, Error> {
        Ok(RunOutcome {
            stdout: self.run(args)?,
            stderr: String::new(),
            code: Some(0),
        })
    }
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
        let out = self.run_capture(args)?;
        if out.code != Some(0) {
            return Err(Error::Backend(format!(
                "restool {} failed (exit {:?}): {}",
                args.join(" "),
                out.code,
                out.stderr.trim()
            )));
        }
        Ok(out.stdout)
    }

    fn run_capture(&self, args: &[&str]) -> Result<RunOutcome, Error> {
        tracing::debug!(binary = %self.binary, ?args, "invoking restool");
        let output = Command::new(&self.binary)
            .args(args)
            .output()
            .map_err(|e| Error::Backend(format!("failed to spawn {}: {e}", self.binary)))?;
        Ok(RunOutcome {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            code: output.status.code(),
        })
    }
}
