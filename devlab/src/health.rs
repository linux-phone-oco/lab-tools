//! Health: the device's checks, and what they say against everything that has worked before.
//! The baseline is every check that has ever passed; a regression is a baseline check that no
//! longer passes. Checks that never passed are known gaps, not alarms; a check that newly passes
//! joins the baseline by itself, so coverage grows as work lands.

use crate::device::Device;
use crate::phone::Phone;
use anyhow::{Context, Result};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct Check {
    pub key: String,
    pub outcome: String,
    pub detail: String,
}

/// The checks' JSON lines ({"test","component","outcome","detail"}); other lines are ignored.
pub fn parse(text: &str) -> Vec<Check> {
    text.lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| {
            Some(Check {
                key: format!("{}/{}", v["component"].as_str()?, v["test"].as_str()?),
                outcome: v["outcome"].as_str()?.to_string(),
                detail: v["detail"].as_str().unwrap_or("").to_string(),
            })
        })
        .collect()
}

/// Baseline checks that no longer pass, with what they say now.
pub fn regressions(baseline: &BTreeSet<String>, now: &[Check]) -> Vec<String> {
    let mut r: Vec<String> = now
        .iter()
        .filter(|c| baseline.contains(&c.key) && c.outcome != "pass")
        .map(|c| format!("{} ({}: {})", c.key, c.outcome, c.detail))
        .collect();
    let seen: BTreeSet<&str> = now.iter().map(|c| c.key.as_str()).collect();
    r.extend(
        baseline
            .iter()
            .filter(|k| !seen.contains(k.as_str()))
            .map(|k| format!("{k} (did not run)")),
    );
    r
}

/// The baseline after a run: what it held, and every check that passed now.
pub fn grown(baseline: &BTreeSet<String>, now: &[Check]) -> BTreeSet<String> {
    let mut b = baseline.clone();
    b.extend(
        now.iter()
            .filter(|c| c.outcome == "pass")
            .map(|c| c.key.clone()),
    );
    b
}

/// Runs the device's checks on the phone, as root; keeps the run in the state directory.
/// Seconds a phone needs after booting before its checks mean anything: services are still starting
/// and the audio processor is still loading. Read earlier, a check fails for the wrong reason.
pub const SETTLE_SECS: u64 = 90;

/// How long is still to wait, for a phone that has been up `up` seconds.
pub fn still_to_settle(up: u64) -> u64 {
    SETTLE_SECS.saturating_sub(up)
}

/// Waits until the phone has been up long enough for its checks to mean something - so nobody sleeps
/// before a health check by hand. An unreadable uptime does not wait: the checks will tell.
pub fn settle(phone: &Phone) {
    let mut said = false;
    while let Some(up) = phone.uptime() {
        let wait = still_to_settle(up);
        if wait == 0 {
            return;
        }
        if !said {
            eprintln!("waiting for the phone to settle (up {up} s of {SETTLE_SECS})");
            said = true;
        }
        std::thread::sleep(std::time::Duration::from_secs(wait.min(15)));
    }
}

pub fn run_checks(device: &Device, manifest_dir: &Path, phone: &Phone) -> Result<Vec<Check>> {
    settle(phone);
    let script_path = manifest_dir.join(&device.tests.checks);
    let script = std::fs::read_to_string(&script_path)
        .with_context(|| format!("reading {}", script_path.display()))?;
    let (_, out) = phone.root(&script)?;
    let dir = device.state_dir().join("results");
    std::fs::create_dir_all(&dir)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    std::fs::write(dir.join(format!("{stamp}.jsonl")), &out)?;
    Ok(parse(&out))
}

fn baseline_path(device: &Device) -> std::path::PathBuf {
    device.state_dir().join("baseline.txt")
}

