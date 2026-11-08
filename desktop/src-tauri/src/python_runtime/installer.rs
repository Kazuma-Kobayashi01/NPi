//! Installs the bundled Python runtime into
//! `<base>/runtimes/python/<version>-<platform>-<sha12>/`.
//!
//! The runtime is immutable once installed: the archive is verified against
//! the bundled manifest's size and SHA-256, extracted into a hidden staging
//! directory next to the target, renamed into place, and then — at its final
//! *relocated* path, where a runtime that cannot find its stdlib, its
//! `lib-dynload` or its shared libraries would fail — checked by running the
//! interpreter (`stdlib`, `ssl`, `ensurepip`) and `uv`. Only after that check
//! does the state manifest name it as installed, so a corrupt or
//! non-relocatable runtime is never published as healthy.
//!
//! Installs are serialised by a process-wide mutex plus the OS file lock on
//! `<base>/state/install.lock`, so two windows (or two processes) starting at
//! once extract the archive once, not twice, and cannot race on the rename.
//! The directory name contains the archive digest, so an upgraded or
//! repackaged runtime installs *next to* the previous one and older
//! directories are never touched or removed: a session that is still running
//! an older runtime keeps working across an upgrade.
//!
//! An existing directory with the same content address is never replaced or
//! deleted: if it passes the same verification it is adopted (the state
//! manifest is rewritten), and if it does not, the install fails closed and
//! names the directory — it may belong to a runtime a live session is using,
//! which cleanup cannot know about.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use tar::Archive;

use super::command::{failure_text, run_bounded, PROBE_TIMEOUT};
use super::lock::FileLock;
use super::manifest::{self, BundledRuntimeManifest, InstalledStateManifest};
use super::paths::EnvPaths;
use super::report_progress;

/// Prefix of an extraction directory; anything matching it before an install
/// starts was abandoned by an interrupted one.
const STAGING_PREFIX: &str = ".staging-";

/// Top-level directory inside the archive. It becomes the runtime directory
/// itself, so `<target>/bin/python3` is where the archive's
/// `python/bin/python3` ends up.
const EXTRACTED_ROOT: &str = "python";

/// Serialises installs within this process; [`FileLock`] does it across
/// processes.
static INSTALL_LOCK: Mutex<()> = Mutex::new(());

/// Makes staging directory names unique within one process.
static STAGING_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Ensure the runtime described by the bundled manifest is installed under
/// `<base>/runtimes/python/<version>-<platform>/`, installing it from the
/// bundled archive if needed.
///
/// Returns the resolved paths, the bundled manifest that was installed, and
/// the absolute runtime directory (which always has the archive's `python/`
/// layout: `bin/python3`, `bin/uv`, `lib/...`).
///
/// # Errors
/// The bundled manifest or archive is missing, the archive's size or SHA-256
/// does not match the manifest, extraction or the post-extraction binary
/// check fails, the install lock cannot be taken, or the final rename or
/// state write fails.
pub fn ensure_runtime_installed(
    resource_dir: &Path,
    override_dir: Option<&Path>,
) -> Result<(EnvPaths, BundledRuntimeManifest, PathBuf), String> {
    let bundled = manifest::load_bundled_manifest(resource_dir)?;
    let paths = EnvPaths::resolve(override_dir);
    let target = paths.runtime_dir(
        &bundled.runtime_version,
        &bundled.platform,
        &bundled.archive_sha256,
    );

    if is_runtime_healthy(&target, &paths, &bundled) {
        return Ok((paths, bundled, target));
    }

    let _process_lock = INSTALL_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let _file_lock = FileLock::acquire(&paths.lock_file())?;
    create_layout(&paths)?;

    // Another process may have finished the install while this one waited.
    if is_runtime_healthy(&target, &paths, &bundled) {
        return Ok((paths, bundled, target));
    }

    // A directory with this content address already exists but the state
    // manifest does not vouch for it: an earlier attempt may have been killed
    // between the rename and the state write, or a previous installation may
    // be in use by a session that outlives this process. Never replace or
    // delete it — prove it works and adopt it; if it does not work, fail
    // closed and leave it for the user.
    if target.exists() {
        verify_runtime_published(&target, &bundled).map_err(|error| {
            format!(
                "the installed Python runtime at {} is damaged ({error}); refusing to replace \
                 a runtime directory that may be in use — remove it manually to reinstall",
                target.display()
            )
        })?;
        record_install(&paths, &bundled, &target)?;
        return Ok((paths, bundled, target));
    }

    clean_stale_staging(&paths);

    let archive = locate_archive(resource_dir, &bundled)?;
    verify_archive(&archive, &bundled)?;
    report_progress(
        "runtime-install",
        &format!(
            "installing Python {} into {}",
            bundled.runtime_version,
            target.display()
        ),
    );

    let staging = paths.runtimes_dir().join(format!(
        "{STAGING_PREFIX}{}-{}",
        std::process::id(),
        STAGING_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));

    let installed = extract_and_install(&staging, &archive, &target, &bundled);
    if installed.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    installed?;

    record_install(&paths, &bundled, &target)?;
    report_progress(
        "runtime-install",
        &format!("published the Python runtime at {}", target.display()),
    );

    Ok((paths, bundled, target))
}

