//! Plugin packages (`.npix`): inspect and install through omp's own CLI (`omp plugin inspect|install
//! <file> --json`), so the package checks (entries under `package/`, no symlinks, a valid `omp`
//! manifest whose entries exist, a `developer.json`) live only in omp (`extensibility/plugins/npix.ts`).
//! Installed plugins are listed (`omp plugin list --json`, each with its source and developer) and
//! uninstalled (`omp plugin uninstall <name> --json`) the same way.
//!
//! Settings → Plugins picks a file, shows what `inspect` reports (name, version, declared
//! extensions/tools/hooks/commands) and installs only after the user confirms. omp loads an
//! installed plugin in every tab started afterwards under the same profile.
//!
//! `profile` is `None` for the built-in profile or `Some(id)` for a named one; callers MUST only
//! pass ids that went through `ProfileStore::resolve_owned`.
use std::path::Path;

use serde_json::Value;

const OMP_PLUGIN: &str = "omp plugin";
const EXTENSION: &str = "npix";

/// What to do with the package.
#[derive(Clone, Copy)]
pub enum Action {
    Inspect,
    Install,
}

/// Run `action` on the package at `path` and return omp's JSON reply.
///
/// Spawns `omp`; callers MUST run this off the main thread.
pub fn run(profile: Option<&str>, path: &str, action: Action) -> Result<Value, String> {
    let file = checked_file(path)?;
    let mut args: Vec<String> = crate::agent::spawn::profile_flag(profile)
        .into_iter()
        .collect();
    args.extend([
        "plugin".to_string(),
        match action {
            Action::Inspect => "inspect",
            Action::Install => "install",
        }
        .to_string(),
        file,
        "--json".to_string(),
    ]);
    let stdout = crate::omp_cli::run(&args, OMP_PLUGIN)?;
    serde_json::from_slice(&stdout)
        .map_err(|e| format!("{OMP_PLUGIN} printed something that is not JSON: {e}"))
}

/// The installed plugins: omp's `{ npm: [...], marketplace: [...] }`, each npm entry with `npix` (installed
/// from a package) and `developer` when its folder carries a `developer.json`.
///
/// Spawns `omp`; callers MUST run this off the main thread.
pub fn list(profile: Option<&str>) -> Result<Value, String> {
    let mut args: Vec<String> = crate::agent::spawn::profile_flag(profile)
        .into_iter()
        .collect();
    args.extend(["plugin", "list", "--json"].map(String::from));
    let stdout = crate::omp_cli::run(&args, OMP_PLUGIN)?;
    serde_json::from_slice(&stdout)
        .map_err(|e| format!("{OMP_PLUGIN} printed something that is not JSON: {e}"))
}

/// Uninstall plugin `name` (an npm package name); omp also removes an unpacked `.npix` copy.
///
/// Spawns `omp`; callers MUST run this off the main thread.
pub fn uninstall(profile: Option<&str>, name: &str) -> Result<(), String> {
    let mut args: Vec<String> = crate::agent::spawn::profile_flag(profile)
        .into_iter()
        .collect();
    args.extend([
        "plugin".to_string(),
        "uninstall".to_string(),
        checked_name(name)?,
        "--json".to_string(),
    ]);
    crate::omp_cli::run(&args, OMP_PLUGIN).map(|_| ())
}

/// An npm package name (`plugin` or `@scope/plugin`): what omp accepts, never flag-shaped.
fn checked_name(name: &str) -> Result<String, String> {
    let ok_part = |part: &str| {
        part.chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '~')
            && part.chars().all(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '.' | '_' | '~')
            })
    };
    let ok = name.strip_prefix('@').map_or_else(
        || ok_part(name),
        |scoped| {
            scoped
                .split_once('/')
                .is_some_and(|(scope, pkg)| ok_part(scope) && ok_part(pkg))
        },
    );
    if ok && name.len() <= 214 {
        Ok(name.to_string())
    } else {
        Err(format!("not a plugin name: {name:?}"))
    }
}

/// An absolute path to an existing `.npix` file, which can only ever be one argv value.
fn checked_file(path: &str) -> Result<String, String> {
    let file = Path::new(path);
    let is_npix = file
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(EXTENSION));
    if !file.is_absolute() || !is_npix {
        return Err(format!(
            "not an absolute path to a .{EXTENSION} file: {path:?}"
        ));
    }
    if !file.is_file() {
        return Err(format!("no such file: {path}"));
    }
    Ok(path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_existing_absolute_npix_files_reach_omp() {
        let dir = std::env::temp_dir().join(format!("npix-check-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let good = dir.join("sample.npix");
        std::fs::write(&good, b"").unwrap();
        assert!(checked_file(good.to_str().unwrap()).is_ok());
        assert!(checked_file("sample.npix").is_err(), "relative");
        assert!(checked_file("--force.npix").is_err(), "flag-shaped");
        assert!(
            checked_file(dir.join("sample.tgz").to_str().unwrap()).is_err(),
            "other extension"
        );
        assert!(
            checked_file(dir.join("missing.npix").to_str().unwrap()).is_err(),
            "missing"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn plugin_names_are_package_names_and_never_flags() {
        for ok in ["sample", "@npi/plugin-sample", "a.b_c~d"] {
            assert!(checked_name(ok).is_ok(), "{ok}");
        }
        for bad in [
            "", "-x", "--force", "Sample", "@npi", "@npi/", "a/b", "../x", "a b",
        ] {
            assert!(checked_name(bad).is_err(), "{bad}");
        }
    }
}
