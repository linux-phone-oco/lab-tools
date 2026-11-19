#!/usr/bin/env rust-script
//! A device repository's own step, as a rust-script that uses lpoco-bringup as a library: it reads the
//! device manifest's [boot] table, packs a header-v0 image from a kernel and a ramdisk, and counts the image
//! against the manifest. Usage: step.rs DEVICE.toml KERNEL RAMDISK OUT.img
//!
//! ```cargo
//! [dependencies]
//! lpoco-bringup = { git = "https://github.com/linux-phone-oco/lab-tools", tag = "lpoco-v0.1.7" }
//! ```
// SPDX-License-Identifier: MIT OR Apache-2.0

use lpoco_bringup::{boot::Boot, image};
use std::path::Path;

fn main() -> Result<(), String> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let [device, kernel, ramdisk, out] = a.as_slice() else {
        return Err("usage: step.rs DEVICE.toml KERNEL RAMDISK OUT.img".into());
    };
    lpoco_bringup::pin::check(Path::new(device), env!("CARGO_PKG_VERSION"))?;
    let boot = Boot::read(Path::new(device))?;
    let read = |p: &String| std::fs::read(p).map_err(|e| format!("cannot read {p}: {e}"));
    let img = image::pack(&boot, &read(kernel)?, &read(ramdisk)?)?;
    let failed = image::check(&img, &boot)
        .iter()
        .filter(|f| matches!(f.verdict, image::Verdict::Fail(_)))
        .count();
    if failed > 0 {
        return Err(format!("{failed} checks failed"));
    }
    std::fs::write(out, &img).map_err(|e| format!("cannot write {out}: {e}"))
}
