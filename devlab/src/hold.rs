// SPDX-License-Identifier: MIT OR Apache-2.0
//! One change to the phone at a time, whoever asks - every session on the lab, the command line and the
//! MCP door alike: an exclusive lock on STATE/phone.lock, held until the command ends (the lock goes with
//! the process). A second change waits for the first, and says that it waits. Reading needs no hold.

use crate::device::Device;
use anyhow::Result;
use std::fs::{File, OpenOptions};
use std::path::Path;

pub fn hold(device: &Device) -> Result<File> {
    hold_in(&device.state_dir())
}

/// The same hold on the lock file under DIR (the test gives it a folder of its own).
fn hold_in(dir: &Path) -> Result<File> {
    std::fs::create_dir_all(dir)?;
    let f = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("phone.lock"))?;
    if f.try_lock().is_err() {
        eprintln!("devlab: another change to the phone is under way - waiting for it to end");
        f.lock()?;
    }
    Ok(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_phone_is_held_by_one_change_at_a_time() {
        // a folder of its own: the lab user's real state folder may be held by another run of these tests
        let dir = std::env::temp_dir().join(format!("devlab-hold-guard-{}", std::process::id()));
        let first = hold_in(&dir).unwrap();
        let second = OpenOptions::new()
            .write(true)
            .open(dir.join("phone.lock"))
            .unwrap();
        assert!(
            second.try_lock().is_err(),
            "held while the first change runs"
        );
        drop(first);
        assert!(second.try_lock().is_ok(), "free once it ends");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
