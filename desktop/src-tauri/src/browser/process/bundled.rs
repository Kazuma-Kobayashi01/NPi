//! The browser the panel runs: the Chromium bundled with the app, never one
//! found on the machine and never one the user points at.
//!
//! The binary is Chrome for Testing's `chrome-headless-shell`, fetched by
//! `scripts/fetch-chromium.sh` into `resources/chromium/` and shipped under
//! the app's resource directory. `cargo test` and `tauri dev` have no
//! resource directory of their own and read the same folder in the source
//! tree.

use std::path::{Path, PathBuf};

/// The folder `scripts/fetch-chromium.sh` unpacks to, under `resources/`.
const BUNDLE_DIR: &str = "chromium/chrome-headless-shell-mac-arm64";
/// The executable inside it.
const BINARY: &str = "chrome-headless-shell";

#[derive(Debug)]
pub struct BundledChromium {
    binary: PathBuf,
}

impl BundledChromium {
    /// The bundled browser of a running app: `<resource dir>/resources/chromium/…`
    /// (Tauri keeps the `resources/` prefix of a bundled path) when that
    /// exists, else `<resource dir>/chromium/…`, else the source tree's copy
    /// (`tauri dev`, `cargo test`). Nothing else is searched: not `PATH`, not
    /// the installed browsers.
    pub fn locate(resource_dir: Option<&Path>) -> Self {
        let in_tree = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources");
        let found = resource_dir
            .into_iter()
            .flat_map(|dir| [dir.join("resources"), dir.to_path_buf()])
            .chain([in_tree.clone()])
            .map(|root| root.join(BUNDLE_DIR).join(BINARY))
            .find(|binary| binary.is_file());
        Self {
            binary: found.unwrap_or_else(|| in_tree.join(BUNDLE_DIR).join(BINARY)),
        }
    }

    /// The source tree's copy (what the real-Chromium tests run).
    #[cfg(test)]
    pub fn in_tree() -> Self {
        Self::locate(None)
    }

    /// A stand-in browser for tests: a fake binary at `binary`.
    #[cfg(test)]
    pub fn at(binary: PathBuf) -> Self {
        Self { binary }
    }

    /// The programs a launch tries, in order: the bundled browser alone, or why
    /// it cannot run.
    pub fn launch_candidates(&self) -> Result<Vec<PathBuf>, String> {
        if crate::program_setting::is_executable_file(&self.binary) {
            Ok(vec![self.binary.clone()])
        } else {
            Err(format!(
                "the bundled Chromium is missing or not executable ({}); run scripts/fetch-chromium.sh",
                self.binary.display()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn fake(root: &Path) -> PathBuf {
        let binary = root.join(BUNDLE_DIR).join(BINARY);
        std::fs::create_dir_all(binary.parent().unwrap()).unwrap();
        std::fs::write(&binary, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        binary
    }

    #[test]
    fn the_resource_dir_wins_with_or_without_the_resources_prefix() {
        let d = TempDir::new("bundled", "prefix");
        let nested = fake(&d.path().join("resources"));
        assert_eq!(BundledChromium::locate(Some(d.path())).binary, nested);
        let d = TempDir::new("bundled", "direct");
        let direct = fake(d.path());
        assert_eq!(BundledChromium::locate(Some(d.path())).binary, direct);
    }

    #[test]
    fn a_missing_browser_is_a_clear_error_and_nothing_else_is_searched() {
        let d = TempDir::new("bundled", "missing");
        let b = BundledChromium { binary: d.path().join("none") };
        let err = b.launch_candidates().unwrap_err();
        assert!(err.contains("bundled Chromium is missing"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn a_present_browser_is_the_only_candidate() {
        let d = TempDir::new("bundled", "present");
        let binary = fake(d.path());
        let b = BundledChromium { binary: binary.clone() };
        assert_eq!(b.launch_candidates().unwrap(), vec![binary]);
    }
}
