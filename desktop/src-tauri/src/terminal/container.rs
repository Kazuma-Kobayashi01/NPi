//! The terminal of a tab whose commands run in the Sandbox VM.
//!
//! The VM plugin publishes `<temp>/npi-vm-routes/<omp pid>.json` (`{"helper": …}`) and its helper listens on
//! `<temp>/npi-vm-routes/<omp pid>.sock`. The terminal runs `<helper> attach <socket> --cwd <folder>` on its pty,
//! which relays the pty to an interactive bash in the guest, so resize, colours and exit codes work as for a
//! local shell. The pid is the one of the omp process this app started for the tab, so the webview never names a
//! program or a path; the plugin is the only writer and runs as this user. Everything is checked before use: an
//! owner-only folder and file, a helper that is an absolute executable, a socket that is a socket.
//!
//! The `npi` binary runs its agent in a child of the process the app starts (a launcher and a worker with the same
//! command line), and the plugin lives in the worker, so the route may be keyed by the started process or by any
//! of its descendants: `route` tries them all.

use std::path::{Path, PathBuf};

use serde::Deserialize;

const ROUTES_DIR: &str = "npi-vm-routes";

/// The program that connects a pty to the VM, and the socket it connects to.
#[derive(Debug, PartialEq, Eq)]
pub struct ContainerRoute {
    pub program: PathBuf,
    pub socket: PathBuf,
}

#[derive(Deserialize)]
struct RouteFile {
    helper: String,
}

const NOT_RUNNING: &str = "the Sandbox VM is not running for this tab (start it with /vm start)";

/// The route of the omp the app started as process `omp_pid`, or of a process below it.
///
/// # Errors
/// There is no VM for that process tree, or something about it is not private to this user.
pub fn route(omp_pid: u32) -> Result<ContainerRoute, String> {
    route_of(
        &std::env::temp_dir().join(ROUTES_DIR),
        &process_tree(omp_pid),
    )
}

/// The first route among `pids` that resolves. Without one, the error of the first pid that had a route file (a
/// route that is refused says why), else the plain "not running".
fn route_of(dir: &Path, pids: &[u32]) -> Result<ContainerRoute, String> {
    let mut first_error = None;
    for pid in pids {
        match route_in(dir, *pid) {
            Ok(route) => return Ok(route),
            Err(error) if error == NOT_RUNNING => {}
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    Err(first_error.unwrap_or_else(|| NOT_RUNNING.to_owned()))
}

/// `pid` and everything below it, parents first (bounded: a runaway tree is not followed forever).
fn process_tree(pid: u32) -> Vec<u32> {
    const MAX: usize = 64;
    let mut all = vec![pid];
    let mut next = 0;
    while next < all.len() && all.len() < MAX {
        let parent = all[next];
        next += 1;
        for child in children_of(parent) {
            if !all.contains(&child) && all.len() < MAX {
                all.push(child);
            }
        }
    }
    all
}

#[cfg(target_os = "macos")]
fn children_of(pid: u32) -> Vec<u32> {
    let Ok(parent) = libc::pid_t::try_from(pid) else {
        return Vec::new();
    };
    let mut buffer = vec![0 as libc::pid_t; 128];
    let bytes =
        libc::c_int::try_from(buffer.len() * std::mem::size_of::<libc::pid_t>()).unwrap_or(0);
    // SAFETY: `buffer` is writable for `bytes` bytes; libproc fills at most that many and returns how many pids.
    let count = unsafe { libc::proc_listchildpids(parent, buffer.as_mut_ptr().cast(), bytes) };
    let count = usize::try_from(count).unwrap_or(0).min(buffer.len());
    buffer[..count]
        .iter()
        .filter_map(|child| u32::try_from(*child).ok())
        .filter(|child| *child != 0)
        .collect()
}

#[cfg(not(target_os = "macos"))]
fn children_of(_pid: u32) -> Vec<u32> {
    Vec::new()
}

#[cfg(unix)]
fn route_in(dir: &Path, omp_pid: u32) -> Result<ContainerRoute, String> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};

    // SAFETY: `geteuid` has no preconditions and cannot fail.
    let uid = unsafe { libc::geteuid() };
    let private = |meta: &std::fs::Metadata, what: &str| -> Result<(), String> {
        if meta.uid() == uid && meta.permissions().mode() & 0o077 == 0 {
            Ok(())
        } else {
            Err(format!(
                "the VM terminal's {what} is not private to this user"
            ))
        }
    };

    let meta = std::fs::symlink_metadata(dir).map_err(|_| NOT_RUNNING.to_owned())?;
    if !meta.is_dir() {
        return Err("the VM terminal's route folder is not a folder".to_owned());
    }
    private(&meta, "route folder")?;

    let file = dir.join(format!("{omp_pid}.json"));
    let meta = std::fs::symlink_metadata(&file).map_err(|_| NOT_RUNNING.to_owned())?;
    if !meta.is_file() {
        return Err("the VM terminal's route is not a file".to_owned());
    }
    private(&meta, "route file")?;
    let text =
        std::fs::read_to_string(&file).map_err(|e| format!("read the VM terminal route: {e}"))?;
    let RouteFile { helper } = serde_json::from_str(&text)
        .map_err(|e| format!("the VM terminal route is unreadable: {e}"))?;

    let program = PathBuf::from(helper);
    let helper_meta = std::fs::metadata(&program)
        .ok()
        .filter(|meta| meta.is_file());
    match helper_meta {
        Some(meta) if program.is_absolute() && meta.permissions().mode() & 0o111 != 0 => {}
        _ => {
            return Err(format!(
                "the VM helper {} is not an executable file",
                program.display()
            ))
        }
    }

    let socket = dir.join(format!("{omp_pid}.sock"));
    let is_socket =
        std::fs::symlink_metadata(&socket).is_ok_and(|meta| meta.file_type().is_socket());
    if !is_socket {
        return Err(NOT_RUNNING.to_owned());
    }
    Ok(ContainerRoute { program, socket })
}

