//! The model catalog and the default model, through omp's own CLI.
//!
//! No session is needed for either: `omp models --json` lists every chat
//! model omp can use right now (the providers it is logged in to, plus the
//! keyless ones), and the default model is the `default` entry of omp's
//! `modelRoles` setting, read and written with `omp config get|set`. Going
//! through the CLI keeps omp the owner of the config file's location,
//! format and layering, exactly as `goal_config` does for its setting.
//!
//! `profile` is `None` for the built-in profile or `Some(id)` for a named
//! one, passed as a global `--profile=<id>` before the subcommand.
//! `omp --profile <id>` silently creates the profile's folder for an
//! unknown id, so callers MUST only pass ids that went through
//! `ProfileStore::resolve_owned`.

use std::sync::{Mutex, PoisonError};

use serde_json::{json, Map, Value};

const OMP_MODELS: &str = "omp models";
const OMP_USAGE: &str = "omp usage";
const OMP_CONFIG: &str = "omp config";

/// The omp setting holding the model of each role (`default`, `smol`, …).
const ROLES_KEY: &str = "modelRoles";
const DEFAULT_ROLE: &str = "default";

/// How much of unexpected CLI output is quoted in an error.
const OUTPUT_SNIPPET_MAX_CHARS: usize = 120;

/// Serialises the read-modify-write of `modelRoles`, so two concurrent
/// changes cannot drop each other's. Guards no data; a poisoned lock is
/// still usable.
static CONFIG_LOCK: Mutex<()> = Mutex::new(());

/// Every chat model omp offers, one object per model:
/// `{provider, id, name, selector, thinking: [levels], contextWindow}`.
///
/// Spawns `omp`; callers MUST run this off the main thread.
pub fn catalog(profile: Option<&str>) -> Result<Value, String> {
    let mut args: Vec<String> = Vec::with_capacity(3);
    args.extend(crate::agent::spawn::profile_flag(profile));
    args.extend(["models", "--json"].map(String::from));
    parse_catalog(&crate::omp_cli::run(&args, OMP_MODELS)?)
}

/// What each signed-in subscription still allows, from `omp usage --json`:
/// one object per provider that reports limits,
/// `{provider, plan, limits: [{label, windowId, durationMs, resetsAt,
/// usedFraction, remainingFraction, unit, status}]}`. A provider used with an
/// API key has no limits and is not listed. Account names and ids in omp's
/// report are dropped (and asked to be redacted) — only the numbers are kept.
/// A forced read invalidates cached reports first; omp still applies its
/// provider failure cooldown.
///
/// Spawns `omp`, which asks the providers over the network (a few seconds);
/// callers MUST run this off the main thread.
pub fn usage_limits(profile: Option<&str>, force: bool) -> Result<Value, String> {
    let mut args: Vec<String> = Vec::with_capacity(4);
    args.extend(crate::agent::spawn::profile_flag(profile));
    if force {
        args.extend(["usage", "invalidate"].map(String::from));
        crate::omp_cli::run(&args, "omp usage invalidate")?;
        args.truncate(args.len() - 2);
    }
    args.extend(["usage", "--json", "--redact"].map(String::from));
    parse_usage(&crate::omp_cli::run(&args, OMP_USAGE)?)
}

/// The `default` model role of `profile` (`provider/id` with an optional
/// `:level` suffix), `None` when unset.
pub fn default_get(profile: Option<&str>) -> Result<Option<String>, String> {
    let _guard = CONFIG_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    read_roles(profile).map(|roles| default_of(&roles))
}

/// Make `selector` (`provider/id`, optionally `:level`) the profile's default
/// model, keeping every other role, and return the default re-read after the
/// write.
pub fn default_set(profile: Option<&str>, selector: &str) -> Result<Option<String>, String> {
    let selector = selector.trim();
    if !valid_selector(selector) {
        return Err(format!("not a model selector: {selector:?}"));
    }
    let _guard = CONFIG_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let mut roles = read_roles(profile)?;
    roles.insert(DEFAULT_ROLE.to_string(), Value::String(selector.to_string()));
    crate::omp_cli::run(&set_args(profile, &roles)?, OMP_CONFIG)?;
    read_roles(profile).map(|roles| default_of(&roles))
}

