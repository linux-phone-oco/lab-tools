//! bringup-swap-s0-reproduces-control: `lpoco-bringup image swap` replaces parts of a control image and keeps
//! its AVB hash footer. Swapping nothing (s0) reproduces the control except the header id and the footer's
//! digest; swapping the ramdisk changes the ramdisk, the header sizes, the id and the footer, and nothing else.
//! The control is made here: a packed image, its id set apart, a footer by avbtool (algorithm NONE). Where
//! avbtool is absent the test says "not measured: no avbtool".

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const PAGE: usize = 4096;
const ID: std::ops::Range<usize> = 576..608; // the header's id field
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
    let d = std::env::temp_dir().join(format!("lpoco-bringup-swap-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn have_avbtool() -> bool {
    Command::new("avbtool")
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

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn avb(args: &[&str]) -> Output {
    Command::new("avbtool").args(args).output().unwrap()
}

/// A manifest whose partition holds 128 KiB, so the control is small.
fn manifest(dir: &Path) -> PathBuf {
    let t = fs::read_to_string(fixture("device.toml"))
        .unwrap()
        .replace("0x2000000", "0x20000");
    let p = dir.join("device.toml");
    fs::write(&p, t).unwrap();
    p
}

/// The control: packed, its id set to a stock-like value, then given an AVB hash footer.
fn control(dir: &Path, device: &Path) -> PathBuf {
    let img = dir.join("control.img");
    let o = bringup(&[
        "image",
        "pack",
        "--device",
        s(device),
        "--kernel",
        &fixture("kernel"),
        "--dtb",
        &fixture("appended.dtb"),
        "--ramdisk",
        &fixture("ramdisk"),
        "--out",
        s(&img),
    ]);
    assert!(o.status.success(), "pack: {}", text(&o.stderr));
    let mut b = fs::read(&img).unwrap();
    b[ID].fill(0xAA);
    fs::write(&img, b).unwrap();
    let o = avb(&[
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
        "--prop",
        "com.example:one",
    ]);
    assert!(o.status.success(), "avbtool: {}", text(&o.stderr));
    img
}

fn differing(a: &[u8], b: &[u8]) -> Vec<usize> {
    assert_eq!(
        a.len(),
        b.len(),
        "the swapped image is not the control's size"
    );
    (0..a.len()).filter(|&i| a[i] != b[i]).collect()
}

#[test]
fn bringup_swap_s0_reproduces_control() {
    if !have_avbtool() {
        eprintln!("not measured: no avbtool");
        return;
    }
    let dir = tmp("s0");
    let device = manifest(&dir);
    let ctl = control(&dir, &device);
    let want = fs::read(&ctl).unwrap();

    // s0: nothing swapped
    let s0 = dir.join("s0.img");
    let o = bringup(&["image", "swap", s(&ctl), "--out", s(&s0)]);
    assert!(o.status.success(), "swap s0: {}", text(&o.stderr));
    let got = fs::read(&s0).unwrap();
    let diff = differing(&want, &got);
    let (in_id, rest): (Vec<usize>, Vec<usize>) = diff.iter().partition(|i| ID.contains(*i));
    assert!(
        in_id.len() <= 32,
        "the id field is 32 bytes, got {} differing",
        in_id.len()
    );
    assert!(
        rest.iter().all(|&i: &usize| i >= 3 * PAGE),
        "s0 changed a part: {rest:?}"
    );
    assert!(
        rest.len() <= 32,
        "s0 changed more than the footer's digest: {} bytes",
        rest.len()
    );
    assert!(
        text(&o.stdout).contains("differs"),
        "swap says what differs: {}",
        text(&o.stdout)
    );

    // the ramdisk swapped
    let rd = dir.join("ramdisk2");
    fs::write(&rd, vec![0x5Au8; 5000]).unwrap();
    let out = dir.join("s1.img");
    let o = bringup(&[
        "image",
        "swap",
        s(&ctl),
        "--ramdisk",
        s(&rd),
        "--out",
        s(&out),
    ]);
    assert!(o.status.success(), "swap ramdisk: {}", text(&o.stderr));
    let got = fs::read(&out).unwrap();
    assert_eq!(got.len(), want.len());
    // the kernel pages are the control's (header page, then the kernel: 6181 bytes in 2 pages)
    assert_eq!(
        &got[PAGE..3 * PAGE],
        &want[PAGE..3 * PAGE],
        "the kernel changed"
    );
    assert_eq!(
        &got[3 * PAGE..3 * PAGE + 5000],
        &vec![0x5Au8; 5000][..],
        "the ramdisk is not the new one"
    );
    assert!(text(&o.stdout).contains("ramdisk"), "{}", text(&o.stdout));
    // the footer is a real one over the new image, with the control's salt and props
    let info = text(&avb(&["info_image", "--image", s(&out)]).stdout);
    assert!(info.contains("Salt:                  aabbccdd"), "{info}");
    assert!(info.contains("Prop: com.example -> 'one'"), "{info}");
    // header page + kernel (2 pages) + ramdisk (2 pages)
    assert!(
        info.contains("Original image size:      20480 bytes"),
        "{info}"
    );
    let digest = |t: &str| {
        t.lines()
            .find(|l| l.contains("Digest:"))
            .unwrap()
            .to_string()
    };
    let before = text(&avb(&["info_image", "--image", s(&ctl)]).stdout);
    assert_ne!(
        digest(&info),
        digest(&before),
        "the digest is not over the new image"
    );
    // and it unpacks with the new ramdisk
    let un = dir.join("un");
    let o = bringup(&[
        "image",
        "unpack",
        s(&out),
        "--out",
        s(&un),
        "--device",
        s(&device),
    ]);
    assert!(o.status.success(), "unpack: {}", text(&o.stderr));
    assert_eq!(fs::read(un.join("ramdisk")).unwrap(), vec![0x5Au8; 5000]);
}

#[test]
fn swap_carries_the_footers_non_default_facts_and_a_footerless_size() {
    if !have_avbtool() {
        eprintln!("not measured: no avbtool");
        return;
    }
    let dir = tmp("facts");
    let device = manifest(&dir);
    let plain = dir.join("plain.img");
    let o = bringup(&[
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
        s(&plain),
    ]);
    assert!(o.status.success(), "pack: {}", text(&o.stderr));
    let rd = dir.join("ramdisk2");
    fs::write(&rd, vec![0x5Au8; 5000]).unwrap();

    // no footer: the control's size is kept, zero-filled
    let mut b = fs::read(&plain).unwrap();
    b.resize(PART.parse().unwrap(), 0);
    let padded = dir.join("padded.img");
    fs::write(&padded, &b).unwrap();
    let out = dir.join("padded-out.img");
    let o = bringup(&[
        "image",
        "swap",
        s(&padded),
        "--ramdisk",
        s(&rd),
        "--out",
        s(&out),
    ]);
    assert!(o.status.success(), "swap: {}", text(&o.stderr));
    assert_eq!(fs::read(&out).unwrap().len(), b.len());

    // a footer with non-default facts: they are carried
    let with = dir.join("with.img");
    fs::copy(&plain, &with).unwrap();
    let o = avb(&[
        "add_hash_footer",
        "--image",
        s(&with),
        "--partition_size",
        PART,
        "--partition_name",
        "boot",
        "--algorithm",
        "NONE",
        "--salt",
        "aabb",
        "--rollback_index",
        "7",
        "--hash_algorithm",
        "sha512",
        "--do_not_use_ab",
    ]);
    assert!(o.status.success(), "avbtool: {}", text(&o.stderr));
    let out = dir.join("with-out.img");
    let o = bringup(&[
        "image",
        "swap",
        s(&with),
        "--ramdisk",
        s(&rd),
        "--out",
        s(&out),
    ]);
    assert!(o.status.success(), "swap: {}", text(&o.stderr));
    let info = text(&avb(&["info_image", "--image", s(&out)]).stdout);
    assert!(info.contains("Rollback Index:           7"), "{info}");
    assert!(info.contains("Hash Algorithm:        sha512"), "{info}");
    assert!(info.contains("Flags:                 1"), "{info}");
}
