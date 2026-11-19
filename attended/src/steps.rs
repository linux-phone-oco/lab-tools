// SPDX-License-Identifier: MIT OR Apache-2.0
//! The library's steps: small, device-agnostic functions that return a [`Step`]; a lane's procedure only chains
//! them with its own values. More come as a second device needs one (boot_once, read_dmesg, check_probe,
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
