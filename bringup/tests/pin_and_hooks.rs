//! The family's pin and the manifest's step hooks: a manifest's `[tools] lpoco = "X.Y"` stops a different
//! family version in one line; a `[boot.hooks]` step runs an executable on the parts before they are packed.

use std::fs;
use std::os::unix::fs::PermissionsExt;
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
    let d = std::env::temp_dir().join(format!("lpoco-bringup-ph-{}-{name}", std::process::id()));
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

fn manifest(dir: &Path, extra_top: &str, hooks: &str) -> String {
    let base = fs::read_to_string(fx("device.toml")).unwrap();
    let text = format!("{base}\n{hooks}\n{extra_top}");
    let p = dir.join("device.toml");
    fs::write(&p, text).unwrap();
    p.to_str().unwrap().to_string()
}

fn script(dir: &Path, name: &str, body: &str) {
    let p = dir.join(name);
    fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
}

fn pack(dev: &str, out: &str) -> Output {
    bringup(&[
        "image",
        "pack",
        "--device",
        dev,
        "--kernel",
        &fx("kernel"),
        "--ramdisk",
        &fx("ramdisk"),
        "--out",
        out,
    ])
}

#[test]
fn bringup_refuses_wrong_pin() {
    let dir = tmp("pin");
    let dev = manifest(&dir, "[tools]\nlpoco = \"99.0\"\n", "");
    let out = dir.join("a.img");
    for args in [
        vec!["image", "check", &fx("kernel"), "--device", &dev],
        vec!["image", "info", &fx("kernel"), "--device", &dev],
    ] {
        let o = bringup(&args);
        assert_eq!(o.status.code(), Some(2));
        let err = String::from_utf8_lossy(&o.stderr).to_string();
        assert_eq!(err.trim().lines().count(), 1, "{err}");
        assert!(
            err.contains("99.0") && err.contains(env!("CARGO_PKG_VERSION")),
            "{err}"
        );
    }
    let o = pack(&dev, out.to_str().unwrap());
    assert_eq!(o.status.code(), Some(2));
    assert!(!out.exists(), "nothing is written");
}

#[test]
fn a_matching_pin_lets_the_job_run() {
    let dir = tmp("pinok");
    let v: Vec<&str> = env!("CARGO_PKG_VERSION").split('.').collect();
    let dev = manifest(
        &dir,
        &format!("[tools]\nlpoco = \"{}.{}\"\n", v[0], v[1]),
        "",
    );
    let out = dir.join("a.img");
    let o = pack(&dev, out.to_str().unwrap());
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

#[test]
fn a_parts_hook_changes_the_ramdisk_before_packing() {
    let dir = tmp("hook");
    script(&dir, "grow.sh", "printf 'X' >> \"$2\"");
    let dev = manifest(&dir, "", "[boot.hooks]\nparts = \"grow.sh\"\n");
    let (a, b) = (dir.join("a.img"), dir.join("b.img"));
    let o = pack(&dev, a.to_str().unwrap());
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let plain = tmp("plain");
    let pdev = manifest(&plain, "", "");
    let o = pack(&pdev, b.to_str().unwrap());
    assert!(o.status.success());
    let (la, lb) = (
        fs::metadata(&a).unwrap().len(),
        fs::metadata(&b).unwrap().len(),
    );
    assert_ne!(fs::read(&a).unwrap(), fs::read(&b).unwrap(), "{la} {lb}");
    // the user's own ramdisk is untouched
    assert_eq!(
        fs::read(fx("ramdisk")).unwrap(),
        fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ramdisk")).unwrap()
    );
}

#[test]
fn a_failing_or_missing_hook_stops_the_job() {
    let dir = tmp("hookfail");
    script(&dir, "bad.sh", "exit 3");
    let out = dir.join("a.img");
    let dev = manifest(&dir, "", "[boot.hooks]\nparts = \"bad.sh\"\n");
    let o = pack(&dev, out.to_str().unwrap());
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("bad.sh"));
    assert!(!out.exists());

    let dir = tmp("hookmissing");
    let out = dir.join("a.img");
    let dev = manifest(&dir, "", "[boot.hooks]\nparts = \"nope.sh\"\n");
    let o = pack(&dev, out.to_str().unwrap());
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("nope.sh"));
    assert!(!out.exists());
}
