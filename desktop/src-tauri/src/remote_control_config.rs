//! The terminal's `/rc` (remote control: a paired phone drives an `npi` session in a terminal) for one
//! profile: omp's `remote.autoStart` (on by default) and `remote.relayUrl`.
//!
//! omp applies both when an interactive session starts, so the app only reads and writes them through omp's own
//! CLI (`omp config get <key> --json` / `omp config set <key> <value>`), like `sandbox_config`; the CLI's
//! `/settings` and this row edit the same values. App tabs run `--mode rpc-ui`, which has no `/rc`.
//!
//! `profile` must have gone through `ProfileStore::resolve_owned`: `omp --profile <id>` silently creates a
//! profile directory for an unknown id.

use std::sync::{Mutex, PoisonError};

use serde::Serialize;
use serde_json::Value;

const AUTO_START_KEY: &str = "remote.autoStart";
const RELAY_URL_KEY: &str = "remote.relayUrl";

/// Names the subcommand in `omp_cli::run`'s failure messages.
const OMP_CONFIG: &str = "omp config";

/// How much of an unexpected `config get` output is quoted in an error.
const OUTPUT_SNIPPET_MAX_CHARS: usize = 120;

/// Serialises a write with the read that follows it. Guards no data; a poisoned lock is still usable.
static CONFIG_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteControlConfig {
    pub auto_start: bool,
    pub relay_url: String,
}

/// Both settings of `profile`. Spawns `omp` twice: callers run it off the main thread.
///
/// # Errors
/// omp fails, or prints a value of the wrong type.
pub fn get(profile: Option<&str>) -> Result<RemoteControlConfig, String> {
    let _guard = CONFIG_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    read(profile)
}

/// Write `remote.autoStart` and return both settings as omp reports them afterwards. Blocking, like [`get`].
///
/// # Errors
/// omp fails.
pub fn set_auto_start(profile: Option<&str>, enabled: bool) -> Result<RemoteControlConfig, String> {
    let _guard = CONFIG_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    crate::omp_cli::run(
        &set_args(
            profile,
            AUTO_START_KEY,
            if enabled { "true" } else { "false" },
        ),
        OMP_CONFIG,
    )?;
    read(profile)
}

/// Write `remote.relayUrl` (trimmed; empty clears it) and return both settings afterwards. Blocking.
///
/// # Errors
/// A non-empty `url` that is not `wss://…` or `ws://…` (nothing is run; omp's `/rc` refuses those too), or omp
/// fails.
pub fn set_relay_url(profile: Option<&str>, url: &str) -> Result<RemoteControlConfig, String> {
    let url = url.trim();
    if !url.is_empty() && !url.starts_with("wss://") && !url.starts_with("ws://") {
        return Err(format!(
            "relay URL must start with wss:// or ws://, got `{url}`"
        ));
    }
    let _guard = CONFIG_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    crate::omp_cli::run(&set_args(profile, RELAY_URL_KEY, url), OMP_CONFIG)?;
    read(profile)
}

fn read(profile: Option<&str>) -> Result<RemoteControlConfig, String> {
    let auto = parse_value(
        AUTO_START_KEY,
        &crate::omp_cli::run(&get_args(profile, AUTO_START_KEY), OMP_CONFIG)?,
    )?;
    let relay = parse_value(
        RELAY_URL_KEY,
        &crate::omp_cli::run(&get_args(profile, RELAY_URL_KEY), OMP_CONFIG)?,
    )?;
    match (auto.as_bool(), relay.as_str()) {
        (Some(auto_start), Some(relay_url)) => Ok(RemoteControlConfig {
            auto_start,
            relay_url: relay_url.to_string(),
        }),
        (None, _) => Err(format!("unexpected {AUTO_START_KEY} from omp: {auto}")),
        (_, None) => Err(format!("unexpected {RELAY_URL_KEY} from omp: {relay}")),
    }
}

/// Argv for `omp [--profile=<id>] config get <key> --json`.
fn get_args(profile: Option<&str>, key: &str) -> Vec<String> {
    let mut args = Vec::with_capacity(5);
    args.extend(crate::agent::spawn::profile_flag(profile));
    args.extend(["config", "get", key, "--json"].map(String::from));
    args
}

/// Argv for `omp [--profile=<id>] config set <key> <value>`: the value is one argv element, no shell involved.
fn set_args(profile: Option<&str>, key: &str, value: &str) -> Vec<String> {
    let mut args = Vec::with_capacity(5);
    args.extend(crate::agent::spawn::profile_flag(profile));
    args.extend(["config", "set", key, value].map(String::from));
    args
}

/// The `value` of `omp config get <key> --json`'s stdout (`{"key":…,"value":…,…}`).
fn parse_value(key: &str, stdout: &[u8]) -> Result<Value, String> {
    let text = String::from_utf8_lossy(stdout);
    let text = text.trim();
    let unexpected = || {
        let snippet: String = text.chars().take(OUTPUT_SNIPPET_MAX_CHARS).collect();
        let ellipsis = if snippet.len() < text.len() {
            "…"
        } else {
            ""
        };
        format!("unexpected {key} from omp: {snippet}{ellipsis}")
    };
    let mut parsed: Value = serde_json::from_str(text).map_err(|_| unexpected())?;
    parsed
        .get_mut("value")
        .map(Value::take)
        .ok_or_else(unexpected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_put_the_profile_flag_before_the_subcommand() {
        assert_eq!(
            get_args(Some("work"), AUTO_START_KEY),
            [
                "--profile=work",
                "config",
                "get",
                "remote.autoStart",
                "--json"
            ]
        );
        assert_eq!(
            set_args(None, RELAY_URL_KEY, ""),
            ["config", "set", "remote.relayUrl", ""]
        );
    }

    #[test]
    fn parse_value_takes_the_value_field_and_quotes_anything_else() {
        let out = br#"{"key":"remote.autoStart","value":true,"type":"boolean"}"#;
        assert_eq!(parse_value(AUTO_START_KEY, out).unwrap(), Value::Bool(true));
        for bad in [r#"{"key":"remote.autoStart"}"#, "not json"] {
            let err = parse_value(AUTO_START_KEY, bad.as_bytes()).unwrap_err();
            assert!(err.starts_with("unexpected remote.autoStart"), "{err}");
        }
    }

    #[test]
    fn a_relay_that_is_not_a_websocket_url_is_refused_before_running_omp() {
        let err = set_relay_url(None, "https://relay.example").unwrap_err();
        assert!(err.contains("wss://"), "{err}");
    }
}
