//! The two manifests of the embedded Python runtime: the one bundled next to
//! the app (what *should* be installed), and the one under `state/` (what
//! *is* installed).

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::paths::EnvPaths;

/// File name of the manifest bundled in the app's resources.
pub const BUNDLED_MANIFEST_FILE: &str = "python-runtime-manifest.json";

/// Description of the Python runtime archive shipped with the app, read from
/// `python-runtime-manifest.json` in the resource directory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BundledRuntimeManifest {
    pub runtime_version: String,
    pub platform: String,
    pub archive_name: String,
    pub archive_sha256: String,
    pub archive_size: u64,
    pub uv_version: String,
    pub cpython_build: String,
    pub components: Vec<String>,
}

/// What the installer wrote to `state/manifest.json` after a successful
/// install: the identity of the runtime on disk and the paths pointing at it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstalledStateManifest {
    pub runtime_version: String,
    pub platform: String,
    pub archive_sha256: String,
    pub uv_version: String,
    pub cpython_build: String,
    pub installed_at_unix: u64,
    pub runtime_path: String,
    pub default_env_path: String,
}

/// Read the bundled manifest, from the resource directory itself or from its
/// `resources/` subdirectory (where Tauri copies `bundle.resources` on some
/// platforms).
///
/// # Errors
/// `"… not found"` when neither location holds the file, otherwise the IO or
/// JSON error naming the file that was read.
pub fn load_bundled_manifest(resource_dir: &Path) -> Result<BundledRuntimeManifest, String> {
    let direct = resource_dir.join(BUNDLED_MANIFEST_FILE);
    let nested = resource_dir.join("resources").join(BUNDLED_MANIFEST_FILE);
    let path = if direct.is_file() {
        direct
    } else if nested.is_file() {
        nested
    } else {
        return Err(format!(
            "bundled Python runtime manifest not found at {} or {}",
            direct.display(),
            nested.display()
        ));
    };
    let bytes = fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|e| format!("{} is not a valid runtime manifest: {e}", path.display()))
}

/// The install state on disk, or `None` when nothing is installed yet (or the
/// file is unreadable/corrupt — a fresh install rewrites it).
#[must_use]
pub fn read_installed_state(paths: &EnvPaths) -> Option<InstalledStateManifest> {
    let bytes = fs::read(paths.manifest_file()).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Write the install state atomically: a temp file in `state/` renamed over
/// `manifest.json`, so a crash never leaves a truncated manifest behind.
///
/// # Errors
/// The IO or JSON error's message.
pub fn write_installed_state(
    paths: &EnvPaths,
    state: &InstalledStateManifest,
) -> Result<(), String> {
    fs::create_dir_all(paths.state_dir())
        .map_err(|e| format!("cannot create {}: {e}", paths.state_dir().display()))?;
    let bytes = serde_json::to_vec_pretty(state).map_err(|e| e.to_string())?;
    crate::json_store::write_atomic(&paths.manifest_file(), &bytes)
        .map_err(|e| format!("cannot write {}: {e}", paths.manifest_file().display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn bundled_manifest() -> BundledRuntimeManifest {
        BundledRuntimeManifest {
            runtime_version: "3.12.16".to_string(),
            platform: "aarch64-apple-darwin".to_string(),
            archive_name: "python-runtime-3.12.16-aarch64-apple-darwin.tar.gz".to_string(),
            archive_sha256: "90793c76f4012dee9bc5e77d7f40cbc416df94d7d380d927c2238fc4310ba34f"
                .to_string(),
            archive_size: 38_758_373,
            uv_version: "0.13.6".to_string(),
            cpython_build: "cpython-3.12.16+20261026-aarch64-apple-darwin-install_only".to_string(),
            components: vec![
                "cpython".to_string(),
                "stdlib".to_string(),
                "pip".to_string(),
                "uv".to_string(),
            ],
        }
    }

    fn installed_state(runtime_path: &Path, default_env_path: &Path) -> InstalledStateManifest {
        InstalledStateManifest {
            runtime_version: "3.12.16".to_string(),
            platform: "aarch64-apple-darwin".to_string(),
            archive_sha256: bundled_manifest().archive_sha256,
            uv_version: "0.13.6".to_string(),
            cpython_build: bundled_manifest().cpython_build,
            installed_at_unix: 1_700_000_000,
            runtime_path: runtime_path.display().to_string(),
            default_env_path: default_env_path.display().to_string(),
        }
    }

    #[test]
    fn bundled_manifest_loads_from_the_resource_dir_and_its_resources_subdir() {
        let root = TempDir::new("python-manifest", "bundled");
        let manifest = bundled_manifest();
        let json = serde_json::to_vec(&manifest).expect("serialize");

        let direct = root.path().join("direct");
        fs::create_dir_all(&direct).expect("mkdir");
        fs::write(direct.join(BUNDLED_MANIFEST_FILE), &json).expect("write");
        assert_eq!(load_bundled_manifest(&direct), Ok(manifest.clone()));

        let nested = root.path().join("nested");
        fs::create_dir_all(nested.join("resources")).expect("mkdir");
        fs::write(nested.join("resources").join(BUNDLED_MANIFEST_FILE), &json).expect("write");
        assert_eq!(load_bundled_manifest(&nested), Ok(manifest));

        let err = load_bundled_manifest(&root.path().join("absent")).expect_err("missing");
        assert!(err.contains("not found"), "{err}");
    }

    #[test]
    fn missing_bundled_manifest_reports_both_tried_paths() {
        let root = TempDir::new("python-manifest", "missing");
        let err = load_bundled_manifest(root.path()).expect_err("missing manifest");
        assert!(err.contains("resources"), "{err}");
    }

    #[test]
    fn install_state_round_trips_through_the_state_directory() {
        let root = TempDir::new("python-manifest", "state");
        let paths = EnvPaths::resolve(Some(root.path()));
        assert_eq!(read_installed_state(&paths), None);

        let state = installed_state(
            &paths.runtime_dir("3.12.16", "aarch64-apple-darwin", "0123456789abcdef"),
            &paths.default_env_dir(),
        );
        write_installed_state(&paths, &state).expect("write state");
        assert_eq!(read_installed_state(&paths), Some(state));

        // camelCase on disk, as the manifest's serde attribute promises.
        let raw = fs::read_to_string(paths.manifest_file()).expect("read manifest");
        assert!(raw.contains("\"runtimeVersion\""), "{raw}");
        assert!(raw.contains("\"archiveSha256\""), "{raw}");
        // The atomic write left its temp file nowhere.
        let leftovers: Vec<_> = fs::read_dir(paths.state_dir())
            .expect("read state dir")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name != "manifest.json")
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[test]
    fn unreadable_state_reads_as_none() {
        let root = TempDir::new("python-manifest", "corrupt");
        let paths = EnvPaths::resolve(Some(root.path()));
        fs::create_dir_all(paths.state_dir()).expect("mkdir");
        fs::write(paths.manifest_file(), b"{not json").expect("write");
        assert_eq!(read_installed_state(&paths), None);
    }
}
