//! The managed Python environment `NPi`'s `python` tool runs in: the
//! versioned virtualenv built from the installed runtime, and package
//! management through that runtime's `uv`.
//!
//! The environment is never rebuilt over a working one. Each runtime identity
//! (version plus archive SHA prefix) gets its own directory under
//! `environments/default/versions/`, and `environments/default/current` — an
//! atomically rewritten pointer — names the one omp should use. An upgrade
//! builds the new version next to the old one, verifies it at its final
//! location, then flips the pointer; the previous version keeps working for
//! whatever is still running in it and is never deleted.
//!
//! The venv is created offline by the base runtime's own
//! `python -I -B -m venv --without-pip`, then `-B -E -s -m ensurepip` seeds
//! pip from the runtime's bundled wheel — no network, no second installer, and
//! no `__pycache__` written into the immutable runtime. Building is serialised
//! across processes by the same OS file lock the runtime installer holds
//! ([`super::lock::FileLock`]), so a second `ensure` waits and reuses the
//! first one's result.
//!
//! Every child goes through [`super::command::run_bounded`]: bounded output,
//! a deadline, and its own process tree.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};

use super::command::{failure_text, run_bounded, LIST_TIMEOUT, PROBE_TIMEOUT, VENV_TIMEOUT};
use super::lock::FileLock;
use super::paths::EnvPaths;
use super::report_progress;

/// One installed distribution, as `uv pip list --format json` reports it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
}

/// What the environment reports about itself: the interpreter and `uv` it
/// runs, their versions, and what is wrong when `ok` is false.
///
/// `pythonPath` is the absolute interpreter omp must use; it points into the
/// published version directory, never into the base runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentHealth {
    pub ok: bool,
    pub python_version: String,
    pub uv_version: String,
    pub python_path: String,
    pub env_path: String,
    pub runtime_path: String,
    pub message: Option<String>,
}

/// Variables a managed Python child keeps: the ones an interpreter, its
/// temporary files and its locale need. Everything else the app holds —
/// model API keys, editor and forge tokens — stays out of the environment.
const INHERITED_ENV: &[&str] = &[
    "HOME",
    "TMPDIR",
    "TMP",
    "TEMP",
    "LANG",
    "LANGUAGE",
    "LC_ALL",
    "LC_CTYPE",
    "LC_MESSAGES",
    "TZ",
    "TERM",
    "COLORTERM",
    "USER",
    "LOGNAME",
    "SHELL",
    // Windows: the interpreter, its DLL lookup and `%TEMP%`.
    "USERPROFILE",
    "SYSTEMROOT",
    "SYSTEMDRIVE",
    "WINDIR",
    "COMSPEC",
    "PATHEXT",
    "APPDATA",
    "LOCALAPPDATA",
    "PROGRAMDATA",
    "PROGRAMFILES",
    "NUMBER_OF_PROCESSORS",
    "PROCESSOR_ARCHITECTURE",
    "OS",
];

/// One environment build at a time in this process: the cross-process
/// [`FileLock`] would make a second thread wait, but holding this first keeps
/// the wait off the file lock.
static ENV_BUILD: Mutex<()> = Mutex::new(());

/// The environment's interpreter: `<env>/bin/python` (`Scripts\python.exe`).
#[must_use]
pub fn env_python_bin(env_dir: &Path) -> PathBuf {
    if cfg!(windows) {
        env_dir.join("Scripts").join("python.exe")
    } else {
        env_dir.join("bin").join("python")
    }
}

/// The runtime's interpreter: `<runtime>/bin/python3`.
#[must_use]
pub fn runtime_python_bin(runtime_dir: &Path) -> PathBuf {
    if cfg!(windows) {
        runtime_dir.join("python.exe")
    } else {
        runtime_dir.join("bin").join("python3")
    }
}

/// The runtime's `uv`: `<runtime>/bin/uv`.
#[must_use]
pub fn runtime_uv_bin(runtime_dir: &Path) -> PathBuf {
    if cfg!(windows) {
        runtime_dir.join("uv.exe")
    } else {
        runtime_dir.join("bin").join("uv")
    }
}

/// The runtime's programs folder, first on a managed child's PATH.
#[must_use]
fn runtime_bin_dir(runtime_dir: &Path) -> PathBuf {
    if cfg!(windows) {
        runtime_dir.to_path_buf()
    } else {
        runtime_dir.join("bin")
    }
}

/// The environment's programs folder, first on a managed child's PATH and
/// where a console script installed by `uv pip install` lands.
#[must_use]
fn env_bin_dir(env_dir: &Path) -> PathBuf {
    if cfg!(windows) {
        env_dir.join("Scripts")
    } else {
        env_dir.join("bin")
    }
}