/// Write the state manifest that publishes `target` as the installed runtime.
fn record_install(
    paths: &EnvPaths,
    bundled: &BundledRuntimeManifest,
    target: &Path,
) -> Result<(), String> {
    let state = InstalledStateManifest {
        runtime_version: bundled.runtime_version.clone(),
        platform: bundled.platform.clone(),
        archive_sha256: bundled.archive_sha256.clone(),
        uv_version: bundled.uv_version.clone(),
        cpython_build: bundled.cpython_build.clone(),
        installed_at_unix: unix_now(),
        runtime_path: target.display().to_string(),
        default_env_path: paths.default_env_dir().display().to_string(),
    };
    manifest::write_installed_state(paths, &state)
}

/// A runtime directory counts as healthy only when both binaries are there
/// *and* the state manifest names this exact runtime and archive: a directory
/// left behind by an interrupted install looks intact by name alone.
fn is_runtime_healthy(runtime: &Path, paths: &EnvPaths, bundled: &BundledRuntimeManifest) -> bool {
    if !python_bin(runtime).is_file() || !uv_bin(runtime).is_file() {
        return false;
    }
    manifest::read_installed_state(paths).is_some_and(|state| {
        state.runtime_version == bundled.runtime_version
            && state.platform == bundled.platform
            && state.archive_sha256 == bundled.archive_sha256
            && Path::new(&state.runtime_path) == runtime
    })
}

/// `bin/python3` of a runtime directory (`bin/python.exe` on Windows).
fn python_bin(runtime: &Path) -> PathBuf {
    if cfg!(windows) {
        runtime.join("bin").join("python.exe")
    } else {
        runtime.join("bin").join("python3")
    }
}

/// `bin/uv` of a runtime directory (`bin/uv.exe` on Windows).
fn uv_bin(runtime: &Path) -> PathBuf {
    if cfg!(windows) {
        runtime.join("bin").join("uv.exe")
    } else {
        runtime.join("bin").join("uv")
    }
}

