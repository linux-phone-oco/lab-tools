#!/usr/bin/env rust-script
//! An attended procedure as a lane writes it: a plain list of named steps (copy this file, change the steps).
//! A failing step stops the run and is named; --dry-run touches nothing; --log FILE is the log to file.
//! phone().sh(..) runs a script on the phone as the user, phone().root(..) as root.
//!
//! ```cargo
//! [dependencies]
//! lpoco-attended = { git = "https://github.com/linux-phone-oco/lab-tools", tag = "lpoco-attended-v0.28.3" }
//! ```
// SPDX-License-Identifier: MIT OR Apache-2.0

use lpoco_attended::prelude::*;

fn main() {
    let _run = procedure("boot the pmicwdt kernel", "xiaomi-surya");

    step("the phone answers", || retry(3, 1, || phone().sh("true")));
    ask_author("hold volume down and plug the cable in / the screen shows the fastboot logo");
    step("put the kernel on it", || {
        phone().root("echo flashing boot.img")
    });
    check("the kernel is the one we flashed", || {
        phone().sh("uname -r").map(|k| k.contains("pmicwdt"))
    });
    check("pmicwdt is bound", || {
        phone().sh("lsmod").map(|m| m.contains("pmicwdt"))
    });
}
