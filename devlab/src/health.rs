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
pub fn run_checks(device: &Device, manifest_dir: &Path, phone: &Phone) -> Result<Vec<Check>> {
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

/// One health check: the regressions (empty when all is well); the baseline grows with new passes.
pub fn health(device: &Device, manifest_dir: &Path, phone: &Phone) -> Result<Vec<String>> {
    let now = run_checks(device, manifest_dir, phone)?;
    let base = load_baseline(device);
    let r = regressions(&base, &now);
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
}