/// Every directory a working install needs.
fn create_layout(paths: &EnvPaths) -> Result<(), String> {
    for dir in [
        paths.runtimes_dir(),
        paths.environments_dir(),
        paths.academic_env_dir(),
        paths.archives_dir(),
        paths.cache_uv_dir(),
        paths.state_dir(),
    ] {
        fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    Ok(())
}

/// Remove staging directories abandoned by an interrupted install. Best
/// effort: a leftover that cannot be removed does not block this install (its
/// own staging name is unique), but it is reported.
fn clean_stale_staging(paths: &EnvPaths) {
    let Ok(entries) = fs::read_dir(paths.runtimes_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry
            .file_name()
            .to_string_lossy()
            .starts_with(STAGING_PREFIX)
        {
            continue;
        }
        let path = entry.path();
        if let Err(e) = fs::remove_dir_all(&path) {
            eprintln!("[omp-desktop] cannot remove stale {}: {e}", path.display());
        }
    }
}

/// The archive sits next to the manifest, either in the resource directory or
/// in its `resources/` subdirectory.
fn locate_archive(
    resource_dir: &Path,
    bundled: &BundledRuntimeManifest,
) -> Result<PathBuf, String> {
    let direct = resource_dir.join(&bundled.archive_name);
    let nested = resource_dir.join("resources").join(&bundled.archive_name);
    if direct.is_file() {
        Ok(direct)
    } else if nested.is_file() {
        Ok(nested)
    } else {
        Err(format!(
            "bundled Python runtime archive {} not found at {} or {}",
            bundled.archive_name,
            direct.display(),
            nested.display()
        ))
    }
}

/// Reject an archive that is not byte-for-byte the one the manifest
/// describes: its size first (cheap), then its SHA-256.
fn verify_archive(archive: &Path, bundled: &BundledRuntimeManifest) -> Result<(), String> {
    let size = fs::metadata(archive)
        .map_err(|e| format!("cannot stat {}: {e}", archive.display()))?
        .len();
    if size != bundled.archive_size {
        return Err(format!(
            "{} is {size} bytes, expected {} (archive does not match the manifest)",
            archive.display(),
            bundled.archive_size
        ));
    }
    let actual = sha256_file(archive)?;
    if !actual.eq_ignore_ascii_case(&bundled.archive_sha256) {
        return Err(format!(
            "{} SHA-256 mismatch: expected {}, got {actual}",
            archive.display(),
            bundled.archive_sha256
        ));
    }
    Ok(())
}

/// SHA-256 of a file's contents, read in 64 KiB chunks so a 40 MB archive is
/// never held in memory.
pub(super) fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(crate::json_store::hex_lower(&hasher.finalize()))
}

/// Extract the archive into `staging`, move its `python/` directory to
/// `target`, and verify it at that final, relocated path. A target that fails
/// verification is removed again so the next attempt re-extracts it.
fn extract_and_install(
    staging: &Path,
    archive: &Path,
    target: &Path,
    bundled: &BundledRuntimeManifest,
) -> Result<(), String> {
    fs::create_dir_all(staging).map_err(|e| format!("cannot create {}: {e}", staging.display()))?;

    let file =
        File::open(archive).map_err(|e| format!("cannot open {}: {e}", archive.display()))?;
    // The stage a supervisor watches for: after this line the unpack runs, so
    // a killed host leaves at most a `.staging-…` directory, which the next
    // install sweeps.
    report_progress(
        "extracting",
        &format!("unpacking {} into {}", archive.display(), staging.display()),
    );
    Archive::new(GzDecoder::new(file))
        .unpack(staging)
        .map_err(|e| format!("cannot extract {}: {e}", archive.display()))?;

    let extracted = staging.join(EXTRACTED_ROOT);
    if !extracted.is_dir() {
        return Err(format!(
            "{} has no {EXTRACTED_ROOT}/ directory at its top level",
            archive.display()
        ));
    }
    #[cfg(target_os = "macos")]
    clear_quarantine(&extracted);
    move_into_place(&extracted, target)?;

    if let Err(error) = verify_runtime_published(target, bundled) {
        let _ = crate::private_fs::remove_dir_retrying(target);
        let _ = fs::remove_dir_all(staging);
        return Err(error);
    }

    let _ = fs::remove_dir_all(staging);
    Ok(())
}

/// Drop the quarantine flag macOS puts on files downloaded in an archive.
/// Best effort: `xattr` fails when the attribute was never set, which is the
/// normal case for an extracted tarball.
#[cfg(target_os = "macos")]
fn clear_quarantine(root: &Path) {
    let mut cmd = Command::new("xattr");
    cmd.arg("-r")
        .arg("-d")
        .arg("com.apple.quarantine")
        .arg(root);
    let _ = crate::spawn_gate::status(&mut cmd);
}

