// SPDX-License-Identifier: MIT OR Apache-2.0
//! The library's steps: small, device-agnostic functions that return a [`Step`]; a lane's procedure only chains
//! them with its own values. Here: ask, dmesg_clean, check_probe, wait_for_phone. More come as a second device needs one (boot_once, read_dmesg,
//! wait_for_usb, back_to_known_good), each keyed to its bring-up step id and taking its device values from
//! device.toml / [layer], never hard-coded.

use crate::{Kind, Step};
use std::io::BufRead;

/// The author does something and looks at something ("what to do / what to check"); "ok" goes on, anything
/// else is what was seen and stops the run; nobody answering stops it too.
pub fn ask(what: &str) -> Step {
    let (to_do, to_check) = what
        .split_once(" / ")
        .unwrap_or((what, "it went as expected"));
    let (to_do, to_check) = (to_do.to_string(), to_check.to_string());
    let name = format!("ask the author: {to_do}");
    let to_do2 = to_do.clone();
    Step::new(Kind::Ask, &name, move || {
        let to_do = to_do2;
        println!("  ATTENDED - please {to_do}.");
        println!("  Check that {to_check}. Answer ok, or say what you saw instead:");
        let mut line = String::new();
        let mut got = 1;
        while got > 0 && line.trim().is_empty() {
            line.clear();
            got = std::io::stdin().lock().read_line(&mut line).unwrap_or(0);
        }
        let answer = line.trim();
        if got > 0 {
            crate::note(
                "asked",
                &format!("ask the author: {to_do}"),
                &format!("check: {to_check}; answer: {answer}"),
            );
        }
        if got == 0 {
            Err("nobody answered".to_string())
        } else if answer.eq_ignore_ascii_case("ok") {
            Ok(())
        } else {
            Err(format!(
                "expected that {to_check}, the author saw: {answer}"
            ))
        }
    })
}

/// The kernel messages that mean the kernel itself is unhappy: an oops, a BUG, a panic, a lockup, a stall.
const TROUBLE: [&str; 9] = [
    "Oops",
    "BUG:",
    "Kernel panic",
    "Call trace:",
    "Call Trace:",
    "Unable to handle kernel",
    "Internal error:",
    "lockup",
    "rcu: INFO",
];

/// The lines of a dmesg that show kernel trouble.
pub fn trouble_lines(dmesg: &str) -> Vec<&str> {
    dmesg
        .lines()
        .filter(|l| TROUBLE.iter().any(|t| l.contains(t)))
        .collect()
}

/// The kernel log (the ring buffer: an early oops that wrapped out is not seen) shows no oops, BUG, panic or
/// lockup (bring-up step dmesg-clean).
pub fn dmesg_clean() -> Step {
    Step::new(Kind::Check, "dmesg shows no oops, BUG or panic", || {
        let log = crate::phone().root("dmesg")?;
        let bad = trouble_lines(&log);
        if bad.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "{} line(s) of trouble, first: {}",
                bad.len(),
                bad[0].trim()
            ))
        }
    })
    .keyed("dmesg-clean")
}

/// A driver is bound: its folder is under /sys/bus/*/drivers. Positive evidence: a dmesg line that only names
/// the driver, or says "deferred probe pending", proves nothing. The name is letters, digits, `_` and `-`.
pub fn check_probe(driver: &str) -> Step {
    let driver = driver.to_string();
    Step::new(
        Kind::Check,
        &format!("the {driver} driver is bound"),
        move || {
            let word = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
            if driver.is_empty() || !driver.chars().all(word) {
                return Err(format!("not a driver name: {driver:?}"));
            }
            crate::phone().sh(&format!("ls -d /sys/bus/*/drivers/{driver} >/dev/null"))
        },
    )
}

/// The phone answers through devlab, tried `tries` times `every` seconds apart. After a reboot command, wait
/// for the phone to go down first: the old instance still answers.
pub fn wait_for_phone(tries: u32, every: u64) -> Step {
    Step::new(Kind::Check, "the phone answers", move || {
        crate::retry(tries, every, || crate::phone().sh("true"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trouble_is_found_in_a_dmesg_and_a_quiet_one_has_none() {
        let log = "[1.0] boot ok\n[2.0] Unable to handle kernel NULL pointer\n[2.1] Call trace:\n[2.2] Oops: 0000\n[2.3] BUG: scheduling\n[2.4] Kernel panic - not syncing\n[2.5] watchdog: BUG: soft lockup\n[2.6] Call Trace:\n[2.7] Internal error: Oops\n[2.8] rcu: INFO: stall\n";
        assert_eq!(trouble_lines(log).len(), 9);
        assert!(trouble_lines("[1.0] all quiet\n[1.1] deferred probe pending\n").is_empty());
    }
}
