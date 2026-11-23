#!/usr/bin/env rust-script
//! An attended procedure with parts: the whole session by default, `--part NAME` for one named part of it
//! (one that needs no prompt and no risky step, so a keeper can run it alone).
//!
//! ```cargo
//! [dependencies]
//! lpoco-attended = { git = "https://github.com/linux-phone-oco/lab-tools", tag = "lpoco-attended-v0.29.0" }
//! ```
// SPDX-License-Identifier: MIT OR Apache-2.0

use lpoco_attended::prelude::*;

fn main() {
    let _run = procedure("a session with parts", "xiaomi-surya");
    match part().as_deref() {
        None => {
            step("the phone answers", || phone().sh("true"));
            ask_author("press the button / the light comes on");
            step("after the prompt", || phone().sh("true"));
        }
        Some("checks") => check_from("only the checks", "research:2026-11-23-checks", || {
            phone().sh("true")
        }),
        Some(other) => unknown_part(other, &["checks"]),
    }
}
