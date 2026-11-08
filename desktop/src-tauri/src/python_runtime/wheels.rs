//! The offline wheelhouse bundled next to the runtime.
//!
//! It is a pinned package suite (numpy, scipy, matplotlib, pandas, sympy and
//! their dependencies) installed into each managed environment version while
//! it is built.
//!
//! `python-runtime-wheels-manifest.json` names the archive, its size and
//! SHA-256, and where the lock and the wheels sit inside it. The archive is
//! verified against the manifest, unpacked into a staging directory under
//! `<base>/archives/`, and installed with the runtime's `uv` using
//! `--offline --no-index --require-hashes`: no index is reached, and a wheel
//! whose digest is not in the lock is refused. A marker file in the
//! environment records the archive digest, so a version directory left by a
//! build that died before the install finished is never published as
//! complete.
//!
//! An app built without the suite has no wheelhouse manifest, and the
//! environment is built with pip alone. A manifest whose archive is missing
//! or altered is an error, like the runtime's own archive.

use std::fs::{self, File};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use tar::Archive;

use super::command::{failure_text, run_bounded};
use super::environment::{env_python_bin, prepare_child_env, runtime_uv_bin, PackageInfo};
use super::paths::EnvPaths;
use super::report_progress;

/// File name of the wheelhouse manifest bundled in the app's resources.
pub const WHEELS_MANIFEST_FILE: &str = "python-runtime-wheels-manifest.json";

/// The file in an environment version directory naming the wheelhouse
/// archive digest installed into it.
const MARKER_FILE: &str = ".npi-wheelhouse";

/// Prefix of a staging directory under `<base>/archives/`.
const STAGING_PREFIX: &str = ".wheels-";

/// Makes staging directory names unique within one process.
static STAGING_COUNTER: AtomicU64 = AtomicU64::new(0);

/// `python-runtime-wheels-manifest.json`, as the build script writes it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WheelhouseManifest {
    pub suite: String,
    pub platform: String,
    pub archive_name: String,
    pub archive_sha256: String,
    pub archive_size: u64,
    /// The lock inside the archive, relative to its root.
    pub lock_file: String,
    /// The wheels folder inside the archive, relative to its root.
    pub wheels_dir: String,
    pub packages: Vec<PackageInfo>,
}

/// A verified-to-exist wheelhouse: its manifest and the archive beside it.
#[derive(Debug, Clone)]
pub struct Wheelhouse {
    pub manifest: WheelhouseManifest,
    pub archive: PathBuf,
}

impl Wheelhouse {
    /// The part of the environment version id that names this wheelhouse:
    /// `<suite>-<first 12 hex of the archive SHA-256>`.
    #[must_use]
    pub fn version_suffix(&self) -> String {
        format!(
            "{}-{}",
            self.manifest.suite,
            self.manifest.archive_sha256[..12].to_ascii_lowercase()
        )
    }

    /// Whether `env_dir` holds a finished install of this wheelhouse.
    #[must_use]
    pub fn is_installed_in(&self, env_dir: &Path) -> bool {
        fs::read_to_string(env_dir.join(MARKER_FILE)).is_ok_and(|raw| {
            raw.trim()
                .eq_ignore_ascii_case(&self.manifest.archive_sha256)
        })
    }
}

/// The bundled wheelhouse, from the resource directory or its `resources/`
/// subdirectory: `None` when the app ships no wheelhouse manifest.
///
/// # Errors
/// An unreadable or invalid manifest, a manifest for another platform than
/// the runtime's, or an archive missing from beside it.
pub fn load_wheelhouse(resource_dir: &Path, platform: &str) -> Result<Option<Wheelhouse>, String> {
    let Some(path) = beside(resource_dir, WHEELS_MANIFEST_FILE) else {
        return Ok(None);
    };
    let bytes = fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let manifest: WheelhouseManifest = serde_json::from_slice(&bytes)
        .map_err(|e| format!("{} is not a valid wheelhouse manifest: {e}", path.display()))?;
    validate(&manifest).map_err(|e| format!("{}: {e}", path.display()))?;
    if manifest.platform != platform {
        return Err(format!(
            "{} is for {}, but the bundled runtime is for {platform}",
            path.display(),
            manifest.platform
        ));
    }
    let archive = beside(resource_dir, &manifest.archive_name).ok_or_else(|| {
        format!(
            "bundled wheelhouse archive {} not found next to {}",
            manifest.archive_name,
            path.display()
        )
    })?;
    Ok(Some(Wheelhouse { manifest, archive }))
}

