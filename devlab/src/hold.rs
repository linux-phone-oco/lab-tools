//! One change to the phone at a time, whoever asks - every session on the lab, the command line and the
//! MCP door alike: an exclusive lock on STATE/phone.lock, held until the command ends (the lock goes with
//! the process). A second change waits for the first, and says that it waits. Reading needs no hold.

use crate::device::Device;
use anyhow::Result;
use std::fs::{File, OpenOptions};

pub fn hold(device: &Device) -> Result<File> {
    std::fs::create_dir_all(device.state_dir())?;
    let f = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(device.state_dir().join("phone.lock"))?;
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
        let d: Device = toml::from_str(
            "[device]\ncodename = \"devlab-hold-guard\"\n[connection]\nuser = \"u\"\naddresses = []",
        )
        .unwrap();
        let first = hold(&d).unwrap();
        let second = OpenOptions::new()
            .write(true)
            .open(d.state_dir().join("phone.lock"))
            .unwrap();
        assert!(
            second.try_lock().is_err(),
            "held while the first change runs"
        );
        drop(first);
        assert!(second.try_lock().is_ok(), "free once it ends");
        let _ = std::fs::remove_dir_all(d.state_dir());
    }
}
