// SPDX-License-Identifier: MIT OR Apache-2.0
//! bringup-cpio-matches-gen-init-cpio: `lpoco-bringup initramfs build` writes, from a gen_init_cpio spec,
//! the very archive the kernel's `gen_init_cpio -t 0` makes (newc, byte for byte), compressed with the codec
//! the device names, the same bytes twice; the gadget address is a variable, and a build that leaves the old
//! address behind fails. The oracle is gen_init_cpio (LPOCO_GEN_INIT_CPIO, else on PATH); where it is
//! absent the byte-for-byte test says "not measured: no gen_init_cpio".

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lpoco-initramfs-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

fn gen_init_cpio() -> Option<String> {
    let c = std::env::var("LPOCO_GEN_INIT_CPIO").unwrap_or_else(|_| "gen_init_cpio".into());
    // gen_init_cpio with no arguments prints its usage and exits 1: it runs, so it is there
    Command::new(&c).arg("--help").output().ok().map(|_| c)
}

fn build(args: &[&str]) -> Output {
    let mut a = vec!["initramfs", "build"];
    a.extend_from_slice(args);
    Command::new(env!("CARGO_BIN_EXE_lpoco-bringup"))
        .args(a)
        .output()
        .unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// A walleye-shaped tree: dirs, device nodes, a binary, a link, an /init and a udhcpd.conf at 172.16.42.
fn tree(d: &Path) -> PathBuf {
    fs::write(
        d.join("busybox"),
        (0..=255u8).cycle().take(5001).collect::<Vec<_>>(),
    )
    .unwrap();
    fs::write(
        d.join("init"),
        "#!/bin/sh\nip addr add 172.16.42.1/24 dev usb0\nmac=ac:10:2a:00:00:01\n",
    )
    .unwrap();
    fs::write(
        d.join("udhcpd.conf"),
        "start 172.16.42.2\nend 172.16.42.2\n",
    )
    .unwrap();
    let spec = d.join("walleye.spec");
    fs::write(
        &spec,
        "dir /bin 755 0 0\ndir /dev 755 0 0\ndir /tmp 1777 0 0\n\
         nod /dev/console 600 0 0 c 5 1\nnod /dev/null 666 0 0 c 1 3\n\
         file /bin/busybox busybox 755 0 0\nslink /bin/sh busybox 777 0 0\n\
         file /init init 755 0 0\nfile /etc/udhcpd.conf udhcpd.conf 644 0 0\n",
    )
    .unwrap();
    spec
}

fn gunzip(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(b)
        .read_to_end(&mut out)
        .unwrap();
    out
}

fn unxz(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    xz2::read::XzDecoder::new(b).read_to_end(&mut out).unwrap();
    out
}

#[test]
fn the_archive_is_byte_for_byte_what_gen_init_cpio_makes() {
    let Some(gen) = gen_init_cpio() else {
        eprintln!("not measured: no gen_init_cpio");
        return;
    };
    let d = tmp("oracle");
    let spec = tree(&d);
    // gen_init_cpio reads paths from the spec as they are: give it the same spec with absolute paths
    let abs: String = fs::read_to_string(&spec)
        .unwrap()
        .lines()
        .map(|l| {
            let mut f: Vec<String> = l.split_whitespace().map(String::from).collect();
            if f[0] == "file" {
                f[2] = format!("{}/{}", s(&d), f[2]);
            }
            f.join(" ") + "\n"
        })
        .collect();
    let abs_spec = d.join("abs.spec");
    fs::write(&abs_spec, abs).unwrap();
    let want = Command::new(&gen)
        .args(["-t", "0", s(&abs_spec)])
        .output()
        .unwrap()
        .stdout;
    assert!(!want.is_empty(), "the oracle wrote nothing");
    for (codec, un) in [("gzip", gunzip as fn(&[u8]) -> Vec<u8>), ("xz", unxz)] {
        let out = d.join(format!("out.{codec}"));
        let o = build(&["--spec", s(&spec), "--out", s(&out), "--codec", codec]);
        assert!(o.status.success(), "{codec}: {}", text(&o.stderr));
        assert!(
            un(&fs::read(&out).unwrap()) == want,
            "{codec}: the archive differs from gen_init_cpio's"
        );
        // the same bytes twice
        let again = d.join(format!("again.{codec}"));
        assert!(
            build(&["--spec", s(&spec), "--out", s(&again), "--codec", codec])
                .status
                .success()
        );
        assert_eq!(
            fs::read(&out).unwrap(),
            fs::read(&again).unwrap(),
            "{codec}: not reproducible"
        );
    }
}

#[test]
fn the_xz_stream_carries_a_crc32_check_for_the_4_4_kernel() {
    let d = tmp("xzcheck");
    let spec = tree(&d);
    let out = d.join("out.xz");
    assert!(
        build(&["--spec", s(&spec), "--out", s(&out), "--codec", "xz"])
            .status
            .success()
    );
    let b = fs::read(&out).unwrap();
    assert_eq!(&b[..6], b"\xfd7zXZ\0");
    assert_eq!(b[7], 0x01, "stream flags: check type 1 is CRC32");
}

#[test]
fn the_gadget_address_is_a_variable() {
    let d = tmp("net");
    let spec = tree(&d);
    let out = d.join("out.gz");
    let o = build(&["--spec", s(&spec), "--out", s(&out), "--net", "43"]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let cpio = String::from_utf8_lossy(&gunzip(&fs::read(&out).unwrap())).into_owned();
    assert!(
        cpio.contains("172.16.43.1/24") && cpio.contains("start 172.16.43.2"),
        "address not rewritten"
    );
    assert!(cpio.contains("ac:10:2b:"), "the ECM MAC byte not rewritten");
    assert!(
        !cpio.contains("172.16.42"),
        "the old address is left behind"
    );
}

#[test]
fn a_build_that_leaves_the_old_address_behind_fails() {
    let d = tmp("left");
    let spec = tree(&d);
    // an address in a form the rewrite (172.16.42. and ac:10:2a:) does not reach
    fs::write(
        d.join("init"),
        "ip addr add 172.16.42.1/24 dev usb0\nip route add 172.16.42/24 dev usb0\n",
    )
    .unwrap();
    let out = d.join("out.gz");
    let o = build(&["--spec", s(&spec), "--out", s(&out), "--net", "43"]);
    assert!(
        !o.status.success(),
        "a left-behind address must stop the build"
    );
    assert!(text(&o.stderr).contains("172.16.42"), "{}", text(&o.stderr));
    assert!(!out.exists(), "no archive is written when the build fails");
}

#[test]
fn a_spec_gen_init_cpio_reads_differently_or_not_at_all_is_refused() {
    let d = tmp("refuse");
    fs::write(d.join("a"), "x").unwrap();
    let out = d.join("o.gz");
    for (why, spec) in [
        ("a hard link", "file /x a 644 0 0 /y"),
        ("a hex uid", "dir /d 755 0x10 0"),
        (
            "a name over PATH_MAX",
            &format!("dir /{} 755 0 0", "n".repeat(5000)),
        ),
        ("an unknown line", "pipe /p 600 0 0"),
    ] {
        fs::write(d.join("s.spec"), spec).unwrap();
        let o = build(&["--spec", s(&d.join("s.spec")), "--out", s(&out)]);
        assert!(!o.status.success(), "{why} must be refused");
        assert!(!out.exists(), "{why}: no archive on failure");
    }
}

#[test]
fn a_decimal_uid_is_read_as_the_oracle_reads_it() {
    let Some(gen) = gen_init_cpio() else {
        eprintln!("not measured: no gen_init_cpio");
        return;
    };
    let d = tmp("uid");
    fs::write(d.join("a"), "x").unwrap();
    let spec = d.join("s.spec");
    fs::write(
        &spec,
        "dir /d 755 010 010\nnod /dev/b 640 5 6 b 8 0\nfile /e a 4755 1000 1000\n",
    )
    .unwrap();
    let want = Command::new(&gen)
        .args(["-t", "0", s(&spec)])
        .current_dir(&d)
        .output()
        .unwrap()
        .stdout;
    let out = d.join("o.gz");
    assert!(build(&["--spec", s(&spec), "--out", s(&out)])
        .status
        .success());
    assert!(
        gunzip(&fs::read(&out).unwrap()) == want,
        "uid 010 is ten, as the oracle reads it"
    );
}
