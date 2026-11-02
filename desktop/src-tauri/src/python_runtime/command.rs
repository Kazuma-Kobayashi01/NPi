//! Bounded helper-command runs for the Python runtime: verifying an
//! interpreter, creating the environment, and talking to `uv`.
//!
//! Every one of them goes through [`crate::spawn_gate`] (so no child inherits
//! Chromium's pipe ends), gets its own process group / job object
//! ([`ProcessSupervisor`]), and is collected by [`crate::child_run`] under a
//! deadline with capped output — a `python -m venv` or `uv pip install` that
//! hangs on a slow disk or a stalled network cannot hold a host call open
//! forever. Stderr is kept even for a successful run because that is where
//! `uv` and `pip` report what they did and why they were slow.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::child_run::{collect_keeping_stderr, WaitError};
use crate::supervisor::ProcessSupervisor;

/// What a helper run produced: its exit state and both streams.
pub struct CommandRun {
    pub success: bool,
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Longest a version probe or an interpreter check may take.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(60);
/// Longest `python -m venv` may take (a cold, first-run bootstrap).
pub const VENV_TIMEOUT: Duration = Duration::from_secs(300);
/// Longest a package install may take.
pub const INSTALL_TIMEOUT: Duration = Duration::from_secs(600);
/// Longest listing the installed packages may take.
pub const LIST_TIMEOUT: Duration = Duration::from_secs(120);

/// stdout past this fails the run; helper output is never a payload.
const OUTPUT_CAP_BYTES: usize = 1024 * 1024;
/// The tail of stderr kept for the error message.
const STDERR_KEEP_BYTES: usize = 64 * 1024;
/// How much of a failing command's output an error message shows.
const ERR_TAIL_CHARS: usize = 400;

/// Run `cmd` to its exit, killing its tree at `limit`. stdin is closed: the
/// caller may be the `--npi-python-host` process whose stdin is the request.
///
/// # Errors
/// When the program cannot be started, does not finish within `limit`,
/// prints more than the output cap, or the run fails internally. A non-zero
/// exit is a [`CommandRun`] with `success == false`.
pub fn run_bounded(cmd: &mut Command, limit: Duration, what: &str) -> Result<CommandRun, String> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    ProcessSupervisor::prepare(cmd);
    let program = cmd.get_program().to_string_lossy().into_owned();
    let mut child = crate::spawn_gate::spawn(cmd)
        .map_err(|e| format!("{what} could not start {program}: {e}"))?;
    let supervisor = ProcessSupervisor::attach(&child);
    let done = collect_keeping_stderr(
        &mut child,
        &supervisor,
        OUTPUT_CAP_BYTES,
        STDERR_KEEP_BYTES,
        Instant::now() + limit,
        |_| Ok(()),
    )
    .map_err(|error| match error {
        WaitError::TimedOut => format!("{what} did not finish within {} s", limit.as_secs()),
        WaitError::Overflow => format!("{what} printed more than {} KiB", OUTPUT_CAP_BYTES / 1024),
        WaitError::Failed(message) => format!("{what} failed: {message}"),
    })?;
    Ok(CommandRun {
        success: done.status.success(),
        code: done.status.code(),
        stdout: done.stdout,
        stderr: done.stderr,
    })
}

/// A failed run's exit state plus the redacted tail of its output, for the
/// message a user or the agent reads.
#[must_use]
pub fn failure_text(run: &CommandRun) -> String {
    let bytes = if run.stderr.is_empty() {
        &run.stdout
    } else {
        &run.stderr
    };
    let raw = String::from_utf8_lossy(bytes);
    let text = crate::widgets::redact::redact_secrets(raw.trim());
    let count = text.chars().count();
    let tail: String = if count <= ERR_TAIL_CHARS {
        text.into_owned()
    } else {
        text.chars().skip(count - ERR_TAIL_CHARS).collect()
    };
    match (run.code, tail.is_empty()) {
        (Some(code), true) => format!("exit {code}"),
        (Some(code), false) => format!("exit {code}: {tail}"),
        (None, true) => "killed".to_string(),
        (None, false) => format!("killed: {tail}"),
    }
}
