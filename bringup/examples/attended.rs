#!/usr/bin/env rust-script
//! An attended procedure as a lane writes it: a plain list of named steps (copy this file, change the steps).
//! A failing step stops the run and is named; --dry-run touches nothing; --log FILE is the log to file.
//! phone(..) runs a script on the phone as the user, phone_root(..) as root, sh(..) on this machine.
//!
//! ```cargo
//! [dependencies]
//! lpoco-bringup = { git = "https://github.com/linux-phone-oco/lab-tools", tag = "lpoco-v0.28.1" }
//! ```
// SPDX-License-Identifier: MIT OR Apache-2.0

use lpoco_bringup::attended::*;

fn main() -> std::process::ExitCode {
    procedure("boot the pmicwdt kernel").run(|p| {
        p.step("the phone answers", phone("true").retry(3, 1))?;
        p.ask_author(
            "hold volume down and plug the cable in",
            "the screen shows the fastboot logo",
        )?;
        p.step("put the kernel on it", sh("echo flashing boot.img"))?;
        p.check(
            "the kernel is the one we flashed",
            phone("uname -r").expect("pmicwdt"),
        )?;
        p.check("pmicwdt is bound", phone("lsmod").expect("pmicwdt"))?;
        Ok(())
    })
}
