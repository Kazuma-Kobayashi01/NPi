//! The embedded Python runtime and the managed environment built on it.
//!
//! `paths` (where everything lives), `manifest` (the bundled runtime's
//! descriptor and the installed state), `lock` (the one OS file lock),
//! `installer` (verifying, unpacking and proving a runtime), `command`
//! (bounded helper runs), `environment` (the versioned virtualenv and its
//! packages) and `manager` (the entry points).
//!
//! Two front doors share those entry points:
//!
//! - the Tauri commands below — status, ensure and the package list — for
//!   the settings pane. They run on a blocking thread: the first call unpacks
//!   a ~38 MB archive and every call may start child processes, neither of
//!   which may hold the event loop that draws the window.
//! - [`run_host`], the `--npi-python-host` re-entry of the app executable:
//!   one JSON request on stdin, one JSON result on stdout, errors on stderr
//!   with a non-zero exit. That is how omp gets an interpreter and installs
//!   packages; the host is the *only* installer, so omp never unpacks an
//!   archive or builds a venv of its own.
//!
//! Nothing here executes user code: `python`'s tool runs its own supervisor
//! over the interpreter `ensure` reports.

pub mod command;
pub mod environment;
pub mod installer;
pub mod lock;
pub mod manager;
pub mod manifest;
pub mod paths;

use std::path::{Path, PathBuf};

use serde::Deserialize;
use tauri::Manager;

pub use environment::{EnvironmentHealth, PackageInfo};
pub use manager::{PythonRuntimeManager, RuntimeStatus};

/// The app bundle's resource folder, where the runtime manifest and archive
/// ship; `None` when Tauri cannot report it (then the dev fallback applies).
fn resources(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.path().resource_dir().ok()
}

/// Run `work` on a blocking thread and join it: extraction and child
/// processes must not block the async runtime's threads either.
async fn blocking<T, F>(work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| format!("join error: {e}"))?
}

/// The runtime's state without unpacking anything: for the settings pane and
/// for deciding whether to offer `python_runtime_ensure`.
///
/// # Errors
/// Only when the blocking task cannot be joined; the status read itself
/// reports a missing install in its value.
#[tauri::command]
pub async fn python_runtime_status() -> Result<RuntimeStatus, String> {
    blocking(|| Ok(PythonRuntimeManager::status(None))).await
}

/// Install the runtime if needed, build the managed environment, and report
/// its health. Idempotent: after the first call this reads the state manifest
/// and runs the version probes.
///
/// # Errors
/// When the resource folder has no matching bundle, verification or extraction
/// fails, the environment cannot be built, or its health check fails.
#[tauri::command]
pub async fn python_runtime_ensure(app: tauri::AppHandle) -> Result<EnvironmentHealth, String> {
    let dir = PythonRuntimeManager::resolve_resource_dir(resources(&app))?;
    blocking(move || PythonRuntimeManager::ensure_ready(&dir, None)).await
}

/// The managed environment's installed distributions, after making sure the
/// runtime and its environment exist.
///
/// # Errors
/// When deployment fails or `uv pip list` cannot be read.
#[tauri::command]
pub async fn python_runtime_packages(app: tauri::AppHandle) -> Result<Vec<PackageInfo>, String> {
    let dir = PythonRuntimeManager::resolve_resource_dir(resources(&app))?;
    blocking(move || PythonRuntimeManager::packages(&dir, None)).await
}

/// One progress line for the supervisor watching the host.
///
/// A single-line record on stderr, which the host's stdout result never
/// shares. Lines look like
/// `npi-python-progress {"stage":"extracting","message":"..."}`; an omp child
/// ignores anything else on stderr.
pub fn report_progress(stage: &str, message: &str) {
    let record = serde_json::json!({
        "stage": stage,
        "message": message.replace(['\n', '\r'], " "),
    });
    eprintln!("npi-python-progress {record}");
}

/// One request of the `--npi-python-host` protocol. Field names are the
/// camelCase ones omp sends; an unknown field is a protocol error rather than
/// a silently ignored typo.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HostRequest {
    action: String,
    /// `list` (the default) or `install`, for `action: "packages"`.
    #[serde(default)]
    operation: Option<String>,
    /// Requirement strings for `operation: "install"`.
    #[serde(default)]
    packages: Vec<String>,
    /// The folder holding the manifest and archive, overriding the runtime's
    /// own discovery.
    #[serde(default)]
    resource_dir: Option<PathBuf>,
}

/// The `--npi-python-host` entry point: read one JSON request from stdin,
/// write one JSON result to stdout, and return the error (for `main` to print
/// and exit non-zero with) when it cannot be answered.
///
/// # Errors
/// A malformed request, an unknown action or operation, an unusable
/// `resourceDir`, or a failed install/build/package operation. Nothing is
/// written to stdout in that case.
pub fn run_host() -> Result<(), String> {
    let mut raw = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut raw)
        .map_err(|e| host_error(format!("could not read the request: {e}")))?;
    let response = handle_host_request(&raw)?;
    println!("{response}");
    Ok(())
}

