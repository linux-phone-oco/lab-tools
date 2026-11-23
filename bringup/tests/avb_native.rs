// SPDX-License-Identifier: MIT OR Apache-2.0
//! image-avb-footer-in-rust: `lpoco-bringup image swap` writes the AVB hash footer itself. With no avbtool on
//! PATH the swap succeeds and its bytes equal what the same swap gives with avbtool doing the footer (the
//! reference: avbtool 1.4.0 add_hash_footer, algorithm NONE). The control is made here by avbtool; where avbtool
//! is absent the test says "not measured: no avbtool".

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const PART: &str = "131072";

fn fixture(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .to_str()
        .unwrap()
        .to_string()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lpoco-bringup-avb-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn have_avbtool() -> bool {
    Command::new("avbtool")
        .arg("--help")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn avb(args: &[&str]) -> Output {
    Command::new("avbtool").args(args).output().unwrap()
}

/// The binary run with a PATH that holds no avbtool.
fn bringup_without_avbtool(dir: &Path, args: &[&str]) -> Output {
    let empty = dir.join("empty-path");
    fs::create_dir_all(&empty).unwrap();
    Command::new(env!("CARGO_BIN_EXE_lpoco-bringup"))
        .env("PATH", &empty)
        .args(args)
        .output()
        .unwrap()
}

fn packed(dir: &Path) -> (PathBuf, PathBuf) {
    let device = dir.join("device.toml");
    let t = fs::read_to_string(fixture("device.toml"))
        .unwrap()
        .replace("0x2000000", "0x20000");
    fs::write(&device, t).unwrap();
    let img = dir.join("plain.img");
    let o = bringup_without_avbtool(
        dir,
        &[
            "image",
            "pack",
            "--device",
            s(&device),
            "--kernel",
            &fixture("kernel"),
            "--dtb",
            &fixture("appended.dtb"),
            "--ramdisk",
            &fixture("ramdisk"),
            "--out",
            s(&img),
        ],
    );
    assert!(o.status.success(), "pack: {}", text(&o.stderr));
    (img, device)
}

/// The footer avbtool writes, for each fact a footer can carry that swap keeps.
fn control(dir: &Path, plain: &Path, name: &str, extra: &[&str]) -> PathBuf {
    let img = dir.join(name);
    fs::copy(plain, &img).unwrap();
    let mut args = vec![
        "add_hash_footer",
        "--image",
        s(&img),
        "--partition_size",
        PART,
        "--partition_name",
        "boot",
        "--algorithm",
        "NONE",
        "--salt",
        "aabbccdd",
    ];
    args.extend_from_slice(extra);
    let o = avb(&args);
    assert!(o.status.success(), "avbtool: {}", text(&o.stderr));
    img
}

/// What avbtool makes of the new body: the swap's body (the control's footer cut off) given the control's
/// facts, by avbtool's own add_hash_footer. Both sides share the control's release string, so the reference
/// is the same call the swap used to make.
fn reference(dir: &Path, swapped: &Path, extra: &[&str]) -> Vec<u8> {
    let b = fs::read(swapped).unwrap();
    let f = &b[b.len() - 64..];
    assert_eq!(&f[..4], b"AVBf", "the swapped image has no footer");
    let original = u64::from_be_bytes(f[12..20].try_into().unwrap()) as usize;
    let want = dir.join("reference.img");
    fs::write(&want, &b[..original]).unwrap();
    let mut args = vec![
        "add_hash_footer",
        "--image",
        s(&want),
        "--partition_size",
        PART,
        "--partition_name",
        "boot",
        "--algorithm",
        "NONE",
        "--salt",
        "aabbccdd",
    ];
    args.extend_from_slice(extra);
    let o = avb(&args);
    assert!(o.status.success(), "avbtool: {}", text(&o.stderr));
    fs::read(&want).unwrap()
}

fn swap_equals_avbtool(name: &str, extra: &[&str]) {
    if !have_avbtool() {
        eprintln!("not measured: no avbtool");
        return;
    }
    let dir = tmp(name);
    let (plain, _device) = packed(&dir);
    let ctl = control(&dir, &plain, "control.img", extra);
    let rd = dir.join("ramdisk2");
    fs::write(&rd, vec![0x5Au8; 5000]).unwrap();
    let out = dir.join("out.img");
    let o = bringup_without_avbtool(
        &dir,
        &[
            "image",
            "swap",
            s(&ctl),
            "--ramdisk",
            s(&rd),
            "--out",
            s(&out),
        ],
    );
    assert!(
        o.status.success(),
        "swap with no avbtool on PATH: {}",
        text(&o.stderr)
    );
    let got = fs::read(&out).unwrap();
    let want = reference(&dir, &out, extra);
    assert_eq!(got.len(), want.len(), "the footer's image size");
    let differing: Vec<usize> = (0..got.len()).filter(|&i| got[i] != want[i]).collect();
    assert!(
        differing.is_empty(),
        "{} bytes differ from avbtool's footer, the first at {:?}",
        differing.len(),
        differing.first()
    );
    // and avbtool accepts it
    let named = dir.join("boot.img");
    fs::copy(&out, &named).unwrap();
    let v = avb(&["verify_image", "--image", s(&named)]);
    assert!(v.status.success(), "verify_image: {}", text(&v.stderr));
}

#[test]
fn the_default_footer_is_avbtools_byte_for_byte() {
    swap_equals_avbtool("default", &["--prop", "com.example:one"]);
}

#[test]
fn a_footer_with_other_facts_is_avbtools_byte_for_byte() {
    swap_equals_avbtool(
        "facts",
        &[
            "--rollback_index",
            "7",
            "--hash_algorithm",
            "sha512",
            "--do_not_use_ab",
            "--prop",
            "a:b",
            "--prop",
            "long.key.name:a longer value, with spaces",
        ],
    );
}