/// `name` in the resource directory or its `resources/` subdirectory.
fn beside(resource_dir: &Path, name: &str) -> Option<PathBuf> {
    [
        resource_dir.join(name),
        resource_dir.join("resources").join(name),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

/// Reject a manifest whose values could not have come from the build script:
/// they become a directory name and paths inside the staging directory.
fn validate(manifest: &WheelhouseManifest) -> Result<(), String> {
    let suite_ok = !manifest.suite.is_empty()
        && manifest
            .suite
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-');
    if !suite_ok {
        return Err(format!("unusable suite name `{}`", manifest.suite));
    }
    let digest = &manifest.archive_sha256;
    if digest.len() != 64 || !digest.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("unusable archive SHA-256 `{digest}`"));
    }
    if Path::new(&manifest.archive_name).components().count() != 1 {
        return Err(format!("unusable archive name `{}`", manifest.archive_name));
    }
    for inner in [&manifest.lock_file, &manifest.wheels_dir] {
        let relative = Path::new(inner)
            .components()
            .all(|part| matches!(part, Component::Normal(_)));
        if inner.is_empty() || !relative {
            return Err(format!("unusable path inside the archive `{inner}`"));
        }
    }
    Ok(())
}

/// Install `wheelhouse` into the environment at `env_dir` offline, then mark
/// the environment with the archive digest.
///
/// # Errors
/// The archive does not match the manifest, cannot be unpacked, lacks its
/// lock or wheels, or `uv pip install` fails or outlives `limit`.
pub fn install(
    paths: &EnvPaths,
    runtime_dir: &Path,
    env_dir: &Path,
    wheelhouse: &Wheelhouse,
    limit: Duration,
) -> Result<(), String> {
    let manifest = &wheelhouse.manifest;
    verify_archive(&wheelhouse.archive, manifest)?;
    let uv = runtime_uv_bin(runtime_dir);
    if !uv.is_file() {
        return Err(format!(
            "the Python runtime is not installed at {}",
            runtime_dir.display()
        ));
    }
    let staging = paths.archives_dir().join(format!(
        "{STAGING_PREFIX}{}-{}",
        std::process::id(),
        STAGING_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let installed = unpack_and_install(
        paths,
        runtime_dir,
        env_dir,
        wheelhouse,
        &uv,
        &staging,
        limit,
    );
    let _ = fs::remove_dir_all(&staging);
    installed?;
    crate::json_store::write_atomic(
        &env_dir.join(MARKER_FILE),
        format!("{}\n", manifest.archive_sha256).as_bytes(),
    )
    .map_err(|e| format!("could not mark {}: {e}", env_dir.display()))
}

/// The unpack and `uv` run of [`install`], whose caller removes `staging`
/// whatever happens.
fn unpack_and_install(
    paths: &EnvPaths,
    runtime_dir: &Path,
    env_dir: &Path,
    wheelhouse: &Wheelhouse,
    uv: &Path,
    staging: &Path,
    limit: Duration,
) -> Result<(), String> {
    let manifest = &wheelhouse.manifest;
    fs::create_dir_all(staging).map_err(|e| format!("cannot create {}: {e}", staging.display()))?;
    report_progress(
        "packages-install",
        &format!("unpacking the {} package suite", manifest.suite),
    );
    let file = File::open(&wheelhouse.archive)
        .map_err(|e| format!("cannot open {}: {e}", wheelhouse.archive.display()))?;
    Archive::new(GzDecoder::new(file))
        .unpack(staging)
        .map_err(|e| format!("cannot extract {}: {e}", wheelhouse.archive.display()))?;
    let lock = staging.join(&manifest.lock_file);
    let wheels = staging.join(&manifest.wheels_dir);
    if !lock.is_file() || !wheels.is_dir() {
        return Err(format!(
            "{} has no {} or {}",
            wheelhouse.archive.display(),
            manifest.lock_file,
            manifest.wheels_dir
        ));
    }

    let mut cmd = Command::new(uv);
    cmd.args(["pip", "install", "--python"])
        .arg(env_python_bin(env_dir))
        .args([
            "--offline",
            "--no-index",
            "--require-hashes",
            "--find-links",
        ])
        .arg(&wheels)
        .arg("--requirement")
        .arg(&lock)
        .arg("--cache-dir")
        .arg(paths.cache_uv_dir())
        .current_dir(env_dir);
    prepare_child_env(&mut cmd, paths, env_dir, runtime_dir)?;
    report_progress(
        "packages-install",
        &format!(
            "installing the {} package suite ({} packages) offline",
            manifest.suite,
            manifest.packages.len()
        ),
    );
    let run = run_bounded(&mut cmd, limit, "installing the bundled package suite")?;
    if !run.success {
        return Err(format!(
            "installing the bundled {} package suite failed ({})",
            manifest.suite,
            failure_text(&run)
        ));
    }
    Ok(())
}

/// Reject an archive that is not byte-for-byte the one the manifest
/// describes: its size first (cheap), then its SHA-256.
fn verify_archive(archive: &Path, manifest: &WheelhouseManifest) -> Result<(), String> {
    let size = fs::metadata(archive)
        .map_err(|e| format!("cannot stat {}: {e}", archive.display()))?
        .len();
    if size != manifest.archive_size {
        return Err(format!(
            "{} is {size} bytes, expected {} (archive does not match the manifest)",
            archive.display(),
            manifest.archive_size
        ));
    }
    let actual = super::installer::sha256_file(archive)?;
    if !actual.eq_ignore_ascii_case(&manifest.archive_sha256) {
        return Err(format!(
            "{} SHA-256 mismatch: expected {}, got {actual}",
            archive.display(),
            manifest.archive_sha256
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(super) mod tests {
    use std::io::Write;

    use flate2::write::GzEncoder;
    use flate2::Compression;

    use super::*;
    use crate::test_support::TempDir;

    const PLATFORM: &str = "aarch64-apple-darwin";

    /// A `.tar.gz` with `wheelhouse/requirements.lock` and one wheel.
    fn write_archive(path: &Path) {
        let mut builder = tar::Builder::new(GzEncoder::new(
            File::create(path).expect("archive"),
            Compression::default(),
        ));
        for (name, body) in [
            (
                "wheelhouse/requirements.lock",
                "demo==1.0 --hash=sha256:00\n",
            ),
            ("wheelhouse/wheels/demo-1.0-py3-none-any.whl", "wheel"),
        ] {
            let mut header = tar::Header::new_ustar();
            header.set_size(body.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, name, body.as_bytes())
                .expect("entry");
        }
        builder
            .into_inner()
            .expect("tar")
            .finish()
            .expect("gzip")
            .flush()
            .expect("flush");
    }

    /// A resource folder with an archive and the manifest describing it.
    pub(in crate::python_runtime) fn resources(dir: &Path) -> WheelhouseManifest {
        fs::create_dir_all(dir).expect("resources");
        let archive = dir.join("python-runtime-wheels-scientific-test.tar.gz");
        write_archive(&archive);
        let manifest = WheelhouseManifest {
            suite: "scientific".to_string(),
            platform: PLATFORM.to_string(),
            archive_name: "python-runtime-wheels-scientific-test.tar.gz".to_string(),
            archive_sha256: super::super::installer::sha256_file(&archive).expect("digest"),
            archive_size: fs::metadata(&archive).expect("size").len(),
            lock_file: "wheelhouse/requirements.lock".to_string(),
            wheels_dir: "wheelhouse/wheels".to_string(),
            packages: vec![PackageInfo {
                name: "demo".to_string(),
                version: "1.0".to_string(),
            }],
        };
        fs::write(
            dir.join(WHEELS_MANIFEST_FILE),
            serde_json::to_vec(&manifest).expect("json"),
        )
        .expect("manifest");
        manifest
    }

    #[test]
    fn a_build_without_the_suite_has_no_wheelhouse() {
        let dir = TempDir::new("python-wheels", "absent");
        assert!(load_wheelhouse(dir.path(), PLATFORM)
            .expect("no manifest is not an error")
            .is_none());
    }

    #[test]
    fn a_manifest_without_its_archive_is_an_error() {
        let dir = TempDir::new("python-wheels", "no-archive");
        let manifest = resources(dir.path());
        fs::remove_file(dir.path().join(&manifest.archive_name)).expect("remove");
        let error = load_wheelhouse(dir.path(), PLATFORM).expect_err("missing archive");
        assert!(error.contains("not found"), "error: {error}");
    }

    #[test]
    fn a_manifest_for_another_platform_or_with_escaping_paths_is_rejected() {
        let dir = TempDir::new("python-wheels", "invalid");
        resources(dir.path());
        let error = load_wheelhouse(dir.path(), "x86_64-apple-darwin").expect_err("platform");
        assert!(error.contains("x86_64-apple-darwin"), "error: {error}");

        let mut manifest = resources(dir.path());
        manifest.lock_file = "../outside.lock".to_string();
        fs::write(
            dir.path().join(WHEELS_MANIFEST_FILE),
            serde_json::to_vec(&manifest).expect("json"),
        )
        .expect("manifest");
        let error = load_wheelhouse(dir.path(), PLATFORM).expect_err("escaping path");
        assert!(error.contains("outside.lock"), "error: {error}");
    }

    #[cfg(unix)]
    #[test]
    fn install_runs_uv_offline_from_the_unpacked_archive_and_marks_the_environment() {
        let dir = TempDir::new("python-wheels", "install");
        resources(&dir.path().join("res"));
        let wheelhouse = load_wheelhouse(&dir.path().join("res"), PLATFORM)
            .expect("load")
            .expect("a wheelhouse");
        let paths = EnvPaths::resolve(Some(&dir.path().join("env")));
        let runtime = dir.path().join("runtime");
        let log = dir.path().join("uv-args");
        // The stand-in uv records its arguments and fails unless the lock it
        // is given was unpacked, as the real one would.
        crate::test_support::executable(
            &runtime_uv_bin(&runtime),
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" > '{}'\n\
                 while [ $# -gt 0 ]; do\n\
                   if [ \"$1\" = --requirement ]; then test -f \"$2\" || exit 3; fi\n\
                   shift\n\
                 done\n",
                log.display()
            ),
        );
        let env_dir = dir.path().join("venv");
        fs::create_dir_all(&env_dir).expect("env");
        assert!(!wheelhouse.is_installed_in(&env_dir));

        install(
            &paths,
            &runtime,
            &env_dir,
            &wheelhouse,
            Duration::from_secs(30),
        )
        .expect("install");

        let args = fs::read_to_string(&log).expect("uv ran");
        for flag in [
            "pip install --python ",
            "--offline",
            "--no-index",
            "--require-hashes",
        ] {
            assert!(args.contains(flag), "missing {flag}: {args}");
        }
        assert!(args.contains("wheelhouse/wheels"), "args: {args}");
        assert!(
            args.contains("wheelhouse/requirements.lock"),
            "args: {args}"
        );
        assert!(wheelhouse.is_installed_in(&env_dir));
        let leftovers = fs::read_dir(paths.archives_dir()).map_or(0, Iterator::count);
        assert_eq!(leftovers, 0, "the staging directory was left behind");
    }

    #[cfg(unix)]
    #[test]
    fn an_altered_archive_is_refused_before_uv_runs() {
        let dir = TempDir::new("python-wheels", "altered");
        let manifest = resources(&dir.path().join("res"));
        let wheelhouse = load_wheelhouse(&dir.path().join("res"), PLATFORM)
            .expect("load")
            .expect("a wheelhouse");
        let mut archive = fs::OpenOptions::new()
            .write(true)
            .open(dir.path().join("res").join(&manifest.archive_name))
            .expect("open");
        archive.write_all(b"x").expect("alter");
        let runtime = dir.path().join("runtime");
        let log = dir.path().join("uv-args");
        crate::test_support::executable(
            &runtime_uv_bin(&runtime),
            &format!("#!/bin/sh\ntouch '{}'\n", log.display()),
        );
        let env_dir = dir.path().join("venv");
        fs::create_dir_all(&env_dir).expect("env");

        let paths = EnvPaths::resolve(Some(&dir.path().join("env")));
        let error = install(
            &paths,
            &runtime,
            &env_dir,
            &wheelhouse,
            Duration::from_secs(30),
        )
        .expect_err("altered archive");
        assert!(error.contains("SHA-256 mismatch"), "error: {error}");
        assert!(!log.exists(), "uv ran on an unverified archive");
        assert!(!wheelhouse.is_installed_in(&env_dir));
    }
}
