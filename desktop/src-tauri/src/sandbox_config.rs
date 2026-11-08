//! The process sandbox switch: omp's `sandbox.mode` (`off`,
//! `workspace-write` or `read-only`) for one profile.
//!
//! omp applies the setting itself when a session starts — on macOS it
//! relaunches under a Seatbelt profile that confines file writes — so the app
//! only reads and writes the value, through omp's own CLI (`omp config get
//! sandbox.mode --json` / `omp config set sandbox.mode <mode>`), exactly like
//! `goal_config`. The CLI's `/settings` and this switch therefore edit one
//! value. A change reaches tabs started (or restarted) afterwards; a running
//! omp keeps the sandbox it started with.
//!
//! `profile` must have gone through `ProfileStore::resolve_owned`: `omp
//! --profile <id>` silently creates a profile directory for an unknown id.

use std::sync::{Mutex, PoisonError};

/// The omp setting this module reads and writes.
const SETTING_KEY: &str = "sandbox.mode";

/// Names the subcommand in `omp_cli::run`'s failure messages.
const OMP_CONFIG: &str = "omp config";

/// How much of an unexpected `config get` output is quoted in an error.
const OUTPUT_SNIPPET_MAX_CHARS: usize = 120;

/// The values omp accepts, in the order the Settings row offers them.
pub const MODES: [&str; 3] = ["off", "workspace-write", "read-only"];

/// Serialises a write with the read that follows it, so a concurrent read
/// never reports the value being replaced. Guards no data; a poisoned lock
/// is still usable.
static CONFIG_LOCK: Mutex<()> = Mutex::new(());

/// The effective `sandbox.mode` of `profile`. Spawns `omp`: callers run it
/// off the main thread.
///
/// # Errors
/// omp fails, or prints something other than one of [`MODES`].
pub fn get(profile: Option<&str>) -> Result<String, String> {
    let _guard = CONFIG_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    read_mode(profile)
}

/// Write `mode` to the profile's `sandbox.mode` and return the value omp
/// reports afterwards. Blocking, like [`get`].
///
/// # Errors
/// `mode` is not one of [`MODES`] (nothing is run), or omp fails.
pub fn set(profile: Option<&str>, mode: &str) -> Result<String, String> {
    if !MODES.contains(&mode) {
        return Err(format!(
            "unknown sandbox mode `{mode}` (expected one of {})",
            MODES.join(", ")
        ));
    }
    let _guard = CONFIG_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    crate::omp_cli::run(&set_args(profile, mode), OMP_CONFIG)?;
    read_mode(profile)
}

fn read_mode(profile: Option<&str>) -> Result<String, String> {
    parse_mode(&crate::omp_cli::run(&get_args(profile), OMP_CONFIG)?)
}

/// Argv for `omp [--profile=<id>] config get sandbox.mode --json`.
fn get_args(profile: Option<&str>) -> Vec<String> {
    let mut args = Vec::with_capacity(5);
    args.extend(crate::agent::spawn::profile_flag(profile));
    args.extend(["config", "get", SETTING_KEY, "--json"].map(String::from));
    args
}

/// Argv for `omp [--profile=<id>] config set sandbox.mode <mode>`.
fn set_args(profile: Option<&str>, mode: &str) -> Vec<String> {
    let mut args = Vec::with_capacity(5);
    args.extend(crate::agent::spawn::profile_flag(profile));
    args.extend(["config", "set", SETTING_KEY, mode].map(String::from));
    args
}

/// Parse `omp config get sandbox.mode --json`'s stdout
/// (`{"key":…,"value":"off",…}`) into one of [`MODES`].
fn parse_mode(stdout: &[u8]) -> Result<String, String> {
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
        Some(mode) if MODES.contains(&mode) => Ok(mode.to_string()),
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
            ["--profile=work", "config", "get", "sandbox.mode", "--json"]
        );
        assert_eq!(
            set_args(None, "read-only"),
            ["config", "set", "sandbox.mode", "read-only"]
        );
    }

    #[test]
    fn parse_mode_reads_a_known_value_and_rejects_anything_else() {
        let out = br#"{"key":"sandbox.mode","value":"workspace-write","type":"enum"}"#;
        assert_eq!(parse_mode(out).unwrap(), "workspace-write");
        for bad in [
            r#"{"value":"strict"}"#,
            r#"{"value":null}"#,
            r#"{"key":"sandbox.mode"}"#,
            "not json",
        ] {
            let err = parse_mode(bad.as_bytes()).unwrap_err();
            assert!(err.starts_with("unexpected sandbox.mode"), "{err}");
        }
    }

    #[test]
    fn set_refuses_an_unknown_mode_before_running_omp() {
        let err = set(None, "strict").unwrap_err();
        assert!(err.contains("unknown sandbox mode"), "{err}");
    }
}
