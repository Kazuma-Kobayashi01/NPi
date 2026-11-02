//! The one OS file lock that serialises every change to the Python tree.
//!
//! Two processes can find the runtime missing or an environment unusable at
//! the same moment — the app itself and an omp kernel (or two kernels) — and
//! both would otherwise extract the same archive or build the same
//! environment over each other. `state/install.lock` is opened and locked
//! exclusively by both the runtime installer and the environment builder, so
//! the second process waits and then sees the first process's result instead
//! of repeating it.
//!
//! The lock is the kernel's: [`File::lock`] blocks until every other handle
//! to the same path has released it, on every platform, and the kernel drops
//! it when the handle closes — including when the process dies mid-install.
//! The file itself is never deleted (deleting it would let a third process
//! lock a new inode while the first is still held).

use std::fs::{File, OpenOptions};
use std::path::Path;

/// An exclusive OS lock, released when the guard drops.
pub struct FileLock {
    /// Held for the lock's lifetime only.
    _file: File,
}

impl FileLock {
    /// Take the exclusive lock at `path`, creating its directory and the
    /// file when needed, and block until no other process holds it.
    ///
    /// # Errors
    /// When the directory or file cannot be created, or the lock cannot be
    /// taken.
    pub fn acquire(path: &Path) -> Result<Self, String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|e| format!("cannot open {}: {e}", path.display()))?;
        file.lock()
            .map_err(|e| format!("cannot lock {}: {e}", path.display()))?;
        Ok(Self { _file: file })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use std::fs::File;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn a_second_handle_cannot_take_a_held_lock() {
        // The installer and the environment builder rely on this: locks are
        // per open file handle, so another process's handle waits even
        // though it opened the same path.
        let dir = TempDir::new("python-lock", "exclusive");
        let path = dir.path().join("state/install.lock");
        let _held = FileLock::acquire(&path).expect("first lock");
        let second = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .expect("open");
        assert!(second.try_lock().is_err(), "the OS lock was not exclusive");
    }

    #[test]
    fn a_waiter_gets_the_lock_when_the_holder_drops_it() {
        let dir = TempDir::new("python-lock", "handoff");
        let path = dir.path().join("state/install.lock");
        let (acquired_tx, acquired_rx) = mpsc::channel();
        let holder = {
            let path = path.clone();
            std::thread::spawn(move || {
                let _lock = FileLock::acquire(&path).expect("holder");
                acquired_tx.send(()).expect("signal");
                std::thread::sleep(Duration::from_millis(150));
            })
        };
        acquired_rx.recv().expect("the holder took the lock");
        // Blocks until the holder's guard drops at the end of its thread.
        let _lock = FileLock::acquire(&path).expect("waiter");
        holder.join().expect("holder thread");
        assert!(File::open(&path).is_ok(), "the lock file is left in place");
    }
}
