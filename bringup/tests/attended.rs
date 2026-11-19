// SPDX-License-Identifier: MIT OR Apache-2.0
//! lpoco-attended: a lane's attended procedure is a short entry file of named steps (examples/attended.rs, run by
//! rust-script against this very checkout). Each step stops the run when it fails and names it; --dry-run walks
//! every step and touches nothing; a prompt-and-wait point records what the author answered; the log is a file
//! a lane can file. The phone is a stand-in: a devlab made of sh that runs the script it is given.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lpoco-attended-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// The example, its git dependency pointed at this checkout (as rust_script_step.rs does), compiled once per test dir.
fn example(dir: &Path) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = fs::read_to_string(root.join("examples/attended.rs")).unwrap();
    let git = src
        .lines()
        .find(|l| l.starts_with("//! lpoco-bringup = { git"))
        .expect("the example depends on lpoco-bringup by git tag");
    let local = format!("//! lpoco-bringup = {{ path = \"{}\" }}", root.display());
    let f = dir.join("attended.rs");
    fs::write(&f, src.replace(git, &local)).unwrap();
    f
}

/// A stand-in phone: `devlab phone sh|root` runs the script on stdin with sh, in `dir/phone`, with `dir/phone/bin`
/// first on PATH; it counts its calls in dir/calls. `lsmod` and `uname` are the stand-in's own.
fn stand_in(dir: &Path, lsmod: &str) -> PathBuf {
    let bin = dir.join("phone/bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("lsmod"), format!("#!/bin/sh\necho '{lsmod}'\n")).unwrap();
    fs::write(bin.join("uname"), "#!/bin/sh\necho 6.1.0-pmicwdt\n").unwrap();
    let devlab = dir.join("devlab");
    fs::write(
        &devlab,
        format!(
            "#!/bin/sh\necho \"$1 $2\" >> {calls}\nPATH={bin}:$PATH exec sh -s\n",
            calls = dir.join("calls").display(),
            bin = bin.display()
        ),
    )
    .unwrap();
    Command::new("chmod")
        .arg("+x")
        .args([&devlab, &bin.join("lsmod"), &bin.join("uname")])
        .status()
        .unwrap();
    devlab
}

fn run(dir: &Path, devlab: &Path, args: &[&str], stdin: &str) -> Output {
    let mut c = Command::new("rust-script")
        .arg(example(dir))
        .args(args)
        .env("LPOCO_ATTENDED_DEVLAB", devlab)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("rust-script is on the lab machine");
    c.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    c.wait_with_output().unwrap()
}

fn said(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

#[test]
fn a_dry_run_walks_every_step_and_touches_nothing() {
    let dir = tmp("dry");
    let devlab = stand_in(&dir, "pmicwdt 16384 0");
    let log = dir.join("dry.log");
    let o = run(
        &dir,
        &devlab,
        &["--dry-run", "--log", log.to_str().unwrap()],
        "",
    );
    let t = said(&o);
    assert_eq!(o.status.code(), Some(0), "{t}");
    for n in [
        "the phone answers",
        "put the kernel on it",
        "pmicwdt is bound",
    ] {
        assert!(t.contains(n), "{n} not walked: {t}");
    }
    assert!(t.contains("hold volume down"), "the ask is not shown: {t}");
    assert!(!dir.join("calls").exists(), "the phone was reached: {t}");
    let l = fs::read_to_string(&log).unwrap();
    assert!(l.contains("dry") && l.contains("pmicwdt is bound"), "{l}");
}

#[test]
fn a_run_goes_through_the_phone_records_the_answer_and_files_a_log() {
    let dir = tmp("real");
    let devlab = stand_in(&dir, "pmicwdt 16384 0");
    let log = dir.join("real.log");
    let o = run(&dir, &devlab, &["--log", log.to_str().unwrap()], "ok\n");
    let t = said(&o);
    assert_eq!(o.status.code(), Some(0), "{t}");
    assert!(
        fs::read_to_string(dir.join("calls"))
            .unwrap()
            .contains("phone sh"),
        "{t}"
    );
    let l = fs::read_to_string(&log).unwrap();
    assert!(l.contains("ok") && l.contains("pmicwdt is bound"), "{l}");
    assert!(
        l.contains("asked") && l.contains("the screen shows the fastboot logo"),
        "{l}"
    );
}

#[test]
fn a_failing_check_stops_the_run_and_names_the_step() {
    let dir = tmp("fail");
    let devlab = stand_in(&dir, "usb_f_fs 4096 0");
    let log = dir.join("fail.log");
    let o = run(&dir, &devlab, &["--log", log.to_str().unwrap()], "ok\n");
    let t = said(&o);
    assert_eq!(o.status.code(), Some(1), "{t}");
    assert!(
        t.contains("pmicwdt is bound"),
        "the failing step is not named: {t}"
    );
    let l = fs::read_to_string(&log).unwrap();
    assert!(l.contains("FAIL") && l.contains("pmicwdt is bound"), "{l}");
}

#[test]
fn an_answer_that_is_not_ok_stops_the_run_at_the_ask_and_is_recorded() {
    let dir = tmp("no");
    let devlab = stand_in(&dir, "pmicwdt 16384 0");
    let log = dir.join("no.log");
    let o = run(
        &dir,
        &devlab,
        &["--log", log.to_str().unwrap()],
        "the screen stayed black\n",
    );
    let t = said(&o);
    assert_eq!(o.status.code(), Some(1), "{t}");
    assert!(t.contains("the screen shows the fastboot logo"), "{t}");
    let l = fs::read_to_string(&log).unwrap();
    assert!(l.contains("the screen stayed black"), "{l}");
    assert!(
        !l.contains("pmicwdt is bound"),
        "went on after the stop: {l}"
    );
}

#[test]
fn nobody_answering_stops_the_run() {
    let dir = tmp("eof");
    let devlab = stand_in(&dir, "pmicwdt 16384 0");
    let o = run(
        &dir,
        &devlab,
        &["--log", dir.join("eof.log").to_str().unwrap()],
        "",
    );
    assert_eq!(o.status.code(), Some(1), "{}", said(&o));
}

#[test]
fn help_lists_what_the_library_reads() {
    let dir = tmp("help");
    let devlab = stand_in(&dir, "x");
    let o = run(&dir, &devlab, &["--help"], "");
    let t = said(&o);
    assert_eq!(o.status.code(), Some(0), "{t}");
    for w in ["--dry-run", "--log", "LPOCO_ATTENDED_DEVLAB"] {
        assert!(t.contains(w), "{w} not in the usage: {t}");
    }
}