/// The runtime interpreter's self-check, run at its final path: it prints
/// the interpreter's identity and the roots it resolves, and imports the
/// modules a relocatable build must carry.
const RUNTIME_PROBE: &str = "import json, ssl, sys, sysconfig\n\
import ensurepip\n\
print(json.dumps({\"version\": \"%d.%d.%d\" % sys.version_info[:3], \
\"base_prefix\": sys.base_prefix, \"stdlib\": sysconfig.get_path(\"stdlib\"), \
\"ssl\": ssl.OPENSSL_VERSION}))";

/// Verify the extracted runtime *where it will run*: an archive that only
/// worked at its build prefix shows up here — the interpreter cannot find
/// its stdlib or `lib-dynload`, `ssl` fails to import, or ensurepip (the
/// offline pip bootstrap `venv` needs) is missing. `uv --version` proves its
/// binary and dylibs too. Called before the state manifest publishes the
/// runtime.
fn verify_runtime_published(
    runtime: &Path,
    bundled: &BundledRuntimeManifest,
) -> Result<(), String> {
    let python = python_bin(runtime);
    if !python.is_file() {
        return Err(format!("extracted runtime is missing {}", python.display()));
    }
    let uv = uv_bin(runtime);
    if !uv.is_file() {
        return Err(format!("extracted runtime is missing {}", uv.display()));
    }
    let mut python_version = Command::new(&python);
    python_version.args(["-I", "-B", "--version"]);
    let mut uv_version = Command::new(&uv);
    uv_version.arg("--version");
    for (program, cmd) in [(&python, &mut python_version), (&uv, &mut uv_version)] {
        let run = run_bounded(cmd, PROBE_TIMEOUT, "the extracted runtime")?;
        if !run.success {
            return Err(format!(
                "{} --version failed ({})",
                program.display(),
                failure_text(&run)
            ));
        }
    }

    let mut cmd = Command::new(&python);
    cmd.args(["-I", "-B", "-c", RUNTIME_PROBE])
        .current_dir(runtime);
    let run = run_bounded(&mut cmd, PROBE_TIMEOUT, "the extracted runtime")?;
    if !run.success {
        return Err(format!(
            "the extracted runtime at {} does not start ({})",
            runtime.display(),
            failure_text(&run)
        ));
    }
    let probe: serde_json::Value = serde_json::from_slice(&run.stdout).map_err(|e| {
        format!(
            "the extracted runtime at {} answered an unreadable probe: {e}",
            runtime.display()
        )
    })?;
    if probe_text(&probe, "version") != bundled.runtime_version.as_str() {
        return Err(format!(
            "the archive reports Python {}, the manifest says {}",
            probe_text(&probe, "version"),
            bundled.runtime_version
        ));
    }
    let runtime_root = fs::canonicalize(runtime).unwrap_or_else(|_| runtime.to_path_buf());
    let inside = |key: &str| {
        fs::canonicalize(probe_text(&probe, key))
            .is_ok_and(|resolved| resolved.starts_with(&runtime_root))
    };
    if !inside("base_prefix") || !inside("stdlib") {
        return Err(format!(
            "the extracted runtime at {} resolves its stdlib outside itself (prefix {}, stdlib {}); the archive is not relocatable",
            runtime.display(),
            probe_text(&probe, "base_prefix"),
            probe_text(&probe, "stdlib")
        ));
    }
    if probe_text(&probe, "ssl").is_empty() {
        return Err(format!(
            "the extracted runtime at {} has no usable SSL module",
            runtime.display()
        ));
    }
    Ok(())
}

/// One string field of a probe result, empty when absent.
fn probe_text<'a>(probe: &'a serde_json::Value, key: &str) -> &'a str {
    probe
        .get(key)
        .and_then(|value| value.as_str())
        .unwrap_or_default()
}

