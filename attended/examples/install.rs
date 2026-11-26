#!/usr/bin/env rust-script
//! The library's install steps chained, as a lane does: `CASE` picks the step, `PART` the partition and `SHA` the sha256
//! the write must read back as, `CONNECTION` the named connection the steps run on (none: the device's default); the
//! tests run each against a stand-in phone.
//!
//! ```cargo
//! [dependencies]
//! lpoco-attended = { git = "https://github.com/linux-phone-oco/lab-tools", tag = "lpoco-attended-v0.45.7" }
//! ```
// SPDX-License-Identifier: MIT OR Apache-2.0

use lpoco_attended::prelude::*;
use lpoco_attended::steps::{adb_write_partition, mount_is_readonly, read_slot};

fn main() {
    let _run = procedure("install steps", "google-walleye");
    let env = |k: &str| std::env::var(k).unwrap_or_default();
    let allowed = std::env::var("ALLOWED").unwrap_or("system_a,system_b".into());
    let on = || match env("CONNECTION").as_str() {
        "" => phone(),
        name => phone_on(name),
    };
    match env("CASE").as_str() {
        "mount-ro" => run_step(mount_is_readonly(on(), &env("POINT"))),
        "write" => run_step(adb_write_partition(
            on(),
            &allowed.split(',').collect::<Vec<_>>(),
            &env("BY_NAME"),
            &env("PART"),
            &env("STAGED"),
            &env("SHA"),
        )),
        _ => run_step(read_slot(on())),
    }
}
