// SPDX-License-Identifier: MIT OR Apache-2.0
//! The host's one lock: `prepare now` and `follow now` change the same programs and files, so a second run waits
//! (follow) or is refused (prepare) while the first is at work. The lock is an advisory `flock` on a file: the kernel
//! gives it back when the run ends however it ends (a kill, a power cut), so no stale lock is ever left to judge. The
//! file also names the holder's process id, for the message only.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::Write;
use std::path::Path;

/// Where the lock lies.
pub fn lock_path(home: &str) -> String {
    format!("{home}/.local/state/lpoco-host/lock")
}

/// The lock while its run holds it; dropped, or the run ending, gives it back.
pub struct Lock {
    _file: File,
}

/// Why the lock was not taken.
pub enum Refused {
    /// Another run holds it: its process id as it wrote it (0 when it had not yet).
    Held(u32),
    /// The lock could not be asked for: nothing says whether another run is at work.
    Failed(String),
}

impl Refused {
    pub fn said(&self, path: &str) -> String {
        match self {
            Refused::Held(pid) => {
                format!("another lpoco-host run (pid {pid}) holds the lock {path}")
            }
            Refused::Failed(why) => format!("cannot take the lock {path}: {why}"),
        }
    }
}

/// Takes the lock for this process, or says why not.
pub fn take(home: &str) -> Result<Lock, Refused> {
    let path = lock_path(home);
    let failed = |what: &str, e: std::io::Error| Refused::Failed(format!("{what}: {e}"));
    if let Some(dir) = Path::new(&path).parent() {
        fs::create_dir_all(dir).map_err(|e| failed("make its folder", e))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|e| failed("open it", e))?;
    match file.try_lock() {
        Ok(()) => {
            file.set_len(0).map_err(|e| failed("write it", e))?;
            writeln!(file, "{}", std::process::id()).map_err(|e| failed("write it", e))?;
            Ok(Lock { _file: file })
        }
        Err(TryLockError::WouldBlock) => Err(Refused::Held(
            fs::read_to_string(&path)
                .ok()
                .and_then(|t| t.trim().parse().ok())
                .unwrap_or(0),
        )),
        Err(TryLockError::Error(e)) => Err(failed("lock it", e)),
    }
}
