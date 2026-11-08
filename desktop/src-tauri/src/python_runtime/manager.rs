//! The `python` tool's entry points.
//!
//! Shared by the Tauri commands and the `--npi-python-host` protocol: the
//! runtime's status without unpacking anything, the lazy deployment
//! (`ensure_ready`) that installs the runtime archive and builds the managed
//! environment, and package list/install.
//!
//! This module is the single place that combines the two halves — the
//! immutable runtime ([`super::installer`]) and the versioned environment
//! ([`super::environment`]) — so omp never needs a second installer of its
//! own.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::environment::{
    check_health, ensure_env, env_python_bin, install_packages, list_packages, published_env_dir,
    runtime_python_bin, runtime_uv_bin, EnvironmentHealth, PackageInfo,
};
use super::installer::ensure_runtime_installed;
use super::manifest::read_installed_state;
use super::paths::EnvPaths;
use super::wheels::load_wheelhouse;

/// What the settings pane shows before any work happens.
///
/// Whether the runtime is unpacked, where, and where its environment lives.
/// Read from the state manifest plus the `current` pointer and the two runtime
/// programs, never by unpacking or probing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    pub installed: bool,
    pub runtime_version: Option<String>,
    pub platform: Option<String>,
    pub runtime_path: Option<String>,
    /// The published environment's interpreter, when there is one.
    pub python_path: Option<String>,
    pub env_base_dir: String,
}

/// The embedded runtime and its managed environment.
///
/// Every method resolves [`EnvPaths`] from `override_dir` — `None` in the
/// app, which then honors `NPI_ENV_DIR` — so a test or a second install can
/// point elsewhere without touching the default.
pub struct PythonRuntimeManager;

impl PythonRuntimeManager {
    /// Whether the runtime is installed, without creating, verifying or
    /// unpacking anything: a state manifest that names a runtime directory
    /// whose `python3` and `uv` are both present, plus the published
    /// environment's interpreter when its `current` pointer names one.
    #[must_use]
    pub fn status(override_dir: Option<&Path>) -> RuntimeStatus {
        let paths = EnvPaths::resolve(override_dir);
        let env_base_dir = paths.base_dir().display().to_string();
        let python_path = published_env_dir(&paths)
            .map(|env_dir| env_python_bin(&env_dir))
            .filter(|python| python.is_file())
            .map(|python| python.display().to_string());
        let Some(state) = read_installed_state(&paths) else {
            return RuntimeStatus {
                installed: false,
                runtime_version: None,
                platform: None,
                runtime_path: None,
                python_path,
                env_base_dir,
            };
        };
        let runtime_path = PathBuf::from(&state.runtime_path);
        let installed =
            runtime_python_bin(&runtime_path).is_file() && runtime_uv_bin(&runtime_path).is_file();
        RuntimeStatus {
            installed,
            runtime_version: Some(state.runtime_version),
            platform: Some(state.platform),
            runtime_path: Some(state.runtime_path),
            python_path,
            env_base_dir,
        }
    }

    /// The environment's health without changing anything: the published
    /// interpreter and `uv` are probed, but nothing is installed or built.
    #[must_use]
    pub fn environment(override_dir: Option<&Path>) -> EnvironmentHealth {
        let paths = EnvPaths::resolve(override_dir);
        match read_installed_state(&paths) {
            Some(state) => check_health(&paths, Path::new(&state.runtime_path)),
            None => EnvironmentHealth {
                ok: false,
                python_version: String::new(),
                uv_version: String::new(),
                python_path: String::new(),
                env_path: String::new(),
                runtime_path: String::new(),
                message: Some(
                    "the Python runtime is not installed (run the ensure action)".to_string(),
                ),
            },
        }
    }

    /// Install the runtime if needed, build the managed environment for that
    /// runtime's identity, and report its health. Idempotent.
    ///
    /// # Errors
    /// When the runtime cannot be installed, the environment cannot be built
    /// or verified, or the environment fails its health check.
    pub fn ensure_ready(
        resource_dir: &Path,
        override_dir: Option<&Path>,
    ) -> Result<EnvironmentHealth, String> {
        let (paths, runtime_dir, _env_dir) = Self::prepared(resource_dir, override_dir)?;
        let health = check_health(&paths, &runtime_dir);
        if !health.ok {
            return Err(health.message.unwrap_or_else(|| {
                format!(
                    "the managed Python environment at {} is not usable",
                    health.env_path
                )
            }));
        }
        Ok(health)
    }

    /// The managed environment's distributions, after making sure the
    /// runtime and its environment exist.
    ///
    /// # Errors
    /// When the runtime or the environment cannot be prepared, or `uv pip
    /// list` fails.
    pub fn packages(
        resource_dir: &Path,
        override_dir: Option<&Path>,
    ) -> Result<Vec<PackageInfo>, String> {
        let (paths, runtime_dir, env_dir) = Self::prepared(resource_dir, override_dir)?;
        list_packages(&paths, &runtime_dir, &env_dir)
    }

