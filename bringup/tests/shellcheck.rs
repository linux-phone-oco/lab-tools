// SPDX-License-Identifier: MIT OR Apache-2.0
//! Every shell script `lpoco-bringup` writes for a phone's initramfs passes shellcheck (backlog bringup-init-finish):
//! the init for each console, with and without RNDIS and the log window, and the files it reads beside itself
//! (udhcpc.script, selftest.sh). The levels are warning and error: notes (SC2012, SC2015 and the like) are the
//! init's busybox idiom and are not held against it.
//!
//! The checker is `shellcheck` on PATH (the CI image has it); on the lab, where it is not installed, name it:
//! LPOCO_SHELLCHECK="docker run --rm -i koalaman/shellcheck:stable". It reads the script on its standard input.
//! A checker that cannot run fails the test: nothing is skipped.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lpoco-shellcheck-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// shellcheck on `script`: Ok when it has nothing at warning or above, else what it said.
fn shellcheck(script: &str) -> Result<(), String> {
    let cmd = std::env::var("LPOCO_SHELLCHECK").unwrap_or_else(|_| "shellcheck".into());
    let mut c = Command::new("sh")
        .arg("-c")
        .arg(format!("{cmd} -s sh -S warning -f gcc -"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("cannot run the checker ({cmd}): {e}"));
    c.stdin
        .take()
        .unwrap()
        .write_all(script.as_bytes())
        .unwrap();
    let o = c.wait_with_output().unwrap();
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    match o.status.code() {
        Some(0) => Ok(()),
        // 1: findings. Anything else (127: no such command, 125: docker itself) is a checker that did not run
        Some(1) => Err(said),
        other => panic!("the checker ({cmd}) did not run: exit {other:?}: {said}"),
    }
}

const BOOT: &str = "[device]\ncodename = \"p\"\n[connection]\nuser = \"u\"\naddresses = [\"a\"]\n\
[boot]\nkernel_offset = 0x8000\nramdisk_offset = 0x1000000\ntags_offset = 0x100\npagesize = 4096\ncmdline = \"x\"\n";

fn init_text(d: &std::path::Path, tag: &str, gadget: &str) -> String {
    let p = d.join(format!("{tag}.toml"));
    fs::write(&p, format!("{BOOT}{gadget}")).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_lpoco-bringup"))
        .args(["initramfs", "init", "--device"])
        .arg(&p)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap()
}

#[test]
fn the_checker_itself_fails_a_script_with_a_real_fault() {
    // a known case first: a checker that passes everything would pass the rest of this file
    let e = shellcheck("#!/bin/sh\nrm -rf $dir/\n").unwrap_err();
    assert!(e.contains("SC2154") || e.contains("SC2115"), "{e}");
    assert!(shellcheck("#!/bin/sh\necho ok\n").is_ok());
}

#[test]
fn the_init_passes_for_each_console_with_and_without_rndis_and_the_window() {
    let d = tmp("init");
    for console in ["acm", "ffs"] {
        for rndis in [false, true] {
            for window in [false, true] {
                let g = format!(
                    "[boot.gadget]\nserial = \"s\"\nproduct = \"P\"\nlabel = \"l\"\nconsole = \"{console}\"\nrndis = {rndis}\nwindow = {window}\n"
                );
                let text = init_text(&d, &format!("{console}-{rndis}-{window}"), &g);
                if let Err(e) = shellcheck(&text) {
                    panic!("init for console={console} rndis={rndis} window={window}:\n{e}");
                }
            }
        }
    }
}

#[test]
fn the_files_the_init_reads_pass() {
    let d = tmp("files");
    let o = Command::new(env!("CARGO_BIN_EXE_lpoco-bringup"))
        .args(["initramfs", "files", "--out"])
        .arg(&d)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    for f in ["udhcpc.script", "selftest.sh"] {
        let text = fs::read_to_string(d.join(f)).unwrap();
        if let Err(e) = shellcheck(&text) {
            panic!("{f}:\n{e}");
        }
    }
}