pub fn load_baseline(device: &Device) -> BTreeSet<String> {
    std::fs::read_to_string(baseline_path(device))
        .map(|t| {
            t.lines()
                .map(str::to_string)
                .filter(|l| !l.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

pub fn save_baseline(device: &Device, b: &BTreeSet<String>) -> Result<()> {
    std::fs::create_dir_all(device.state_dir())?;
    std::fs::write(
        baseline_path(device),
        b.iter().map(|k| format!("{k}\n")).collect::<String>(),
    )?;
    Ok(())
}

/// The baseline a run is held to. A run that did not listen is not asked about what only listening
/// can tell: the heard checks are out of its scope, not regressions that "did not run".
pub fn in_scope(
    baseline: &BTreeSet<String>,
    heard_keys: &[String],
    listened: bool,
) -> BTreeSet<String> {
    if listened {
        return baseline.clone();
    }
    baseline
        .iter()
        .filter(|k| !heard_keys.contains(k))
        .cloned()
        .collect()
}

/// One health check: the regressions (empty when all is well); the baseline grows with new passes.
/// With `listen`, the lab's ear joins in: a speaker heard once is in the baseline, and silent after
/// that is a regression like any other.
pub fn health(
    device: &Device,
    manifest_dir: &Path,
    phone: &Phone,
    listen: bool,
) -> Result<Vec<String>> {
    let mut now = run_checks(device, manifest_dir, phone)?;
    if listen {
        let heard = crate::hear::hear(device, phone)?;
        let dir = device.state_dir().join("results");
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let lines: String = heard
            .iter()
            .map(|c| {
                let (component, test) = c.key.split_once('/').unwrap_or(("", &c.key));
                format!("{}\n", serde_json::json!({"test": test, "component": component, "outcome": c.outcome, "detail": c.detail}))
            })
            .collect();
        std::fs::write(dir.join(format!("{stamp}-hear.jsonl")), lines)?;
        now.extend(heard);
    }
    let base = load_baseline(device);
    let heard_keys: Vec<String> = device
        .hear
        .tones()
        .iter()
        .map(|t| format!("{}/{}", t.component, t.name))
        .collect();
    let r = regressions(&in_scope(&base, &heard_keys, listen), &now);
    save_baseline(device, &grown(&base, &now))?;
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(key: &str, outcome: &str) -> Check {
        Check {
            key: key.into(),
            outcome: outcome.into(),
            detail: "d".into(),
        }
    }

    #[test]
    fn checks_parse_from_json_lines_and_noise_is_ignored() {
        let t = "noise\n{\"test\":\"sound-card\",\"component\":\"audio-speaker\",\"outcome\":\"pass\",\"detail\":\"present\"}\n";
        assert_eq!(
            parse(t),
            [Check {
                key: "audio-speaker/sound-card".into(),
                outcome: "pass".into(),
                detail: "present".into()
            }]
        );
    }

    #[test]
    fn only_what_worked_before_can_regress() {
        let base: BTreeSet<String> = ["a/x".to_string(), "b/y".to_string()].into();
        let now = [c("a/x", "pass"), c("b/y", "fail"), c("c/z", "fail")];
        assert_eq!(
            regressions(&base, &now),
            ["b/y (fail: d)"],
            "c/z never passed: a known gap, not an alarm"
        );
    }

    #[test]
    fn a_baseline_check_that_did_not_run_is_a_regression() {
        let base: BTreeSet<String> = ["a/x".to_string()].into();
        assert_eq!(regressions(&base, &[]), ["a/x (did not run)"]);
    }

    #[test]
    fn the_baseline_grows_with_every_new_pass_and_forgets_nothing() {
        let base: BTreeSet<String> = ["a/x".to_string()].into();
        let g = grown(&base, &[c("a/x", "fail"), c("c/z", "pass")]);
        assert_eq!(g, ["a/x".to_string(), "c/z".to_string()].into());
    }

    #[test]
    fn a_run_that_did_not_listen_is_not_asked_what_only_listening_tells() {
        let base: BTreeSet<String> = ["audio-speaker/heard-left", "audio-speaker/card"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let heard = vec!["audio-speaker/heard-left".to_string()];
        let ran = vec![Check {
            key: "audio-speaker/card".into(),
            outcome: "pass".into(),
            detail: String::new(),
        }];
        assert!(
            regressions(&in_scope(&base, &heard, false), &ran).is_empty(),
            "silent run: the heard check is out of scope"
        );
        let r = regressions(&in_scope(&base, &heard, true), &ran);
        assert_eq!(
            r,
            vec!["audio-speaker/heard-left (did not run)".to_string()],
            "a listening run answers for it"
        );
        let silent = vec![
            ran[0].clone(),
            Check {
                key: "audio-speaker/heard-left".into(),
                outcome: "fail".into(),
                detail: "NOT HEARD".into(),
            },
        ];
        assert_eq!(
            regressions(&in_scope(&base, &heard, true), &silent).len(),
            1,
            "heard before, silent now: a regression"
        );
    }

    #[test]
    fn a_fresh_boot_waits_and_a_settled_phone_does_not() {
        assert_eq!(still_to_settle(30), 60);
        assert_eq!(still_to_settle(SETTLE_SECS), 0);
        assert_eq!(still_to_settle(5000), 0);
    }
}