/// The version directory name for a runtime identity:
/// `py<version>-<first 12 hex of the archive SHA-256>`.
///
/// # Errors
/// When the manifest's version or digest cannot form a single safe path
/// component (they are app-authored, so this is a broken manifest, not user
/// input).
pub fn env_version_id(runtime_version: &str, archive_sha256: &str) -> Result<String, String> {
    if runtime_version.trim().is_empty() {
        return Err("the bundled runtime manifest has no runtime version".to_string());
    }
    let digest = archive_sha256.trim();
    if digest.len() < 12 || !digest.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "the bundled runtime manifest has an unusable archive SHA-256 `{archive_sha256}`"
        ));
    }
    let id = format!(
        "py{}-{}",
        runtime_version.trim(),
        digest[..12].to_ascii_lowercase()
    );
    if !is_version_id(&id) {
        return Err(format!(
            "the bundled runtime manifest has an unusable runtime version `{runtime_version}`"
        ));
    }
    Ok(id)
}

/// Whether `id` is one path component this module could have written.
fn is_version_id(id: &str) -> bool {
    !id.is_empty()
        && id != "."
        && id != ".."
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+'))
}

/// The published environment directory, from the `current` pointer: `None`
/// when there is no pointer or it does not name a safe version directory.
#[must_use]
pub fn published_env_dir(paths: &EnvPaths) -> Option<PathBuf> {
    let raw = fs::read_to_string(paths.default_env_current_file()).ok()?;
    let id = raw.trim();
    if !is_version_id(id) {
        return None;
    }
    Some(paths.env_version_dir(id))
}

/// Create `path` (and its parents) private to this user.
fn create_private_dir(path: &Path) -> Result<(), String> {
    crate::private_fs::private_dirs(true)
        .create(path)
        .map_err(|e| format!("could not create {}: {e}", path.display()))
}

/// Remove an environment directory this module owns and has just found
/// unusable, with retries for a file another thread's child still holds open.
fn remove_env_dir(env_dir: &Path) -> Result<(), String> {
    if !env_dir.exists() {
        return Ok(());
    }
    crate::private_fs::remove_dir_retrying(env_dir).map_err(|e| {
        format!(
            "could not remove the unusable environment at {}: {e}",
            env_dir.display()
        )
    })
}

/// The isolated environment for one managed Python child, applied to `cmd`
/// before the caller's own variables: the inherited allowlist
/// ([`INHERITED_ENV`]), then PATH — the environment's `bin`, the runtime's,
/// then the trusted system PATH, never a relative entry.
///
/// `UV_CACHE_DIR` keeps downloads in `NPi`'s own cache. Callers pass `-B` to
/// every interpreter they start: `-I` ignores `PYTHONDONTWRITEBYTECODE`, so
/// that flag is what keeps the immutable runtime free of `__pycache__`.
///
/// # Errors
/// When PATH cannot be joined (an entry holding the platform separator).
fn prepare_child_env(
    cmd: &mut Command,
    paths: &EnvPaths,
    env_dir: &Path,
    runtime_dir: &Path,
) -> Result<(), String> {
    cmd.env_clear();
    for name in INHERITED_ENV {
        if let Some(value) = std::env::var_os(name) {
            cmd.env(name, value);
        }
    }
    let mut dirs = vec![env_bin_dir(env_dir), runtime_bin_dir(runtime_dir)];
    if let Some(system) = crate::child_path::trusted_child_path() {
        dirs.extend(std::env::split_paths(&system));
    }
    let path = std::env::join_paths(dirs).map_err(|e| format!("could not build PATH: {e}"))?;
    cmd.env("PATH", path)
        .env("UV_CACHE_DIR", paths.cache_uv_dir())
        .env("PYTHONNOUSERSITE", "1")
        .env("PYTHONDONTWRITEBYTECODE", "1");
    Ok(())
}

/// `<bin> <flags>`'s version sentence, reduced to the version:
/// `Python 3.12.16` and `uv 0.13.6 (abc 2026-10-26)` both answer what the UI
/// shows.
fn reported_version(bin: &Path, flags: &[&str], what: &str) -> Result<String, String> {
    let mut cmd = Command::new(bin);
    cmd.args(flags);
    let run = run_bounded(&mut cmd, PROBE_TIMEOUT, what)?;
    if !run.success {
        return Err(format!("{what} failed ({})", failure_text(&run)));
    }
    let text = String::from_utf8_lossy(&run.stdout);
    let Some(version) = text.split_whitespace().nth(1) else {
        return Err(format!("{what} printed no version"));
    };
    Ok(version.trim_end_matches(',').to_string())
}

/// The interpreter probe of a published environment: in one start it proves
/// the stdlib and SSL it links resolve from *this* directory (`sys.prefix`),
/// and that the offline bootstrap left pip importable.
const ENV_PROBE: &str = "import json, ssl, sys\n\
import pip\n\
print(json.dumps({\"prefix\": sys.prefix, \"version\": \"%d.%d.%d\" % sys.version_info[:3], \
\"pip\": pip.__version__, \"ssl\": ssl.OPENSSL_VERSION}))";

