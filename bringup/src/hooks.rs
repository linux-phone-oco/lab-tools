// SPDX-License-Identifier: MIT OR Apache-2.0
//! Step hooks: a device's manifest names an executable for a step (`[boot.hooks] parts = "hook.sh"`), and the
//! step calls it with the part paths as arguments. A hook that is missing or exits non-zero stops the job.

use std::path::Path;
use std::process::Command;

/// Run `exe` (relative paths are the manifest folder's) with `args`; Err in one line when it cannot start or fails.
pub fn run(step: &str, exe: &str, manifest_dir: &Path, args: &[&Path]) -> Result<(), String> {
    let path = manifest_dir.join(exe);
    if !path.is_file() {
        return Err(format!(
            "hook {step}: {exe} is not there ({})",
            path.display()
        ));
    }
    let status = Command::new(&path)
        .args(args)
        .status()
        .map_err(|e| format!("hook {step}: cannot run {exe}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("hook {step}: {exe} {status}"))
    }
}
