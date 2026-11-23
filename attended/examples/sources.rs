#!/usr/bin/env rust-script
//! A check names its source: `check_from(what, source, ..)` says where the value it expects came from - a research
//! row (`research:ID`), a doc (a `.md` path) or a file of text a phone printed (a path under a `research` folder),
//! all read from the device.toml's folder. `--part NAME` picks the check, for the tests of the dry run.
//!
//! ```cargo
//! [dependencies]
//! lpoco-attended = { git = "https://github.com/linux-phone-oco/lab-tools", tag = "lpoco-attended-v0.44.6" }
//! ```
// SPDX-License-Identifier: MIT OR Apache-2.0

use lpoco_attended::prelude::*;

fn main() {
    let _run = procedure("sources", "xiaomi-surya");
    let kernel = || phone().sh("uname -r").map(|k| !k.is_empty());
    let what = "the kernel is named";
    match part().as_deref() {
        Some("none") => check(what, kernel),
        Some("research") => check_from(what, "research:2026-11-23-uname", kernel),
        Some("captured") => check_from(what, "research/phone/uname.txt", kernel),
        Some("doc") => check_from(what, "docs/boot.md", kernel),
        Some("missing") => check_from(what, "research/phone/gone.txt", kernel),
        Some("empty") => check_from(what, "research/phone/empty.txt", kernel),
        Some("typed") => check_from(what, "tests/typed.txt", kernel),
        Some("blank") => check_from(what, "", kernel),
        Some("link") => check_from(what, "research/phone/out.txt", kernel),
        Some("nested") => check_from(what, "tests/research/typed.txt", kernel),
        Some(other) => unknown_part(
            other,
            &[
                "none", "research", "captured", "doc", "missing", "empty", "typed", "blank",
                "link", "nested",
            ],
        ),
        None => {}
    }
}
