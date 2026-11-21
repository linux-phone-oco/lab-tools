// SPDX-License-Identifier: MIT OR Apache-2.0
//! The phone's keeper: the one session that owns the phone's flow, taking up its queue (docs: tad q
//! phone-queue) one change at a time. devlab still holds the phone for every change (hold.rs); the lease
//! says which session is the keeper, so that two keepers never run - one taken by a waker that finds the
//! queue with work and no keeper, one started by hand.
//!
//!   devlab keeper take NAME      take the lease (exit 1, saying who holds it, when another keeper is live)
//!   devlab keeper renew NAME     a keeper renews it as it works: one left unrenewed for LAPSE has ended
//!   devlab keeper release NAME   a keeper that ends gives it back: found by its name from any folder, no device.toml needed
//!   devlab keeper status         who holds it, and since when it was renewed; or free
//!
//! The lease is a directory, STATE/keeper.lease (made atomically: two takers cannot both have it),
//! holding the keeper's name; its age is the time since its last renewal.

use crate::device::Device;
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// A keeper that has not renewed its lease for this long has ended (a session that stopped, or was cut off).
pub const LAPSE: Duration = Duration::from_secs(90 * 60);

fn lease(device: &Device) -> PathBuf {
    device.state_dir().join("keeper.lease")
}

/// Who holds the lease at `dir`, and how long since it was renewed.
fn holder(dir: &Path) -> Option<(String, Duration)> {
    let name = std::fs::read_to_string(dir.join("holder")).ok()?;
    let age = std::fs::metadata(dir.join("holder"))
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .unwrap_or_default();
    Some((name.trim().to_string(), age))
}

fn valid(name: &str) -> Result<()> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        bail!("{name:?}: a keeper's name is letters, digits, - _ and . (its session's name)");
    }
    Ok(())
}

pub fn take_at(dir: &Path, name: &str, lapse: Duration) -> Result<String> {
    valid(name)?;
    std::fs::create_dir_all(dir.parent().unwrap_or(Path::new(".")))?;
    for _ in 0..2 {
        if std::fs::create_dir(dir).is_ok() {
            std::fs::write(dir.join("holder"), format!("{name}\n"))?;
            return Ok(format!("{name} keeps the phone"));
        }
        match holder(dir) {
            Some((who, _)) if who == name => {
                std::fs::write(dir.join("holder"), format!("{name}\n"))?;
                return Ok(format!(
                    "{name} keeps the phone (it held the lease already)"
                ));
            }
            Some((who, age)) if age < lapse => {
                bail!(
                    "{who} keeps the phone (renewed {} min ago) - one keeper at a time",
                    age.as_secs() / 60
                )
            }
            // lapsed, or a lease half made: it is taken away, and taken
            _ => {
                let _ = std::fs::remove_dir_all(dir);
            }
        }
    }
    bail!("the lease could not be taken")
}

pub fn renew_at(dir: &Path, name: &str) -> Result<String> {
    match holder(dir) {
        Some((who, _)) if who == name => {
            std::fs::write(dir.join("holder"), format!("{name}\n"))?;
            Ok(format!("{name} keeps the phone (renewed)"))
        }
        Some((who, _)) => bail!("{who} holds the lease, not {name}"),
        None => bail!("nobody holds the lease: devlab keeper take {name}"),
    }
}

/// Gives back every lease under `root` (one folder per codename) that `name` holds; an error naming the leases
/// held when it holds none. No device.toml is needed: a keeper whose worktree is gone still releases.
pub fn release_named_at(root: &Path, name: &str) -> Result<String> {
    valid(name)?;
    let mut held = Vec::new();
    let mut mine = Vec::new();
    let mut codenames: Vec<_> = std::fs::read_dir(root)
        .map(|d| {
            d.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    codenames.sort();
    for code in codenames {
        let dir = root.join(&code).join("keeper.lease");
        if let Some((who, _)) = holder(&dir) {
            if who == name {
                std::fs::remove_dir_all(&dir)?;
                mine.push(code);
            } else {
                held.push(format!("{code}: {who}"));
            }
        }
    }
    if !mine.is_empty() {
        return Ok(format!("{name} gave back the lease of {}", mine.join(", ")));
    }
    if held.is_empty() {
        bail!("{name} holds no lease: no lease is held");
    }
    bail!("{name} holds no lease; leases held: {}", held.join("; "))
}

pub fn status_at(dir: &Path, lapse: Duration) -> String {
    match holder(dir) {
        Some((who, age)) if age < lapse => {
            format!("kept by {who}, renewed {} min ago", age.as_secs() / 60)
        }
        Some((who, age)) => format!(
            "free: {who}'s lease lapsed {} min ago",
            age.saturating_sub(lapse).as_secs() / 60
        ),
        None => "free".into(),
    }
}

pub fn take(device: &Device, name: &str) -> Result<String> {
    take_at(&lease(device), name, LAPSE)
}
pub fn renew(device: &Device, name: &str) -> Result<String> {
    renew_at(&lease(device), name)
}
pub fn release(name: &str) -> Result<String> {
    release_named_at(&crate::device::state_root(), name)
}
pub fn status(device: &Device) -> String {
    status_at(&lease(device), LAPSE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_keeper_at_a_time_and_a_lapsed_one_gives_way() {
        let dir = std::env::temp_dir().join(format!("devlab-keeper-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let l = dir.join("a-phone/keeper.lease");
        let hour = Duration::from_secs(3600);
        assert_eq!(status_at(&l, hour), "free");
        assert!(take_at(&l, "keeper-1", hour).is_ok());
        let no = take_at(&l, "keeper-2", hour).unwrap_err().to_string();
        assert!(no.contains("keeper-1 keeps the phone"), "{no}");
        assert!(
            take_at(&l, "keeper-1", hour).is_ok(),
            "taking it again is renewing"
        );
        assert!(renew_at(&l, "keeper-2").is_err());
        assert!(release_named_at(&dir, "keeper-2").is_err());
        assert!(status_at(&l, hour).starts_with("kept by keeper-1"));
        // a lease not renewed within the lapse is taken by the next keeper
        assert!(take_at(&l, "keeper-2", Duration::ZERO).is_ok());
        assert!(status_at(&l, hour).starts_with("kept by keeper-2"));
        assert!(release_named_at(&dir, "keeper-2").is_ok());
        assert_eq!(status_at(&l, hour), "free");
        assert!(take_at(&l, "../x", hour).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