/// Whether the environment at `env_dir` is complete and usable in place:
/// its interpreter starts, resolves its prefix to that directory (not the
/// base runtime), has SSL, and has pip.
fn verify_env(paths: &EnvPaths, runtime_dir: &Path, env_dir: &Path) -> Result<String, String> {
    let python = env_python_bin(env_dir);
    if !python.is_file() {
        return Err(format!(
            "the managed Python environment at {} has no interpreter",
            env_dir.display()
        ));
    }
    let mut cmd = Command::new(&python);
    cmd.args(["-I", "-B", "-c", ENV_PROBE]).current_dir(env_dir);
    prepare_child_env(&mut cmd, paths, env_dir, runtime_dir)?;
    let run = run_bounded(&mut cmd, PROBE_TIMEOUT, "the environment's python")?;
    if !run.success {
        return Err(format!(
            "the environment at {} does not start ({}); rebuild it",
            env_dir.display(),
            failure_text(&run)
        ));
    }
    let probe: serde_json::Value = serde_json::from_slice(&run.stdout).map_err(|e| {
        format!(
            "the environment at {} answered an unreadable probe: {e}",
            env_dir.display()
        )
    })?;
    let prefix = probe
        .get("prefix")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let same_dir = fs::canonicalize(prefix).ok() == fs::canonicalize(env_dir).ok();
    if !same_dir {
        return Err(format!(
            "the environment at {} resolves its prefix to {prefix}, not to itself",
            env_dir.display()
        ));
    }
    let pip = probe
        .get("pip")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if pip.is_empty() {
        return Err(format!(
            "the environment at {} has no pip (the runtime's ensurepip bootstrap did not run)",
            env_dir.display()
        ));
    }
    let version = probe
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let ssl = probe
        .get("ssl")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if ssl.is_empty() {
        return Err(format!(
            "the environment at {} has no usable SSL module",
            env_dir.display()
        ));
    }
    Ok(version)
}

/// Publish `env_dir` as the environment omp uses: the `current` pointer is
/// rewritten atomically, so a reader sees either the old version or the new
/// one, never a half-written name.
fn publish_env(paths: &EnvPaths, env_dir: &Path) -> Result<(), String> {
    let id = env_dir
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|id| is_version_id(id))
        .ok_or_else(|| {
            format!(
                "cannot publish {}: its name is not a version id",
                env_dir.display()
            )
        })?;
    let pointer = paths.default_env_current_file();
    crate::json_store::write_atomic(&pointer, format!("{id}\n").as_bytes())
        .map_err(|e| format!("could not write {}: {e}", pointer.display()))
}

/// The managed environment directory for `runtime_version` /
/// `archive_sha256`, created from `runtime_dir` if that identity's version
/// directory is missing or unusable and published through the `current`
/// pointer.
///
/// Only `environments/default/versions/<id>` is written — the runtime
/// directory is read, never modified, and other environment versions are
/// never touched.
///
/// # Errors
/// When the runtime is not installed, the version id is unusable, the venv
/// cannot be created, or it fails the post-build verification.
pub fn ensure_env(
    paths: &EnvPaths,
    runtime_dir: &Path,
    runtime_version: &str,
    archive_sha256: &str,
) -> Result<PathBuf, String> {
    ensure_env_bounded(
        paths,
        runtime_dir,
        runtime_version,
        archive_sha256,
        VENV_TIMEOUT,
    )
}

