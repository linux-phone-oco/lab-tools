// SPDX-License-Identifier: MIT OR Apache-2.0
//! bringup-bisect-runs-declared-series: `lpoco-bringup image bisect FILE --out DIR` builds the series of swaps a
//! file declares (walleye's s0, s0n, s1, s2, s3) from one control, writes DIR/boot-NAME.img for each, counts
//! that each image differs from the control in exactly the parts it declares (and its AVB footer verifies, or is
//! absent where declared so), and exits 1 on any miss. The kernel part may be put in an lz4 frame, as the stock
//! kernel is. The control here is made: an lz4-framed kernel with its dtb appended, a footer by avbtool. Where
//! avbtool or lz4 is absent the test says "not measured".

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const PART: &str = "131072";
const LZ4_MAGIC: [u8; 4] = [0x04, 0x22, 0x4d, 0x18];

fn fixture(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .to_str()
        .unwrap()
        .to_string()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "lpoco-bringup-bisect-{}-{name}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn have(tool: &str) -> bool {
    Command::new(tool)
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

/// The control: an lz4-framed kernel + appended dtb, packed against a manifest whose partition holds 128 KiB,
/// then given an AVB hash footer (algorithm NONE).
fn control(dir: &Path) -> PathBuf {
    let device = dir.join("device.toml");
    let t = fs::read_to_string(fixture("device.toml"))
        .unwrap()
        .replace("0x2000000", "0x20000");
    fs::write(&device, t).unwrap();
    let frame = Command::new("lz4")
        .args(["-9", "-BI", "-B7", "-c", &fixture("kernel")])
        .output()
        .unwrap()
        .stdout;
    fs::write(dir.join("kernel.lz4"), &frame).unwrap();
    let img = dir.join("control.img");
    let o = bringup(&[
        "image",
        "pack",
        "--device",
        s(&device),
        "--kernel",
        s(&dir.join("kernel.lz4")),
        "--dtb",
        &fixture("appended.dtb"),
        "--ramdisk",
        &fixture("ramdisk"),
        "--out",
        s(&img),
    ]);
    assert!(o.status.success(), "pack: {}", text(&o.stderr));
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

#[test]
fn bringup_bisect_runs_declared_series() {
    if !have("avbtool") || !have("lz4") {
        eprintln!("not measured: no avbtool or lz4");
        return;
    }
    let dir = tmp("series");
    let ctl = control(&dir);
    let ctl_bytes = fs::read(&ctl).unwrap();
    // the parts to swap in: a ramdisk, a kernel (raw: bisect frames it), a dtb differing in one string byte
    fs::write(dir.join("ramdisk2"), vec![0x5Au8; 5000]).unwrap();
    fs::write(dir.join("kernel2"), vec![0x4Du8; 9000]).unwrap();
    let mut dtb = fs::read(fixture("appended.dtb")).unwrap();
    let last = dtb.len() - 2;
    dtb[last] ^= 0x01;
    fs::write(dir.join("dtb2"), &dtb).unwrap();
    let series = dir.join("series.toml");
    fs::write(
        &series,
        r#"control = "control.img"
[[image]]
name = "s0"
[[image]]
name = "s0n"
footer = false
[[image]]
name = "s1"
ramdisk = "ramdisk2"
[[image]]
name = "s2"
kernel = "kernel2"
lz4 = true
[[image]]
name = "s3"
dtb = "dtb2"
"#,
    )
    .unwrap();
    let out = dir.join("out");
    let o = bringup(&["image", "bisect", s(&series), "--out", s(&out)]);
    assert!(
        o.status.success(),
        "bisect: {} {}",
        text(&o.stdout),
        text(&o.stderr)
    );
    let said = text(&o.stdout);
    for n in ["s0", "s0n", "s1", "s2", "s3"] {
        assert!(
            out.join(format!("boot-{n}.img")).is_file(),
            "no boot-{n}.img: {said}"
        );
        assert!(
            said.contains(&format!("ok   {n} ")),
            "{n} not counted ok: {said}"
        );
    }
    assert!(said.contains("5 images: 5 ok, 0 failed"), "{said}");

    // s0: the control's size and footer; s0n: no footer, smaller
    let s0 = fs::read(out.join("boot-s0.img")).unwrap();
    assert_eq!(s0.len(), ctl_bytes.len());
    let s0n = out.join("boot-s0n.img");
    assert!(
        !avb(&["info_image", "--image", s(&s0n)]).status.success(),
        "s0n has a footer"
    );
    assert!(fs::metadata(&s0n).unwrap().len() < ctl_bytes.len() as u64);

    // s2: the kernel part is an lz4 frame of the new kernel, the dtb the control's still
    let un = out.join("un-s2");
    let o = bringup(&[
        "image",
        "unpack",
        s(&out.join("boot-s2.img")),
        "--out",
        s(&un),
    ]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let blob = fs::read(un.join("kernel")).unwrap();
    assert_eq!(blob[..4], LZ4_MAGIC);
    // the frame alone: lz4 -dc on the frame plus the dtb behind it exits on that trailing garbage and (lz4 1.11, async
    // output) may drop what it decoded, empty at random under load; so the dtb is cut off before decoding
    let dtb_len = fs::metadata(un.join("appended.dtb")).unwrap().len() as usize;
    let framed = un.join("frame.lz4");
    fs::write(&framed, &blob[..blob.len() - dtb_len]).unwrap();
    let plain = Command::new("lz4")
        .args(["-dc", s(&framed)])
        .output()
        .unwrap();
    assert!(plain.status.success(), "lz4 -dc: {}", text(&plain.stderr));
    assert_eq!(&plain.stdout[..9000], &vec![0x4Du8; 9000][..]);
    assert_eq!(
        fs::read(un.join("appended.dtb")).unwrap(),
        fs::read(fixture("appended.dtb")).unwrap()
    );

    // s3: the dtb is the new one
    let un3 = out.join("un-s3");
    assert!(bringup(&[
        "image",
        "unpack",
        s(&out.join("boot-s3.img")),
        "--out",
        s(&un3)
    ])
    .status
    .success());
    assert_eq!(fs::read(un3.join("appended.dtb")).unwrap(), dtb);
}

#[test]
fn bringup_bisect_names_a_miss() {
    let dir = tmp("miss");
    let series = dir.join("series.toml");
    fs::write(
        &series,
        "control = \"nowhere.img\"\n[[image]]\nname = \"s0\"\n",
    )
    .unwrap();
    let o = bringup(&["image", "bisect", s(&series), "--out", s(&dir.join("out"))]);
    assert_eq!(o.status.code(), Some(2));
    assert!(
        text(&o.stderr).contains("nowhere.img"),
        "{}",
        text(&o.stderr)
    );
}

#[test]
fn bringup_bisect_counts_a_part_that_differs_undeclared_or_not_at_all() {
    if !have("avbtool") || !have("lz4") {
        eprintln!("not measured: no avbtool or lz4");
        return;
    }
    let dir = tmp("count");
    let ctl = control(&dir);
    // a ramdisk declared but equal to the control's: a miss
    let o = bringup(&["image", "unpack", s(&ctl), "--out", s(&dir.join("un"))]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    fs::copy(dir.join("un/ramdisk"), dir.join("ramdisk-same")).unwrap();
    let series = dir.join("series.toml");
    fs::write(
        &series,
        "control = \"control.img\"\n[[image]]\nname = \"same\"\nramdisk = \"ramdisk-same\"\n[[image]]\nname = \"ok\"\n",
    )
    .unwrap();
    let o = bringup(&["image", "bisect", s(&series), "--out", s(&dir.join("out"))]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o.stdout));
    let said = text(&o.stdout);
    assert!(said.contains("FAIL same"), "{said}");
    assert!(said.contains("equals the control's"), "{said}");
    assert!(said.contains("ok   ok "), "{said}");
    assert!(said.contains("2 images: 1 ok, 1 failed"), "{said}");
}

#[test]
fn bringup_bisect_refuses_a_footer_that_runs_past_the_image() {
    let dir = tmp("badfooter");
    let mut c = vec![0u8; 4096];
    let n = c.len();
    c[n - 64..n - 60].copy_from_slice(b"AVBf");
    c[n - 64 + 12..n - 64 + 20].copy_from_slice(&u64::MAX.to_be_bytes());
    fs::write(dir.join("c.img"), c).unwrap();
    let series = dir.join("series.toml");
    fs::write(&series, "control = \"c.img\"\n[[image]]\nname = \"s0\"\n").unwrap();
    let o = bringup(&["image", "bisect", s(&series), "--out", s(&dir.join("out"))]);
    assert_eq!(o.status.code(), Some(2), "{}", text(&o.stderr));
    assert!(text(&o.stderr).contains("runs past"), "{}", text(&o.stderr));
}