/// Answer one host request, returning exactly one JSON value serialized on
/// one line.
fn handle_host_request(raw: &str) -> Result<String, String> {
    let request: HostRequest =
        serde_json::from_str(raw).map_err(|e| host_error(format!("invalid request: {e}")))?;
    match request.action.as_str() {
        "ensure" => {
            let dir = host_resource_dir(request.resource_dir.as_deref())?;
            let health = PythonRuntimeManager::ensure_ready(&dir, None).map_err(host_error)?;
            json_line(&health)
        }
        "environment" => {
            let health = PythonRuntimeManager::environment(None);
            json_line(&health)
        }
        "packages" => {
            let operation = request.operation.as_deref().unwrap_or("list");
            let dir = host_resource_dir(request.resource_dir.as_deref())?;
            let packages = match operation {
                "list" => {
                    if !request.packages.is_empty() {
                        return Err(host_error(
                            "operation `list` does not accept package specifiers",
                        ));
                    }
                    PythonRuntimeManager::packages(&dir, None).map_err(host_error)?
                }
                "install" => {
                    if request.packages.is_empty() {
                        return Err(host_error(
                            "operation `install` needs at least one package specifier",
                        ));
                    }
                    PythonRuntimeManager::install(&dir, None, &request.packages)
                        .map_err(host_error)?
                }
                other => {
                    return Err(host_error(format!(
                        "unknown packages operation `{other}` (expected list or install)"
                    )));
                }
            };
            json_line(&packages)
        }
        other => Err(host_error(format!(
            "unknown action `{other}` (expected ensure, environment or packages)"
        ))),
    }
}

/// The resource folder for one request: an explicit `resourceDir` must be an
/// absolute existing directory; otherwise the executable's own resources or
/// `NPI_PYTHON_RESOURCE_DIR`.
fn host_resource_dir(explicit: Option<&Path>) -> Result<PathBuf, String> {
    match explicit {
        Some(dir) if dir.is_absolute() && dir.is_dir() => Ok(dir.to_path_buf()),
        Some(dir) => Err(host_error(format!(
            "resourceDir {} is not an absolute existing directory",
            dir.display()
        ))),
        None => PythonRuntimeManager::resolve_host_resource_dir().map_err(host_error),
    }
}

/// Serialize a response for stdout: one line, no embedded newlines.
fn json_line(value: &impl serde::Serialize) -> Result<String, String> {
    serde_json::to_string(value)
        .map_err(|e| host_error(format!("could not encode the result: {e}")))
}

