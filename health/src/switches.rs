// SPDX-License-Identifier: MIT OR Apache-2.0
//! The switches: tests chosen off, each with its reason, who set it and when. They live in the suite's own
//! settings on the phone (/etc/lpoco-health/switches), one tab-separated line a switch:
//!
//!   TEST <TAB> REASON <TAB> BY <TAB> AT
//!
//! A switch is a choice somebody made; what the phone finds out by itself (no SIM card, no network, a part
//! it has not got) is a detected condition (conditions.rs) and never lands here. The file is plain on
//! purpose: shell checks read it too (lib.sh switched_off), and a person can read it with cat.

use std::path::Path;

pub const FILE: &str = "/etc/lpoco-health/switches";

#[derive(Debug, Clone, PartialEq)]
pub struct Switch {
    pub test: String,
    pub reason: String,
    pub by: String,
    pub at: String,
}

fn clean(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c == '\t' || c == '\n' || c == '\r' {
                ' '
            } else {
                c
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn parse(text: &str) -> Vec<Switch> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let mut f = l.splitn(4, '\t');
            let test = f.next()?.trim();
            if test.is_empty() {
                return None;
            }
            Some(Switch {
                test: test.into(),
                reason: f.next().unwrap_or("").into(),
                by: f.next().unwrap_or("").into(),
                at: f.next().unwrap_or("").into(),
            })
        })
        .collect()
}

pub fn render(switches: &[Switch]) -> String {
    let mut s = String::from(
        "# lpoco-health switches: TEST, REASON, BY, AT (tab-separated). Change them with lpoco-health on|off.\n",
    );
    for w in switches {
        s.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            clean(&w.test),
            clean(&w.reason),
            clean(&w.by),
            clean(&w.at)
        ));
    }
    s
}

pub fn load(path: &Path) -> Vec<Switch> {
    parse(&std::fs::read_to_string(path).unwrap_or_default())
}

pub fn save(path: &Path, switches: &[Switch]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("new");
    std::fs::write(&tmp, render(switches))?;
    std::fs::rename(&tmp, path)
}

/// Turns TEST off with a reason (a switch already there is replaced: the latest reason and hand count).
/// A reason is required: a test off without one is a test forgotten.
pub fn off(
    switches: &mut Vec<Switch>,
    test: &str,
    reason: &str,
    by: &str,
    at: &str,
) -> Result<(), String> {
    let (test, reason) = (clean(test), clean(reason));
    if test.is_empty() {
        return Err("which test? (lpoco-health list says them)".into());
    }
    if reason.is_empty() {
        return Err(format!(
            "{test}: a test is switched off with a reason (--reason WORDS)"
        ));
    }
    switches.retain(|w| w.test != test);
    switches.push(Switch {
        test,
        reason,
        by: clean(by),
        at: clean(at),
    });
    switches.sort_by(|a, b| a.test.cmp(&b.test));
    Ok(())
}

/// Turns TEST on again; false when it was not off.
pub fn on(switches: &mut Vec<Switch>, test: &str) -> bool {
    let n = switches.len();
    switches.retain(|w| w.test != test.trim());
    switches.len() != n
}

pub fn find<'a>(switches: &'a [Switch], test: &str) -> Option<&'a Switch> {
    switches.iter().find(|w| w.test == test)
}

/// What a switched-off test reports in its place.
pub fn said(w: &Switch) -> String {
    format!(
        "switched off: {} (by {}, {})",
        w.reason,
        if w.by.is_empty() { "unknown" } else { &w.by },
        if w.at.is_empty() {
            "unknown time"
        } else {
            &w.at
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_switch_needs_a_reason_and_round_trips_through_the_file() {
        let mut s = Vec::new();
        assert!(off(&mut s, "sms-loopback", "  ", "me", "t").is_err());
        off(
            &mut s,
            "sms-loopback",
            "no\tSIM\nplan",
            "grenudi",
            "2026-11-17T10:00:00Z",
        )
        .unwrap();
        let back = parse(&render(&s));
        assert_eq!(back, s);
        assert_eq!(back[0].reason, "no SIM plan");
    }

    #[test]
    fn off_replaces_and_on_removes() {
        let mut s = Vec::new();
        off(&mut s, "a", "one", "x", "t1").unwrap();
        off(&mut s, "a", "two", "y", "t2").unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(find(&s, "a").unwrap().reason, "two");
        assert!(on(&mut s, "a"));
        assert!(!on(&mut s, "a"));
    }
}
