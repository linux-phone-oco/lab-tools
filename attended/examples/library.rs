#!/usr/bin/env rust-script
//! The library's own steps chained, as a lane does: `CASE` picks the chain (the tests run each against a stand-in phone).
//!
//! ```cargo
//! [dependencies]
//! lpoco-attended = { git = "https://github.com/linux-phone-oco/lab-tools", tag = "lpoco-attended-v0.29.0" }
//! ```
// SPDX-License-Identifier: MIT OR Apache-2.0

use lpoco_attended::prelude::*;
use lpoco_attended::steps::{check_probe, dmesg_clean, wait_for_phone};

fn main() {
    let _run = procedure("library steps", "xiaomi-surya");
    match std::env::var("CASE").as_deref() {
        Ok("dmesg") => run_step(dmesg_clean()),
        Ok("probe-missing") => run_step(check_probe("absent_drv")),
        Ok("probe-bound") => run_step(check_probe("bound_drv")),
        Ok("probe-bad-name") => run_step(check_probe("x; reboot")),
        _ => run_step(wait_for_phone(3, 0)),
    }
}
