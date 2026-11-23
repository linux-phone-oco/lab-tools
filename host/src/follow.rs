// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco host follow` (decision 2026-11-19-host-follows-releases (b)): the decisions it makes, kept apart from
//! what it does so each can be tested alone - which release is newest, whether a phone's keeper holds the lab, the
//! pin line a landing changes, whether a plan needs root, and the user units that run it.

use crate::Device;
use std::time::Duration;

/// A keeper that has not renewed its lease for this long has ended (devlab keeper.rs LAPSE, the same number).
pub const LEASE_LAPSE: Duration = Duration::from_secs(90 * 60);
/// The user unit pair the declared `[follow]` becomes.
pub const FOLLOW_TIMER: &str = "lpoco-host-follow.timer";
pub const FOLLOW_SERVICE: &str = "lpoco-host-follow.service";
/// Who a landed pin is signed as.
pub const SIGNER: &str = "lab-host-follow";

/// `X.Y.Z` as numbers, or None when it is not a full version.
pub fn version_key(v: &str) -> Option<(u64, u64, u64)> {
    if !v.split('.').all(|p| {
        p == "0" || (!p.starts_with('0') && !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
    }) {
        return None;
    }
    let mut it = v.split('.');
    let key = (
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
    );
    it.next().is_none().then_some(key)
}

/// The newest family release in GitHub's releases list (an array of objects with `tag_name`): the highest
/// `lpoco-vX.Y.Z`, whatever other programs' tags stand among them.
pub fn newest_family_version(releases_json: &str) -> Result<Option<String>, String> {
    let v: serde_json::Value = serde_json::from_str(releases_json)
        .map_err(|e| format!("the releases list is not JSON: {e}"))?;
    let items = v.as_array().ok_or("the releases list is not an array")?;
    Ok(items
        .iter()
        .filter_map(|r| r.get("tag_name")?.as_str()?.strip_prefix("lpoco-v"))
        .filter_map(|v| version_key(v).map(|k| (k, v.to_string())))
        .max()
        .map(|(_, v)| v))
}

/// Whether `newest` is a later release than `pinned`.
pub fn is_newer(newest: &str, pinned: &str) -> bool {
    matches!((version_key(newest), version_key(pinned)), (Some(n), Some(p)) if n > p)
}

/// Where a phone's keeper lease lies, as devlab keeps it.
pub fn lease_holder_file(home: &str, codename: &str) -> String {
    format!("{home}/.local/state/devlab/{codename}/keeper.lease/holder")
}

/// The phones whose keeper holds the lab now: (codename, holder). `age_of` gives a lease's holder name and the
/// time since its last renewal; a lease older than the lapse has ended.
pub fn held_leases(
    home: &str,
    devices: &[Device],
    age_of: &dyn Fn(&str) -> Option<(String, Duration)>,
) -> Vec<(String, String)> {
    let mut held: Vec<(String, String)> = devices
        .iter()
        .filter_map(|d| {
            let (who, age) = age_of(&lease_holder_file(home, &d.codename))?;
            (age < LEASE_LAPSE).then(|| (d.codename.clone(), who))
        })
        .collect();
    held.sort();
    held.dedup();
    held
}

/// lab.toml with the `lpoco` pin moved from `from` to `to`, every other byte as it was. None when no line is the
/// pin at `from` (the file says something else, so nothing is landed).
pub fn bump_pin(lab_text: &str, from: &str, to: &str) -> Option<String> {
    let want = format!("lpoco = \"{from}\"");
    let mut done = false;
    let lines: Vec<String> = lab_text
        .split('\n')
        .map(|l| {
            if !done && l == want {
                done = true;
                format!("lpoco = \"{to}\"")
            } else {
                l.to_string()
            }
        })
        .collect();
    done.then(|| lines.join("\n"))
}

/// The root commands a `plan` lists (None when its output has no such section): what a release needs root for is what
/// its own host plans that the host as it stands does not.
pub fn plan_root_commands(plan_out: &str) -> Option<Vec<String>> {
    let mut lines = plan_out.lines();
    lines.find(|l| l.starts_with("root commands, through"))?;
    Some(
        lines
            .take_while(|l| !l.trim().is_empty())
            .map(|l| l.trim().to_string())
            .filter(|l| l != "none")
            .collect(),
    )
}

/// `every reaches a unit file: a number and a unit (`15min`, `1h`).
pub fn valid_every(table: &str, s: &str) -> Result<(), String> {
    let digits = s.chars().take_while(|c| c.is_ascii_digit()).count();
    let unit = &s[digits..];
    if digits > 0 && !unit.is_empty() && unit.chars().all(|c| c.is_ascii_lowercase()) {
        Ok(())
    } else {
        Err(format!(
            "{table} every = {s:?} must be a number and a unit, as 15min or 1h"
        ))
    }
}

/// A time of day as minutes after midnight: `HH:MM`, two digits each.
pub fn parse_clock(s: &str) -> Option<u32> {
    let (h, m) = s.split_once(':')?;
    let two = |x: &str| x.len() == 2 && x.chars().all(|c| c.is_ascii_digit());
    if !two(h) || !two(m) {
        return None;
    }
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// `[follow] window`: `HH:MM-HH:MM`, local time, from (included) to (excluded); it may cross midnight. Minutes after
/// midnight, start and end.
pub fn parse_window(table: &str, s: &str) -> Result<(u32, u32), String> {
    let bad = || format!("{table} window = {s:?} must be two clock times, as 02:00-05:00");
    let (a, b) = s.split_once('-').ok_or_else(bad)?;
    match (parse_clock(a), parse_clock(b)) {
        (Some(from), Some(to)) if from != to => Ok((from, to)),
        _ => Err(bad()),
    }
}

/// Whether the minute of the day lies in the window.
pub fn in_window((from, to): (u32, u32), minute: u32) -> bool {
    if from < to {
        (from..to).contains(&minute)
    } else {
        minute >= from || minute < to
    }
}

pub fn timer_text(every: &str) -> String {
    format!("[Unit]\nDescription=lpoco host follow: look for a newer family release\n\n[Timer]\nOnBootSec=5min\nOnUnitActiveSec={every}\n\n[Install]\nWantedBy=timers.target\n")
}

/// The service. `lab` is the lab.toml the host was prepared from when that is not the default one: the service reads
/// that file, not the default place.
pub fn service_text(lab: Option<&str>) -> String {
    let args = lab
        .map(|l| format!(" --lab {}", unit_quoted(l)))
        .unwrap_or_default();
    format!("[Unit]\nDescription=lpoco host follow: install a newer family release in a lease-free window, check it, land the pin\n\n[Service]\nType=oneshot\nExecStart=%h/.local/bin/lpoco-host follow now{args}\n")
}

/// A word of an ExecStart line in double quotes, with what systemd reads inside them escaped: backslash, quote,
/// newline, `%` specifiers and `$` variables.
pub fn unit_quoted(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '%' => out.push_str("%%"),
            '$' => out.push_str("$$"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A line of the check that says it failed: `LABEL: FAIL, why` or `FAIL why`, the two shapes it prints (an ok line
/// that has FAIL inside a path or a serial is not one).
fn is_fail_line(l: &str) -> bool {
    l.starts_with("FAIL ") || l.contains(": FAIL, ")
}

/// What a host's `check` said: whether it failed, and the lines that say why. Only a `FAIL` line fails the lab
/// (practice 12: what was not measured is not known yet, never failing). An exit of 1 that has no FAIL line but does
/// have `not measured` lines is such a check, as older hosts exited; any other exit (an error, a signal) fails.
pub fn judge_check(exit: Option<i32>, stdout: &str) -> (bool, String) {
    let fails: Vec<&str> = stdout.lines().filter(|l| is_fail_line(l)).collect();
    let unmeasured = stdout.lines().any(|l| l.contains(": not measured, "));
    let ok = fails.is_empty() && (exit == Some(0) || (exit == Some(1) && unmeasured));
    let why = if fails.is_empty() {
        let lines: Vec<&str> = stdout.lines().collect();
        lines[lines.len().saturating_sub(3)..].join(" | ")
    } else {
        fails.join(" | ")
    };
    (ok, why)
}

/// The name the follow keeps each phone under while it replaces the programs.
pub const LEASE_HOLDER: &str = "lpoco-host-follow";

/// Every phone's keeper lease, held by the follow: a keeper that wants a phone meanwhile is told the phone is kept
/// (devlab keeper take), so it never runs programs of two versions. Given back when this is dropped.
pub struct Leases {
    dirs: Vec<std::path::PathBuf>,
}

impl Drop for Leases {
    fn drop(&mut self) {
        for d in &self.dirs {
            // only a lease that is still the follow's: a keeper never takes one that is live, but be exact
            let ours =
                std::fs::read_to_string(d.join("holder")).is_ok_and(|h| h.trim() == LEASE_HOLDER);
            if ours {
                let _ = std::fs::remove_dir_all(d);
            }
        }
    }
}

/// Takes the lease of each phone, as devlab does (a directory made atomically, the holder's name in it); a lease
/// that has lapsed, or was half made, is taken over. A lease that is live is not: the phone and who keeps it
/// are returned, and what was taken is given back.
pub fn take_leases(
    home: &str,
    devices: &[Device],
    age_of: &dyn Fn(&str) -> Option<(String, Duration)>,
) -> Result<Leases, (String, String)> {
    let mut taken = Leases { dirs: Vec::new() };
    let mut codenames: Vec<&str> = devices.iter().map(|d| d.codename.as_str()).collect();
    codenames.sort();
    codenames.dedup();
    for code in codenames {
        let holder_file = lease_holder_file(home, code);
        let dir = std::path::Path::new(&holder_file)
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_default();
        let unusable = |why: &str| {
            (
                code.to_string(),
                format!("the lease could not be taken ({why})"),
            )
        };
        if let Some(parent) = dir.parent() {
            std::fs::create_dir_all(parent).map_err(|e| unusable(&e.to_string()))?;
        }
        let mut have = false;
        for _ in 0..2 {
            if std::fs::create_dir(&dir).is_ok() {
                have = true;
                break;
            }
            match age_of(&holder_file) {
                Some((who, age)) if !who.is_empty() && who != LEASE_HOLDER && age < LEASE_LAPSE => {
                    return Err((code.to_string(), who));
                }
                // lapsed, a half-made lease, or one a killed follow left: taken away, then taken
                _ => {
                    let _ = std::fs::remove_dir_all(&dir);
                }
            }
        }
        if !have {
            return Err(unusable("it kept being made by another"));
        }
        taken.dirs.push(dir.clone());
        std::fs::write(dir.join("holder"), format!("{LEASE_HOLDER}\n"))
            .map_err(|e| unusable(&e.to_string()))?;
    }
    Ok(taken)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_newest_family_tag_wins_over_numbers_as_text_and_over_other_programs() {
        let j = r#"[{"tag_name":"lpoco-lab-v0.40.0"},{"tag_name":"lpoco-v0.9.0"},{"tag_name":"lpoco-v0.10.0"},{"tag_name":"attended-v1"}]"#;
        assert_eq!(newest_family_version(j).unwrap().as_deref(), Some("0.10.0"));
        assert_eq!(newest_family_version("[]").unwrap(), None);
        assert!(newest_family_version("{}").is_err());
        assert!(is_newer("0.10.0", "0.9.9") && !is_newer("0.9.9", "0.9.9"));
    }

    #[test]
    fn a_pin_bump_changes_the_one_line_and_nothing_else() {
        let t = "[tools]\n# lpoco = \"0.2.0\"\nlpoco = \"0.2.0\"\nother = 1\n";
        assert_eq!(
            bump_pin(t, "0.2.0", "0.2.1").unwrap(),
            "[tools]\n# lpoco = \"0.2.0\"\nlpoco = \"0.2.1\"\nother = 1\n"
        );
        assert_eq!(bump_pin(t, "0.1.0", "0.2.1"), None);
        assert_eq!(bump_pin("lpoco = \"0.2.0\" \n", "0.2.0", "0.2.1"), None);
    }

    #[test]
    fn the_root_commands_of_a_plan_are_those_listed_and_a_plan_without_the_section_says_so() {
        assert_eq!(
            plan_root_commands("x\nroot commands, through `sudo`:\n    none\n\ntools:\n"),
            Some(vec![])
        );
        assert_eq!(
            plan_root_commands("root commands, through `sudo`:\n    install a b\n\ntools:\n"),
            Some(vec!["install a b".to_string()])
        );
        assert_eq!(plan_root_commands("no such section"), None);
    }

    #[test]
    fn a_word_in_an_exec_line_is_quoted_so_systemd_reads_it_back_whole() {
        assert_eq!(unit_quoted("/a b/lab.toml"), "\"/a b/lab.toml\"");
        assert_eq!(unit_quoted("a%h$X\"\\\n"), "\"a%%h$$X\\\"\\\\\\n\"");
        assert!(service_text(None).ends_with("follow now\n"));
        assert!(service_text(Some("/x y")).ends_with("follow now --lab \"/x y\"\n"));
    }

    #[test]
    fn an_interval_is_a_number_and_a_unit() {
        assert!(valid_every("[follow]", "15min").is_ok() && valid_every("[follow]", "1h").is_ok());
        for bad in ["", "min", "15", "15 min", "15min\nExecStart=x", "-1h"] {
            assert!(valid_every("[follow]", bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_window_is_two_clock_times_and_may_cross_midnight() {
        let w = parse_window("[follow]", "02:00-05:30").unwrap();
        assert_eq!(w, (120, 330));
        assert!(in_window(w, 120) && in_window(w, 329) && !in_window(w, 330) && !in_window(w, 119));
        let night = parse_window("[follow]", "22:00-04:00").unwrap();
        assert!(in_window(night, 23 * 60) && in_window(night, 0) && in_window(night, 239));
        assert!(!in_window(night, 240) && !in_window(night, 12 * 60));
        for bad in [
            "",
            "2:00-5:00",
            "02:00",
            "02:00-02:00",
            "24:00-05:00",
            "02:60-05:00",
            "02:00-05:00\nx",
            "02:00 - 05:00",
        ] {
            assert!(parse_window("[follow]", bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_clock_is_hours_and_minutes() {
        assert_eq!(parse_clock("07:05"), Some(425));
        assert_eq!(parse_clock("7:05"), None);
        assert_eq!(parse_clock("24:00"), None);
    }

    const UNMEASURED: &str = "tool x: not measured, its pin is not known yet\n";

    #[test]
    fn exit_zero_with_only_not_measured_lines_passes() {
        assert!(judge_check(Some(0), UNMEASURED).0);
    }

    #[test]
    fn exit_one_passes_only_when_it_says_what_it_did_not_measure() {
        assert!(judge_check(Some(1), UNMEASURED).0);
        assert!(!judge_check(Some(1), "").0);
        assert!(!judge_check(Some(1), "tool x: ok, 1\n").0);
    }

    #[test]
    fn an_error_exit_or_a_signal_fails_whatever_was_printed() {
        assert!(!judge_check(Some(2), UNMEASURED).0);
        assert!(!judge_check(None, UNMEASURED).0);
    }

    #[test]
    fn a_fail_line_fails_even_at_exit_zero_and_is_named_alone_in_order() {
        let out = format!(
            "tool a: FAIL, one\n{UNMEASURED}FAIL two\ncommand c: FAIL, missing from PATH\n"
        );
        let (ok, why) = judge_check(Some(0), &out);
        assert!(!ok);
        assert_eq!(
            why,
            "tool a: FAIL, one | FAIL two | command c: FAIL, missing from PATH"
        );
    }

    #[test]
    fn an_ok_line_with_fail_inside_a_path_is_not_a_failure() {
        let out =
            "command c: ok, /opt/FAIL/bin/c (wanted by x)\nread: /srv/FAIL.toml from origin/main\n";
        assert!(judge_check(Some(0), out).0);
    }

    #[test]
    fn without_a_fail_line_the_last_three_lines_are_given_in_order() {
        assert_eq!(judge_check(Some(2), "a\nb\nc\nd\n").1, "b | c | d");
    }
}
