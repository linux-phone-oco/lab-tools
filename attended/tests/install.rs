// SPDX-License-Identifier: MIT OR Apache-2.0
//! The install steps lifted from walleye (read_slot, mount_is_readonly, adb_write_partition), each run by the example
//! against a stand-in phone: a devlab made of sh that runs the script it is given, with `getprop` and `cat /proc/mounts`
//! answering the text a phone printed (tests/fixtures/SOURCES.md) and real `dd`, `wc` and `sha256sum` on files that
//! stand for the partitions.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const BLOCK: usize = 4096;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lpoco-install-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(d.join("phone/bin")).unwrap();
    fs::create_dir_all(d.join("by-name")).unwrap();
    fs::write(
        d.join("device.toml"),
        "[device]\ncodename = \"google-walleye\"\n",
    )
    .unwrap();
    let bin = d.join("phone/bin");
    // the phone answers what a phone printed: the slot, and /proc/mounts
    fs::write(
        bin.join("getprop"),
        format!(
            "#!/bin/sh\ncat {}\n",
            fixture("slot-suffix-on-b.txt").display()
        ),
    )
    .unwrap();
    fs::write(
        bin.join("cat"),
        format!(
            "#!/bin/sh\nif [ \"$1\" = /proc/mounts ]; then exec /bin/cat {}; fi\nexec /bin/cat \"$@\"\n",
            fixture("proc-mounts-lower-ro.txt").display()
        ),
    )
    .unwrap();
    fs::write(
        d.join("devlab"),
        format!(
            "#!/bin/sh\necho \"$*\" >> {calls}\nPATH={bin}:$PATH exec sh -s\n",
            calls = d.join("calls").display(),
            bin = bin.display()
        ),
    )
    .unwrap();
    for f in ["devlab", "phone/bin/getprop", "phone/bin/cat"] {
        assert!(Command::new("chmod")
            .arg("+x")
            .arg(d.join(f))
            .status()
            .unwrap()
            .success());
    }
    d
}

/// An image of `blocks` whole blocks of `byte`, staged at dir/staged.img; its sha256 as sha256sum prints it.
fn staged(dir: &Path, blocks: usize, byte: u8) -> String {
    fs::write(dir.join("staged.img"), vec![byte; blocks * BLOCK]).unwrap();
    sha256(&dir.join("staged.img"))
}

