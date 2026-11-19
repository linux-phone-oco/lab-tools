// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco-health data plan|spent`: what mobile data a check may spend, and what it spent - a ledger
//! of the month, so a phone that boots fifty times a day never eats its SIM's allowance.
//!
//!   lpoco-health data plan  [--budget-mb 250] [--ledger FILE]
//!   lpoco-health data spent BYTES [--weekly] [--budget-mb 250] [--ledger FILE]
//!
//! plan says, as one JSON line, the month (UTC), the bytes spent in it, the budget, whether the budget
//! is spent (`over`: the check skips), whether this run is the week's throughput run (`weekly`: none in
//! the last 7 days) and so how much to download (`download_bytes`: 100 kB, or 5 MB weekly).
//! spent adds a run's bytes to the month (a new month starts from nothing) and, with --weekly, marks
//! the week's run as made; it prints the same line, as it stands after. The ledger is a small JSON file,
//! /var/lib/lpoco-health/data/ledger.json by default - beside the reports, not among them, so pruning
//! old runs never forgets what the month spent.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const LEDGER: &str = "/var/lib/lpoco-health/data/ledger.json";
const BUDGET_MB: u64 = 250;
const DAILY_BYTES: u64 = 100_000;
const WEEKLY_BYTES: u64 = 5_000_000;
const WEEK_SECS: u64 = 7 * 86_400;

/// What the month has spent, as the ledger keeps it.
#[derive(Debug, Clone, Default, PartialEq)]
struct Ledger {
    month: String,
    bytes: u64,
    runs: u64,
    /// when the last weekly throughput run was made (Unix seconds), 0 when never
    weekly_at: u64,
}

fn month_of(secs: u64) -> String {
    let (y, m, _) = crate::civil((secs / 86_400) as i64);
    format!("{y:04}-{m:02}")
}

impl Ledger {
    fn read(path: &Path) -> Ledger {
        let v: Value = std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(Value::Null);
        Ledger {
            month: v["month"].as_str().unwrap_or("").into(),
            bytes: v["bytes"].as_u64().unwrap_or(0),
            runs: v["runs"].as_u64().unwrap_or(0),
            weekly_at: v["weekly_at"].as_u64().unwrap_or(0),
        }
    }

    /// The ledger as it stands in the month of `now`: a new month starts from nothing (the week's
    /// run is kept: a week runs across months).
    fn in_month(self, now: u64) -> Ledger {
        let month = month_of(now);
        if self.month == month {
            self
        } else {
            Ledger {
                month,
                bytes: 0,
                runs: 0,
                weekly_at: self.weekly_at,
            }
        }
    }

    fn said(&self, now: u64, budget_mb: u64) -> Value {
        let budget = budget_mb * 1_000_000;
        let weekly = now.saturating_sub(self.weekly_at) >= WEEK_SECS;
        json!({"month": self.month, "used_bytes": self.bytes, "runs": self.runs, "budget_bytes": budget,
            "over": self.bytes >= budget, "weekly": weekly,
            "download_bytes": if weekly { WEEKLY_BYTES } else { DAILY_BYTES }})
    }
}

/// `lpoco-health data ...` at `now`: what to print, or what was asked wrongly.
fn run(args: &[String], now: u64) -> Result<Value, String> {
    let (verb, rest) = args.split_first().ok_or("plan or spent")?;
    let (mut budget_mb, mut ledger, mut weekly, mut bytes) =
        (BUDGET_MB, PathBuf::from(LEDGER), false, None);
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--weekly" => weekly = true,
            "--budget-mb" => {
                let v = it.next().ok_or("--budget-mb needs a value")?;
                budget_mb = v
                    .parse()
                    .map_err(|_| format!("--budget-mb {v}: not a whole number"))?;
            }
            "--ledger" => ledger = it.next().ok_or("--ledger needs a file")?.into(),
            n if bytes.is_none() && verb == "spent" => {
                bytes = Some(
                    n.parse::<u64>()
                        .map_err(|_| format!("{n}: not a count of bytes"))?,
                )
            }
            n => return Err(format!("{n}: not asked here")),
        }
    }
    let l = Ledger::read(&ledger).in_month(now);
    match verb.as_str() {
        "plan" => Ok(l.said(now, budget_mb)),
        "spent" => {
            let l = Ledger {
                bytes: l.bytes + bytes.ok_or("spent BYTES: how many")?,
                runs: l.runs + 1,
                weekly_at: if weekly { now } else { l.weekly_at },
                ..l
            };
            let v = json!({"month": l.month, "bytes": l.bytes, "runs": l.runs, "weekly_at": l.weekly_at});
            if let Some(dir) = ledger.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
            std::fs::write(&ledger, format!("{v}\n"))
                .map_err(|e| format!("{}: {e}", ledger.display()))?;
            Ok(l.said(now, budget_mb))
        }
        v => Err(format!("{v}: plan or spent")),
    }
}

