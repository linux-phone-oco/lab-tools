// SPDX-License-Identifier: MIT OR Apache-2.0
//! What `image unpack` writes packs back to the very image it came from (no mkbootimg needed), and a manifest
//! without a [boot] table is refused in words.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fx(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .to_str()
        .unwrap()
        .to_string()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lpoco-bringup-rt-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn bringup(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lpoco-bringup"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn unpack_then_pack_gives_the_image_back() {
    let dir = tmp("rt");
    let s = |n: &str| dir.join(n).to_str().unwrap().to_string();
    let (img, again, parts) = (s("a.img"), s("b.img"), s("parts"));
    let o = bringup(&[
        "image",
        "pack",
        "--device",
        &fx("device.toml"),
        "--kernel",
        &fx("kernel"),
        "--dtb",
        &fx("appended.dtb"),
        "--ramdisk",
        &fx("ramdisk"),
        "--out",
        &img,
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let o = bringup(&["image", "unpack", &img, "--out", &parts]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let o = bringup(&[
        "image",
        "pack",
        "--device",
        &format!("{parts}/boot.toml"),
        "--kernel",
        &format!("{parts}/kernel"),
        "--ramdisk",
        &format!("{parts}/ramdisk"),
        "--out",
        &again,
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(fs::read(&img).unwrap(), fs::read(&again).unwrap());
}

#[test]
fn a_manifest_without_boot_is_refused_with_exit_2() {
    let dir = tmp("noboot");
    let toml = dir.join("device.toml");
    fs::write(&toml, "[device]\ncodename = \"x\"\n").unwrap();
    let o = bringup(&[
        "image",
        "check",
        &fx("kernel"),
        "--device",
        toml.to_str().unwrap(),
    ]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("no [boot] table"));
}