/// Prefix an error the way the host prints it: a single line main forwards to
/// stderr, distinguishable from progress lines.
fn host_error(message: impl Into<String>) -> String {
    format!(
        "npi-python-error: {}",
        message.into().replace(['\n', '\r'], " ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn error_of(request: &str) -> String {
        handle_host_request(request).expect_err("a request error")
    }

    #[test]
    fn malformed_requests_are_protocol_errors() {
        let error = error_of("not json");
        assert!(error.starts_with("npi-python-error: "), "{error}");
        assert!(error.contains("invalid request"), "{error}");

        let error = error_of(r#"{"action":"ensure","extra":true}"#);
        assert!(error.contains("invalid request"), "{error}");
        assert!(error.contains("extra"), "{error}");
    }

    #[test]
    fn unknown_actions_and_operations_are_rejected() {
        let error = error_of(r#"{"action":"uninstall"}"#);
        assert!(error.contains("unknown action"), "{error}");
        let error = error_of(r#"{"action":"packages","operation":"freeze"}"#);
        assert!(error.contains("unknown packages operation"), "{error}");
        assert!(!error.contains('\n'), "{error}");
    }

    #[test]
    fn package_operations_validate_their_arguments() {
        let dir = crate::test_support::TempDir::new("python-host", "args");
        let request = |operation: &str, packages: &str| {
            format!(
                r#"{{"action":"packages","operation":"{operation}","packages":{packages},"resourceDir":"{}"}}"#,
                dir.path().display()
            )
        };
        let error = error_of(&request("install", "[]"));
        assert!(error.contains("at least one package"), "{error}");
        let error = error_of(&request("list", r#"["demo"]"#));
        assert!(error.contains("does not accept"), "{error}");
    }

    #[test]
    fn a_relative_resource_dir_is_rejected() {
        let error = error_of(r#"{"action":"ensure","resourceDir":"relative/dir"}"#);
        assert!(error.contains("absolute existing directory"), "{error}");
    }

    /// A synthetic bundled runtime: an archive whose interpreter builds a
    /// stand-in venv, a manifest naming it, and a fake `uv` that lists one
    /// package and installs without touching the network.
    #[cfg(unix)]
    mod fixture {
        use super::super::*;
        use crate::test_support::{executable, TempDir};
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::fs;

        pub const PLATFORM: &str = "aarch64-apple-darwin";

        pub fn resources(temp: &TempDir) -> PathBuf {
            let resources = temp.path().join("resources");
            let tree = temp.path().join("tree");
            let runtime_python = "#!/bin/sh\n\
                root=$(cd \"$(dirname \"$0\")/..\" && pwd -P)\n\
                if [ \"$4\" = \"venv\" ]; then\n\
                  mkdir -p \"$6/bin\"\n\
                  cat > \"$6/bin/python\" <<EOF\n\
#!/bin/sh\n\
case \"\\$*\" in\n\
  *--version*) echo 'Python 3.12.16' ;;\n\
  *) echo '{\"prefix\": \"$6\", \"version\": \"3.12.16\", \"pip\": \"26.2.2\", \"ssl\": \"OpenSSL 3.5.9\"}' ;;\n\
esac\n\
EOF\n\
                  chmod 755 \"$6/bin/python\"\n\
                elif [ \"$3\" = \"-c\" ]; then\n\
                  echo \"{\\\"version\\\": \\\"3.12.16\\\", \\\"base_prefix\\\": \\\"$root\\\", \\\"stdlib\\\": \\\"$root/lib/python3.12\\\", \\\"ssl\\\": \\\"OpenSSL 3.5.9\\\"}\"\n\
                else\n\
                  echo 'Python 3.12.16'\n\
                fi\n\
                exit 0\n";
            executable(&tree.join("python/bin/python3"), runtime_python);
            executable(
                &tree.join("python/bin/uv"),
                "#!/bin/sh\ncase \"$*\" in *--version*) echo 'uv 0.13.6' ;; *list*) echo '[{\"name\":\"demo\",\"version\":\"1.0\"}]' ;; esac\nexit 0\n",
            );
            let stdlib = tree.join("python/lib/python3.12");
            fs::create_dir_all(&stdlib).expect("stdlib");
            fs::write(stdlib.join("os.py"), "# stdlib\n").expect("stdlib file");

            let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
            {
                let mut builder = tar::Builder::new(&mut encoder);
                builder
                    .append_dir_all("python", tree.join("python"))
                    .expect("tar");
                builder.finish().expect("tar finish");
            }
            let bytes = encoder.finish().expect("gzip");
            let archive_name = format!("python-runtime-3.12.16-{PLATFORM}.tar.gz");
            fs::create_dir_all(&resources).expect("resources dir");
            fs::write(resources.join(&archive_name), &bytes).expect("archive");
            let manifest = serde_json::json!({
                "runtimeVersion": "3.12.16",
                "platform": PLATFORM,
                "archiveName": archive_name,
                "archiveSha256": crate::json_store::hex_sha256(&bytes),
                "archiveSize": bytes.len(),
                "uvVersion": "0.13.6",
                "cpythonBuild": "cpython-test",
                "components": ["cpython", "stdlib", "pip", "uv"],
            });
            fs::write(
                resources.join(manifest::BUNDLED_MANIFEST_FILE),
                serde_json::to_vec_pretty(&manifest).expect("manifest"),
            )
            .expect("manifest file");
            resources
        }
    }

    #[cfg(unix)]
    #[test]
    fn the_host_protocol_ensures_reports_and_lists_packages() {
        use std::sync::Mutex;
        // `NPI_ENV_DIR` is process-global; this is the only test in the crate
        // that sets it.
        static ENV_LOCK: Mutex<()> = Mutex::new(());
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let temp = crate::test_support::TempDir::new("python-host", "protocol");
        let resources = fixture::resources(&temp);
        let env_base = temp.path().join("env");
        std::env::set_var("NPI_ENV_DIR", &env_base);

        let ensure = format!(
            r#"{{"action":"ensure","resourceDir":"{}"}}"#,
            resources.display()
        );
        let raw = handle_host_request(&ensure).expect("ensure");
        assert!(!raw.contains('\n'), "{raw}");
        let health: EnvironmentHealth = serde_json::from_str(&raw).expect("health json");
        assert!(health.ok, "message: {:?}", health.message);
        assert!(
            Path::new(&health.python_path).is_file(),
            "{:?}",
            health.python_path
        );
        assert!(health
            .python_path
            .contains("/environments/default/versions/"));
        assert_eq!(
            Path::new(&health.env_path),
            Path::new(&health.python_path)
                .parent()
                .and_then(Path::parent)
                .expect("env dir")
        );

        let environment = handle_host_request(r#"{"action":"environment"}"#).expect("environment");
        let report: EnvironmentHealth = serde_json::from_str(&environment).expect("health json");
        assert!(report.ok, "message: {:?}", report.message);
        assert_eq!(report.python_path, health.python_path);

        let list = format!(
            r#"{{"action":"packages","operation":"list","resourceDir":"{}"}}"#,
            resources.display()
        );
        let packages: Vec<PackageInfo> =
            serde_json::from_str(&handle_host_request(&list).expect("list")).expect("packages");
        assert_eq!(
            packages,
            vec![PackageInfo {
                name: "demo".to_string(),
                version: "1.0".to_string(),
            }]
        );

        let install = format!(
            r#"{{"action":"packages","operation":"install","packages":["demo"],"resourceDir":"{}"}}"#,
            resources.display()
        );
        let packages: Vec<PackageInfo> =
            serde_json::from_str(&handle_host_request(&install).expect("install"))
                .expect("packages");
        assert_eq!(packages[0].name, "demo");

        std::env::remove_var("NPI_ENV_DIR");
    }
}