/// `lpoco-health data ...`: the exit code.
pub fn main(args: &[String]) -> i32 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    match run(args, now) {
        Ok(v) => {
            println!("{v}");
            0
        }
        Err(e) => {
            eprintln!("lpoco-health data: {e} (usage: data plan|spent BYTES [--weekly] [--budget-mb N] [--ledger FILE])");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-11-17 12:00 UTC
    const NOV: u64 = 1_794_916_800;

    fn ask(ledger: &Path, words: &str, now: u64) -> Value {
        let mut a: Vec<String> = words.split_whitespace().map(str::to_string).collect();
        a.extend(["--ledger".into(), ledger.to_string_lossy().into_owned()]);
        run(&a, now).unwrap()
    }

    fn ledger(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("health-data-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d.join("ledger.json")
    }

    #[test]
    fn a_month_is_utc() {
        assert_eq!(month_of(NOV), "2026-11");
        assert_eq!(
            month_of(1_796_083_199),
            "2026-11",
            "the last second of November"
        );
        assert_eq!(month_of(1_796_083_200), "2026-12");
    }

    #[test]
    fn the_first_run_is_the_weeks_and_then_the_days_small_one() {
        let l = ledger("weekly");
        let p = ask(&l, "plan", NOV);
        assert_eq!(p["weekly"], true, "never measured: this run is the week's");
        assert_eq!(p["download_bytes"], 5_000_000);
        ask(&l, "spent 5200000 --weekly", NOV);
        let p = ask(&l, "plan", NOV + 3600);
        assert_eq!(p["weekly"], false);
        assert_eq!(p["download_bytes"], 100_000);
        assert_eq!(
            ask(&l, "plan", NOV + WEEK_SECS)["weekly"],
            true,
            "a week on"
        );
    }

    #[test]
    fn runs_add_up_in_their_month_and_a_new_month_starts_from_nothing() {
        let l = ledger("month");
        ask(&l, "spent 150000", NOV);
        let p = ask(&l, "spent 160000", NOV + 60);
        assert_eq!(p["used_bytes"], 310_000);
        assert_eq!(p["runs"], 2);
        assert_eq!(ask(&l, "plan", NOV + 30 * 86_400)["used_bytes"], 0);
    }

    #[test]
    fn a_spent_budget_says_over() {
        let l = ledger("budget");
        ask(&l, "spent 1200000", NOV);
        assert_eq!(ask(&l, "plan --budget-mb 1", NOV)["over"], true);
        assert_eq!(ask(&l, "plan", NOV)["over"], false, "250 MB by default");
    }

    #[test]
    fn a_wrong_ask_is_said() {
        let l = ledger("wrong");
        let a = |w: &str| {
            run(
                &w.split_whitespace().map(str::to_string).collect::<Vec<_>>(),
                NOV,
            )
        };
        assert!(a("").is_err());
        assert!(a("spend 3").is_err());
        assert!(a("spent").is_err());
        assert!(a("spent many").is_err());
        assert!(a("plan 3").is_err());
        assert!(a("plan --budget-mb lots").is_err());
        assert!(!l.exists(), "nothing written by a wrong ask");
    }
}
