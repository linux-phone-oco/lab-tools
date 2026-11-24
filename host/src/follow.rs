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
            (age < LEASE_LAPSE && who != LEASE_HOLDER).then(|| (d.codename.clone(), who))
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
    if digits > 0
        && !unit.is_empty()
        && unit.chars().all(|c| c.is_ascii_lowercase())
        && every_seconds(s).is_some()
    {
        Ok(())
    } else {
        Err(format!(
            "{table} every = {s:?} must be a number and a unit (s, min, h, d, w), as 15min or 1h"
        ))
    }
}

/// `every` in seconds (`15min`, `1h`, `90s`, `1d`); None for a unit systemd reads that this does not.
pub fn every_seconds(s: &str) -> Option<u64> {
    let digits = s.chars().take_while(char::is_ascii_digit).count();
    let n: u64 = s[..digits].parse().ok()?;
    let unit = match &s[digits..] {
        "s" | "sec" | "second" | "seconds" => 1,
        "m" | "min" | "minute" | "minutes" => 60,
        "h" | "hr" | "hour" | "hours" => 3600,
        "d" | "day" | "days" => 86_400,
        "w" | "week" | "weeks" => 604_800,
        _ => return None,
    };
    n.checked_mul(unit)
}

/// What a failed try leaves: `RELEASE START-SECOND`, the release a follow stepped back from and when its run began.
pub fn parse_tried(text: &str) -> Option<(String, u64)> {
    let (release, at) = text.trim().split_once(' ')?;
    Some((release.to_string(), at.parse().ok()?))
}

/// What a timer run may come early by: systemd's default `AccuracySec`.
const RETRY_SLACK: u64 = 60;

