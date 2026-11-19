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
    fs::write(d.join("device.toml"), manifest("xiaomi-surya")).unwrap();
    d
}

/// A folder with no device.toml in it (a temp folder is not under any other).
fn bare(name: &str) -> PathBuf {
    let d = tmp(name);
    fs::remove_file(d.join("device.toml")).unwrap();
    d
}

/// The least a device repo holds: its codename.
fn manifest(codename: &str) -> String {
    format!("[device]\ncodename = \"{codename}\"\n")
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
            "#!/bin/sh\necho \"$*\" >> {calls}\nPATH={bin}:$PATH exec sh -s\n",
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
    for w in [
        "--dry-run",
        "--log",
        "LPOCO_ATTENDED_DEVLAB",
        "LPOCO_ATTENDED_SYSFS_USB",
    ] {
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
        .current_dir(&dir)
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
        .current_dir(dir)
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

#[test]
fn read_dmesg_needs_the_line_in_the_log_and_names_what_was_missing() {
    let has = library(
        &tmp("rd-has"),
        "dmesg-has",
        "[3.0] pmic_glink: probed ok",
        0,
    );
    assert_eq!(has.status.code(), Some(0), "{}", said(&has));
    let lacks = library(&tmp("rd-lacks"), "dmesg-has", "[3.0] something else", 0);
    assert_eq!(lacks.status.code(), Some(1), "{}", said(&lacks));
    assert!(
        said(&lacks).contains("pmic_glink: probed"),
        "{}",
        said(&lacks)
    );
}

/// A fake /sys/bus/usb/devices with one device of this vendor:product (or none).
fn usb_tree(dir: &Path, id: Option<(&str, &str)>) -> PathBuf {
    let root = dir.join("sysusb");
    fs::create_dir_all(root.join("usb1")).unwrap();
    fs::write(root.join("usb1/idVendor"), "1d6b\n").unwrap();
    fs::write(root.join("usb1/idProduct"), "0002\n").unwrap();
    if let Some((v, p)) = id {
        fs::create_dir_all(root.join("1-2")).unwrap();
        fs::write(root.join("1-2/idVendor"), format!("{v}\n")).unwrap();
        fs::write(root.join("1-2/idProduct"), format!("{p}\n")).unwrap();
    }
    root
}

fn usb(dir: &Path, root: &Path) -> Output {
    Command::new("rust-script")
        .arg(example_named(dir, "library"))
        .env("CASE", "usb-seen")
        .env("LPOCO_ATTENDED_SYSFS_USB", root)
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn wait_for_usb_passes_when_the_id_is_on_the_bus_and_fails_naming_the_id_when_not() {
    let dir = tmp("usb-seen");
    let root = usb_tree(&dir, Some(("18d1", "4ee0")));
    let o = usb(&dir, &root);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    let dir = tmp("usb-absent");
    let root = usb_tree(&dir, Some(("18d1", "4ee7")));
    let o = usb(&dir, &root);
    assert_eq!(o.status.code(), Some(1), "{}", said(&o));
    let err = String::from_utf8_lossy(&o.stderr).to_string();
    assert!(err.contains("18d1:4ee0 is not on the USB bus"), "{err}");
}

#[test]
fn wait_for_usb_sees_a_device_that_arrives_late_refuses_a_bad_id_and_names_an_unreadable_folder() {
    let dir = tmp("usb-late");
    let root = usb_tree(&dir, None);
    fs::create_dir_all(root.join("1-2:1.0")).unwrap();
    let child = Command::new("rust-script")
        .arg(example_named(&dir, "library"))
        .env("CASE", "usb-late")
        .env("LPOCO_ATTENDED_SYSFS_USB", &root)
        .current_dir(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let late = root.join("1-2");
    for _ in 0..200 {
        if std::fs::read_to_string(dir.join("attended-library-steps.log")).is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    fs::create_dir_all(&late).unwrap();
    fs::write(late.join("idVendor"), "18D1\n").unwrap();
    fs::write(late.join("idProduct"), "4EE0\n").unwrap();
    let o = child.wait_with_output().unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    let dir = tmp("usb-bad");
    let bad = Command::new("rust-script")
        .arg(example_named(&dir, "library"))
        .env("CASE", "usb-bad")
        .env("LPOCO_ATTENDED_SYSFS_USB", dir.join("none"))
        .current_dir(&dir)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&bad.stderr).contains("not a USB id"),
        "{}",
        said(&bad)
    );
    let gone = usb(&dir, &dir.join("none"));
    assert!(
        String::from_utf8_lossy(&gone.stderr).contains("cannot read"),
        "{}",
        said(&gone)
    );
}

/// The library example with `CASE` (and `KERNEL`, where the case reads it) chosen against a stand-in `devlab`
/// that writes every call's whole command line to dir/args, exits `code` for `kernel try` and runs the phone
/// script (as `stand_in` does) for the rest.
fn devlab_calls(dir: &Path, case: &str, kernel: &str, code: u32) -> Output {
    let devlab = stand_in(dir, "");
    // every way a script could take down the machine it runs on is a fake that only writes a line
    let bin = dir.join("phone/bin");
    for cmd in [
        "systemd-run",
        "systemctl",
        "reboot",
        "shutdown",
        "poweroff",
        "sync",
    ] {
        let fake = bin.join(cmd);
        let fail = if cmd == "systemd-run" {
            "if [ -e DIR/reboot-fails ]; then exit 1; fi"
        } else {
            "true"
        };
        let body = format!("#!/bin/sh\necho \"{cmd} $*\" >> DIR/rebooted\n{fail}\n")
            .replace("DIR", &dir.display().to_string());
        fs::write(&fake, body).unwrap();
        assert!(Command::new("chmod")
            .arg("+x")
            .arg(&fake)
            .status()
            .unwrap()
            .success());
    }
    fs::write(
        &devlab,
        format!(
            "#!/bin/sh\necho \"$*\" >> {args}\n[ \"$1\" = --device ] && shift 2
case \"$1 $2\" in \"kernel try\") echo 'trying once'; exit {code};; esac\nPATH={bin}:$PATH exec sh -s\n",
            args = dir.join("args").display(),
            bin = dir.join("phone/bin").display()
        ),
    )
    .unwrap();
    Command::new("rust-script")
        .arg(example_named(dir, "library"))
        .env("CASE", case)
        .env("KERNEL", kernel)
        .args(if case.ends_with("-dry") {
            vec!["--dry-run"]
        } else {
            vec![]
        })
        .env("LPOCO_ATTENDED_DEVLAB", &devlab)
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn boot_once_asks_devlab_for_one_try_of_the_named_kernel_and_names_a_refusal() {
    let dir = tmp("boot-once");
    let o = devlab_calls(&dir, "boot-once", "calls4", 0);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    assert_eq!(
        fs::read_to_string(dir.join("args")).unwrap().trim(),
        format!("--device {}/device.toml kernel try calls4", dir.display())
    );
    let o = devlab_calls(&tmp("boot-once-refused"), "boot-once", "calls4", 1);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(1), "{t}");
    assert!(
        t.contains("boot calls4 once") && t.contains("trying once"),
        "{t}"
    );
}

#[test]
fn boot_once_refuses_a_name_devlab_would_not_take_before_reaching_it() {
    for bad in ["calls+", "Calls", "a b", ""] {
        let dir = tmp("boot-once-bad");
        let o = devlab_calls(&dir, "boot-once", bad, 0);
        let t = said(&o);
        assert_eq!(o.status.code(), Some(1), "{bad:?}: {t}");
        assert!(t.contains("not a test kernel name"), "{bad:?}: {t}");
        assert!(!dir.join("args").exists(), "{bad:?} reached devlab");
    }
}

#[test]
fn back_to_known_good_reboots_the_phone_into_its_default_and_a_dry_run_does_not() {
    let dir = tmp("back-good");
    let o = devlab_calls(&dir, "back", "", 0);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    assert_eq!(
        fs::read_to_string(dir.join("args")).unwrap().trim(),
        format!("--device {}/device.toml phone root", dir.display())
    );
    assert!(
        fs::read_to_string(dir.join("rebooted"))
            .unwrap()
            .contains("systemctl reboot"),
        "the phone was not told to reboot"
    );
    let dir = tmp("back-good-dry");
    let o = devlab_calls(&dir, "back-dry", "", 0);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    assert!(!dir.join("args").exists(), "a dry run reached devlab");
    assert!(!dir.join("rebooted").exists());
}

#[test]
fn a_reboot_that_was_not_scheduled_fails_the_step_and_a_dry_boot_once_reaches_nothing() {
    let dir = tmp("back-good-fails");
    // the stand-in phone is made by the helper, so mark the failure after it: pre-create the marker
    fs::write(dir.join("reboot-fails"), "").unwrap();
    let o = devlab_calls(&dir, "back", "", 0);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(1), "{t}");
    assert!(t.contains("reboot into the default kernel"), "{t}");
    let dir = tmp("boot-once-dry");
    let o = devlab_calls(&dir, "boot-once-dry", "calls4", 0);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    assert!(!dir.join("args").exists(), "a dry run reached devlab");
}

#[test]
fn wait_for_usb_is_keyed_to_the_bring_up_step_gadget_enumerates() {
    let dir = tmp("usb-keyed");
    let log = dir.join("k.log");
    let o = Command::new("rust-script")
        .arg(example_named(&dir, "library"))
        .args(["--dry-run", "--log", log.to_str().unwrap()])
        .env("CASE", "usb-seen")
        .current_dir(&dir)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    let l = fs::read_to_string(&log).unwrap();
    assert!(
        l.contains("USB device 18d1:4ee0 is on the bus")
            && l.contains("(bring-up step gadget-enumerates)"),
        "{l}"
    );
}

fn run_parts(dir: &Path, devlab: &Path, args: &[&str], stdin: &str) -> Output {
    let mut c = Command::new("rust-script")
        .arg(example_named(dir, "parts"))
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

#[test]
fn until_first_prompt_runs_what_comes_before_it_and_stops_cleanly_without_asking() {
    let dir = tmp("until");
    let devlab = stand_in(&dir, "x");
    let log = dir.join("until.log");
    let o = run_parts(
        &dir,
        &devlab,
        &["--until", "FIRST_PROMPT", "--log", log.to_str().unwrap()],
        "ok\n",
    );
    let t = said(&o);
    assert_eq!(o.status.code(), Some(0), "{t}");
    assert!(t.contains("the phone answers"), "{t}");
    assert!(
        !t.contains("after the prompt"),
        "went on past the prompt: {t}"
    );
    assert!(t.contains("stopped before the first prompt"), "{t}");
    assert!(t.contains("after 1 step(s)"), "{t}");
    let l = fs::read_to_string(&log).unwrap();
    assert!(l.contains("stop") && l.contains("press the button"), "{l}");
    assert!(!l.contains("asked"), "the author was asked: {l}");
}

#[test]
fn until_first_prompt_in_a_dry_run_stops_at_the_same_place() {
    let dir = tmp("until-dry");
    let devlab = stand_in(&dir, "x");
    let o = run_parts(&dir, &devlab, &["--dry-run", "--until", "FIRST_PROMPT"], "");
    let t = said(&o);
    assert_eq!(o.status.code(), Some(0), "{t}");
    assert!(!t.contains("after the prompt"), "{t}");
    assert!(!dir.join("calls").exists(), "the phone was reached: {t}");
    assert!(t.contains("dry run"), "{t}");
}

#[test]
fn until_takes_only_the_point_it_knows() {
    let dir = tmp("until-bad");
    let devlab = stand_in(&dir, "x");
    let o = run_parts(&dir, &devlab, &["--until", "NEVER"], "");
    assert_eq!(o.status.code(), Some(1), "{}", said(&o));
}

#[test]
fn a_part_runs_alone_and_an_unknown_part_is_refused_naming_the_known_ones() {
    let dir = tmp("part");
    let devlab = stand_in(&dir, "x");
    let o = run_parts(&dir, &devlab, &["--part", "checks"], "");
    let t = said(&o);
    assert_eq!(o.status.code(), Some(0), "{t}");
    assert!(
        t.contains("only the checks") && !t.contains("the phone answers"),
        "{t}"
    );
    let o = run_parts(&dir, &devlab, &["--part", "nope"], "");
    let t = said(&o);
    assert_eq!(o.status.code(), Some(2), "{t}");
    assert!(t.contains("nope") && t.contains("checks"), "{t}");
}

#[test]
fn a_part_asked_of_a_procedure_that_has_none_runs_nothing() {
    let dir = tmp("no-parts");
    let devlab = stand_in(&dir, "pmicwdt 16384 0");
    let o = run(&dir, &devlab, &["--part", "x"], "ok\n");
    let t = said(&o);
    assert_eq!(o.status.code(), Some(2), "{t}");
    assert!(t.contains("no parts"), "{t}");
    assert!(!dir.join("calls").exists(), "the phone was reached: {t}");
}

/// The example run from `cwd`, against the stand-in `devlab` of `dir`, with `args`.
fn run_from(cwd: &Path, dir: &Path, args: &[&str]) -> Output {
    let devlab = stand_in(dir, "pmicwdt 16384 0");
    Command::new("rust-script")
        .arg(example(dir))
        .args(args)
        .env("LPOCO_ATTENDED_DEVLAB", devlab)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

/// Every line devlab was called with, from the stand-in's file.
fn devlab_lines(dir: &Path) -> Vec<String> {
    fs::read_to_string(dir.join("calls"))
        .unwrap()
        .lines()
        .map(String::from)
        .collect()
}

#[test]
fn devlab_is_handed_the_manifest_named_by_the_flag_not_the_one_the_folder_finds() {
    let dir = tmp("device-flag");
    let other = tmp("device-flag-other");
    fs::write(other.join("device.toml"), manifest("xiaomi-surya")).unwrap();
    // run from `other`, whose own device.toml is also surya's; the flag points at dir's
    let flagged = dir.join("device.toml");
    let o = run_from(&other, &dir, &["--device", flagged.to_str().unwrap()]);
    assert_eq!(
        o.status.code(),
        Some(1),
        "stops at the ask, no answer: {}",
        said(&o)
    );
    let lines = devlab_lines(&dir);
    assert!(!lines.is_empty());
    for l in &lines {
        assert!(
            l.starts_with(&format!("--device {} phone ", flagged.display())),
            "{l}"
        );
    }
    assert!(lines.iter().any(|l| l.contains("phone sh")), "{lines:?}");
}

#[test]
fn the_manifest_is_found_from_a_folder_below_it_and_a_relative_flag_is_made_absolute() {
    let dir = tmp("device-found");
    let below = dir.join("procedures/deep");
    fs::create_dir_all(&below).unwrap();
    let o = run_from(&below, &dir, &[]);
    assert_eq!(o.status.code(), Some(1), "{}", said(&o));
    let want = format!("--device {}/device.toml phone sh", dir.display());
    assert!(
        devlab_lines(&dir)[0].starts_with(&want),
        "{:?}",
        devlab_lines(&dir)
    );
    let dir = tmp("device-relative");
    let o = run_from(&dir, &dir, &["--device", "device.toml"]);
    assert_eq!(o.status.code(), Some(1), "{}", said(&o));
    let want = format!("--device {}/device.toml phone sh", dir.display());
    assert!(
        devlab_lines(&dir)[0].starts_with(&want),
        "{:?}",
        devlab_lines(&dir)
    );
}

#[test]
fn a_procedure_for_another_device_is_refused_before_any_step_even_in_a_dry_run() {
    let dir = tmp("other-device");
    fs::write(dir.join("device.toml"), manifest("google-walleye")).unwrap();
    let o = run_from(&dir, &dir, &["--dry-run"]);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(2), "{t}");
    assert!(
        t.contains("xiaomi-surya") && t.contains("google-walleye"),
        "{t}"
    );
    assert!(!t.contains("the phone answers"), "a step ran: {t}");
    assert!(!dir.join("calls").exists());
}

#[test]
fn no_device_manifest_or_a_wrong_one_is_named_before_any_step() {
    let dir = bare("no-manifest");
    let o = run_from(&dir, &dir, &["--dry-run"]);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(2), "{t}");
    assert!(t.contains("device.toml") && t.contains("--device"), "{t}");
    let dir = tmp("bad-manifest");
    fs::write(dir.join("device.toml"), "[devise]\ncodename = \"x\"\n").unwrap();
    let o = run_from(&dir, &dir, &["--dry-run"]);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(2), "{t}");
    assert!(t.contains("device.toml") && t.contains("devise"), "{t}");
    assert!(!t.contains("the phone answers"), "a step ran: {t}");
}

#[test]
fn a_refusal_comes_before_the_log_and_names_a_manifest_that_is_missing_or_has_no_device() {
    let dir = tmp("refuse-log");
    fs::write(dir.join("device.toml"), manifest("google-walleye")).unwrap();
    let log = dir.join("refused.log");
    let o = run_from(&dir, &dir, &["--dry-run", "--log", log.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(2), "{}", said(&o));
    assert!(
        !log.exists(),
        "the log was opened before the device was checked"
    );
    assert!(!dir.join("attended-boot-the-pmicwdt-kernel.log").exists());
    fs::write(dir.join("device.toml"), "[hear]\n").unwrap();
    let t = said(&run_from(&dir, &dir, &["--dry-run"]));
    assert!(t.contains("codename is missing"), "{t}");
    let t = said(&run_from(
        &dir,
        &dir,
        &["--dry-run", "--device", "nowhere.toml"],
    ));
    assert!(
        t.contains("cannot read") && t.contains("nowhere.toml"),
        "{t}"
    );
}

#[test]
fn the_log_holds_what_the_phone_said_to_a_passing_step_under_that_step() {
    let dir = tmp("output");
    let devlab = stand_in(&dir, "pmicwdt 16384 0");
    let log = dir.join("output.log");
    let o = run(&dir, &devlab, &["--log", log.to_str().unwrap()], "ok\n");
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    let l = fs::read_to_string(&log).unwrap();
    let line = l
        .lines()
        .find(|l| l.contains(" out pmicwdt is bound: "))
        .unwrap_or_else(|| panic!("no output line for the passing step: {l}"));
    assert!(line.contains("pmicwdt 16384 0"), "{line}");
}

#[test]
fn a_dry_run_logs_no_output_and_long_output_is_cut_and_says_so() {
    let dir = tmp("output-cut");
    let devlab = stand_in(&dir, "pmicwdt 16384 0");
    let long = "x".repeat(9000);
    fs::write(
        dir.join("phone/bin/lsmod"),
        format!("#!/bin/sh\necho pmicwdt; echo {long}\n"),
    )
    .unwrap();
    let log = dir.join("cut.log");
    let o = run(&dir, &devlab, &["--log", log.to_str().unwrap()], "ok\n");
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    let l = fs::read_to_string(&log).unwrap();
    let line = l
        .lines()
        .find(|l| l.contains(" out pmicwdt is bound: "))
        .expect("an out line");
    assert!(
        line.len() < 4200 && line.contains("[cut: the last 4000 of "),
        "{}",
        line.len()
    );
    let dry = dir.join("dry.log");
    run(
        &dir,
        &devlab,
        &["--dry-run", "--log", dry.to_str().unwrap()],
        "",
    );
    assert!(!fs::read_to_string(&dry).unwrap().contains(" out "));
}

#[test]
fn a_cut_never_splits_a_character_and_two_steps_keep_their_own_output() {
    let dir = tmp("output-utf8");
    let devlab = stand_in(&dir, "pmicwdt 16384 0");
    let long = "é".repeat(3000);
    fs::write(
        dir.join("phone/bin/lsmod"),
        format!("#!/bin/sh\necho pmicwdt\necho {long}\n"),
    )
    .unwrap();
    let log = dir.join("utf8.log");
    let o = run(&dir, &devlab, &["--log", log.to_str().unwrap()], "ok\n");
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    let l = fs::read_to_string(&log).unwrap();
    let line = l
        .lines()
        .find(|l| l.contains(" out pmicwdt is bound: [cut: "))
        .unwrap_or_else(|| panic!("no cut line: {l}"));
    assert!(line.ends_with('é') && line.len() < 4200, "{}", line.len());
    assert!(
        !l.lines()
            .filter(|l| l.contains(" out "))
            .any(|l| l.contains("pmicwdt is bound") && l.contains("6.1.0-pmicwdt")),
        "one step's output is filed under another: {l}"
    );
}

/// The library example's `gadget-seen` case against a device.toml that declares (or not) a gadget's usb_id.
fn gadget(dir: &Path, usb_id: Option<&str>, bus: &Path, args: &[&str]) -> Output {
    let id = usb_id.map_or(String::new(), |i| format!("usb_id = \"{i}\"\n"));
    fs::write(
        dir.join("device.toml"),
        format!(
            "{}[[host.gadget]]\nserial = \"s\"\naccess = [\"tty\"]\n{id}",
            manifest("xiaomi-surya")
        ),
    )
    .unwrap();
    Command::new("rust-script")
        .arg(example_named(dir, "library"))
        .args(args)
        .env("CASE", "gadget-seen")
        .env("LPOCO_ATTENDED_SYSFS_USB", bus)
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn wait_for_gadget_looks_for_the_usb_id_the_device_declares_not_one_the_procedure_carries() {
    let dir = tmp("gadget-seen");
    let bus = usb_tree(&dir, Some(("18d1", "4ee0")));
    let o = gadget(&dir, Some("18D1:4EE0"), &bus, &[]);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    let other = tmp("gadget-other");
    let bus = usb_tree(&other, Some(("18d1", "4ee0")));
    let o = gadget(&other, Some("0525:a4a7"), &bus, &[]);
    assert_eq!(o.status.code(), Some(1), "{}", said(&o));
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("0525:a4a7 is not on the USB bus"),
        "{}",
        said(&o)
    );
}

#[test]
fn a_device_without_the_usb_id_fails_the_dry_run_naming_the_key_and_touches_nothing() {
    for args in [&["--dry-run"][..], &[][..]] {
        let dir = tmp("gadget-missing");
        let bus = usb_tree(&dir, Some(("18d1", "4ee0")));
        let o = gadget(&dir, None, &bus, args);
        let t = said(&o);
        assert_eq!(o.status.code(), Some(2), "{t}");
        assert!(
            t.contains("[[host.gadget]] usb_id") && t.contains("device.toml"),
            "{t}"
        );
        assert!(!t.contains("dry run, not run"), "a step was walked: {t}");
    }
}

#[test]
fn the_first_gadget_that_declares_a_usb_id_is_the_one_waited_for() {
    let dir = tmp("gadget-second");
    let bus = usb_tree(&dir, Some(("18d1", "4ee0")));
    let o = gadget(&dir, None, &bus, &[]);
    assert_eq!(o.status.code(), Some(2), "{}", said(&o));
    fs::write(
        dir.join("device.toml"),
        format!(
            "{}[[host.gadget]]\nserial = \"a\"\naccess = [\"tty\"]\n[[host.gadget]]\nserial = \"b\"\naccess = [\"tty\"]\nusb_id = \"18d1:4ee0\"\n",
            manifest("xiaomi-surya")
        ),
    )
    .unwrap();
    let o = Command::new("rust-script")
        .arg(example_named(&dir, "library"))
        .args(["--dry-run"])
        .env("CASE", "gadget-seen")
        .current_dir(&dir)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    assert!(
        said(&o).contains("USB device 18d1:4ee0 is on the bus"),
        "{}",
        said(&o)
    );
}