fn default_of(roles: &Map<String, Value>) -> Option<String> {
    roles
        .get(DEFAULT_ROLE)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// `provider/id` with an optional `:level`: non-empty, one `/`-separated
/// provider, no whitespace or control characters (it ends up in a YAML file).
fn valid_selector(selector: &str) -> bool {
    !selector.is_empty()
        && selector.len() <= 200
        && selector.contains('/')
        && !selector.starts_with('/')
        && !selector.chars().any(|c| c.is_whitespace() || c.is_control())
}

fn read_roles(profile: Option<&str>) -> Result<Map<String, Value>, String> {
    let mut args: Vec<String> = Vec::with_capacity(5);
    args.extend(crate::agent::spawn::profile_flag(profile));
    args.extend(["config", "get", ROLES_KEY, "--json"].map(String::from));
    parse_roles(&crate::omp_cli::run(&args, OMP_CONFIG)?)
}

/// Argv for `omp [--profile=<id>] config set modelRoles '<json object>'`: the
/// object text is ONE argv element, so no shell or quoting layer is involved.
fn set_args(profile: Option<&str>, roles: &Map<String, Value>) -> Result<Vec<String>, String> {
    let value = serde_json::to_string(roles)
        .map_err(|e| format!("could not encode {ROLES_KEY}: {e}"))?;
    let mut args: Vec<String> = Vec::with_capacity(5);
    args.extend(crate::agent::spawn::profile_flag(profile));
    args.extend(["config", "set", ROLES_KEY].map(String::from));
    args.push(value);
    Ok(args)
}

fn snippet_error(what: &str, stdout: &[u8]) -> String {
    let text = String::from_utf8_lossy(stdout);
    let text = text.trim();
    let snippet: String = text.chars().take(OUTPUT_SNIPPET_MAX_CHARS).collect();
    let ellipsis = if snippet.len() < text.len() { "…" } else { "" };
    format!("unexpected {what} from omp: {snippet}{ellipsis}")
}

/// `omp config get modelRoles --json` prints
/// `{"key":…,"value":{…},"type":"record",…}`; an unset record may be `null`.
fn parse_roles(stdout: &[u8]) -> Result<Map<String, Value>, String> {
    let parsed: Value =
        serde_json::from_slice(stdout).map_err(|_| snippet_error(ROLES_KEY, stdout))?;
    match parsed.get("value") {
        Some(Value::Object(map)) => Ok(map.clone()),
        Some(Value::Null) | None => Ok(Map::new()),
        Some(_) => Err(snippet_error(ROLES_KEY, stdout)),
    }
}

/// `omp models --json` prints `{"models":[{provider, kind, id, selector, name,
/// thinking, contextWindow, cost, …}]}`. Only chat models, and only the fields
/// the UI uses.
fn parse_catalog(stdout: &[u8]) -> Result<Value, String> {
    let parsed: Value =
        serde_json::from_slice(stdout).map_err(|_| snippet_error("model list", stdout))?;
    let models = parsed
        .get("models")
        .and_then(Value::as_array)
        .ok_or_else(|| snippet_error("model list", stdout))?;
    let rows: Vec<Value> = models
        .iter()
        .filter(|m| m.get("kind").and_then(Value::as_str).unwrap_or("chat") == "chat")
        .filter_map(|m| {
            let provider = m.get("provider")?.as_str()?;
            let id = m.get("id")?.as_str()?;
            Some(json!({
                "provider": provider,
                "id": id,
                "name": m.get("name").and_then(Value::as_str).unwrap_or(id),
                "selector": m.get("selector").and_then(Value::as_str)
                    .map_or_else(|| format!("{provider}/{id}"), str::to_owned),
                "thinking": m.get("thinking").cloned().unwrap_or_else(|| json!([])),
                "contextWindow": m.get("contextWindow").cloned().unwrap_or(Value::Null),
            }))
        })
        .collect();
    Ok(Value::Array(rows))
}

/// `omp usage --json` prints `{"reports":[{provider, limits:[{label, window:
/// {id, durationMs, resetsAt}, amount:{usedFraction, remainingFraction, unit},
/// status}], metadata:{planType, …}}], "capacity":{…}, …}`.
fn parse_usage(stdout: &[u8]) -> Result<Value, String> {
    let parsed: Value =
        serde_json::from_slice(stdout).map_err(|_| snippet_error("usage report", stdout))?;
    let reports = parsed
        .get("reports")
        .and_then(Value::as_array)
        .ok_or_else(|| snippet_error("usage report", stdout))?;
    let rows: Vec<Value> = reports
        .iter()
        .filter_map(|r| {
            let provider = r.get("provider")?.as_str()?;
            let limits: Vec<Value> = r
                .get("limits")
                .and_then(Value::as_array)
                .map(|ls| {
                    ls.iter()
                        .map(|l| {
                            json!({
                                "label": l.get("label").and_then(Value::as_str).unwrap_or(""),
                                "windowId": l.pointer("/window/id").cloned().unwrap_or(Value::Null),
                                "durationMs": l.pointer("/window/durationMs").cloned().unwrap_or(Value::Null),
                                "resetsAt": l.pointer("/window/resetsAt").cloned().unwrap_or(Value::Null),
                                "usedFraction": l.pointer("/amount/usedFraction").cloned().unwrap_or(Value::Null),
                                "remainingFraction": l.pointer("/amount/remainingFraction").cloned().unwrap_or(Value::Null),
                                "unit": l.pointer("/amount/unit").cloned().unwrap_or(Value::Null),
                                "status": l.get("status").cloned().unwrap_or(Value::Null),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            if limits.is_empty() {
                return None;
            }
            Some(json!({
                "provider": provider,
                "plan": r.pointer("/metadata/planType").cloned().unwrap_or(Value::Null),
                "limits": limits,
            }))
        })
        .collect();
    Ok(Value::Array(rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_are_read_from_the_value_field() {
        let out = br#"{"key":"modelRoles","value":{"default":"a/b:medium","smol":"c/d"},"type":"record"}"#;
        let roles = parse_roles(out).unwrap();
        assert_eq!(default_of(&roles).as_deref(), Some("a/b:medium"));
        assert_eq!(roles.len(), 2);
    }

    #[test]
    fn an_unset_record_reads_as_empty() {
        assert!(parse_roles(br#"{"key":"modelRoles","value":null}"#).unwrap().is_empty());
        assert!(parse_roles(br#"{"key":"modelRoles"}"#).unwrap().is_empty());
        assert_eq!(default_of(&Map::new()), None);
    }

    #[test]
    fn a_roles_value_that_is_not_an_object_is_an_error() {
        let err = parse_roles(br#"{"value":["x"]}"#).unwrap_err();
        assert!(err.starts_with("unexpected modelRoles from omp"), "{err}");
        assert!(parse_roles(b"not json").is_err());
    }

    #[test]
    fn set_args_pass_the_whole_object_as_one_element_and_keep_other_roles() {
        let mut roles = Map::new();
        roles.insert("smol".into(), json!("c/d"));
        roles.insert(DEFAULT_ROLE.into(), json!("a/b:high"));
        let args = set_args(Some("work"), &roles).unwrap();
        assert_eq!(&args[..3], ["--profile=work", "config", "set"]);
        assert_eq!(args[3], "modelRoles");
        let back: Value = serde_json::from_str(&args[4]).unwrap();
        assert_eq!(back["smol"], "c/d");
        assert_eq!(back["default"], "a/b:high");
        assert_eq!(args.len(), 5);
    }

    #[test]
    fn selectors_are_checked_before_they_reach_the_config_file() {
        assert!(valid_selector("anthropic/claude-sonnet-5-5"));
        assert!(valid_selector("openai/gpt-5:high"));
        for bad in ["", "no-slash", "/lead", "a/b c", "a/b\nc", "a/b\t"] {
            assert!(!valid_selector(bad), "{bad:?}");
        }
        assert!(default_set(None, "bad selector").is_err());
    }

    #[test]
    fn the_catalog_keeps_chat_models_and_the_fields_the_ui_uses() {
        let out = br#"{"models":[
            {"provider":"anthropic","kind":"chat","id":"m1","selector":"anthropic/m1","name":"M One","contextWindow":1000,"thinking":["low","high"],"cost":{"input":1}},
            {"provider":"openai","kind":"image","id":"img","name":"Img"},
            {"provider":"openai","id":"m2"}
        ]}"#;
        let rows = parse_catalog(out).unwrap();
        let rows = rows.as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["selector"], "anthropic/m1");
        assert_eq!(rows[0]["thinking"], json!(["low", "high"]));
        assert!(rows[0].get("cost").is_none());
        assert_eq!(rows[1]["name"], "m2");
        assert_eq!(rows[1]["selector"], "openai/m2");
        assert_eq!(rows[1]["thinking"], json!([]));
    }

    #[test]
    fn usage_keeps_the_limit_numbers_and_drops_the_account() {
        let out = br#"{"reports":[
            {"provider":"anthropic","limits":[{"id":"x","label":"5 hours","window":{"id":"5h","durationMs":18000000,"resetsAt":1793392284000},
              "amount":{"used":26,"limit":100,"remaining":74,"usedFraction":0.26,"remainingFraction":0.74,"unit":"percent"},"status":"ok"}],
             "metadata":{"planType":"max","email":"a@b.c","accountId":"secret"}},
            {"provider":"deepseek","limits":[]},
            {"limits":[]}
        ],"capacity":{}}"#;
        let rows = parse_usage(out).unwrap();
        let rows = rows.as_array().unwrap();
        assert_eq!(rows.len(), 1, "a provider without limits is not listed");
        assert_eq!(rows[0]["provider"], "anthropic");
        assert_eq!(rows[0]["plan"], "max");
        assert_eq!(rows[0]["limits"][0]["label"], "5 hours");
        assert_eq!(rows[0]["limits"][0]["remainingFraction"], 0.74);
        assert_eq!(rows[0]["limits"][0]["resetsAt"], 1793392284000_u64);
        let text = rows[0].to_string();
        assert!(!text.contains("a@b.c") && !text.contains("secret"), "{text}");
    }

    #[test]
    fn an_unreadable_usage_report_is_an_error() {
        let err = parse_usage(b"x").unwrap_err();
        assert!(err.starts_with("unexpected usage report from omp: x"), "{err}");
        assert!(parse_usage(br#"{"reports":5}"#).is_err());
    }

    #[test]
    fn an_unreadable_catalog_is_an_error_quoting_the_output() {
        let err = parse_catalog(b"nope").unwrap_err();
        assert!(err.starts_with("unexpected model list from omp: nope"), "{err}");
        assert!(parse_catalog(br#"{"models":3}"#).is_err());
    }
}
