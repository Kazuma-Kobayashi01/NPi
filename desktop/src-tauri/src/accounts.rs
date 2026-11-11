//! Signed-in OAuth accounts, through omp's own CLI (`omp account`).
//!
//! Settings → Accounts lists every account grouped by provider, activates one
//! per provider (the only one omp then uses for it; the rest are idle), puts
//! a provider back to rotating, and names accounts. omp owns the credential
//! store, so every change is one `omp account … --json` call and the reply is
//! the re-read list: `[{provider, providerName, credentialId, name, label?,
//! email?, detail, state: "active"|"idle"|"rotating", inUse}]`.
//!
//! `profile` is `None` for the built-in profile or `Some(id)` for a named one;
//! callers MUST only pass ids that went through `ProfileStore::resolve_owned`.

use serde_json::Value;

const OMP_ACCOUNT: &str = "omp account";
/// omp's own limit on an account name (`MAX_ACCOUNT_LABEL_LENGTH`).
const MAX_NAME_CHARS: usize = 64;

/// What to do before the list is read back.
pub enum Action<'a> {
    List,
    Activate {
        provider: &'a str,
        credential_id: u64,
    },
    Rotate {
        provider: &'a str,
    },
    Rename {
        provider: &'a str,
        credential_id: u64,
        name: Option<&'a str>,
    },
}

/// Run `action` and return the account list omp reads back afterwards.
///
/// Spawns `omp`; callers MUST run this off the main thread.
pub fn run(profile: Option<&str>, action: &Action<'_>) -> Result<Value, String> {
    let mut args: Vec<String> = crate::agent::spawn::profile_flag(profile)
        .into_iter()
        .collect();
    args.extend(["account", "--json"].map(String::from));
    match action {
        Action::List => {}
        Action::Activate {
            provider,
            credential_id,
        } => {
            args.extend([
                "activate".to_string(),
                checked_provider(provider)?,
                credential_id.to_string(),
            ]);
        }
        Action::Rotate { provider } => {
            args.extend(["rotate".to_string(), checked_provider(provider)?]);
        }
        Action::Rename {
            provider,
            credential_id,
            name,
        } => {
            args.extend([
                "rename".to_string(),
                checked_provider(provider)?,
                credential_id.to_string(),
                checked_name(*name)?,
            ]);
        }
    }
    parse_accounts(&crate::omp_cli::run(&args, OMP_ACCOUNT)?)
}

/// A provider id as omp writes them (`openai-codex`, `google-gemini-cli`).
fn checked_provider(provider: &str) -> Result<String, String> {
    let ok = provider
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric())
        && provider.len() <= 64
        && provider
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if ok {
        Ok(provider.to_string())
    } else {
        Err(format!("not a provider id: {provider:?}"))
    }
}

/// The name argument: `-` clears the name. A name may not start with `-`, so
/// omp never reads it as a flag.
fn checked_name(name: Option<&str>) -> Result<String, String> {
    let name = name.map(str::trim).unwrap_or_default();
    if name.is_empty() {
        return Ok("-".to_string());
    }
    if name.starts_with('-') {
        return Err("an account name cannot start with \"-\"".to_string());
    }
    if name.chars().any(char::is_control) {
        return Err("an account name cannot contain control characters".to_string());
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(format!(
            "an account name holds at most {MAX_NAME_CHARS} characters"
        ));
    }
    Ok(name.to_string())
}

fn parse_accounts(stdout: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(stdout)
        .map_err(|e| format!("{OMP_ACCOUNT} printed something that is not JSON: {e}"))?;
    if value.is_array() {
        Ok(value)
    } else {
        Err(format!("{OMP_ACCOUNT} did not print a list"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_ids_are_checked() {
        assert_eq!(checked_provider("openai-codex").unwrap(), "openai-codex");
        assert!(checked_provider("").is_err());
        assert!(checked_provider("a b").is_err());
        assert!(checked_provider("--json").is_err());
    }

    #[test]
    fn names_clear_trim_and_refuse_flags() {
        assert_eq!(checked_name(None).unwrap(), "-");
        assert_eq!(checked_name(Some("  ")).unwrap(), "-");
        assert_eq!(
            checked_name(Some(" GPT-Plus-Kazuma ")).unwrap(),
            "GPT-Plus-Kazuma"
        );
        assert!(checked_name(Some("-x")).is_err());
        assert!(checked_name(Some("a\nb")).is_err());
        assert!(checked_name(Some(&"x".repeat(65))).is_err());
    }

    #[test]
    fn only_a_json_list_is_accepted() {
        assert!(parse_accounts(br#"[{"provider":"anthropic"}]"#)
            .unwrap()
            .is_array());
        assert!(parse_accounts(b"{}").is_err());
        assert!(parse_accounts(b"Usage: npi account").is_err());
    }
}
