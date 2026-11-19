// SPDX-License-Identifier: MIT OR Apache-2.0
//! The library's steps: small, device-agnostic functions that return a [`Step`]; a lane's procedure only chains
//! them with its own values. Here: ask, dmesg_clean, check_probe, wait_for_phone, read_dmesg, wait_for_usb, boot_once,
//! back_to_known_good. More come as a second device needs one, each keyed to its bring-up step id and taking its device values from
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

/// The kernel log (as root) holds a line with this text; the failure names the text. A positive look at what the
/// kernel said, as `check_probe` is for a driver (bring-up steps read it for their own message).
pub fn read_dmesg(text: &str) -> Step {
    let text = text.to_string();
    Step::new(Kind::Check, &format!("dmesg says \"{text}\""), move || {
        if text.trim().is_empty() {
            return Err("not a dmesg text: it is empty".into());
        }
        let log = crate::phone().root("dmesg")?;
        if log.contains(&text) {
            Ok(())
        } else {
            Err(format!("no dmesg line holds \"{text}\""))
        }
    })
}

/// The phone is on this computer's USB bus as `vendor:product` (four hex digits each, e.g. `18d1:4ee0`), tried
/// `tries` times `every` seconds apart. The host's own view (sysfs), so it needs no phone answer: it tells a
/// phone that booted to a gadget from one that did not. `LPOCO_ATTENDED_SYSFS_USB` points at another bus folder
/// (the tests' fake). Keyed to bring-up step gadget-enumerates, but it proves only the vendor:product: the serial
/// half of that step is not checked, and the id is the lane's to give (our own kernel's gadget, not the stock one).
pub fn wait_for_usb(id: &str, tries: u32, every: u64) -> Step {
    let id = id.to_ascii_lowercase();
    Step::new(
        Kind::Check,
        &format!("USB device {id} is on the bus"),
        move || {
            let ok = id.split_once(':').is_some_and(|(v, p)| {
                [v, p]
                    .iter()
                    .all(|h| h.len() == 4 && h.chars().all(|c| c.is_ascii_hexdigit()))
            });
            if !ok {
                return Err(format!("not a USB id (vendor:product): {id:?}"));
            }
            let root = crate::sysfs_usb();
            crate::retry(tries, every, || {
                usb_ids(root)?
                    .contains(&id)
                    .then_some(())
                    .ok_or(format!("{id} is not on the USB bus"))
            })
        },
    )
    .keyed("gadget-enumerates")
}

/// Boot the named test kernel once, through devlab's `kernel try` (never around it): devlab refuses an entry with
/// no try left, reboots, waits for the phone and measures the boot it came to; its output is the step's.
/// Any later boot is the default again. The name is the kernel's own (lowercase letters, digits, `-`: devlab's
/// rule, so a `+`, which is systemd-boot's try counter, is refused here before the phone is reached).
pub fn boot_once(name: &str) -> Step {
    let name = name.to_string();
    Step::new(Kind::Do, &format!("boot {name} once"), move || {
        let word = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-';
        if name.is_empty() || !name.chars().all(word) {
            return Err(format!(
                "not a test kernel name: {name:?} (lowercase letters, digits and -)"
            ));
        }
        let out = crate::devlab(&["kernel", "try", &name])?;
        print!("{out}");
        Ok::<(), String>(())
    })
}

/// Go back to the phone's known good: tell the phone, as root, to reboot, so the default entry (the stock or the
/// kept kernel; a try is used up) boots. It does not wait: follow it with [`wait_for_phone`], after the phone
/// has gone down, as that step says.
pub fn back_to_known_good() -> Step {
    Step::new(Kind::Do, "reboot into the default kernel", || {
        crate::phone()
            .root("set -e\nsync\nsystemd-run --on-active=2 systemctl reboot >/dev/null\necho rebooting\n")
            .map(|_| ())
    })
}

/// The `vendor:product` of every device under a sysfs USB devices folder.
fn usb_ids(root: &str) -> Result<Vec<String>, String> {
    let read = |d: &std::path::Path, f: &str| {
        std::fs::read_to_string(d.join(f))
            .ok()
            .map(|s| s.trim().to_ascii_lowercase())
    };
    let dir = std::fs::read_dir(root).map_err(|e| format!("cannot read {root}: {e}"))?;
    Ok(dir
        .flatten()
        .filter_map(|e| {
            Some(format!(
                "{}:{}",
                read(&e.path(), "idVendor")?,
                read(&e.path(), "idProduct")?
            ))
        })
        .collect())
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