/// [`ensure_env`] with the venv deadline spelled out (the tests use a short
/// one).
fn ensure_env_bounded(
    paths: &EnvPaths,
    runtime_dir: &Path,
    runtime_version: &str,
    archive_sha256: &str,
    venv_timeout: std::time::Duration,
) -> Result<PathBuf, String> {
    let id = env_version_id(runtime_version, archive_sha256)?;
    let env_dir = paths.env_version_dir(&id);
    if published_env_dir(paths).as_deref() == Some(env_dir.as_path())
        && verify_env(paths, runtime_dir, &env_dir).is_ok()
    {
        return Ok(env_dir);
    }

    let _build = ENV_BUILD.lock().unwrap_or_else(PoisonError::into_inner);
    let _file = FileLock::acquire(&paths.lock_file())?;
    // Another process may have published this very version while we waited,
    // or a crash left a complete one unpublished: publish after verifying
    // rather than building again.
    if verify_env(paths, runtime_dir, &env_dir).is_ok() {
        publish_env(paths, &env_dir)?;
        return Ok(env_dir);
    }

    let runtime_python = runtime_python_bin(runtime_dir);
    if !runtime_python.is_file() {
        return Err(format!(
            "the Python runtime is not installed at {}",
            runtime_dir.display()
        ));
    }
    // An unpublished version directory that failed verification (a crash
    // mid-build, a half-written venv) is ours and unusable: rebuild it.
    remove_env_dir(&env_dir)?;
    create_private_dir(&paths.cache_uv_dir())?;
    let parent = paths.default_env_versions_dir();
    create_private_dir(&parent)?;

    report_progress(
        "environment-build",
        &format!("creating the Python environment at {}", env_dir.display()),
    );
    // `--without-pip` keeps the venv module's own bootstrap out of it: that
    // one starts the runtime interpreter without `-B` and would write
    // `__pycache__` into the immutable runtime. pip is then bootstrapped as
    // its own offline step from the runtime's bundled ensurepip wheel.
    let mut cmd = Command::new(&runtime_python);
    cmd.args(["-I", "-B", "-m", "venv", "--without-pip"])
        .arg(&env_dir)
        .current_dir(&parent);
    prepare_child_env(&mut cmd, paths, &env_dir, runtime_dir)?;
    let run = match run_bounded(&mut cmd, venv_timeout, "creating the Python environment") {
        Ok(run) => run,
        Err(error) => {
            let _ = remove_env_dir(&env_dir);
            return Err(error);
        }
    };
    if !run.success {
        let _ = remove_env_dir(&env_dir);
        return Err(format!(
            "could not create the Python environment at {}: `python -m venv` failed ({})",
            env_dir.display(),
            failure_text(&run)
        ));
    }

    // No `-I` here: ensurepip forwards `-I` to the pip subprocess it starts
    // (`sys.flags.isolated`), and that child would then ignore the
    // `PYTHONDONTWRITEBYTECODE=1` this environment sets — writing
    // `__pycache__` into the immutable runtime. `-E -s -B` isolate the
    // bootstrap itself without propagating an isolated flag.
    let mut pip = Command::new(env_python_bin(&env_dir));
    pip.args([
        "-B",
        "-E",
        "-s",
        "-m",
        "ensurepip",
        "--upgrade",
        "--default-pip",
    ])
    .current_dir(&env_dir);
    prepare_child_env(&mut pip, paths, &env_dir, runtime_dir)?;
    let run = match run_bounded(&mut pip, venv_timeout, "bootstrapping pip") {
        Ok(run) => run,
        Err(error) => {
            let _ = remove_env_dir(&env_dir);
            return Err(error);
        }
    };
    if !run.success {
        let _ = remove_env_dir(&env_dir);
        return Err(format!(
            "could not bootstrap pip offline into {} from the runtime's ensurepip ({})",
            env_dir.display(),
            failure_text(&run)
        ));
    }

    // Verify at the final path, before the pointer can publish it: a venv is
    // only a set of paths into the runtime, and pip's own scripts embed the
    // directory they were built in.
    let verified = verify_env(paths, runtime_dir, &env_dir);
    if let Err(error) = verified {
        let _ = remove_env_dir(&env_dir);
        return Err(error);
    }
    publish_env(paths, &env_dir)?;
    report_progress(
        "environment-build",
        &format!("published the Python environment at {}", env_dir.display()),
    );
    Ok(env_dir)
}

/// The environment's state as the settings pane and omp see it: the
/// interpreter and `uv` versions, the paths they were read from, and what is
/// wrong when `ok` is false.
#[must_use]
pub fn check_health(paths: &EnvPaths, runtime_dir: &Path) -> EnvironmentHealth {
    let published = published_env_dir(paths);
    let env_python = published.as_deref().map(env_python_bin);
    let uv = runtime_uv_bin(runtime_dir);
    let mut problems: Vec<String> = Vec::new();
    let python_version = match &env_python {
        Some(python) if python.is_file() => reported_version(
            python,
            &["-I", "-B", "--version"],
            "the environment's python",
        )
        .unwrap_or_else(|e| {
            problems.push(e);
            String::new()
        }),
        _ => {
            problems.push(format!(
                "the managed environment is not created under {}",
                paths.default_env_dir().display()
            ));
            String::new()
        }
    };
    let uv_version = if uv.is_file() {
        reported_version(&uv, &["--version"], "uv").unwrap_or_else(|e| {
            problems.push(e);
            String::new()
        })
    } else {
        problems.push(format!(
            "the Python runtime is not installed at {}",
            runtime_dir.display()
        ));
        String::new()
    };
    EnvironmentHealth {
        ok: problems.is_empty(),
        python_version,
        uv_version,
        python_path: env_python
            .filter(|python| python.is_file())
            .map_or_else(String::new, |python| python.display().to_string()),
        env_path: published.map_or_else(String::new, |dir| dir.display().to_string()),
        runtime_path: runtime_dir.display().to_string(),
        message: if problems.is_empty() {
            None
        } else {
            Some(problems.join("; "))
        },
    }
}

/// Reject what `uv` would read as an option, or that cannot be an argv entry
/// at all: an argument starting with `-` would let a caller smuggle
/// `--target`/`-r <file>` past the package list, and a control byte cannot be
/// in an argv entry.
///
/// # Errors
/// An empty list or specifier, an option-like specifier, or a control
/// character.
fn validate_packages(packages: &[String]) -> Result<(), String> {
    if packages.is_empty() {
        return Err("no packages to install".to_string());
    }
    for raw in packages {
        if raw.chars().any(char::is_control) {
            return Err(format!(
                "package specifier `{}` contains a control character",
                raw.escape_default()
            ));
        }
        let spec = raw.trim();
        if spec.is_empty() {
            return Err("package specifier is empty".to_string());
        }
        if spec.starts_with('-') {
            return Err(format!("package specifier `{spec}` looks like an option"));
        }
    }
    Ok(())
}