    /// Install `packages` (requirement strings) into the managed environment,
    /// after making sure it exists, and return the resulting package list.
    ///
    /// # Errors
    /// When the runtime or the environment cannot be prepared, a specifier is
    /// rejected, or `uv pip install` fails.
    pub fn install(
        resource_dir: &Path,
        override_dir: Option<&Path>,
        packages: &[String],
    ) -> Result<Vec<PackageInfo>, String> {
        let (paths, runtime_dir, env_dir) = Self::prepared(resource_dir, override_dir)?;
        install_packages(&paths, &runtime_dir, &env_dir, packages)
    }

    /// The runtime and environment every command needs, installed and built,
    /// with the bundled package suite when the app ships one.
    fn prepared(
        resource_dir: &Path,
        override_dir: Option<&Path>,
    ) -> Result<(EnvPaths, PathBuf, PathBuf), String> {
        let (paths, bundled, runtime_dir) = ensure_runtime_installed(resource_dir, override_dir)?;
        let wheelhouse = load_wheelhouse(resource_dir, &bundled.platform)?;
        let env_dir = ensure_env(
            &paths,
            &runtime_dir,
            &bundled.runtime_version,
            &bundled.archive_sha256,
            wheelhouse.as_ref(),
        )?;
        Ok((paths, runtime_dir, env_dir))
    }

    /// The folder holding `python-runtime-manifest.json` and the archive:
    /// `NPI_PYTHON_RESOURCE_DIR` when set, else the app bundle's resource
    /// folder, else the crate's own `resources/` in debug builds. Existence
    /// is left to [`super::manifest::load_bundled_manifest`], which names the
    /// folders it tried.
    ///
    /// # Errors
    /// In a release build with neither an environment override nor an app
    /// resource folder (a debug build falls back to the crate's resources).
    pub fn resolve_resource_dir(app_resource_dir: Option<PathBuf>) -> Result<PathBuf, String> {
        if let Some(dir) = env_resource_dir() {
            return Ok(dir);
        }
        if let Some(dir) = app_resource_dir {
            return Ok(dir);
        }
        dev_resource_dir()
    }

    /// [`Self::resolve_resource_dir`] for the `--npi-python-host` process,
    /// which has no Tauri app handle: the explicit override, then the
    /// running executable's `Contents/Resources` (macOS `.app`), then the
    /// debug-only crate fallback.
    ///
    /// # Errors
    /// When the executable cannot be located, or neither a bundle resources
    /// folder nor a debug fallback exists.
    pub fn resolve_host_resource_dir() -> Result<PathBuf, String> {
        if let Some(dir) = env_resource_dir() {
            return Ok(dir);
        }
        let exe = std::env::current_exe()
            .map_err(|e| format!("could not locate the app executable: {e}"))?;
        if let Some(resources) = exe
            .parent()
            .and_then(Path::parent)
            .map(|contents| contents.join("Resources"))
        {
            if resources.is_dir() {
                return Ok(resources);
            }
        }
        dev_resource_dir()
    }
}

/// `NPI_PYTHON_RESOURCE_DIR`, when set to a non-empty path.
fn env_resource_dir() -> Option<PathBuf> {
    std::env::var_os("NPI_PYTHON_RESOURCE_DIR")
        .map(PathBuf::from)
        .filter(|dir| !dir.as_os_str().is_empty())
}