#[cfg(not(unix))]
fn route_in(_dir: &Path, _omp_pid: u32) -> Result<ContainerRoute, String> {
    Err("the Sandbox VM terminal is available on macOS only".to_owned())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;

    fn scratch(name: &str) -> PathBuf {
        // Short on purpose: a unix socket path must stay under SUN_LEN, which a long `$TMPDIR` would break.
        let dir = PathBuf::from("/tmp").join(format!("npi-rt-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        dir
    }

    fn publish(dir: &Path, pid: u32, helper: &str) -> UnixListener {
        let file = dir.join(format!("{pid}.json"));
        std::fs::write(&file, format!("{{\"helper\":\"{helper}\"}}")).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        UnixListener::bind(dir.join(format!("{pid}.sock"))).unwrap()
    }

    #[test]
    fn a_private_route_resolves_to_the_helper_and_its_socket() {
        let dir = scratch("ok");
        let _socket = publish(&dir, 4242, "/bin/sh");

        let route = route_in(&dir, 4242).unwrap();

        assert_eq!(route.program, PathBuf::from("/bin/sh"));
        assert_eq!(route.socket, dir.join("4242.sock"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_route_of_a_descendant_process_is_found() {
        let dir = scratch("tree");
        let _socket = publish(&dir, 5151, "/bin/sh");

        let route = route_of(&dir, &[5000, 5100, 5151]).unwrap();

        assert_eq!(route.socket, dir.join("5151.sock"));
        // A refused route is reported, not hidden behind the pids without one.
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(route_of(&dir, &[5000, 5151])
            .unwrap_err()
            .contains("not private"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_process_tree_lists_children_and_grandchildren() {
        let mut parent = std::process::Command::new("/bin/sh")
            .args(["-c", "/bin/sleep 20 & wait"])
            .spawn()
            .unwrap();
        let root = parent.id();
        let mut tree = Vec::new();
        for _ in 0..50 {
            tree = process_tree(root);
            if tree.len() >= 2 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let _ = parent.kill();
        let _ = parent.wait();

        assert_eq!(tree[0], root);
        assert!(tree.len() >= 2, "no child found below {root}: {tree:?}");
    }

    #[test]
    fn no_route_means_the_vm_is_not_running() {
        let dir = scratch("none");
        assert!(route_in(&dir, 1).unwrap_err().contains("not running"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_folder_others_can_read_is_refused() {
        let dir = scratch("open");
        let _socket = publish(&dir, 7, "/bin/sh");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();

        assert!(route_in(&dir, 7).unwrap_err().contains("not private"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_helper_that_is_not_executable_or_not_absolute_is_refused() {
        let dir = scratch("helper");
        let _socket = publish(&dir, 8, "relative/helper");
        assert!(route_in(&dir, 8).unwrap_err().contains("not an executable"));

        let plain = dir.join("plain.txt");
        std::fs::write(&plain, "x").unwrap();
        std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o644)).unwrap();
        std::fs::remove_file(dir.join("8.sock")).unwrap();
        let _socket = publish(&dir, 8, plain.to_str().unwrap());
        assert!(route_in(&dir, 8).unwrap_err().contains("not an executable"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stale_route_without_a_socket_is_not_running() {
        let dir = scratch("stale");
        let socket = publish(&dir, 9, "/bin/sh");
        drop(socket);
        std::fs::remove_file(dir.join("9.sock")).unwrap();

        assert!(route_in(&dir, 9).unwrap_err().contains("not running"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
