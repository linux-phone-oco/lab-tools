// SPDX-License-Identifier: MIT OR Apache-2.0
//! The device a procedure is for: its `device.toml`, found and checked once, before any step or the log. The
//! values a step needs are read from the manifest's own tables (decision
//! 2026-11-19-procedure-values-from-the-named-device).

use lpoco_manifest::Manifest;
use std::path::PathBuf;

const FILE: &str = "device.toml";

/// The named device's manifest file: `given` (`--device`), or the first device.toml from the current folder
/// upwards (as devlab finds it). It must parse as the declared manifest and be the named device's: a procedure
/// for one phone never runs against another's file. Only the schema and the codename are checked here; devlab
/// judges the rest (its connection) when a step reaches the phone.
pub fn load(given: Option<&str>, device: &str) -> Result<PathBuf, String> {
    let path = match given {
        Some(p) => PathBuf::from(p),
        None => find_upwards(device)?,
    };
    let path = std::path::absolute(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let m = Manifest::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let codename = m.device.as_ref().map(|d| d.codename.as_str());
    if codename != Some(device) {
        return Err(format!(
            "{} is for {}, the procedure is for {device}",
            path.display(),
            codename.unwrap_or("no device ([device] codename is missing)")
        ));
    }
    Ok(path)
}

fn find_upwards(device: &str) -> Result<PathBuf, String> {
    let start = std::env::current_dir()
        .map_err(|e| format!("cannot tell the current folder to look for {FILE} in: {e}"))?;
    start
        .ancestors()
        .map(|d| d.join(FILE))
        .find(|f| f.is_file())
        .ok_or_else(|| {
            format!(
                "no {FILE} here or above {} for {device} (or pass --device FILE)",
                start.display()
            )
        })
}
