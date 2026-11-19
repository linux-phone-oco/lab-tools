// SPDX-License-Identifier: MIT OR Apache-2.0
//! Named step groups (examples/groups.rs, run by rust-script against this very checkout): a group is one line in
//! the entry file, its steps are logged under its name, --dry-run lists them nested, --from can name one, and a
//! group's way back runs once, innermost first, when a step after the group's start fails. The phone is a stand-in
//! devlab made of sh that runs the script it is given in the test's folder.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lpoco-groups-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    fs::write(
        d.join("device.toml"),
        "[device]\ncodename = \"xiaomi-surya\"\n",
    )
    .unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = fs::read_to_string(root.join("examples/groups.rs")).unwrap();
    let git = src
        .lines()
        .find(|l| l.starts_with("//! lpoco-attended = { git"))
        .expect("the example depends on lpoco-attended by git tag");
    let local = format!("//! lpoco-attended = {{ path = \"{}\" }}", root.display());
    fs::write(d.join("groups.rs"), src.replace(git, &local)).unwrap();
    let devlab = d.join("devlab");
    fs::write(
        &devlab,
        format!(
            "#!/bin/sh\necho \"$*\" >> {}\nexec sh -s\n",
            d.join("calls").display()
        ),
    )
    .unwrap();
    Command::new("chmod")
        .arg("+x")
        .arg(&devlab)
        .status()
        .unwrap();
    d
}

fn run(dir: &Path, args: &[&str]) -> (Output, String, String) {
    let log = dir.join("run.log");
    let o = Command::new("rust-script")
        .arg(dir.join("groups.rs"))
        .args(args)
        .args(["--log", log.to_str().unwrap()])
        .env("LPOCO_ATTENDED_DEVLAB", dir.join("devlab"))
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .expect("rust-script is on the lab machine");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    let log = fs::read_to_string(&log).unwrap_or_default();
    (o, said, log)
}

fn waybacks(dir: &Path) -> String {
    fs::read_to_string(dir.join("waybacks")).unwrap_or_default()
}

#[test]
fn two_groups_and_a_step_run_in_order_each_steps_logged_under_its_group() {
    let dir = tmp("order");
    let (o, said, log) = run(&dir, &[]);
    assert_eq!(o.status.code(), Some(0), "{said}");
    let at = |t: &str| {
        said.find(t)
            .unwrap_or_else(|| panic!("{t} not said: {said}"))
    };
    assert!(
        at("reach the phone") < at("the phone answers")
            && at("the phone answers") < at("do the work")
            && at("do the work") < at("the inner part")
            && at("the inner part") < at("work unless told not to")
            && at("work unless told not to") < at("the work is done")
            && at("the work is done") < at("all done"),
        "{said}"
    );
    assert!(said.contains("all 5 steps passed"), "{said}");
    for l in [
        "begin reach the phone > the phone answers",
        "ok reach the phone > the shell is up",
        "begin do the work > the inner part > work unless told not to",
        "ok do the work > the work is done",
        "begin all done",
    ] {
        assert!(log.contains(l), "{l} not in the log:\n{log}");
    }
    assert_eq!(waybacks(&dir), "", "a way back ran on a run that passed");
}

#[test]
fn a_dry_run_lists_the_groups_nested_names_the_way_backs_and_reaches_nothing() {
    let dir = tmp("dry");
    let (o, said, _) = run(&dir, &["--dry-run"]);
    assert_eq!(o.status.code(), Some(0), "{said}");
    for l in [
        "group: reach the phone (way back: leave the outer mark)",
        "  check 1: the phone answers - dry run, not run",
        "group: do the work",
        "  group: the inner part (way back: leave the inner mark)",
        "    step 3: work unless told not to - dry run, not run",
        "  check 4: the work is done - dry run, not run",
        "step 5: all done - dry run, not run",
    ] {
        assert!(said.lines().any(|x| x == l), "{l} not listed:\n{said}");
    }
    assert!(!dir.join("calls").exists(), "the phone was reached: {said}");
}