fn sha256(f: &Path) -> String {
    let o = Command::new("sha256sum").arg(f).output().unwrap();
    String::from_utf8(o.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_string()
}

fn partition(dir: &Path, name: &str, blocks: usize, byte: u8) -> PathBuf {
    let p = dir.join("by-name").join(name);
    fs::write(&p, vec![byte; blocks * BLOCK]).unwrap();
    p
}

fn run(dir: &Path, envs: &[(&str, &str)]) -> Output {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = fs::read_to_string(root.join("examples/install.rs")).unwrap();
    let git = src
        .lines()
        .find(|l| l.starts_with("//! lpoco-attended = { git"))
        .unwrap();
    let local = format!("//! lpoco-attended = {{ path = \"{}\" }}", root.display());
    let f = dir.join("install.rs");
    fs::write(&f, src.replace(git, &local)).unwrap();
    Command::new("rust-script")
        .arg(&f)
        .envs(envs.iter().copied())
        .env("LPOCO_ATTENDED_DEVLAB", dir.join("devlab"))
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn said(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn write_case<'a>(dir: &'a Path, part: &'a str, sha: &'a str) -> Vec<(&'a str, String)> {
    vec![
        ("CASE", "write".into()),
        ("BY_NAME", dir.join("by-name").display().to_string()),
        ("PART", part.into()),
        ("STAGED", dir.join("staged.img").display().to_string()),
        ("SHA", sha.into()),
    ]
}

fn run_write(dir: &Path, part: &str, sha: &str) -> Output {
    let envs = write_case(dir, part, sha);
    let refs: Vec<(&str, &str)> = envs.iter().map(|(k, v)| (*k, v.as_str())).collect();
    run(dir, &refs)
}

#[test]
fn read_slot_passes_on_the_suffix_a_phone_on_slot_b_printed() {
    let d = tmp("slot");
    let o = run(&d, &[("CASE", "slot")]);
    assert!(o.status.success(), "{}", said(&o));
}

#[test]
fn read_slot_fails_naming_what_the_phone_said_when_it_is_neither_slot() {
    let d = tmp("slot-bad");
    fs::write(d.join("phone/bin/getprop"), "#!/bin/sh\necho ''\n").unwrap();
    let o = run(&d, &[("CASE", "slot")]);
    assert!(!o.status.success(), "{}", said(&o));
    assert!(said(&o).contains("slot"), "{}", said(&o));
}

#[test]
fn mount_is_readonly_passes_for_the_mount_a_phone_printed_with_ro() {
    let d = tmp("mount-ro");
    let o = run(&d, &[("CASE", "mount-ro"), ("POINT", "/lower")]);
    assert!(o.status.success(), "{}", said(&o));
}

#[test]
fn mount_is_readonly_fails_for_a_read_write_mount_and_for_none() {
    let d = tmp("mount-rw");
    let o = run(&d, &[("CASE", "mount-ro"), ("POINT", "/")]);
    assert!(!o.status.success(), "{}", said(&o));
    assert!(said(&o).contains("rw"), "{}", said(&o));
    let o = run(&d, &[("CASE", "mount-ro"), ("POINT", "/nowhere")]);
    assert!(!o.status.success(), "{}", said(&o));
    assert!(said(&o).contains("/nowhere"), "{}", said(&o));
}

#[test]
fn adb_write_partition_writes_the_other_slot_and_passes_when_the_read_back_hashes_the_same() {
    let d = tmp("write-ok");
    let sha = staged(&d, 3, 0xAB);
    let target = partition(&d, "system_a", 5, 0x11);
    let other = partition(&d, "system_b", 5, 0x22);
    let o = run_write(&d, "system_a", &sha);
    assert!(o.status.success(), "{}", said(&o));
    let after = fs::read(&target).unwrap();
    assert_eq!(&after[..3 * BLOCK], &vec![0xAB; 3 * BLOCK][..]);
    assert_eq!(&after[3 * BLOCK..], &vec![0x11; 2 * BLOCK][..], "notrunc");
    assert_eq!(fs::read(&other).unwrap(), vec![0x22; 5 * BLOCK]);
}

#[test]
fn adb_write_partition_refuses_the_active_slot_and_writes_nothing() {
    let d = tmp("write-active");
    let sha = staged(&d, 3, 0xAB);
    let active = partition(&d, "system_b", 5, 0x22);
    let o = run_write(&d, "system_b", &sha);
    assert!(!o.status.success(), "{}", said(&o));
    assert!(said(&o).contains("system_b"), "{}", said(&o));
    assert_eq!(fs::read(&active).unwrap(), vec![0x22; 5 * BLOCK]);
}

#[test]
fn adb_write_partition_refuses_a_partition_outside_the_allow_list_and_writes_nothing() {
    let d = tmp("write-list");
    let sha = staged(&d, 3, 0xAB);
    let boot = partition(&d, "boot_a", 5, 0x33);
    let o = run_write(&d, "boot_a", &sha);
    assert!(!o.status.success(), "{}", said(&o));
    assert!(said(&o).contains("boot_a"), "{}", said(&o));
    assert_eq!(fs::read(&boot).unwrap(), vec![0x33; 5 * BLOCK]);
}

#[test]
fn adb_write_partition_refuses_a_staged_image_that_is_not_the_declared_sha256_before_writing() {
    let d = tmp("write-staged");
    staged(&d, 3, 0xAB);
    let target = partition(&d, "system_a", 5, 0x11);
    let wrong = "0".repeat(64);
    let o = run_write(&d, "system_a", &wrong);
    assert!(!o.status.success(), "{}", said(&o));
    assert_eq!(fs::read(&target).unwrap(), vec![0x11; 5 * BLOCK]);
}

#[test]
fn adb_write_partition_fails_when_the_read_back_differs_from_the_declared_sha256() {
    let d = tmp("write-readback");
    let sha = staged(&d, 3, 0xAB);
    partition(&d, "system_a", 5, 0x11);
    // a card that does not keep what is written: its dd writes zeros where the image was to go
    fs::write(
        d.join("phone/bin/dd"),
        "#!/bin/sh\ncase \"$*\" in *of=*by-name*) exec /bin/dd if=/dev/zero \"$2\" \"$3\" \"$4\" \"$5\" 2>/dev/null;; esac\nexec /bin/dd \"$@\"\n",
    )
    .unwrap();
    Command::new("chmod")
        .arg("+x")
        .arg(d.join("phone/bin/dd"))
        .status()
        .unwrap();
    let o = run_write(&d, "system_a", &sha);
    assert!(!o.status.success(), "{}", said(&o));
    assert!(said(&o).contains("read back"), "{}", said(&o));
}

#[test]
fn adb_write_partition_refuses_an_image_that_is_not_whole_blocks() {
    let d = tmp("write-blocks");
    fs::write(d.join("staged.img"), vec![1u8; BLOCK + 1]).unwrap();
    let sha = sha256(&d.join("staged.img"));
    let target = partition(&d, "system_a", 5, 0x11);
    let o = run_write(&d, "system_a", &sha);
    assert!(!o.status.success(), "{}", said(&o));
    assert_eq!(fs::read(&target).unwrap(), vec![0x11; 5 * BLOCK]);
}

#[test]
fn adb_write_partition_refuses_an_allow_listed_partition_with_no_slot_and_writes_nothing() {
    let d = tmp("write-noslot");
    let sha = staged(&d, 3, 0xAB);
    let data = partition(&d, "userdata", 5, 0x44);
    let envs = write_case(&d, "userdata", &sha);
    let mut refs: Vec<(&str, &str)> = envs.iter().map(|(k, v)| (*k, v.as_str())).collect();
    refs.push(("ALLOWED", "userdata"));
    let o = run(&d, &refs);
    assert!(!o.status.success(), "{}", said(&o));
    assert_eq!(fs::read(&data).unwrap(), vec![0x44; 5 * BLOCK]);
}

#[test]
fn adb_write_partition_refuses_a_missing_partition_node_and_creates_none() {
    let d = tmp("write-nonode");
    let sha = staged(&d, 3, 0xAB);
    let o = run_write(&d, "system_a", &sha);
    assert!(!o.status.success(), "{}", said(&o));
    assert!(!d.join("by-name/system_a").exists(), "no file was made");
}

#[test]
fn read_slot_on_a_named_connection_asks_devlab_for_that_connection_and_the_default_names_none() {
    let d = tmp("slot-named");
    let o = run(&d, &[("CASE", "slot"), ("CONNECTION", "android")]);
    assert!(o.status.success(), "{}", said(&o));
    let calls = fs::read_to_string(d.join("calls")).unwrap();
    assert!(
        calls
            .lines()
            .all(|l| l.ends_with("phone root --connection android")),
        "{calls}"
    );
    let d = tmp("slot-default");
    let o = run(&d, &[("CASE", "slot")]);
    assert!(o.status.success(), "{}", said(&o));
    let calls = fs::read_to_string(d.join("calls")).unwrap();
    assert!(calls.lines().all(|l| l.ends_with("phone root")), "{calls}");
}

#[test]
fn adb_write_partition_on_a_named_connection_reads_the_slot_and_writes_through_that_connection() {
    let d = tmp("write-named");
    let sha = staged(&d, 3, 0xAB);
    partition(&d, "system_a", 5, 0x11);
    let mut envs = write_case(&d, "system_a", &sha);
    envs.push(("CONNECTION", "android".into()));
    let refs: Vec<(&str, &str)> = envs.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let o = run(&d, &refs);
    assert!(o.status.success(), "{}", said(&o));
    let calls = fs::read_to_string(d.join("calls")).unwrap();
    assert!(calls.lines().count() >= 2, "{calls}");
    assert!(
        calls
            .lines()
            .all(|l| l.ends_with("phone root --connection android")),
        "{calls}"
    );
}

#[test]
fn a_step_on_a_connection_that_does_not_answer_fails_naming_the_connection() {
    let d = tmp("slot-silent");
    fs::write(
        d.join("devlab"),
        "#!/bin/sh\necho \"connection android does not answer\" >&2\nexit 1\n",
    )
    .unwrap();
    let o = run(&d, &[("CASE", "slot"), ("CONNECTION", "android")]);
    assert!(!o.status.success(), "{}", said(&o));
    assert!(said(&o).contains("connection android"), "{}", said(&o));
}
