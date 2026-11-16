//! Where agent commands run: omp's `exec.backend` for one profile. Empty runs the bash tool's commands on this
//! machine; `sandbox-vm/default` runs them in the virtual machine of the Sandbox VM plugin.
//!
//! omp reads the setting itself when a session starts, so the app only reads and writes the value through omp's
//! own CLI (`omp config get exec.backend --json` / `omp config set exec.backend <backend>`), exactly like
//! `sandbox_config`. The CLI's `/settings` and this switch therefore edit one value. A change reaches tabs
//! started (or restarted) afterwards. A backend that is chosen but whose plugin is not loaded stops `bash` with
//! an error; omp never falls back to this machine.
//!
//! `profile` must have gone through `ProfileStore::resolve_owned`: `omp --profile <id>` silently creates a
//! profile directory for an unknown id.

use std::sync::{Mutex, PoisonError};

/// The omp setting this module reads and writes.
const SETTING_KEY: &str = "exec.backend";

/// Names the subcommand in `omp_cli::run`'s failure messages.
const OMP_CONFIG: &str = "omp config";

/// How much of an unexpected `config get` output is quoted in an error.
const OUTPUT_SNIPPET_MAX_CHARS: usize = 120;

/// The values the Settings row offers, in its order: this machine, then the VM.
pub const BACKENDS: [&str; 2] = ["", "sandbox-vm/default"];

/// Serialises a write with the read that follows it, so a concurrent read never reports the value being
/// replaced. Guards no data; a poisoned lock is still usable.
static CONFIG_LOCK: Mutex<()> = Mutex::new(());

/// The effective `exec.backend` of `profile`. Spawns `omp`: callers run it off the main thread.
///
/// # Errors
/// omp fails, or prints something other than one of [`BACKENDS`].
pub fn get(profile: Option<&str>) -> Result<String, String> {
    let _guard = CONFIG_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    read_backend(profile)
}

/// Write `backend` to the profile's `exec.backend` and return the value omp reports afterwards. Blocking, like
/// [`get`].
///
/// # Errors
/// `backend` is not one of [`BACKENDS`] (nothing is run), or omp fails.
pub fn set(profile: Option<&str>, backend: &str) -> Result<String, String> {
    if !BACKENDS.contains(&backend) {
        return Err(format!(
            "unknown exec backend `{backend}` (expected one of {BACKENDS:?})"
        ));
    }
    let _guard = CONFIG_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    crate::omp_cli::run(&set_args(profile, backend), OMP_CONFIG)?;
    read_backend(profile)
}

fn read_backend(profile: Option<&str>) -> Result<String, String> {
    parse_backend(&crate::omp_cli::run(&get_args(profile), OMP_CONFIG)?)
}

/// Argv for `omp [--profile=<id>] config get exec.backend --json`.
fn get_args(profile: Option<&str>) -> Vec<String> {
    let mut args = Vec::with_capacity(5);
    args.extend(crate::agent::spawn::profile_flag(profile));
    args.extend(["config", "get", SETTING_KEY, "--json"].map(String::from));
    args
}

/// Argv for `omp [--profile=<id>] config set exec.backend <backend>`.
fn set_args(profile: Option<&str>, backend: &str) -> Vec<String> {
    let mut args = Vec::with_capacity(5);
    args.extend(crate::agent::spawn::profile_flag(profile));
    args.extend(["config", "set", SETTING_KEY, backend].map(String::from));
    args
}

/// Parse `omp config get exec.backend --json`'s stdout (`{"key":…,"value":"",…}`) into one of [`BACKENDS`].
fn parse_backend(stdout: &[u8]) -> Result<String, String> {
    let text = String::from_utf8_lossy(stdout);
    let text = text.trim();
    let unexpected = || {
        let snippet: String = text.chars().take(OUTPUT_SNIPPET_MAX_CHARS).collect();
        let ellipsis = if snippet.len() < text.len() {
            "…"
        } else {
            ""
        };
        format!("unexpected {SETTING_KEY} from omp: {snippet}{ellipsis}")
    };
    let parsed: serde_json::Value = serde_json::from_str(text).map_err(|_| unexpected())?;
    match parsed.get("value").and_then(serde_json::Value::as_str) {
        Some(backend) if BACKENDS.contains(&backend) => Ok(backend.to_string()),
        _ => Err(unexpected()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_put_the_profile_flag_before_the_subcommand() {
        assert_eq!(
            get_args(Some("work")),
            ["--profile=work", "config", "get", "exec.backend", "--json"]
        );
        assert_eq!(
            set_args(None, "sandbox-vm/default"),
            ["config", "set", "exec.backend", "sandbox-vm/default"]
        );
    }

    #[test]
    fn parse_backend_reads_a_known_value_and_rejects_anything_else() {
        let vm = br#"{"key":"exec.backend","value":"sandbox-vm/default","type":"string"}"#;
        assert_eq!(parse_backend(vm).unwrap(), "sandbox-vm/default");
        let here = br#"{"key":"exec.backend","value":"","type":"string"}"#;
        assert_eq!(parse_backend(here).unwrap(), "");
        for bad in [
            r#"{"value":"other/backend"}"#,
            r#"{"value":null}"#,
            r#"{"key":"exec.backend"}"#,
            "not json",
        ] {
            let err = parse_backend(bad.as_bytes()).unwrap_err();
            assert!(err.starts_with("unexpected exec.backend"), "{err}");
        }
    }

    #[test]
    fn set_refuses_an_unknown_backend_before_running_omp() {
        let err = set(None, "other/backend").unwrap_err();
        assert!(err.contains("unknown exec backend"), "{err}");
    }
}