#[test]
fn a_failing_step_runs_the_way_backs_of_its_groups_once_innermost_first_and_stops_the_run() {
    let dir = tmp("fail");
    fs::write(dir.join("fail-here"), "").unwrap();
    let (o, said, log) = run(&dir, &[]);
    assert_eq!(o.status.code(), Some(1), "{said}");
    assert!(
        said.contains("work unless told not to") && said.contains("do the work > the inner part"),
        "the failing step and its groups are not named: {said}"
    );
    assert!(
        !said.contains("the work is done"),
        "the run went on: {said}"
    );
    assert!(!said.contains("all done"), "the run went on: {said}");
    assert_eq!(waybacks(&dir), "inner\nouter\n", "{said}");
    assert!(
        log.contains("FAIL do the work > the inner part > work unless told not to"),
        "{log}"
    );
    assert!(
        log.contains("way-back") && log.contains("leave the inner mark"),
        "the way back is not in the log:\n{log}"
    );
}

#[test]
fn a_group_that_never_started_has_no_way_back_to_run() {
    let dir = tmp("later");
    fs::write(dir.join("fail-here"), "").unwrap();
    // starting at the inner part: the outer "reach the phone" group is skipped whole, so its way back is not owed
    let (o, said, _) = run(&dir, &["--from", "the inner part"]);
    assert_eq!(o.status.code(), Some(1), "{said}");
    assert_eq!(waybacks(&dir), "inner\n", "{said}");
}

#[test]
fn from_can_name_a_group_or_a_step_in_one() {
    let dir = tmp("from");
    let (o, said, _) = run(&dir, &["--from", "do the work"]);
    assert_eq!(o.status.code(), Some(0), "{said}");
    assert!(
        said.contains("the phone answers - skipped (before --from)"),
        "{said}"
    );
    assert!(said.contains("work unless told not to") && said.contains("all done"));
    assert!(
        !said.contains("work unless told not to - skipped"),
        "a group named by --from was skipped: {said}"
    );
    let (o, said, _) = run(&dir, &["--from", "the work is done"]);
    assert_eq!(o.status.code(), Some(0), "{said}");
    assert!(
        said.contains("work unless told not to - skipped (before --from)"),
        "{said}"
    );
    let (o, said, _) = run(&dir, &["--from", "no such group"]);
    assert_eq!(o.status.code(), Some(2), "{said}");
    assert!(
        said.contains("\"reach the phone\"") && said.contains("\"the inner part\""),
        "the groups are not among the names offered: {said}"
    );
}

/// The library's groups (examples/ready.rs) over a device.toml that declares its kernels.
fn ready(name: &str, manifest: &str, args: &[&str]) -> (Output, String) {
    let dir = tmp(name);
    fs::write(dir.join("device.toml"), manifest).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = fs::read_to_string(root.join("examples/ready.rs")).unwrap();
    let git = src
        .lines()
        .find(|l| l.starts_with("//! lpoco-attended = { git"))
        .unwrap();
    let local = format!("//! lpoco-attended = {{ path = \"{}\" }}", root.display());
    fs::write(dir.join("ready.rs"), src.replace(git, &local)).unwrap();
    let o = Command::new("rust-script")
        .arg(dir.join("ready.rs"))
        .args(args)
        .args(["--log", dir.join("run.log").to_str().unwrap()])
        .env("LPOCO_ATTENDED_DEVLAB", dir.join("devlab"))
        .current_dir(&dir)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    (o, said)
}

const KERNELS: &str =
    "[device]\ncodename = \"xiaomi-surya\"\n[kernels]\ntest = [\"pmicwdt\", \"mainline\"]\nkept = \"mainline\"\n";

