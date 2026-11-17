//! bringup-pack-matches-mkbootimg: `lpoco-bringup image pack` writes, for walleye's header facts (device.toml
//! [boot]), the very bytes mkbootimg writes; `image check` fails a wrong ramdisk offset and passes the right one.
//! mkbootimg (android-tools) is the oracle; where it is absent the test says "not measured: no mkbootimg".

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn fx(name: &str) -> String {
    fixture(name).to_str().unwrap().to_string()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lpoco-bringup-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn have_mkbootimg() -> bool {
    Command::new("mkbootimg")
        .arg("--help")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn bringup(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lpoco-bringup"))
        .args(args)
        .output()
        .unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// The kernel blob walleye boots: the kernel with its device tree appended.
fn kernel_blob(dir: &Path) -> PathBuf {
    let mut b = fs::read(fixture("kernel")).unwrap();
    b.extend(fs::read(fixture("appended.dtb")).unwrap());
    let p = dir.join("kernel-dtb");
    fs::write(&p, b).unwrap();
    p
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

/// mkbootimg's image for the fixture's [boot] facts, with the os words when given.
fn oracle(dir: &Path, os: Option<(&str, &str)>) -> PathBuf {
    let out = dir.join("oracle.img");
    let kernel = kernel_blob(dir);
    let mut cmd = Command::new("mkbootimg");
    cmd.args([
        "--header_version",
        "0",
        "--base",
        "0x0",
        "--kernel_offset",
        "0x8000",
        "--ramdisk_offset",
        "0x1000000",
        "--second_offset",
        "0xf00000",
        "--tags_offset",
        "0x100",
        "--pagesize",
        "4096",
        "--cmdline",
        "earlycon=msm_serial_dm,0xc1b0000 console=ttyGS0,115200",
        "--kernel",
        s(&kernel),
        "--ramdisk",
        &fx("ramdisk"),
        "-o",
        s(&out),
    ]);
    if let Some((v, p)) = os {
        cmd.args(["--os_version", v, "--os_patch_level", p]);
    }
    assert!(cmd.status().unwrap().success(), "mkbootimg failed");
    out
}

#[test]
fn bringup_pack_matches_mkbootimg() {
    if !have_mkbootimg() {
        eprintln!("not measured: no mkbootimg");
        return;
    }
    for (name, os) in [("plain", None), ("os-words", Some(("8.1.0", "2020-01")))] {
        let dir = tmp(name);
        let want = oracle(&dir, os);
        let mine = dir.join("mine.img");
        let (dtb, kernel, ramdisk) = (fx("appended.dtb"), fx("kernel"), fx("ramdisk"));
        let device = fx("device.toml");
        let mut args = vec![
            "image",
            "pack",
            "--device",
            &device,
            "--kernel",
            &kernel,
            "--dtb",
            &dtb,
            "--ramdisk",
            &ramdisk,
            "--out",
            s(&mine),
        ];
        // the os words come from [boot] in the manifest; the second case adds them
        let toml;
        if let Some((v, p)) = os {
            let t = fs::read_to_string(fixture("device.toml")).unwrap()
                + &format!("os_version = \"{v}\"\nos_patch_level = \"{p}\"\n");
            toml = dir.join("device.toml");
            fs::write(&toml, t).unwrap();
            args[3] = s(&toml);
        }
        let o = bringup(&args);
        assert!(
            o.status.success(),
            "{name}: pack failed: {}",
            text(&o.stderr)
        );
        assert_eq!(
            fs::read(&mine).unwrap(),
            fs::read(&want).unwrap(),
            "{name}: image pack differs from mkbootimg"
        );
    }
}

#[test]
fn bringup_check_fails_a_wrong_ramdisk_offset_and_passes_the_right_one() {
    if !have_mkbootimg() {
        eprintln!("not measured: no mkbootimg");
        return;
    }
    let dir = tmp("check");
    let img = oracle(&dir, None);
    let right = fixture("device.toml");
    let o = bringup(&["image", "check", s(&img), "--device", s(&right)]);
    assert!(
        o.status.success(),
        "right offset refused: {}{}",
        text(&o.stdout),
        text(&o.stderr)
    );
    let wrong = dir.join("wrong.toml");
    let t = fs::read_to_string(&right)
        .unwrap()
        .replace("ramdisk_offset = 0x1000000", "ramdisk_offset = 0x2000000");
    fs::write(&wrong, t).unwrap();
    let o = bringup(&["image", "check", s(&img), "--device", s(&wrong)]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o.stdout));
    let out = text(&o.stdout);
    assert!(
        out.contains("FAIL ramdisk offset"),
        "the miss is named: {out}"
    );
}

#[test]
fn bringup_check_counts_what_the_device_expects() {
    if !have_mkbootimg() {
        eprintln!("not measured: no mkbootimg");
        return;
    }
    let dir = tmp("counts");
    let img = oracle(&dir, None);
    let o = bringup(&["image", "check", s(&img), "--device", &fx("device.toml")]);
    let out = text(&o.stdout);
    for line in [
        "ok   magic",
        "ok   header version",
        "ok   size within partition",
        "ok   kernel offset",
        "ok   ramdisk offset",
        "ok   tags offset",
        "ok   page size",
        "ok   os words",
        "ok   cmdline",
        "ok   appended dtb",
        "ok   qcom,msm-id",
    ] {
        assert!(out.contains(line), "missing `{line}` in:\n{out}");
    }
}