/// The runtime's `uv`, or the error the UI shows when it is missing.
fn require_uv(runtime_dir: &Path) -> Result<PathBuf, String> {
    let uv = runtime_uv_bin(runtime_dir);
    if !uv.is_file() {
        return Err(format!(
            "the Python runtime is not installed at {}",
            runtime_dir.display()
        ));
    }
    Ok(uv)
}

/// The distributions installed in the managed environment, by name.
///
/// # Errors
/// When `uv` is missing, does not answer within [`LIST_TIMEOUT`], or prints
/// something other than the JSON list `--format json` promises.
pub fn list_packages(
    paths: &EnvPaths,
    runtime_dir: &Path,
    env_dir: &Path,
) -> Result<Vec<PackageInfo>, String> {
    list_packages_bounded(paths, runtime_dir, env_dir, LIST_TIMEOUT)
}

/// [`list_packages`] with the deadline spelled out (the tests use a short
/// one).
fn list_packages_bounded(
    paths: &EnvPaths,
    runtime_dir: &Path,
    env_dir: &Path,
    limit: std::time::Duration,
) -> Result<Vec<PackageInfo>, String> {
    let uv = require_uv(runtime_dir)?;
    let env_python = env_python_bin(env_dir);
    let mut cmd = Command::new(&uv);
    cmd.args(["pip", "list", "--python"])
        .arg(&env_python)
        .args(["--format", "json"])
        .current_dir(env_dir);
    prepare_child_env(&mut cmd, paths, env_dir, runtime_dir)?;
    let run = run_bounded(&mut cmd, limit, "listing the installed packages")?;
    if !run.success {
        return Err(format!("`uv pip list` failed ({})", failure_text(&run)));
    }
    let mut packages: Vec<PackageInfo> = match serde_json::from_slice(&run.stdout) {
        Ok(packages) => packages,
        Err(e) => {
            let raw = String::from_utf8_lossy(&run.stdout);
            let preview = crate::widgets::redact::redact_secrets(raw.trim());
            return Err(format!(
                "could not read `uv pip list` output: {e}: {preview}"
            ));
        }
    };
    packages.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(packages)
}

