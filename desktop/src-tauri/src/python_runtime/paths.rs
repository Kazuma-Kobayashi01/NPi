//! Path resolution for `NPi`'s embedded Python runtime and its managed
//! environment.
//!
//! Every directory the installer, the environment builder and omp's runtime
//! lookup touch is derived here, from one base directory, so they cannot
//! drift apart.
//!
//! Hierarchy under `<base>` (`$NPI_ENV_DIR`, or an explicit override):
//! ```text
//! <base>/
//! ├── runtimes/python/
//! │   └── <version>-<platform>-<sha12>/ # Immutable runtime, content-addressed
//! │       ├── bin/ (python3, uv, ...)
//! │       ├── lib/
//! │       └── ...
//! ├── environments/default/             # The default managed environment
//! │   ├── current                      # Pointer file: the active version id
//! │   └── versions/
//! │       └── py<version>-<sha12>/      # A complete virtualenv
//! ├── cache/
//! │   └── uv/                           # uv's download/build cache
//! └── state/
//!     ├── manifest.json                 # Installation status & metadata
//!     └── install.lock                  # Cross-process install/build lock
//! ```
//!
//! Both the runtime directory and the environment version are named after
//! the archive's contents (a SHA-256 prefix), not just its version: a
//! repackaged or upgraded runtime gets a new directory, so nothing a running
//! session still uses is ever replaced or deleted. The environment pointer is
//! rewritten atomically after a new version has been verified, which is what
//! makes an upgrade a single-file switch.

use std::env;
use std::path::{Path, PathBuf};

/// Central path resolver for `NPi`'s embedded Python runtime and its managed
/// environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvPaths {
    pub base: PathBuf,
}

impl EnvPaths {
    /// Resolve the environment base directory, in priority order: an explicit
    /// `override_dir`, then a non-empty `NPI_ENV_DIR`, then
    /// `<home>/.local/share/npi/env`.
    ///
    /// The home directory comes from `HOME` (or `USERPROFILE` on Windows,
    /// where `HOME` is not normally set). With neither variable set, the
    /// relative `.npi-env` keeps the app working from its own directory
    /// rather than writing somewhere unintended.
    #[must_use]
    pub fn resolve(override_dir: Option<&Path>) -> Self {
        if let Some(dir) = override_dir {
            return Self {
                base: dir.to_path_buf(),
            };
        }
        if let Some(dir) = env::var_os("NPI_ENV_DIR")
            .map(PathBuf::from)
            .filter(|dir| !dir.as_os_str().is_empty())
        {
            return Self { base: dir };
        }
        Self {
            base: home_dir().map_or_else(
                || PathBuf::from(".npi-env"),
                |home| home.join(".local").join("share").join("npi").join("env"),
            ),
        }
    }

    /// Base directory root `<base>`.
    #[must_use]
    pub fn base_dir(&self) -> &Path {
        &self.base
    }

    /// `<base>/runtimes/python/` — every installed runtime version lives here.
    #[must_use]
    pub fn runtimes_dir(&self) -> PathBuf {
        self.base.join("runtimes").join("python")
    }

    /// `<base>/runtimes/python/<version>-<platform>-<sha12>/`.
    ///
    /// The archive SHA prefix is part of the name so an upgraded or
    /// repackaged runtime installs next to the old one instead of over it:
    /// a session still running the previous runtime keeps its files.
    #[must_use]
    pub fn runtime_dir(&self, version: &str, platform: &str, archive_sha256: &str) -> PathBuf {
        let digest = archive_sha256
            .get(..12)
            .unwrap_or(archive_sha256)
            .to_ascii_lowercase();
        self.runtimes_dir()
            .join(format!("{version}-{platform}-{digest}"))
    }

    /// `<base>/environments/`
    #[must_use]
    pub fn environments_dir(&self) -> PathBuf {
        self.base.join("environments")
    }

    /// `<base>/environments/default/` — the managed environment's container:
    /// the `current` pointer and the versioned virtualenvs.
    #[must_use]
    pub fn default_env_dir(&self) -> PathBuf {
        self.environments_dir().join("default")
    }

    /// `<base>/environments/default/versions/`
    #[must_use]
    pub fn default_env_versions_dir(&self) -> PathBuf {
        self.default_env_dir().join("versions")
    }

