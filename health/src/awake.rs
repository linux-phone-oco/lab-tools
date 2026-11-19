// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco-health awake`: the lab phone's keep-awake (backlog lpoco-awake), the kernel wake lock `lpoco-awake`.
//! WORKAROUND(sxmo-autosuspend-backoff): the lock stands in for sxmo backing off a failed suspend; without the fix its retry every second drops the lab's USB link.
//!
//! Carries surya-lab's lab/devlab-usb-awake into the package. On USB a suspend cannot complete, and one that
//! does while the port blips leaves the idle phone silent; so the lock is held always, and sxmo's autosuspend
//! (which blocks on /sys/power/wakeup_count) never suspends the phone. Deliberate suspends (`suspend cycle`)
//! write /sys/power/state themselves and are not held back. With /etc/lpoco/allow-autosleep (a battery test)
//! the lock is held only while the UDC is configured, or while a gate the manifest named (`awake_gates` in
//! [health], AWAKE_GATES in health.env) says the phone is unsafe to suspend. A gate this package does not
//! know is named on stderr; where the gates decide (allow-autosleep) it is refused and nothing is held, and
//! where the lock is held always the phone stays awake and only the known gates are read.
//!
//! Every path lies under LPOCO_HEALTH_ROOT (empty on the phone).

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const LOCK: &str = "lpoco-awake";
const ENV: &str = "etc/lpoco-health/health.env";
const ALLOW: &str = "etc/lpoco/allow-autosleep";

/// The gates this package knows, by the token a manifest names.
const GATES: [&str; 1] = ["bluetooth-not-up"];

fn unquote(v: &str) -> String {
    let v = v.trim();
    match v.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')) {
        Some(inner) => inner.replace("'\\''", "'"),
        None => v.to_string(),
    }
}

/// The gates the env file names, in order.
fn named_gates(env_text: &str) -> Vec<String> {
    env_text
        .lines()
        .filter_map(|l| l.strip_prefix("AWAKE_GATES="))
        .next_back()
        .map(|v| unquote(v).split_whitespace().map(str::to_string).collect())
        .unwrap_or_default()
}

/// Gates named that this package does not know.
fn unknown(gates: &[String]) -> Vec<&str> {
    gates
        .iter()
        .map(String::as_str)
        .filter(|g| !GATES.contains(g))
        .collect()
}

fn udc_configured(root: &Path) -> bool {
    let Ok(dir) = fs::read_dir(root.join("sys/class/udc")) else {
        return false;
    };
    let mut names: Vec<_> = dir.flatten().map(|e| e.path()).collect();
    names.sort();
    names
        .first()
        .and_then(|p| fs::read_to_string(p.join("state")).ok())
        .is_some_and(|s| s.trim() == "configured")
}

/// hci0 not UP, or an rfkill process stuck in D state (surya's r4 wedge).
fn bluetooth_not_up(root: &Path) -> bool {
    let up = Command::new("hciconfig")
        .arg("hci0")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("UP RUNNING"))
        .unwrap_or(false);
    if !up {
        return true;
    }
    let Ok(procs) = fs::read_dir(root.join("proc")) else {
        return false;
    };
    procs.flatten().any(|e| {
        let Ok(stat) = fs::read_to_string(e.path().join("stat")) else {
            return false;
        };
        // "PID (comm) STATE ...": comm may hold spaces and parentheses, so cut at the last ')'
        let (Some(open), Some(close)) = (stat.find('('), stat.rfind(')')) else {
            return false;
        };
        stat[open + 1..close] == *"rfkill" && stat[close + 1..].trim_start().starts_with('D')
    })
}

fn unsafe_by(gate: &str, root: &Path) -> bool {
    match gate {
        "bluetooth-not-up" => bluetooth_not_up(root),
        _ => false,
    }
}

/// Whether the lock is wanted now.
fn wanted(root: &Path, gates: &[String]) -> bool {
    !root.join(ALLOW).exists() || udc_configured(root) || gates.iter().any(|g| unsafe_by(g, root))
}

fn step(root: &Path, gates: &[String]) -> std::io::Result<bool> {
    let want = wanted(root, gates);
    if want {
        fs::write(root.join("sys/power/wake_lock"), LOCK)?;
    } else {
        match fs::write(root.join("sys/power/wake_unlock"), LOCK) {
            // the kernel answers EINVAL (or ENOENT) for a lock it never saw: already released
            Err(e) if matches!(e.kind(), ErrorKind::InvalidInput | ErrorKind::NotFound) => {}
            other => other?,
        }
    }
    Ok(want)
}

/// Run: one step (`once`), or every five seconds until stopped (the unit lets the lock go when it ends).
pub fn run(root: &Path, once: bool) -> i32 {
    let env_text = match fs::read_to_string(root.join(ENV)) {
        Ok(t) => t,
        Err(e) if e.kind() == ErrorKind::NotFound => String::new(),
        Err(e) => {
            eprintln!("lpoco-health: awake: cannot read {ENV}: {e}");
            return 2;
        }
    };
    let mut gates = named_gates(&env_text);
    let bad: Vec<String> = unknown(&gates).iter().map(|s| s.to_string()).collect();
    if !bad.is_empty() {
        eprintln!(
            "lpoco-health: awake: no gate is named {} (this package knows: {})",
            bad.join(", "),
            GATES.join(", ")
        );
        // Where the gates decide, a manifest this package cannot read holds nothing. Where the lock is
        // held always they decide nothing: the phone stays awake, and the known gates are all that is read.
        if root.join(ALLOW).exists() {
            return 2;
        }
        gates.retain(|g| !bad.contains(g));
    }
    loop {
        if let Err(e) = step(root, &gates) {
            eprintln!("lpoco-health: awake: cannot write the wake lock: {e}");
            if once {
                return 2;
            }
        }
        if once {
            return 0;
        }
        std::thread::sleep(Duration::from_secs(5));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_env_names_the_gates_as_devlab_writes_them() {
        assert_eq!(named_gates("X='1'\nAWAKE_GATES='a b'\n"), vec!["a", "b"]);
        assert!(named_gates("AWAKE_GATES=''\n").is_empty());
        assert!(named_gates("").is_empty());
    }

    #[test]
    fn a_gate_nobody_knows_is_named() {
        let g = vec!["bluetooth-not-up".to_string(), "sunspots".to_string()];
        assert_eq!(unknown(&g), vec!["sunspots"]);
    }
}