/// Install `packages` into the managed environment by requirement string
/// (`requests`, `numpy==2.1.0`, `black>=24`), never into the runtime, and
/// return the environment's package list afterwards.
///
/// # Errors
/// When a specifier is rejected, `uv` is missing, the install does not finish
/// within [`INSTALL_TIMEOUT`], or it fails.
pub fn install_packages(
    paths: &EnvPaths,
    runtime_dir: &Path,
    env_dir: &Path,
    packages: &[String],
) -> Result<Vec<PackageInfo>, String> {
    validate_packages(packages)?;
    let uv = require_uv(runtime_dir)?;
    let env_python = env_python_bin(env_dir);
    let mut cmd = Command::new(&uv);
    cmd.args(["pip", "install", "--python"])
        .arg(&env_python)
        .arg("--cache-dir")
        .arg(paths.cache_uv_dir());
    for spec in packages {
        cmd.arg(spec.trim());
    }
    cmd.current_dir(env_dir);
    prepare_child_env(&mut cmd, paths, env_dir, runtime_dir)?;
    report_progress(
        "packages-install",
        &format!("installing {}", packages.join(", ")),
    );
    let run = run_bounded(
        &mut cmd,
        super::command::INSTALL_TIMEOUT,
        "installing the packages",
    )?;
    if !run.success {
        return Err(format!("`uv pip install` failed ({})", failure_text(&run)));
    }
    list_packages(paths, runtime_dir, env_dir)
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::fs;

    use super::*;
    #[cfg(unix)]
    use crate::test_support::executable;
    use crate::test_support::TempDir;

    /// A runtime whose `python3` and `uv` are stand-in shell scripts.
    #[cfg(unix)]
    fn fake_runtime(runtime: &Path, python_body: &str, uv_body: &str) {
        executable(&runtime_python_bin(runtime), python_body);
        executable(&runtime_uv_bin(runtime), uv_body);
    }

    /// A runtime `python3` that builds a fake venv at the directory named by
    /// `-m venv`: the created interpreter answers the probe with its own
    /// directory as the prefix and a pip version, and the log records each
    /// invocation of the runtime interpreter.
    #[cfg(unix)]
    fn fake_venv_runtime(log: &Path) -> String {
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{log}'\n\
             if [ \"$4\" = \"venv\" ]; then\n\
               mkdir -p \"$6/bin\"\n\
               cat > \"$6/bin/python\" <<EOF\n\
#!/bin/sh\n\
case \"\\$*\" in\n\
  *--version*) echo 'Python 3.12.16' ;;\n\
  *) echo '{{\"prefix\": \"$6\", \"version\": \"3.12.16\", \"pip\": \"26.2.2\", \"ssl\": \"OpenSSL 3.5.9\"}}' ;;\n\
esac\n\
EOF\n\
               chmod 755 \"$6/bin/python\"\n\
             fi\n\
             exit 0\n",
            log = log.display(),
        )
    }

    fn test_paths(dir: &Path) -> EnvPaths {
        EnvPaths::resolve(Some(dir))
    }

    #[cfg(unix)]
    #[test]
    fn bins_follow_the_layout() {
        let env_dir = Path::new("/tmp/env/default");
        let runtime = Path::new("/tmp/runtime");
        assert_eq!(env_python_bin(env_dir), env_dir.join("bin").join("python"));
        assert_eq!(
            runtime_python_bin(runtime),
            runtime.join("bin").join("python3")
        );
        assert_eq!(runtime_uv_bin(runtime), runtime.join("bin").join("uv"));
    }

    #[test]
    fn version_ids_are_single_safe_components() {
        assert_eq!(
            env_version_id("3.12.16", "90793c76f4012dee").expect("id"),
            "py3.12.16-90793c76f401"
        );
        assert!(env_version_id("3.12.16", "not-a-digest").is_err());
        assert!(env_version_id("../../evil", "90793c76f4012dee").is_err());
        assert!(env_version_id("", "90793c76f4012dee").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn ensure_env_builds_a_versioned_environment_and_publishes_the_pointer() {
        let dir = TempDir::new("python-env", "build");
        let paths = test_paths(dir.path());
        let runtime = paths.runtime_dir("3.12.16", "test", "aaaaaaaaaaaaaaaa");
        let log = runtime.join("python-args");
        fake_runtime(&runtime, &fake_venv_runtime(&log), "#!/bin/sh\nexit 0\n");

        let env_dir = ensure_env(&paths, &runtime, "3.12.16", "aaaaaaaaaaaaaaaa").expect("ensure");
        let id = env_version_id("3.12.16", "aaaaaaaaaaaaaaaa").expect("id");
        assert_eq!(env_dir, paths.env_version_dir(&id));
        let python = env_python_bin(&env_dir);
        assert_eq!(published_env_dir(&paths), Some(env_dir.clone()));
        assert_eq!(
            fs::read_to_string(paths.default_env_current_file())
                .expect("pointer")
                .trim(),
            id
        );
        assert!(python.is_file(), "the environment's interpreter exists");
        let args = fs::read_to_string(&log).expect("the runtime ran");
        assert!(args.contains("-m venv"), "args: {args}");

        // A second ensure takes the published, verified environment: the
        // runtime interpreter is not started again.
        fs::remove_file(&log).expect("clear the log");
        assert_eq!(
            ensure_env(&paths, &runtime, "3.12.16", "aaaaaaaaaaaaaaaa").expect("reuse"),
            env_dir
        );
        assert!(!log.exists(), "the venv was rebuilt");
    }

    #[cfg(unix)]
    #[test]
    fn an_environment_without_pip_is_rejected() {
        let dir = TempDir::new("python-env", "no-pip");
        let paths = test_paths(dir.path());
        let runtime = paths.runtime_dir("3.12.16", "test", "aaaaaaaaaaaaaaaa");
        let body = "#!/bin/sh\n\
             if [ \"$4\" = \"venv\" ]; then\n\
               mkdir -p \"$6/bin\"\n\
               cat > \"$6/bin/python\" <<EOF\n\
#!/bin/sh\n\
case \"\\$*\" in\n\
  *--version*) echo 'Python 3.12.16' ;;\n\
  *) echo '{\"prefix\": \"$6\", \"version\": \"3.12.16\", \"ssl\": \"OpenSSL 3.5.9\"}' ;;\n\
esac\n\
EOF\n\
               chmod 755 \"$6/bin/python\"\n\
             fi\n\
             exit 0\n";
        fake_runtime(&runtime, body, "#!/bin/sh\nexit 0\n");

        let error = ensure_env(&paths, &runtime, "3.12.16", "aaaaaaaaaaaaaaaa")
            .expect_err("an environment without pip");
        assert!(error.contains("no pip"), "error: {error}");
        let id = env_version_id("3.12.16", "aaaaaaaaaaaaaaaa").expect("id");
        assert!(
            !paths.env_version_dir(&id).exists(),
            "the rejected environment was left behind"
        );
        assert_eq!(published_env_dir(&paths), None);
    }

    #[cfg(unix)]
    #[test]
    fn ensure_env_upgrades_without_deleting_the_previous_version() {
        let dir = TempDir::new("python-env", "upgrade");
        let paths = test_paths(dir.path());
        let first_runtime = paths.runtime_dir("3.12.16", "test", "aaaaaaaaaaaaaaaa");
        let second_runtime = paths.runtime_dir("3.12.17", "test", "bbbbbbbbbbbbbbbb");
        for runtime in [&first_runtime, &second_runtime] {
            let log = runtime.join("python-args");
            fake_runtime(runtime, &fake_venv_runtime(&log), "#!/bin/sh\nexit 0\n");
        }

        let old = ensure_env(&paths, &first_runtime, "3.12.16", "aaaaaaaaaaaaaaaa")
            .expect("first environment");
        let new = ensure_env(&paths, &second_runtime, "3.12.17", "bbbbbbbbbbbbbbbb")
            .expect("second environment");
        assert_ne!(old, new);
        assert!(
            env_python_bin(&old).is_file(),
            "the previous environment was deleted"
        );
        assert!(env_python_bin(&new).is_file());
        assert_eq!(
            published_env_dir(&paths),
            Some(
                paths.env_version_dir(&env_version_id("3.12.17", "bbbbbbbbbbbbbbbb").expect("id"))
            )
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_damaged_published_environment_is_rebuilt() {
        let dir = TempDir::new("python-env", "damaged");
        let paths = test_paths(dir.path());
        let runtime = paths.runtime_dir("3.12.16", "test", "aaaaaaaaaaaaaaaa");
        let log = runtime.join("python-args");
        fake_runtime(&runtime, &fake_venv_runtime(&log), "#!/bin/sh\nexit 0\n");

        let env_dir = ensure_env(&paths, &runtime, "3.12.16", "aaaaaaaaaaaaaaaa").expect("ensure");
        let python = env_python_bin(&env_dir);
        fs::remove_file(&python).expect("damage the environment");
        let rebuilt = ensure_env(&paths, &runtime, "3.12.16", "aaaaaaaaaaaaaaaa").expect("rebuild");
        assert_eq!(rebuilt, env_dir);
        assert!(python.is_file());
        assert_eq!(
            fs::read_to_string(&log)
                .expect("the runtime ran")
                .matches("-m venv")
                .count(),
            2,
            "the damaged environment was not rebuilt"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_hung_environment_build_is_bounded() {
        let dir = TempDir::new("python-env", "bounded");
        let paths = test_paths(dir.path());
        let runtime = paths.runtime_dir("3.12.16", "test", "aaaaaaaaaaaaaaaa");
        fake_runtime(
            &runtime,
            "#!/bin/sh\nexec sleep 30\n",
            "#!/bin/sh\nexit 0\n",
        );

        let error = ensure_env_bounded(
            &paths,
            &runtime,
            "3.12.16",
            "aaaaaaaaaaaaaaaa",
            std::time::Duration::from_millis(150),
        )
        .expect_err("a hung venv");
        assert!(error.contains("did not finish within"), "error: {error}");
        assert_eq!(published_env_dir(&paths), None);
    }

    #[test]
    fn ensure_env_requires_the_runtime() {
        let dir = TempDir::new("python-env", "no-runtime");
        let paths = test_paths(dir.path());
        let runtime = paths.runtime_dir("3.12.16", "test", "aaaaaaaaaaaaaaaa");

        let error =
            ensure_env(&paths, &runtime, "3.12.16", "aaaaaaaaaaaaaaaa").expect_err("no runtime");
        assert!(error.contains("not installed"), "error: {error}");
    }

    #[cfg(unix)]
    #[test]
    fn check_health_reports_the_published_environment() {
        let dir = TempDir::new("python-env", "health");
        let paths = test_paths(dir.path());
        let runtime = paths.runtime_dir("3.12.16", "test", "aaaaaaaaaaaaaaaa");
        let log = runtime.join("python-args");
        fake_runtime(
            &runtime,
            &fake_venv_runtime(&log),
            "#!/bin/sh\necho 'uv 0.13.6 (abc 2026-10-26)'\n",
        );
        let env_dir = ensure_env(&paths, &runtime, "3.12.16", "aaaaaaaaaaaaaaaa").expect("ensure");

        let health = check_health(&paths, &runtime);
        assert!(health.ok, "message: {:?}", health.message);
        assert_eq!(health.python_version, "3.12.16");
        assert_eq!(health.uv_version, "0.13.6");
        assert_eq!(
            health.python_path,
            env_python_bin(&env_dir).display().to_string()
        );
        assert_eq!(health.env_path, env_dir.display().to_string());
        assert_eq!(health.runtime_path, runtime.display().to_string());
        assert_eq!(health.message, None);
    }

    #[test]
    fn check_health_reports_a_missing_environment() {
        let dir = TempDir::new("python-env", "health-missing");
        let paths = test_paths(dir.path());
        let runtime = paths.runtime_dir("3.12.16", "test", "aaaaaaaaaaaaaaaa");

        let health = check_health(&paths, &runtime);
        assert!(!health.ok);
        assert_eq!(health.python_version, "");
        assert_eq!(health.uv_version, "");
        assert_eq!(health.python_path, "");
        assert_eq!(health.env_path, "");
        let message = health.message.expect("a message");
        assert!(message.contains("not created"), "message: {message}");
        assert!(message.contains("not installed"), "message: {message}");
    }

    #[test]
    fn install_rejects_option_like_specifiers() {
        assert!(validate_packages(&[]).is_err());
        assert!(validate_packages(&["   ".to_string()]).is_err());
        assert!(validate_packages(&["--target=/tmp/x".to_string()]).is_err());
        assert!(validate_packages(&["-r/tmp/requirements.txt".to_string()]).is_err());
        assert!(validate_packages(&["requests\n".to_string()]).is_err());
        assert!(validate_packages(&["requests".to_string()]).is_ok());
        assert!(validate_packages(&["numpy==2.1.0".to_string()]).is_ok());
    }

    /// A published-looking environment and runtime whose `uv` is `uv_body`.
    #[cfg(unix)]
    fn uv_fixture(dir: &Path, uv_body: &str) -> (EnvPaths, PathBuf, PathBuf, PathBuf) {
        let paths = test_paths(dir);
        let runtime = paths.runtime_dir("3.12.16", "test", "aaaaaaaaaaaaaaaa");
        let log = runtime.join("uv-args");
        fake_runtime(&runtime, "#!/bin/sh\nexit 0\n", uv_body);
        let env_dir =
            paths.env_version_dir(&env_version_id("3.12.16", "aaaaaaaaaaaaaaaa").expect("id"));
        executable(&env_python_bin(&env_dir), "#!/bin/sh\nexit 0\n");
        (paths, runtime, env_dir, log)
    }

    #[cfg(unix)]
    #[test]
    fn list_packages_parses_uv_json() {
        let dir = TempDir::new("python-env", "list");
        let uv_body = "#!/bin/sh\nprintf '%s\\n' \"$*\" > \"$(dirname \"$0\")/../uv-args\"\nprintf '[{\"name\":\"packaging\",\"version\":\"24.2\"},{\"name\":\"anyio\",\"version\":\"4.6.2\"}]\\n'\n";
        let (paths, runtime, env_dir, _) = uv_fixture(dir.path(), uv_body);

        let packages = list_packages(&paths, &runtime, &env_dir).expect("list");
        assert_eq!(
            packages,
            vec![
                PackageInfo {
                    name: "anyio".to_string(),
                    version: "4.6.2".to_string(),
                },
                PackageInfo {
                    name: "packaging".to_string(),
                    version: "24.2".to_string(),
                },
            ]
        );
        let args = fs::read_to_string(runtime.join("uv-args")).expect("uv ran");
        assert!(args.starts_with("pip list --python "), "args: {args}");
        assert!(args.contains("--format json"), "args: {args}");
        assert!(
            args.contains(&env_python_bin(&env_dir).display().to_string()),
            "args: {args}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_hung_uv_is_bounded() {
        let dir = TempDir::new("python-env", "uv-bounded");
        let (paths, runtime, env_dir, _) = uv_fixture(dir.path(), "#!/bin/sh\nexec sleep 30\n");
        let error = list_packages_bounded(
            &paths,
            &runtime,
            &env_dir,
            std::time::Duration::from_millis(150),
        )
        .expect_err("a hung uv");
        assert!(error.contains("did not finish within"), "error: {error}");
    }

    #[cfg(unix)]
    #[test]
    fn install_packages_installs_then_lists() {
        let dir = TempDir::new("python-env", "install");
        let uv_body = "#!/bin/sh\nlog=\"$(dirname \"$0\")/../uv-args\"\nprintf '%s\\n' \"$*\" >> \"$log\"\nif [ \"$2\" = \"list\" ]; then printf '[{\"name\":\"demo\",\"version\":\"1.0\"}]\\n'; fi\n";
        let (paths, runtime, env_dir, _) = uv_fixture(dir.path(), uv_body);

        let packages =
            install_packages(&paths, &runtime, &env_dir, &["demo".to_string()]).expect("install");
        assert_eq!(
            packages,
            vec![PackageInfo {
                name: "demo".to_string(),
                version: "1.0".to_string(),
            }]
        );
        let args = fs::read_to_string(runtime.join("uv-args")).expect("uv ran");
        let mut lines = args.lines();
        let install = lines.next().expect("an install line");
        assert!(
            install.starts_with("pip install --python "),
            "args: {install}"
        );
        assert!(install.contains("--cache-dir"), "args: {install}");
        assert!(install.ends_with(" demo"), "args: {install}");
        assert!(
            lines.next().expect("a list line").starts_with("pip list "),
            "the package list is read after installing"
        );
    }

    #[cfg(unix)]
    #[test]
    fn install_rejects_before_touching_the_environment() {
        let dir = TempDir::new("python-env", "install-reject");
        let (paths, runtime, env_dir, _) = uv_fixture(dir.path(), "#!/bin/sh\nexit 0\n");

        let error = install_packages(&paths, &runtime, &env_dir, &["--target=/tmp/x".to_string()])
            .expect_err("a rejected specifier");
        assert!(error.contains("option"), "error: {error}");
        assert!(!runtime.join("uv-args").exists(), "uv was started");
    }
}
