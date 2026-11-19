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
    example_named(dir, "attended")
}

fn example_named(dir: &Path, name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = fs::read_to_string(root.join(format!("examples/{name}.rs"))).unwrap();
    let git = src
        .lines()
        .find(|l| l.starts_with("//! lpoco-attended = { git"))
        .expect("the example depends on lpoco-attended by git tag");
    let local = format!("//! lpoco-attended = {{ path = \"{}\" }}", root.display());
    let f = dir.join(format!("{name}.rs"));
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

#[test]
fn a_panic_in_a_step_is_not_reported_as_a_pass() {
    let dir = tmp("panic");
    let devlab = stand_in(&dir, "x");
    let src = fs::read_to_string(example(&dir)).unwrap().replace(
        "step(\"the phone answers\"",
        "step(\"it panics\", || -> () { panic!(\"boom\") });\n    step(\"the phone answers\"",
    );
    fs::write(dir.join("attended.rs"), src).unwrap();
    let log = dir.join("panic.log");
    let mut c = Command::new("rust-script")
        .arg(dir.join("attended.rs"))
        .args(["--log", log.to_str().unwrap()])
        .env("LPOCO_ATTENDED_DEVLAB", &devlab)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(c.stdin.take());
    let o = c.wait_with_output().unwrap();
    let t = said(&o);
    assert!(!o.status.success(), "{t}");
    assert!(!t.contains("steps passed"), "{t}");
    let l = fs::read_to_string(&log).unwrap();
    assert!(
        l.contains("PANIC") && !l.contains("all steps passed"),
        "{l}"
    );
}

#[test]
fn a_dry_run_reaches_no_phone_even_from_main() {
    let dir = tmp("drymain");
    let devlab = stand_in(&dir, "x");
    let src = fs::read_to_string(example(&dir)).unwrap().replace(
        "step(\"the phone answers\"",
        "assert!(phone().sh(\"true\").is_err());\n    step(\"the phone answers\"",
    );
    fs::write(dir.join("attended.rs"), src).unwrap();
    let o = run_file(
        &dir,
        &devlab,
        &["--dry-run", "--log", dir.join("d.log").to_str().unwrap()],
    );
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    assert!(!dir.join("calls").exists());
}

fn run_file(dir: &Path, devlab: &Path, args: &[&str]) -> Output {
    Command::new("rust-script")
        .arg(dir.join("attended.rs"))
        .args(args)
        .env("LPOCO_ATTENDED_DEVLAB", devlab)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

/// The library example with `CASE` chosen, against a stand-in phone whose dmesg is `dmesg` and whose `ls` finds
/// only /drivers/bound_drv; `up_after` failed answers come first (a phone still booting).
fn library(dir: &Path, case: &str, dmesg: &str, up_after: u32) -> Output {
    let devlab = stand_in(dir, "");
    let bin = dir.join("phone/bin");
    fs::write(
        bin.join("dmesg"),
        format!("#!/bin/sh\ncat <<'END'\n{dmesg}\nEND\n"),
    )
    .unwrap();
    fs::write(
        bin.join("ls"),
        "#!/bin/sh\ncase \"$*\" in *drivers/bound_drv*) exit 0;; esac\nexit 2\n",
    )
    .unwrap();
    let state = dir.join("phone/answers");
    fs::write(
        &devlab,
        format!(
            "#!/bin/sh\nn=$(cat {s} 2>/dev/null || echo 0); echo $((n+1)) > {s}\n[ $n -lt {up_after} ] && exit 1\nPATH={bin}:$PATH exec sh -s\n",
            s = state.display(),
            bin = bin.display()
        ),
    )
    .unwrap();
    Command::new("chmod")
        .arg("+x")
        .args([&devlab, &bin.join("dmesg"), &bin.join("ls")])
        .status()
        .unwrap();
    let mut c = Command::new("rust-script")
        .arg(example_named(dir, "library"))
        .env("CASE", case)
        .env("LPOCO_ATTENDED_DEVLAB", &devlab)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("rust-script is on the lab machine");
    c.stdin.take();
    c.wait_with_output().unwrap()
}

#[test]
fn dmesg_clean_passes_a_quiet_log_and_stops_at_an_oops_naming_it() {
    let dir = tmp("dmesg-quiet");
    let o = library(
        &dir,
        "dmesg",
        "[1.0] boot ok\n[1.1] deferred probe pending",
        0,
    );
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    let dir = tmp("dmesg-oops");
    let o = library(&dir, "dmesg", "[1.0] boot ok\n[2.0] Oops: 0000 [#1] SMP", 0);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(1), "{t}");
    assert!(
        t.contains("Oops: 0000") && t.contains("dmesg shows no oops"),
        "{t}"
    );
}

#[test]
fn check_probe_needs_the_driver_bound_and_refuses_a_name_that_is_not_one() {
    let bound = library(&tmp("probe-bound"), "probe-bound", "", 0);
    assert_eq!(bound.status.code(), Some(0), "{}", said(&bound));
    let missing = library(
        &tmp("probe-missing"),
        "probe-missing",
        "[1] absent_drv: bound",
        0,
    );
    assert_eq!(missing.status.code(), Some(1), "{}", said(&missing));
    let dir = tmp("probe-bad");
    let bad = library(&dir, "probe-bad-name", "", 0);
    assert!(said(&bad).contains("not a driver name"), "{}", said(&bad));
    assert!(
        !dir.join("phone/answers").exists(),
        "a bad name reached the phone"
    );
}

#[test]
fn wait_for_phone_passes_when_the_phone_answers_late_and_fails_when_it_never_does() {
    let late = library(&tmp("wait-late"), "wait", "", 2);
    assert_eq!(late.status.code(), Some(0), "{}", said(&late));
    let never = library(&tmp("wait-never"), "wait", "", 9);
    assert_eq!(never.status.code(), Some(1), "{}", said(&never));
}