/// Seconds a release that failed still waits before it is tried again: None when it is another release than the one
/// that failed, or `every` (less a minute: systemd places a timer run within its accuracy, and the stamp is taken after the unit started) has passed since that run began, or the stamp is from a clock ahead of this one.
pub fn retry_wait(
    tried: Option<(String, u64)>,
    release: &str,
    every: u64,
    now: u64,
) -> Option<u64> {
    let (failed, began) = tried?;
    if failed != release {
        return None;
    }
    if began > now {
        return None;
    }
    let due = began.saturating_add(every.saturating_sub(RETRY_SLACK));
    due.checked_sub(now).filter(|left| *left > 0)
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

/// How many rounds of its timer a follow may miss before `check` calls it stale.
pub const STALE_ROUNDS: u64 = 3;

/// What `check` says of follow's last success (the epoch second it kept, if any) against `[follow] every`:
/// (fails, the line). A follow that has not finished once here is not known yet, never failing (practice 12).
pub fn judge_last_success(every: &str, ok: Option<u64>, now: u64) -> Option<(bool, String)> {
    let limit = every_seconds(every)?.checked_mul(STALE_ROUNDS)?;
    let Some(at) = ok else {
        return Some((
            false,
            "follow: not measured, no follow has finished here yet".into(),
        ));
    };
    let age = now.saturating_sub(at);
    let ago = format!("{} min ago", age / 60);
    Some(if age > limit {
        (
            true,
            format!("follow: FAIL, its last success was {ago}, more than {STALE_ROUNDS} rounds of every = {every}"),
        )
    } else {
        (false, format!("follow: ok, its last success was {ago}"))
    })
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

/// Why the leases were not all taken.
#[derive(Debug, PartialEq)]
pub enum NotTaken {
    /// A keeper's lease is live: the phone and who keeps it. A wait, not a failure.
    Kept { phone: String, who: String },
    /// The lease could not be made or moved: a failure to say, not a wait.
    Unusable { phone: String, why: String },
}

/// Takes the lease of each phone: the lease is made whole (a folder holding the holder's name) beside its place and
/// moved there, so it is never seen half made (a move onto a folder that has files fails). A live lease is never
/// taken, whoever its holder reads as (a renewing keeper's holder file is empty for an instant); one that lapsed,
/// was half made, or is a killed follow's own is moved aside, then taken. What was taken is given back when one is
/// refused.
pub fn take_leases(
    home: &str,
    devices: &[Device],
    age_of: &dyn Fn(&str) -> Option<(String, Duration)>,
) -> Result<Leases, NotTaken> {
    use std::fs;
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
        let unusable = |what: &str, e: std::io::Error| NotTaken::Unusable {
            phone: code.to_string(),
            why: format!("{what}: {e}"),
        };
        let state = dir
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_default();
        fs::create_dir_all(&state).map_err(|e| unusable("make its folder", e))?;
        let whole = state.join(format!("keeper.lease.follow-{}", std::process::id()));
        let _ = fs::remove_dir_all(&whole);
        fs::create_dir(&whole).map_err(|e| unusable("make the lease", e))?;
        fs::write(whole.join("holder"), format!("{LEASE_HOLDER}\n"))
            .map_err(|e| unusable("write the holder", e))?;
        let mut moved = false;
        for pass in 0..2 {
            if fs::rename(&whole, &dir).is_ok() {
                moved = true;
                break;
            }
            match age_of(&holder_file) {
                Some((who, age)) if age < LEASE_LAPSE && who != LEASE_HOLDER => {
                    let _ = fs::remove_dir_all(&whole);
                    return Err(NotTaken::Kept {
                        phone: code.to_string(),
                        who,
                    });
                }
                _ if pass == 0 => {
                    let aside = state.join(format!("keeper.lease.aside-{}", std::process::id()));
                    let _ = fs::remove_dir_all(&aside);
                    if let Err(e) = fs::rename(&dir, &aside) {
                        if e.kind() != std::io::ErrorKind::NotFound {
                            let _ = fs::remove_dir_all(&whole);
                            return Err(unusable("move the old lease aside", e));
                        }
                    }
                    let _ = fs::remove_dir_all(&aside);
                }
                _ => {}
            }
        }
        if !moved {
            let _ = fs::remove_dir_all(&whole);
            return Err(NotTaken::Unusable {
                phone: code.to_string(),
                why: "another keeper made its lease as this one was taken".into(),
            });
        }
        taken.dirs.push(dir);
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
    fn an_interval_in_seconds_and_what_a_failed_try_still_waits() {
        assert_eq!(every_seconds("15min"), Some(900));
        assert_eq!(every_seconds("1h"), Some(3600));
        assert_eq!(every_seconds("2days"), Some(172_800));
        assert_eq!(every_seconds("5fortnights"), None);
        let tried = || parse_tried("0.2.1 1000\n");
        assert_eq!(retry_wait(tried(), "0.2.1", 900, 1500), Some(340));
        assert_eq!(
            retry_wait(tried(), "0.2.1", 900, 1900),
            None,
            "the timer's own beat is not held"
        );
        assert_eq!(retry_wait(tried(), "0.2.2", 900, 1100), None);
        assert_eq!(retry_wait(None, "0.2.1", 900, 1100), None);
        assert_eq!(
            retry_wait(tried(), "0.2.1", 900, 1899),
            None,
            "a timer run a second early is not held"
        );
        assert_eq!(
            retry_wait(parse_tried("0.2.1 90000"), "0.2.1", 900, 1000),
            None,
            "a stamp from a clock ahead holds nothing"
        );
        assert!(
            valid_every("[follow]", "5ms").is_err(),
            "a unit every_seconds cannot read must not pass quietly"
        );
        assert_eq!(parse_tried("0.2.1"), None);
        assert_eq!(parse_tried("0.2.1 soon"), None);
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

#[cfg(test)]
mod lease_tests {
    use super::*;
    use std::fs;

    fn device(code: &str) -> Device {
        Device {
            path: Default::default(),
            codename: code.into(),
            host: Default::default(),
            lpoco_pin: None,
            fastboot_serial: None,
            usb_address: None,
            pieces: vec![],
            user_configs: vec![],
        }
    }

    fn home(name: &str) -> String {
        let h = std::env::temp_dir().join(format!("lpoco-leases-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&h);
        fs::create_dir_all(&h).unwrap();
        h.to_string_lossy().into_owned()
    }

    fn age(file: &str) -> Option<(String, Duration)> {
        let who = fs::read_to_string(file).ok()?;
        let t = fs::metadata(file).ok()?.modified().ok()?;
        Some((
            who.trim().into(),
            std::time::SystemTime::now()
                .duration_since(t)
                .unwrap_or_default(),
        ))
    }

    fn live_lease(h: &str, code: &str, holder: &str) {
        let d = format!("{h}/.local/state/devlab/{code}/keeper.lease");
        fs::create_dir_all(&d).unwrap();
        fs::write(format!("{d}/holder"), holder).unwrap();
    }

    fn present(h: &str, code: &str) -> bool {
        std::path::Path::new(&lease_holder_file(h, code)).exists()
    }

    #[test]
    fn a_live_keepers_lease_on_the_second_phone_refuses_and_gives_back_the_first() {
        let h = home("second");
        live_lease(&h, "b-phone", "a-keeper\n");
        let devs = [device("a-phone"), device("b-phone")];
        let got = take_leases(&h, &devs, &age).err().unwrap();
        assert_eq!(
            got,
            NotTaken::Kept {
                phone: "b-phone".into(),
                who: "a-keeper".into()
            }
        );
        assert!(!present(&h, "a-phone"), "the first lease was kept");
        assert_eq!(
            fs::read_to_string(lease_holder_file(&h, "b-phone")).unwrap(),
            "a-keeper\n"
        );
    }

    #[test]
    fn a_lease_whose_holder_reads_empty_while_a_keeper_renews_is_live_not_half_made() {
        let h = home("empty");
        live_lease(&h, "a-phone", "");
        let got = take_leases(&h, &[device("a-phone")], &age).err().unwrap();
        assert!(matches!(got, NotTaken::Kept { .. }), "{got:?}");
        assert!(present(&h, "a-phone"));
    }

    #[test]
    fn a_killed_follows_own_leftover_is_taken_over_and_not_counted_as_a_keeper() {
        let h = home("leftover");
        live_lease(&h, "a-phone", &format!("{LEASE_HOLDER}\n"));
        let devs = [device("a-phone")];
        assert!(held_leases(&h, &devs, &age).is_empty());
        let leases = take_leases(&h, &devs, &age).unwrap();
        assert!(present(&h, "a-phone"));
        drop(leases);
        assert!(!present(&h, "a-phone"));
    }

    #[test]
    fn a_lapsed_lease_is_taken_and_a_dropped_lease_is_given_back_only_while_it_is_ours() {
        let h = home("lapsed");
        live_lease(&h, "a-phone", "old-keeper\n");
        let long_ago = |f: &str| age(f).map(|(w, _)| (w, LEASE_LAPSE + Duration::from_secs(1)));
        let leases = take_leases(&h, &[device("a-phone")], &long_ago).unwrap();
        assert_eq!(
            fs::read_to_string(lease_holder_file(&h, "a-phone")).unwrap(),
            format!("{LEASE_HOLDER}\n")
        );
        // a keeper that took it since is not robbed of it
        fs::write(lease_holder_file(&h, "a-phone"), "new-keeper\n").unwrap();
        drop(leases);
        assert!(present(&h, "a-phone"));
    }

    #[test]
    fn a_state_folder_that_cannot_be_made_is_unusable_not_a_wait() {
        let h = home("unusable");
        fs::write(format!("{h}/.local"), "a file where a folder should be").unwrap();
        let got = take_leases(&h, &[device("a-phone")], &age).err().unwrap();
        assert!(matches!(got, NotTaken::Unusable { .. }), "{got:?}");
    }
}

/// `judge_check` for a host whose named tools are off their pins on purpose (a follow is about to move them): their
/// `tool NAME: FAIL, ...` lines are not failures, and the exit 1 they cause is not either when nothing else failed.
/// Nor is the `follow: FAIL, ` line (follow's own staleness): only a follow that runs can clear it.
pub fn judge_check_without(exit: Option<i32>, stdout: &str, tools: &[String]) -> (bool, String) {
    let ignored = |l: &str| {
        l.starts_with("follow: FAIL, ")
            || tools.iter().any(|t| {
                // only a copy found off its pin: one not found at all is prepare's to bring in
                l.strip_prefix(&format!("tool {t}: FAIL, pinned "))
                    .and_then(|rest| rest.split_once(" found "))
                    .is_some_and(|(_, found)| found != "not found")
            })
    };
    let dropped = stdout.lines().any(ignored);
    let kept: Vec<&str> = stdout.lines().filter(|l| !ignored(l)).collect();
    let exit = if dropped && exit == Some(1) {
        Some(0)
    } else {
        exit
    };
    judge_check(exit, &kept.join("\n"))
}

#[cfg(test)]
mod without_tests {
    use super::*;

    #[test]
    fn a_tool_off_its_pin_on_purpose_is_not_a_failure_but_another_one_is() {
        let out = "tool rhizoid: FAIL, pinned v2 found v1\n";
        let t = ["rhizoid".to_string()];
        assert!(judge_check_without(Some(1), out, &t).0);
        assert!(!judge_check(Some(1), out).0);
        let two = format!("{out}tool lpoco: FAIL, pinned 2 found 1\n");
        assert!(!judge_check_without(Some(1), &two, &t).0);
        assert!(!judge_check_without(Some(2), out, &t).0);
    }

    #[test]
    fn only_a_tool_found_off_its_pin_is_ignored_not_one_that_is_not_there_or_fails_otherwise() {
        // source: the lines host/src/main.rs prints: `tool {}: FAIL, pinned {} found {}` where found is the
        // installed version or ToolPlan::found_text's "not found" (host/src/lib.rs); a tool never installed
        // is prepare's to bring in (moved_tools), so follow cannot clear it
        let t = ["rhizoid".to_string()];
        for out in [
            "tool rhizoid: FAIL, pinned v2 found not found\n",
            "tool rhizoid: FAIL, missing\n",
            "tool rhizoid: FAIL, anything else\n",
        ] {
            assert!(!judge_check_without(Some(1), out, &t).0, "{out}");
        }
        let behind = "tool rhizoid: FAIL, pinned v2 found v1\n";
        assert!(judge_check_without(Some(1), behind, &t).0);
        let unstamped = "tool rhizoid: FAIL, pinned v2 found on PATH, rev not known (no stamp)\n";
        assert!(judge_check_without(Some(1), unstamped, &t).0);
    }
}

#[cfg(test)]
mod last_success {
    use super::*;

    #[test]
    fn a_success_within_three_rounds_is_ok_and_older_fails_and_none_is_not_known_yet() {
        assert_eq!(
            judge_last_success("15min", Some(1000), 1000 + 2700),
            Some((false, "follow: ok, its last success was 45 min ago".into()))
        );
        let (fails, line) = judge_last_success("15min", Some(1000), 1000 + 2701).unwrap();
        assert!(fails && line.starts_with("follow: FAIL, its last success was 45 min ago"));
        let (fails, line) = judge_last_success("15min", None, 5).unwrap();
        assert!(!fails && line.contains("not measured"));
        assert_eq!(judge_last_success("soon", Some(1), 2), None);
    }
}