/// Move the freshly extracted runtime to `target`. The rename is atomic
/// within the `runtimes/python/` directory, so a reader never sees a
/// partially-copied tree; other versions' directories are left alone. A
/// target that exists is a bug in the caller's ordering, not something to
/// delete: the install path only runs after proving it absent.
fn move_into_place(extracted: &Path, target: &Path) -> Result<(), String> {
    if target.exists() {
        return Err(format!(
            "refusing to replace the existing runtime at {}",
            target.display()
        ));
    }
    fs::rename(extracted, target).map_err(|e| {
        format!(
            "cannot move {} to {}: {e}",
            extracted.display(),
            target.display()
        )
    })
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

#[cfg(all(test, unix))]
mod tests {
    use super::super::manifest::{read_installed_state, BUNDLED_MANIFEST_FILE};
    use super::*;
    use crate::test_support::{executable, TempDir};
    use flate2::write::GzEncoder;
    use flate2::Compression;

    const PLATFORM: &str = "aarch64-apple-darwin";

    /// A scratch resource directory (manifest + archive) and a scratch
    /// environment base directory, both under one [`TempDir`].
    struct Fixture {
        temp: TempDir,
    }

    impl Fixture {
        fn new(label: &str) -> Self {
            let temp = TempDir::new("python-installer", label);
            fs::create_dir_all(temp.path().join("resources")).expect("create resources dir");
            Self { temp }
        }

        fn resources(&self) -> PathBuf {
            self.temp.path().join("resources")
        }

        fn env(&self) -> PathBuf {
            self.temp.path().join("env")
        }

        /// Write a synthetic archive and manifest for `version` into the
        /// resource directory, returning the manifest that was written.
        fn publish_runtime(&self, version: &str) -> BundledRuntimeManifest {
            let tree = self.temp.path().join(format!("archive-tree-{version}"));
            let python = python_script(version);
            let bytes = build_archive(&tree, &python);
            let archive_name = format!("python-runtime-{version}-{PLATFORM}.tar.gz");
            fs::write(self.resources().join(&archive_name), &bytes).expect("write archive");
            let bundled = BundledRuntimeManifest {
                runtime_version: version.to_string(),
                platform: PLATFORM.to_string(),
                archive_name,
                archive_sha256: crate::json_store::hex_sha256(&bytes),
                archive_size: bytes.len() as u64,
                uv_version: "0.13.6".to_string(),
                cpython_build: format!("cpython-{version}+20261026-{PLATFORM}-install_only"),
                components: vec![
                    "cpython".to_string(),
                    "stdlib".to_string(),
                    "pip".to_string(),
                    "uv".to_string(),
                ],
            };
            fs::write(
                self.resources().join(BUNDLED_MANIFEST_FILE),
                serde_json::to_vec_pretty(&bundled).expect("serialize manifest"),
            )
            .expect("write manifest");
            bundled
        }

        /// The content-addressed runtime directory the manifest installs to.
        fn runtime_dir(&self, bundled: &BundledRuntimeManifest) -> PathBuf {
            self.env().join("runtimes").join("python").join(format!(
                "{}-{PLATFORM}-{}",
                bundled.runtime_version,
                &bundled.archive_sha256[..12]
            ))
        }

        fn runtimes(&self) -> PathBuf {
            self.env().join("runtimes").join("python")
        }

        fn installed_entries(&self) -> Vec<PathBuf> {
            let Ok(entries) = fs::read_dir(self.runtimes()) else {
                return Vec::new();
            };
            entries.flatten().map(|entry| entry.path()).collect()
        }

        fn install(&self) -> Result<(EnvPaths, BundledRuntimeManifest, PathBuf), String> {
            let resources = self.resources();
            let env = self.env();
            ensure_runtime_installed(&resources, Some(env.as_path()))
        }

        fn staging_dirs(&self) -> Vec<PathBuf> {
            self.installed_entries()
                .into_iter()
                .filter(|path| {
                    path.file_name()
                        .is_some_and(|name| name.to_string_lossy().starts_with(STAGING_PREFIX))
                })
                .collect()
        }
    }

    /// The stand-in interpreter used by the synthetic archive: it answers
    /// `--version` and the runtime probe from its *own* final location, the
    /// way a relocatable build does.
    fn python_script(version: &str) -> String {
        format!(
            "#!/bin/sh\n\
             root=$(cd \"$(dirname \"$0\")/..\" && pwd -P)\n\
             case \"$*\" in\n\
               *--version*) echo 'Python {version}' ;;\n\
               *) echo \"{{\\\"version\\\": \\\"{version}\\\", \\\"base_prefix\\\": \\\"$root\\\", \\\"stdlib\\\": \\\"$root/lib/python3.12\\\", \\\"ssl\\\": \\\"OpenSSL 3.5.9\\\"}}\" ;;\n\
             esac\n\
             exit 0\n"
        )
    }

    /// A `python/` tree with executable `bin/python3` and `bin/uv`, plus a
    /// stdlib file, packed into a tar.gz just like the real runtime archive.
    fn build_archive(tree: &Path, python: &str) -> Vec<u8> {
        executable(&tree.join("python/bin/python3"), python);
        executable(&tree.join("python/bin/uv"), "#!/bin/sh\necho 'uv 0.13.6'\n");
        let stdlib = tree.join("python/lib/python3.12");
        fs::create_dir_all(&stdlib).expect("create stdlib dir");
        fs::write(stdlib.join("os.py"), "# stdlib\n").expect("write stdlib");

        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        {
            let mut builder = tar::Builder::new(&mut encoder);
            builder
                .append_dir_all("python", tree.join("python"))
                .expect("append python tree");
            builder.finish().expect("finish tar");
        }
        encoder.finish().expect("finish gzip")
    }

    #[test]
    fn fresh_install_extracts_the_runtime_and_records_state() {
        let fx = Fixture::new("fresh");
        let bundled = fx.publish_runtime("3.12.16");

        let (paths, installed_manifest, runtime) = fx.install().expect("install");
        assert_eq!(installed_manifest, bundled);
        assert_eq!(runtime, fx.runtime_dir(&bundled));
        assert_eq!(paths.base_dir(), fx.env().as_path());

        assert!(runtime.join("bin/python3").is_file());
        assert!(runtime.join("bin/uv").is_file());
        assert!(runtime.join("lib/python3.12/os.py").is_file());
        // The archive's `python/` top level became the runtime directory.
        assert!(!runtime.join("python").exists());
        assert!(paths.cache_uv_dir().is_dir());

        let state = read_installed_state(&paths).expect("state manifest");
        assert_eq!(state.runtime_version, "3.12.16");
        assert_eq!(state.platform, PLATFORM);
        assert_eq!(state.archive_sha256, bundled.archive_sha256);
        assert_eq!(state.runtime_path, runtime.display().to_string());
        assert_eq!(
            state.default_env_path,
            paths.default_env_dir().display().to_string()
        );
        assert!(state.installed_at_unix > 0);
        assert!(fx.staging_dirs().is_empty(), "{:?}", fx.staging_dirs());

        // A second call takes the healthy fast path and rewrites nothing.
        let (_, _, again) = fx.install().expect("second install");
        assert_eq!(again, runtime);
        assert_eq!(read_installed_state(&paths), Some(state));
    }

    #[test]
    fn sha256_mismatch_is_rejected_and_staging_is_cleaned_up() {
        let fx = Fixture::new("sha");
        let mut bundled = fx.publish_runtime("3.12.16");
        let target = fx.runtime_dir(&bundled);
        bundled.archive_sha256 = "0".repeat(64);
        fs::write(
            fx.resources().join(BUNDLED_MANIFEST_FILE),
            serde_json::to_vec_pretty(&bundled).expect("serialize"),
        )
        .expect("write manifest");

        let err = fx
            .install()
            .expect_err("mismatched archive must be rejected");
        assert!(err.contains("SHA-256 mismatch"), "{err}");
        assert!(!target.exists());
        assert!(fx.staging_dirs().is_empty(), "{:?}", fx.staging_dirs());
        assert!(
            fx.installed_entries().is_empty(),
            "{:?}",
            fx.installed_entries()
        );
        let env = fx.env();
        let paths = EnvPaths::resolve(Some(env.as_path()));
        assert!(read_installed_state(&paths).is_none());
    }

    #[test]
    fn interrupted_staging_directory_is_recovered() {
        let fx = Fixture::new("stale");
        let bundled = fx.publish_runtime("3.12.16");
        let stale = fx.runtimes().join(format!("{STAGING_PREFIX}9999-0"));
        fs::create_dir_all(stale.join("python/bin")).expect("create stale staging");
        fs::write(stale.join("python/bin/half"), b"truncated").expect("write junk");

        let (_, _, runtime) = fx.install().expect("install over stale staging");
        assert_eq!(runtime, fx.runtime_dir(&bundled));
        assert!(runtime.join("bin/python3").is_file());
        assert!(!stale.exists(), "stale staging dir was not removed");
        assert!(fx.staging_dirs().is_empty(), "{:?}", fx.staging_dirs());
    }

    #[test]
    fn a_damaged_runtime_directory_is_never_replaced() {
        let fx = Fixture::new("damaged");
        let bundled = fx.publish_runtime("3.12.16");
        let damaged = fx.runtime_dir(&bundled);
        fs::create_dir_all(damaged.join("bin")).expect("create damaged runtime");
        fs::write(damaged.join("bin/python3"), b"not a binary").expect("write junk");

        let error = fx
            .install()
            .expect_err("a damaged runtime must fail closed");
        assert!(error.contains("refusing to replace"), "error: {error}");
        assert_eq!(
            fs::read(damaged.join("bin/python3")).expect("the directory is left alone"),
            b"not a binary"
        );
        assert!(fx.staging_dirs().is_empty(), "{:?}", fx.staging_dirs());
        let env = fx.env();
        let paths = EnvPaths::resolve(Some(env.as_path()));
        assert!(read_installed_state(&paths).is_none());
    }

    #[test]
    fn a_complete_unrecorded_runtime_is_adopted_without_extracting() {
        let fx = Fixture::new("adopt");
        let bundled = fx.publish_runtime("3.12.16");
        let (paths, _, runtime) = fx.install().expect("install");
        // The state manifest is lost (a kill between the rename and the state
        // write); the archive is gone too, so only adoption can succeed.
        fs::remove_file(paths.manifest_file()).expect("drop the state");
        fs::remove_file(fx.resources().join(&bundled.archive_name)).expect("drop the archive");

        let (_, _, adopted) = fx.install().expect("adopt");
        assert_eq!(adopted, runtime);
        assert!(runtime.join("bin/python3").is_file());
        assert_eq!(
            read_installed_state(&paths).map(|state| state.runtime_path),
            Some(runtime.display().to_string())
        );
    }

    #[test]
    fn a_runtime_that_fails_its_relocated_probe_is_rejected() {
        let fx = Fixture::new("probe");
        // Answers `--version` but cannot run the probe: a build whose stdlib
        // did not survive relocation.
        let tree = fx.temp.path().join("archive-tree-3.12.16");
        let python =
            "#!/bin/sh\ncase \"$*\" in *--version*) echo 'Python 3.12.16' ;; *) exit 9 ;; esac\n";
        let bytes = build_archive(&tree, python);
        let archive_name = format!("python-runtime-3.12.16-{PLATFORM}.tar.gz");
        fs::write(fx.resources().join(&archive_name), &bytes).expect("write archive");
        let bundled = BundledRuntimeManifest {
            runtime_version: "3.12.16".to_string(),
            platform: PLATFORM.to_string(),
            archive_name,
            archive_sha256: crate::json_store::hex_sha256(&bytes),
            archive_size: bytes.len() as u64,
            uv_version: "0.13.6".to_string(),
            cpython_build: "test".to_string(),
            components: vec!["cpython".to_string()],
        };
        fs::write(
            fx.resources().join(BUNDLED_MANIFEST_FILE),
            serde_json::to_vec_pretty(&bundled).expect("serialize"),
        )
        .expect("write manifest");

        let error = fx.install().expect_err("a non-relocatable runtime");
        assert!(error.contains("does not start"), "error: {error}");
        assert!(
            !fx.runtime_dir(&bundled).exists(),
            "the rejected runtime was published"
        );
        assert!(fx.staging_dirs().is_empty(), "{:?}", fx.staging_dirs());
        let env = fx.env();
        let paths = EnvPaths::resolve(Some(env.as_path()));
        assert!(read_installed_state(&paths).is_none());
    }

    #[test]
    #[allow(clippy::needless_collect)]
    fn concurrent_installs_serialise_and_install_once() {
        let fx = Fixture::new("concurrent");
        let bundled = fx.publish_runtime("3.12.16");

        let results: Vec<_> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..4).map(|_| scope.spawn(|| fx.install())).collect();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("thread panicked"))
                .collect()
        });

        let runtime = fx.runtime_dir(&bundled);
        for result in results {
            let (_, _, installed) = result.expect("concurrent install");
            assert_eq!(installed, runtime);
        }
        assert!(runtime.join("bin/python3").is_file());
        assert!(fx.staging_dirs().is_empty(), "{:?}", fx.staging_dirs());
        let env = fx.env();
        let paths = EnvPaths::resolve(Some(env.as_path()));
        assert_eq!(
            read_installed_state(&paths).map(|state| state.runtime_version),
            Some("3.12.16".to_string())
        );
    }

    #[test]
    fn upgrade_keeps_the_older_version_directory() {
        let fx = Fixture::new("upgrade");
        let old_bundled = fx.publish_runtime("3.12.16");
        let (paths, _, old_runtime) = fx.install().expect("install 3.12.16");
        assert_eq!(old_runtime, fx.runtime_dir(&old_bundled));

        let newer = fx.publish_runtime("3.12.17");
        let (_, _, new_runtime) = fx.install().expect("install 3.12.17");
        assert_eq!(new_runtime, fx.runtime_dir(&newer));
        assert_ne!(new_runtime, old_runtime);

        assert!(
            old_runtime.join("bin/python3").is_file(),
            "older runtime removed"
        );
        assert!(new_runtime.join("bin/python3").is_file());
        let state = read_installed_state(&paths).expect("state");
        assert_eq!(state.runtime_version, newer.runtime_version);
        assert_eq!(state.archive_sha256, newer.archive_sha256);
        assert_eq!(state.runtime_path, new_runtime.display().to_string());
    }

    #[test]
    fn a_repackaged_archive_installs_beside_the_previous_one() {
        let fx = Fixture::new("repackage");
        let first = fx.publish_runtime("3.12.16");
        let (paths, _, first_runtime) = fx.install().expect("first install");
        assert_eq!(first_runtime, fx.runtime_dir(&first));

        // Same version, different bytes (a regenerated archive).
        fs::write(
            fx.temp.path().join("archive-tree-3.12.16/python/extra.txt"),
            b"extra\n",
        )
        .expect("add a file");
        let repackaged = fx.publish_runtime("3.12.16");
        assert_ne!(repackaged.archive_sha256, first.archive_sha256);
        let (_, _, second_runtime) = fx.install().expect("repackaged install");

        assert_eq!(second_runtime, fx.runtime_dir(&repackaged));
        assert_ne!(second_runtime, first_runtime);
        assert!(
            first_runtime.join("bin/python3").is_file(),
            "the runtime in use was replaced"
        );
        assert_eq!(
            read_installed_state(&paths).map(|state| state.archive_sha256),
            Some(repackaged.archive_sha256)
        );
    }

    #[test]
    fn missing_archive_is_reported() {
        let fx = Fixture::new("noarchive");
        let bundled = fx.publish_runtime("3.12.16");
        fs::remove_file(fx.resources().join(&bundled.archive_name)).expect("remove archive");

        let err = fx.install().expect_err("missing archive");
        assert!(err.contains("not found"), "{err}");
        assert!(!fx.runtime_dir(&bundled).exists());
    }
}
