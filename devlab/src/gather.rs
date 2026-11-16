//! Gathering: every report the phone keeps that the lab does not have yet, brought into the device's
//! lab repository (tests/results/RUN.jsonl, as each run was written), and every run not yet filed,
//! filed into the picture's tables - linux-phone-oco/docs' test_results, a row per test, and
//! test_runs, a row per run with its counts. Nothing is retyped: the rows are the report's lines.

use crate::phone::Phone;
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const ON_PHONE: &str = "/var/lib/devlab/health";

/// A report's name as the phone writes it: RUN.jsonl, RUN being YYYYMMDD-HHMMSS.
pub fn is_report(name: &str) -> bool {
    let Some(run) = name.strip_suffix(".jsonl") else {
        return false;
    };
    let b = run.as_bytes();
    b.len() == 15
        && b[8] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 8 || c.is_ascii_digit())
}

/// Brings every report the phone has and `results` lacks; the names brought.
pub fn fetch(phone: &Phone, results: &Path) -> Result<Vec<String>> {
    std::fs::create_dir_all(results)?;
    let (_, listing) = phone.root(&format!("ls {ON_PHONE} 2>/dev/null\n"))?;
    let mut brought = Vec::new();
    for name in listing.lines().map(str::trim).filter(|n| is_report(n)) {
        let here = results.join(name);
        if here.exists() {
            continue;
        }
        let bytes = phone
            .root_bytes(&format!("cat {ON_PHONE}/{name}\n"))
            .with_context(|| format!("bringing {name}"))?;
        std::fs::write(&here, bytes)?;
        brought.push(name.to_string());
    }
    Ok(brought)
}

/// "2026-11-16T23:27:31Z" as the tables keep a moment: "2026-11-16 23:27:31".
fn moment(at: &str) -> String {
    at.trim_end_matches('Z').replacen('T', " ", 1)
}

/// A report's lines as test_results rows, and its test_runs row; None for a report with no tests.
pub fn rows(run: &str, report: &str) -> Option<(Vec<Value>, Value)> {
    let lines: Vec<Value> = report
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["test"].is_string() && v["component"].is_string())
        .collect();
    let first = lines.first()?;
    let s = |v: &Value, k: &str| v[k].as_str().unwrap_or_default().to_string();
    let results: Vec<Value> = lines
        .iter()
        .map(|v| {
            json!({
                "id": format!("{run}-{}-{}", s(v, "component"), s(v, "test")),
                "run": run,
                "device_id": s(v, "device"),
                "component_id": s(v, "component"),
                "test": s(v, "test"),
                "outcome": s(v, "outcome"),
                "detail": s(v, "detail"),
                "run_at": moment(&s(v, "at")),
                "kernel": s(v, "kernel"),
                "shell": s(v, "shell"),
            })
        })
        .collect();
    let n = |o: &str| lines.iter().filter(|v| v["outcome"] == o).count();
    let summary = json!({
        "id": run,
        "device_id": s(first, "device"),
        "run_at": moment(&s(first, "at")),
        "kernel": s(first, "kernel"),
        "shell": s(first, "shell"),
        "passed": n("pass"),
        "failed": n("fail"),
        "skipped": n("skip"),
    });
    Some((results, summary))
}

fn ids(file: &Path) -> BTreeSet<String> {
    std::fs::read_to_string(file)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|v| v["id"].as_str().map(str::to_string))
        .collect()
}

fn append(file: &Path, rows: &[Value]) -> Result<()> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)?;
    for r in rows {
        writeln!(f, "{r}")?;
    }
    Ok(())
}

/// Files every run in `results` that the picture's tables do not hold yet; the runs filed.
pub fn file(results: &Path, docs: &Path) -> Result<Vec<String>> {
    let runs_file = docs.join("data/test_runs.json");
    let results_file = docs.join("data/test_results.json");
    let filed = ids(&runs_file);
    let mut names: Vec<PathBuf> = std::fs::read_dir(results)
        .with_context(|| format!("reading {}", results.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    names.sort();
    let mut done = Vec::new();
    for p in names {
        let run = p
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if filed.contains(&run) {
            continue;
        }
        if let Some((rs, summary)) = rows(&run, &std::fs::read_to_string(&p)?) {
            append(&results_file, &rs)?;
            append(&runs_file, &[summary])?;
            done.push(run);
        }
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_phones_run_reports_are_brought() {
        assert!(is_report("20261116-232731.jsonl"));
        assert!(!is_report("latest.jsonl"));
        assert!(!is_report("20261116-232731.evidence"));
        assert!(!is_report(".20261116-232731.jsonl.part"));
    }

    #[test]
    fn a_report_becomes_the_tables_rows_without_retyping() {
        let r = concat!(
            "{\"run\":\"20261116-232731\",\"device\":\"xiaomi-surya\",\"boot\":\"b\",\"at\":\"2026-11-16T23:27:31Z\",\"kernel\":\"k\",\"shell\":\"sxmo\",\"layer\":\"base\",\"component\":\"wifi\",\"test\":\"wifi-interface\",\"outcome\":\"pass\",\"detail\":\"interface wlan0\"}\n",
            "{\"run\":\"20261116-232731\",\"device\":\"xiaomi-surya\",\"boot\":\"b\",\"at\":\"2026-11-16T23:27:31Z\",\"kernel\":\"k\",\"shell\":\"sxmo\",\"layer\":\"base\",\"component\":\"nfc\",\"test\":\"nfc-device\",\"outcome\":\"skip\",\"detail\":\"\"}\n",
        );
        let (rs, run) = rows("20261116-232731", r).unwrap();
        assert_eq!(rs.len(), 2);
        assert_eq!(rs[0]["id"], "20261116-232731-wifi-wifi-interface");
        assert_eq!(rs[0]["run_at"], "2026-11-16 23:27:31");
        assert_eq!(rs[0]["device_id"], "xiaomi-surya");
        assert_eq!(
            (run["passed"].as_u64(), run["skipped"].as_u64()),
            (Some(1), Some(1))
        );
        assert!(rows("x", "noise\n").is_none());
    }

    #[test]
    fn a_run_is_filed_once() {
        let d = std::env::temp_dir().join(format!("devlab-gather-{}", std::process::id()));
        let (results, docs) = (d.join("results"), d.join("docs"));
        std::fs::create_dir_all(&results).unwrap();
        std::fs::create_dir_all(docs.join("data")).unwrap();
        std::fs::write(
            results.join("20261116-232731.jsonl"),
            "{\"device\":\"d\",\"at\":\"2026-11-16T23:27:31Z\",\"component\":\"wifi\",\"test\":\"t\",\"outcome\":\"pass\",\"detail\":\"\"}\n",
        )
        .unwrap();
        assert_eq!(file(&results, &docs).unwrap(), ["20261116-232731"]);
        assert!(file(&results, &docs).unwrap().is_empty(), "filed already");
        let _ = std::fs::remove_dir_all(&d);
    }
}