#[test]
fn phone_ready_and_leave_known_good_are_groups_whose_kernels_come_from_device_toml() {
    let (o, said) = ready("lib", KERNELS, &["--dry-run"]);
    assert_eq!(o.status.code(), Some(0), "{said}");
    for l in [
        "group: the phone is ready on pmicwdt (way back: reboot into the default kernel (kept: mainline))",
        "  step 1: boot pmicwdt once - dry run, not run",
        "  check 2: the phone answers - dry run, not run",
        "  check 3: the shell answers - dry run, not run",
        "check 4: the driver is bound - dry run, not run",
        "group: leave the phone on its known good (settles the way backs of finished groups)",
        "  step 5: reboot into the default kernel (kept: mainline) - dry run, not run",
        "  check 6: the phone goes down - dry run, not run",
        "  check 7: the phone answers - dry run, not run",
    ] {
        assert!(said.lines().any(|x| x == l), "{l} not listed:\n{said}");
    }
}

#[test]
fn a_device_that_lists_no_such_test_kernel_stops_the_run_before_it_starts() {
    let (o, said) = ready(
        "lib-missing",
        "[device]\ncodename = \"xiaomi-surya\"\n[kernels]\ntest = [\"other\"]\n",
        &["--dry-run"],
    );
    assert_eq!(o.status.code(), Some(2), "{said}");
    assert!(
        said.contains("pmicwdt") && said.contains("[kernels] test"),
        "{said}"
    );
}

#[test]
fn a_step_that_panics_still_runs_the_way_backs_owed_and_exits_1() {
    let dir = tmp("panic");
    fs::write(dir.join("panic-here"), "").unwrap();
    let (o, said, log) = run(&dir, &[]);
    assert_eq!(o.status.code(), Some(1), "{said}");
    assert_eq!(waybacks(&dir), "inner\nouter\n", "{said}");
    assert!(
        log.contains("PANIC") && log.contains("FAIL"),
        "the panic is not in the log:\n{log}"
    );
}

#[test]
fn a_settling_group_that_finished_leaves_the_way_backs_of_finished_groups_settled() {
    let dir = tmp("settled");
    fs::write(dir.join("settle-here"), "").unwrap();
    fs::write(dir.join("fail-after"), "").unwrap();
    let (o, said, log) = run(&dir, &[]);
    assert_eq!(o.status.code(), Some(1), "{said}");
    assert!(said.contains("work after settling"), "{said}");
    assert_eq!(
        waybacks(&dir),
        "",
        "a way back ran after the phone was put back: {said}"
    );
    assert!(!log.contains("way-back"), "{log}");
}

#[test]
fn a_settling_group_that_failed_settles_nothing() {
    let dir = tmp("settle-failed");
    fs::write(dir.join("settle-here"), "").unwrap();
    fs::write(dir.join("fail-settling"), "").unwrap();
    let (o, said, _) = run(&dir, &[]);
    assert_eq!(o.status.code(), Some(1), "{said}");
    assert_eq!(waybacks(&dir), "inner\nouter\n", "{said}");
}

#[test]
fn without_a_settling_group_a_later_failure_still_runs_the_way_backs_of_finished_groups() {
    let dir = tmp("not-settled");
    fs::write(dir.join("fail-here"), "").unwrap();
    let (o, said, _) = run(&dir, &[]);
    assert_eq!(o.status.code(), Some(1), "{said}");
    assert_eq!(waybacks(&dir), "inner\nouter\n", "{said}");
}

#[test]
fn a_settling_group_nested_deeper_settles_the_finished_groups_but_not_the_open_one_around_it() {
    let dir = tmp("settle-nested");
    fs::write(dir.join("settle-nested"), "").unwrap();
    fs::write(dir.join("fail-after"), "").unwrap();
    let (o, said, _) = run(&dir, &[]);
    assert_eq!(o.status.code(), Some(1), "{said}");
    assert_eq!(waybacks(&dir), "wrapper\n", "{said}");
}