    /// `<base>/environments/default/versions/<version_id>/`
    #[must_use]
    pub fn env_version_dir(&self, version_id: &str) -> PathBuf {
        self.default_env_versions_dir().join(version_id)
    }

    /// `<base>/environments/default/current` — the atomically rewritten
    /// pointer naming the active version directory.
    #[must_use]
    pub fn default_env_current_file(&self) -> PathBuf {
        self.default_env_dir().join("current")
    }

    /// `<base>/environments/academic/` — reserved for the optional expansion
    /// environment; nothing writes it yet.
    #[must_use]
    pub fn academic_env_dir(&self) -> PathBuf {
        self.environments_dir().join("academic")
    }

    /// `<base>/archives/` — host-managed archives that outlive the app bundle
    /// (kept for a future cached download; the bundled archive is read from
    /// the resource directory).
    #[must_use]
    pub fn archives_dir(&self) -> PathBuf {
        self.base.join("archives")
    }

    /// `<base>/cache/`
    #[must_use]
    pub fn cache_dir(&self) -> PathBuf {
        self.base.join("cache")
    }

    /// `<base>/cache/uv/`
    #[must_use]
    pub fn cache_uv_dir(&self) -> PathBuf {
        self.cache_dir().join("uv")
    }

    /// `<base>/state/`
    #[must_use]
    pub fn state_dir(&self) -> PathBuf {
        self.base.join("state")
    }

    /// `<base>/state/manifest.json`
    #[must_use]
    pub fn manifest_file(&self) -> PathBuf {
        self.state_dir().join("manifest.json")
    }

    /// `<base>/state/install.lock` — the OS lock both the runtime installer
    /// and the environment builder take.
    #[must_use]
    pub fn lock_file(&self) -> PathBuf {
        self.state_dir().join("install.lock")
    }
}

/// `HOME`, else `USERPROFILE`, else none.
fn home_dir() -> Option<PathBuf> {
    ["HOME", "USERPROFILE"]
        .into_iter()
        .find_map(env::var_os)
        .map(PathBuf::from)
        .filter(|dir| !dir.as_os_str().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_wins_over_npi_env_dir() {
        // `NPI_ENV_DIR` is read only when no override is given, so an
        // override cannot be shadowed by the environment.
        let paths = EnvPaths::resolve(Some(Path::new("/tmp/npi-env-override")));
        assert_eq!(paths.base_dir(), Path::new("/tmp/npi-env-override"));
    }

    #[test]
    fn every_managed_path_hangs_off_the_base() {
        let base = Path::new("/tmp/npi-env-tree");
        let paths = EnvPaths::resolve(Some(base));
        assert_eq!(paths.runtimes_dir(), base.join("runtimes/python"));
        assert_eq!(
            paths.runtime_dir("3.12.16", "aarch64-apple-darwin", "0123456789abcdef"),
            base.join("runtimes/python/3.12.16-aarch64-apple-darwin-0123456789ab")
        );
        assert_eq!(paths.environments_dir(), base.join("environments"));
        assert_eq!(paths.default_env_dir(), base.join("environments/default"));
        assert_eq!(
            paths.env_version_dir("py3.12.16-0123456789ab"),
            base.join("environments/default/versions/py3.12.16-0123456789ab")
        );
        assert_eq!(
            paths.default_env_current_file(),
            base.join("environments/default/current")
        );
        assert_eq!(paths.cache_uv_dir(), base.join("cache/uv"));
        assert_eq!(paths.manifest_file(), base.join("state/manifest.json"));
        assert_eq!(paths.lock_file(), base.join("state/install.lock"));
    }

    #[test]
    fn the_runtime_directory_is_named_after_the_archive_contents() {
        let paths = EnvPaths::resolve(Some(Path::new("/tmp/npi-env-tree")));
        let first = paths.runtime_dir("3.12.16", "linux-x86_64", "aaaaaaaaaaaaaaaa");
        let repackaged = paths.runtime_dir("3.12.16", "linux-x86_64", "bbbbbbbbbbbbbbbb");
        assert_ne!(
            first, repackaged,
            "a repackaged archive must not reuse the directory in use"
        );
        // A short digest is used whole rather than producing a bare name.
        assert_eq!(
            paths.runtime_dir("3.12.16", "linux-x86_64", "abc"),
            paths.runtimes_dir().join("3.12.16-linux-x86_64-abc")
        );
    }
}
