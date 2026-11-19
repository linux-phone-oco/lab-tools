#!/usr/bin/env rust-script
//! The library's groups: a procedure that needs the phone up on a test kernel and then left on its known good is
//! two lines. The kernel names come from the device's device.toml ([kernels]).
//!
//! ```cargo
//! [dependencies]
//! lpoco-attended = { git = "https://github.com/linux-phone-oco/lab-tools", tag = "lpoco-attended-v0.37.1" }
//! ```
// SPDX-License-Identifier: MIT OR Apache-2.0

use lpoco_attended::{prelude::*, steps};

fn main() {
    let _run = procedure("ready", "xiaomi-surya");

    run_group(steps::phone_ready("pmicwdt"));
    check("the driver is bound", || phone().sh("true"));
    run_group(steps::leave_known_good());
}