/// The crate's `resources/`, where `tauri dev` and the tests find the
/// manifest and archive. Never used by a release build: a shipped app must
/// find its resources next to itself, not at the build machine's path.
fn dev_resource_dir() -> Result<PathBuf, String> {
    if cfg!(debug_assertions) {
        return Ok(PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources"
        )));
    }
    Err(
        "the bundled Python runtime resources were not found next to the app \
         (set NPI_PYTHON_RESOURCE_DIR to override)"
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::python_runtime::environment::env_version_id;
    use crate::python_runtime::manifest::{write_installed_state, InstalledStateManifest};
    use crate::test_support::TempDir;
    use std::fs;

    const PLATFORM: &str = "aarch64-apple-darwin";
    const SHA: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

    fn installed_state(runtime: &Path) -> InstalledStateManifest {
        InstalledStateManifest {
            runtime_version: "3.12.16".to_string(),
            platform: PLATFORM.to_string(),
            archive_sha256: SHA.to_string(),
            uv_version: "0.13.6".to_string(),
            cpython_build: "cpython".to_string(),
            installed_at_unix: 1_760_000_000,
            runtime_path: runtime.display().to_string(),
            default_env_path: runtime
                .parent()
                .expect("a runtime parent")
                .join("environments")
                .join("default")
                .display()
                .to_string(),
        }
    }

    /// A runtime directory holding the two programs the status check asks for.
    fn runtime_programs(runtime: &Path) {
        for program in [runtime_python_bin(runtime), runtime_uv_bin(runtime)] {
            if let Some(dir) = program.parent() {
                fs::create_dir_all(dir).expect("a program folder");
            }
            fs::write(&program, "#!/bin/sh\nexit 0\n").expect("a program");
        }
    }

    /// Publish an environment version for the runtime, as `ensure_env` would.
    fn publish_env(paths: &EnvPaths) -> PathBuf {
        let id = env_version_id("3.12.16", SHA).expect("id");
        let env_dir = paths.env_version_dir(&id);
        let python = env_python_bin(&env_dir);
        fs::create_dir_all(python.parent().expect("bin")).expect("env bin");
        fs::write(&python, "#!/bin/sh\nexit 0\n").expect("env python");
        write_installed_state(
            paths,
            &installed_state(&paths.runtime_dir("3.12.16", PLATFORM, SHA)),
        )
        .expect("state");
        fs::write(paths.default_env_current_file(), format!("{id}\n")).expect("pointer");
        python
    }

    #[test]
    fn status_is_not_installed_without_a_state_manifest() {
        let dir = TempDir::new("python-manager", "status-none");
        let status = PythonRuntimeManager::status(Some(dir.path()));
        assert!(!status.installed);
        assert_eq!(status.runtime_version, None);
        assert_eq!(status.runtime_path, None);
        assert_eq!(status.python_path, None);
        assert_eq!(status.env_base_dir, dir.path().display().to_string());
    }

    #[test]
    fn status_reports_an_installed_runtime_and_its_published_interpreter() {
        let dir = TempDir::new("python-manager", "status-installed");
        let paths = EnvPaths::resolve(Some(dir.path()));
        let runtime = paths.runtime_dir("3.12.16", PLATFORM, SHA);
        runtime_programs(&runtime);
        let python = publish_env(&paths);

        let status = PythonRuntimeManager::status(Some(dir.path()));
        assert!(status.installed);
        assert_eq!(status.runtime_version.as_deref(), Some("3.12.16"));
        assert_eq!(status.platform.as_deref(), Some(PLATFORM));
        assert_eq!(
            status.runtime_path.as_deref(),
            Some(runtime.display().to_string().as_str())
        );
        assert_eq!(
            status.python_path.as_deref(),
            Some(python.display().to_string().as_str())
        );
    }

    #[test]
    fn status_is_not_installed_when_the_runtime_directory_is_gone() {
        let dir = TempDir::new("python-manager", "status-stale");
        let paths = EnvPaths::resolve(Some(dir.path()));
        let runtime = paths.runtime_dir("3.12.16", PLATFORM, SHA);
        write_installed_state(&paths, &installed_state(&runtime)).expect("state");

        let status = PythonRuntimeManager::status(Some(dir.path()));
        assert!(!status.installed);
        assert_eq!(status.runtime_version.as_deref(), Some("3.12.16"));
        assert!(status.runtime_path.is_some());
        assert_eq!(status.python_path, None);
    }

    #[test]
    fn environment_is_a_read_only_report() {
        let dir = TempDir::new("python-manager", "environment");
        let health = PythonRuntimeManager::environment(Some(dir.path()));
        assert!(!health.ok);
        assert_eq!(health.python_path, "");
        assert_eq!(health.env_path, "");
        assert!(health.message.expect("a message").contains("not installed"));
        assert!(
            !EnvPaths::resolve(Some(dir.path()))
                .default_env_dir()
                .exists(),
            "the read-only report created directories"
        );
    }

    #[test]
    fn resolve_resource_dir_prefers_the_override_then_the_app_folder() {
        let previous = std::env::var_os("NPI_PYTHON_RESOURCE_DIR");
        std::env::set_var("NPI_PYTHON_RESOURCE_DIR", "/tmp/npi-test-resources");
        assert_eq!(
            PythonRuntimeManager::resolve_resource_dir(Some(PathBuf::from("/tmp/npi-app")))
                .expect("override"),
            PathBuf::from("/tmp/npi-test-resources")
        );
        std::env::remove_var("NPI_PYTHON_RESOURCE_DIR");
        assert_eq!(
            PythonRuntimeManager::resolve_resource_dir(Some(PathBuf::from("/tmp/npi-app")))
                .expect("app folder"),
            PathBuf::from("/tmp/npi-app")
        );
        // The debug/test build falls back to the crate's resources.
        assert_eq!(
            PythonRuntimeManager::resolve_resource_dir(None).expect("dev fallback"),
            PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/resources"))
        );
        match previous {
            Some(value) => std::env::set_var("NPI_PYTHON_RESOURCE_DIR", value),
            None => std::env::remove_var("NPI_PYTHON_RESOURCE_DIR"),
        }
    }
}
