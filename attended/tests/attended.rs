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
    fs::write(bin.join("uname"), format!("#!/bin/sh\nif [ -e {d}/uname-release ]; then cat {d}/uname-release; else echo 6.1.0-pmicwdt; fi\n", d = dir.display())).unwrap();
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
        &[
            "--log",
            dir.join("eof.log").to_str().unwrap(),
            "--answer-limit",
            "1",
        ],
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
        fs::write(root.join("1-2/serial"), "s\n").unwrap();
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
    // the device declares its test kernel unless the test says it declares none (a `no-kernels` file) or its own
    let own = fs::read_to_string(dir.join("device.toml")).unwrap();
    if !own.contains("[kernels]") && !dir.join("no-kernels").exists() {
        fs::write(
            dir.join("device.toml"),
            format!("{own}[kernels]\ntest = [\"calls4\"]\n"),
        )
        .unwrap();
    }
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
case \"$1 $2\" in \"kernel try\") echo 'trying once'; exit {code};; \"fastboot boot\") echo 'booted from memory'; exit {code};; \"phone put\") cp \"$3\" \"$4\"; exit 0;; \"fastboot state\") if [ -e {dir}/state-fails ]; then echo 'no serial' >&2; exit 1; fi; if {{ [ -e {dir}/not-in-fastboot ] && ! grep -q 'restart2 bootloader' {dir}/rebooted 2>/dev/null; }} || [ -e {dir}/never-in-fastboot ]; then echo 'x: S is not in fastboot now (off)'; else echo 'x: S is in fastboot'; fi; exit 0;; esac\nPATH={bin}:$PATH exec sh -s\n",
            args = dir.join("args").display(),
            dir = dir.display(),
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
        .envs(
            dir.join("sysusb")
                .exists()
                .then_some(("LPOCO_ATTENDED_SYSFS_USB", dir.join("sysusb"))),
        )
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
    // the phone is asked which kernel runs first, then the one try is asked for
    assert_eq!(
        fs::read_to_string(dir.join("args")).unwrap().trim(),
        format!(
            "--device {d}/device.toml phone sh\n--device {d}/device.toml kernel try calls4",
            d = dir.display()
        )
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
        .env("LPOCO_ATTENDED_ANSWER_LIMIT", "1")
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

/// The physical example, its git dependency pointed at this checkout.
fn run_physical(dir: &Path, args: &[&str], stdin: &str) -> Output {
    let devlab = stand_in(dir, "x");
    let mut c = Command::new("rust-script")
        .arg(example_named(dir, "physical"))
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
fn each_physical_act_goes_through_the_person_who_is_told_what_to_do_and_what_to_check_and_the_answers_are_logged(
) {
    let dir = tmp("phys-person");
    let log = dir.join("p.log");
    let o = run_physical(&dir, &["--log", log.to_str().unwrap()], &"ok\n".repeat(9));
    let t = said(&o);
    assert_eq!(o.status.code(), Some(0), "{t}");
    for w in [
        "press the power button",
        "hold the volume down button",
        "unplug the USB cable",
        "plug the USB cable back in",
        "touch the Settings icon",
        "swipe up from the bottom edge",
        "listen to the phone",
        "Check that the logo shows",
        "Check that you can hear the ringtone",
    ] {
        assert!(t.contains(w), "{w} not said to the person: {t}");
    }
    let l = fs::read_to_string(&log).unwrap();
    for w in [
        "press the power button",
        "unplug the USB cable",
        "answer: ok",
    ] {
        assert!(l.contains(w), "{w} not logged: {l}");
    }
    assert_eq!(l.matches("answer: ok").count(), 9, "{l}");
}

#[test]
fn a_physical_act_the_person_says_did_not_go_stops_the_run_at_that_act_and_names_what_was_seen() {
    let dir = tmp("phys-no");
    let o = run_physical(&dir, &[], "ok\nthe phone stayed dark\n");
    let t = said(&o);
    assert_eq!(o.status.code(), Some(1), "{t}");
    assert!(t.contains("hold the volume down button"), "{t}");
    assert!(t.contains("the phone stayed dark"), "{t}");
    assert!(
        !t.contains("unplug the USB cable"),
        "went on after the stop: {t}"
    );
}

#[test]
fn a_dry_run_lists_the_doer_of_each_act_and_asks_no_one() {
    let dir = tmp("phys-dry");
    let o = run_physical(&dir, &["--dry-run"], "");
    let t = said(&o);
    assert_eq!(o.status.code(), Some(0), "{t}");
    let lines: Vec<&str> = t
        .lines()
        .filter(|l| l.contains("dry run, not run"))
        .collect();
    assert_eq!(lines.len(), 9, "{t}");
    assert!(
        lines.iter().all(|l| l.ends_with("(by person)")),
        "a doer is missing: {t}"
    );
    assert!(
        !t.contains("ATTENDED"),
        "the person was asked in a dry run: {t}"
    );
}

#[test]
fn a_bench_that_claims_an_act_does_it_instead_of_the_person_and_only_that_act() {
    let dir = tmp("phys-rig");
    // the person answers eight times: the nine acts less the one the relay claims
    let o = run_physical(&dir, &["--part", "rig"], &"ok\n".repeat(8));
    let t = said(&o);
    assert_eq!(o.status.code(), Some(0), "{t}");
    assert!(t.contains("the relay did: unplug the USB cable"), "{t}");
    assert!(
        !t.contains("ATTENDED - please unplug the USB cable"),
        "the person was asked for the relay's act: {t}"
    );
    assert!(
        t.contains("ATTENDED - please plug the USB cable back in"),
        "{t}"
    );
    let dry = run_physical(&dir, &["--part", "rig", "--dry-run"], "");
    let d = said(&dry);
    let unplug = d
        .lines()
        .find(|l| l.contains("unplug the USB cable"))
        .unwrap();
    let replug = d
        .lines()
        .find(|l| l.contains("plug the USB cable back in"))
        .unwrap();
    assert!(unplug.ends_with("(by relay)"), "{d}");
    assert!(replug.ends_with("(by person)"), "{d}");
}

#[test]
fn a_run_stopped_at_the_first_prompt_passes_an_act_a_bench_does_and_stops_at_the_first_a_person_does(
) {
    let dir = tmp("phys-until");
    let log = dir.join("u.log");
    // the relay claims press_power: it is not a prompt, the next act (hold_button) is the person's
    let o = run_physical(
        &dir,
        &[
            "--part",
            "rig-power",
            "--until",
            "FIRST_PROMPT",
            "--log",
            log.to_str().unwrap(),
        ],
        "",
    );
    let t = said(&o);
    assert_eq!(o.status.code(), Some(0), "{t}");
    assert!(t.contains("the relay did: press the power button"), "{t}");
    assert!(
        t.contains(
            "stopped before the first prompt (\"hold the volume down button\") after 1 step(s)"
        ),
        "{t}"
    );
    assert!(!t.contains("ATTENDED"), "{t}");
    // without the relay the first act is the person's: the run stops before it
    let o = run_physical(&dir, &["--until", "FIRST_PROMPT"], "");
    let t = said(&o);
    assert!(
        t.contains("stopped before the first prompt (\"press the power button\") after 0 step(s)"),
        "{t}"
    );
}

#[test]
fn the_log_says_who_did_each_act_and_a_plain_ask_keeps_its_old_lines() {
    let dir = tmp("phys-log");
    let log = dir.join("l.log");
    let o = run_physical(
        &dir,
        &["--part", "rig", "--log", log.to_str().unwrap()],
        &"ok\n".repeat(8),
    );
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    let l = fs::read_to_string(&log).unwrap();
    assert!(
        l.contains("begin unplug the USB cable: ask by relay"),
        "{l}"
    );
    assert!(
        l.contains("begin press the power button: ask\n"),
        "a person's act is logged as ever: {l}"
    );
    // an ask_author step prints and logs as it did before the benches
    let dir = tmp("phys-ask");
    let devlab = stand_in(&dir, "pmicwdt 16384 0");
    let log = dir.join("a.log");
    let o = run(&dir, &devlab, &["--log", log.to_str().unwrap()], "ok\n");
    let t = said(&o);
    assert!(
        t.contains("ask 2: ask the author: hold volume down and plug the cable in\n"),
        "{t}"
    );
    assert!(!t.contains("by person"), "{t}");
}

/// A device.toml that declares its kernels, written where the procedure runs.
fn declare_kernels(dir: &Path, kernels: &str) {
    let mut m = fs::read_to_string(dir.join("device.toml")).unwrap();
    m.push_str(&format!("[kernels]\n{kernels}\n"));
    fs::write(dir.join("device.toml"), m).unwrap();
}

#[test]
fn boot_once_takes_only_a_kernel_the_device_declares_and_names_the_key_when_it_does_not() {
    // declared: it goes through as before
    let dir = tmp("kern-ok");
    declare_kernels(&dir, "test = [\"calls4\"]");
    let o = devlab_calls(&dir, "boot-once", "calls4", 0);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    assert!(
        fs::read_to_string(dir.join("args"))
            .unwrap()
            .contains("kernel try calls4"),
        "a declared kernel did not reach devlab"
    );
    // not listed: the dry run fails naming the key and the kernel, and reaches nothing
    let dir = tmp("kern-unlisted");
    declare_kernels(&dir, "test = [\"other\"]");
    let o = devlab_calls(&dir, "boot-once-dry", "calls4", 0);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(2), "{t}");
    assert!(
        t.contains("[kernels] test") && t.contains("calls4") && t.contains("device.toml"),
        "{t}"
    );
    assert!(!dir.join("args").exists(), "a dry run reached devlab");
    // no [kernels] table at all
    let dir = tmp("kern-none");
    fs::write(dir.join("no-kernels"), "").unwrap();
    let o = devlab_calls(&dir, "boot-once-dry", "calls4", 0);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(2), "{t}");
    assert!(t.contains("[kernels] test"), "{t}");
}

#[test]
fn back_to_known_good_needs_the_kernels_table_and_names_the_kept_kernel_it_returns_to() {
    let dir = tmp("kern-back-none");
    fs::write(dir.join("no-kernels"), "").unwrap();
    let o = devlab_calls(&dir, "back-dry", "", 0);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(2), "{t}");
    assert!(
        t.contains("[kernels]") && t.contains("back_to_known_good"),
        "{t}"
    );
    let dir = tmp("kern-back-kept");
    declare_kernels(&dir, "test = [\"calls4\"]\nkept = \"calls4\"");
    let o = devlab_calls(&dir, "back-dry", "", 0);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(0), "{t}");
    assert!(
        t.contains("reboot into the default kernel (kept: calls4)"),
        "{t}"
    );
    let dir = tmp("kern-back-stock");
    declare_kernels(&dir, "test = [\"calls4\"]");
    let o = devlab_calls(&dir, "back-dry", "", 0);
    assert!(
        said(&o).contains("reboot into the default kernel (stock)"),
        "{}",
        said(&o)
    );
}

#[test]
fn a_name_devlab_would_refuse_is_named_as_such_even_when_the_device_declares_no_kernels() {
    let dir = tmp("kern-bad-none");
    fs::write(dir.join("no-kernels"), "").unwrap();
    let o = devlab_calls(&dir, "boot-once", "calls+", 0);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(1), "{t}");
    assert!(t.contains("not a test kernel name"), "{t}");
    let dir = tmp("kern-empty-list");
    declare_kernels(&dir, "test = []");
    let o = devlab_calls(&dir, "boot-once-dry", "calls4", 0);
    assert!(said(&o).contains("lists: no kernel"), "{}", said(&o));
}

/// The example with `lines` put before its first step, run with --dry-run in a folder whose device.toml is `toml`.
fn with_values(name: &str, toml: &str, lines: &str) -> Output {
    let dir = tmp(name);
    fs::write(dir.join("device.toml"), toml).unwrap();
    let devlab = stand_in(&dir, "x");
    let src = fs::read_to_string(example(&dir)).unwrap().replace(
        "step(\"the phone answers\"",
        &format!("{lines}\n    step(\"the phone answers\""),
    );
    fs::write(dir.join("attended.rs"), src).unwrap();
    let mut c = Command::new("rust-script")
        .arg(dir.join("attended.rs"))
        .args(["--dry-run", "--log", dir.join("v.log").to_str().unwrap()])
        .env("LPOCO_ATTENDED_DEVLAB", &devlab)
        .current_dir(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(c.stdin.take());
    c.wait_with_output().unwrap()
}

const VALUES: &str = "[device]\ncodename = \"xiaomi-surya\"\n[health]\nbattery_floor_mv = 3700\nbattery_source = \"fastboot-getvar\"\n[procedures.boot-the-pmicwdt-kernel]\nwindow = 120\nname = \"abc\"\n";
const READ: &str = "println!(\"floor={} window={} name={}\", dev(\"health.battery_floor_mv\").int(), proc(\"window\").int(), proc(\"name\").text());";

#[test]
fn a_procedure_reads_the_devices_keys_and_its_own_values_by_name() {
    let o = with_values("keys", VALUES, READ);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(0), "{t}");
    assert!(t.contains("floor=3700 window=120 name=abc"), "{t}");
}

#[test]
fn a_missing_key_fails_the_dry_run_naming_it_before_any_step() {
    let no_proc = VALUES.replace("window = 120\n", "");
    let o = with_values("no-proc", &no_proc, READ);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(2), "{t}");
    assert!(
        t.contains("procedures.boot-the-pmicwdt-kernel.window"),
        "{t}"
    );
    let no_dev = VALUES.replace(
        "battery_floor_mv = 3700\nbattery_source = \"fastboot-getvar\"\n",
        "",
    );
    let o = with_values("no-dev", &no_dev, READ);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(2), "{t}");
    assert!(t.contains("health.battery_floor_mv"), "{t}");
    let o = with_values("wrong-type", VALUES, "let _ = proc(\"name\").int();");
    let t = said(&o);
    assert_eq!(o.status.code(), Some(2), "{t}");
    assert!(
        t.contains("procedures.boot-the-pmicwdt-kernel.name") && t.contains("number"),
        "{t}"
    );
}

#[test]
fn a_call_that_outlasts_the_limit_is_cut_and_fails_its_step_naming_the_limit() {
    let dir = tmp("limit");
    let devlab = dir.join("devlab");
    // a phone that hangs: the call never answers, and what it started stays on the pipes
    fs::write(&devlab, "#!/bin/sh\nsleep 60\n").unwrap();
    Command::new("chmod")
        .arg("+x")
        .arg(&devlab)
        .status()
        .unwrap();
    let log = dir.join("limit.log");
    let began = std::time::Instant::now();
    let o = run(
        &dir,
        &devlab,
        &["--log", log.to_str().unwrap(), "--call-limit", "2"],
        "ok\n",
    );
    let t = said(&o);
    assert!(
        began.elapsed().as_secs() < 45,
        "the hang was waited out: {t}"
    );
    assert_eq!(o.status.code(), Some(1), "{t}");
    assert!(
        t.contains("the phone answers") && t.contains("2 s"),
        "the step and the limit are not named: {t}"
    );
    let l = fs::read_to_string(&log).unwrap();
    assert!(l.contains("FAIL") && l.contains("2 s"), "{l}");
}

#[test]
fn a_call_limit_that_is_not_a_number_of_seconds_is_refused_before_any_step() {
    let dir = tmp("limit-bad");
    let devlab = stand_in(&dir, "x");
    let o = run(&dir, &devlab, &["--call-limit", "soon"], "ok\n");
    let t = said(&o);
    assert_eq!(o.status.code(), Some(2), "{t}");
    assert!(t.contains("call-limit") || t.contains("call limit"), "{t}");
    assert!(!dir.join("calls").exists(), "a step ran: {t}");
}

#[test]
fn from_skips_the_earlier_steps_logs_them_skipped_and_starts_at_the_named_one() {
    let dir = tmp("from");
    let devlab = stand_in(&dir, "pmicwdt 16384 0");
    let log = dir.join("from.log");
    let o = run(
        &dir,
        &devlab,
        &[
            "--from",
            "put the kernel on it",
            "--log",
            log.to_str().unwrap(),
        ],
        "",
    );
    let t = said(&o);
    assert_eq!(o.status.code(), Some(0), "{t}");
    assert!(t.contains("the phone answers - skipped"), "{t}");
    assert!(
        t.contains("put the kernel on it") && !t.contains("put the kernel on it - skipped"),
        "{t}"
    );
    let calls = fs::read_to_string(dir.join("calls")).unwrap();
    assert_eq!(
        calls.lines().count(),
        3,
        "the skipped steps reached the phone: {calls}"
    );
    let l = fs::read_to_string(&log).unwrap();
    assert!(
        l.contains("skip the phone answers") && l.contains("--from"),
        "{l}"
    );
}

#[test]
fn an_unknown_from_exits_2_listing_the_steps_there_are_and_runs_nothing() {
    let dir = tmp("from-bad");
    let devlab = stand_in(&dir, "x");
    let o = run(&dir, &devlab, &["--from", "no such step"], "");
    let t = said(&o);
    assert_eq!(o.status.code(), Some(2), "{t}");
    assert!(
        t.contains("no such step")
            && t.contains("the phone answers")
            && t.contains("put the kernel on it"),
        "{t}"
    );
    assert!(!dir.join("calls").exists(), "a step reached the phone: {t}");
}

#[test]
fn a_dry_run_shows_what_from_would_skip() {
    let dir = tmp("from-dry");
    let devlab = stand_in(&dir, "x");
    let o = run(
        &dir,
        &devlab,
        &["--dry-run", "--from", "put the kernel on it"],
        "",
    );
    let t = said(&o);
    assert_eq!(o.status.code(), Some(0), "{t}");
    assert!(t.contains("the phone answers - skipped"), "{t}");
    assert!(t.contains("put the kernel on it - dry run"), "{t}");
}

#[test]
fn a_run_that_ended_at_until_names_the_from_that_resumes_it() {
    let dir = tmp("from-resume");
    let devlab = stand_in(&dir, "x");
    let log = dir.join("resume.log");
    let o = run(
        &dir,
        &devlab,
        &["--until", "FIRST_PROMPT", "--log", log.to_str().unwrap()],
        "",
    );
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    let t = said(&o);
    assert!(
        t.contains("resume with --from \"ask the author: hold volume down"),
        "{t}"
    );
    let l = fs::read_to_string(&log).unwrap();
    assert!(l.contains("--from"), "{l}");
}

#[test]
fn boot_once_of_the_kernel_that_already_runs_is_skipped_and_logged_already_running() {
    let dir = tmp("skip-boot");
    declare_kernels(&dir, "test = [\"pmicwdt\"]");
    let o = devlab_calls(&dir, "boot-once", "pmicwdt", 0);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(0), "{t}");
    assert!(t.contains("already running"), "{t}");
    let args = fs::read_to_string(dir.join("args")).unwrap();
    assert!(!args.contains("kernel try"), "a try was asked for: {args}");
}

#[test]
fn boot_once_of_any_other_kernel_still_boots_even_when_its_name_is_part_of_the_running_one() {
    let dir = tmp("skip-boot-other");
    declare_kernels(&dir, "test = [\"pmic\"]");
    let o = devlab_calls(&dir, "boot-once", "pmic", 0);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    let args = fs::read_to_string(dir.join("args")).unwrap();
    assert!(args.contains("kernel try pmic"), "{args}");
}

#[test]
fn back_to_known_good_is_skipped_when_the_kept_kernel_already_runs_and_not_otherwise() {
    let dir = tmp("skip-back");
    declare_kernels(&dir, "test = [\"pmicwdt\"]\nkept = \"pmicwdt\"");
    let o = devlab_calls(&dir, "back", "", 0);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(0), "{t}");
    assert!(t.contains("already running"), "{t}");
    assert!(
        !dir.join("rebooted").exists(),
        "the phone was rebooted: {t}"
    );
    let dir = tmp("skip-back-other");
    declare_kernels(&dir, "test = [\"pmicwdt\", \"calls4\"]\nkept = \"calls4\"");
    let o = devlab_calls(&dir, "back", "", 0);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    assert!(dir.join("rebooted").exists(), "the phone was not rebooted");
}

/// A device that boots its test kernel from memory: its image, its fastboot serial, the gadget it then shows.
fn declare_fastboot_boot(dir: &Path, device_wait: u32) {
    let mut m = fs::read_to_string(dir.join("device.toml")).unwrap();
    m.push_str(&format!(
        "[fastboot]\nserial = \"FA81\"\n[[host.gadget]]\nserial = \"s\"\naccess = [\"tty\"]\nusb_id = \"1d6b:0104\"\n\
         [boot]\nkernel_offset = 0\nramdisk_offset = 0\ntags_offset = 0\npagesize = 4096\ncmdline = \"x\"\ndevice_wait = {device_wait}\n\
         [kernels]\ntest = [\"z21\"]\nboot = \"fastboot\"\n[images]\ndir = \"build/images\"\n[images.z21]\npath = \"images-z21/boot-z21.img\"\n"
    ));
    fs::write(dir.join("device.toml"), m).unwrap();
}

#[test]
fn boot_once_on_a_fastboot_device_boots_the_declared_image_from_memory_and_waits_for_its_gadget() {
    let dir = tmp("boot-fastboot");
    declare_fastboot_boot(&dir, 4);
    usb_tree(&dir, Some(("1d6b", "0104")));
    let o = devlab_calls(&dir, "boot-once", "z21", 0);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    // the image of [images.z21] under [images] dir goes to fastboot boot; no boot entry is tried and no login is asked
    let args = fs::read_to_string(dir.join("args")).unwrap();
    let want = format!(
        "--device {d}/device.toml fastboot boot now {d}/build/images/images-z21/boot-z21.img",
        d = dir.display()
    );
    assert!(args.lines().any(|l| l == want), "{args}");
    assert!(!args.contains("kernel try"), "{args}");
    // the gadget never shows: the step fails naming the id it waited for
    let dir = tmp("boot-fastboot-nogadget");
    declare_fastboot_boot(&dir, 1);
    usb_tree(&dir, None);
    let o = devlab_calls(&dir, "boot-once", "z21", 0);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(1), "{t}");
    assert!(t.contains("1d6b:0104 is not on the USB bus"), "{t}");
    // fastboot refuses: the step fails with what it said, and no gadget is waited for
    let dir = tmp("boot-fastboot-refused");
    declare_fastboot_boot(&dir, 4);
    usb_tree(&dir, None);
    let o = devlab_calls(&dir, "boot-once", "z21", 1);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(1), "{t}");
    assert!(
        t.contains("booted from memory") && !t.contains("not on the USB bus"),
        "{t}"
    );
    // a dry run reaches nothing
    let dir = tmp("boot-fastboot-dry");
    declare_fastboot_boot(&dir, 4);
    let o = devlab_calls(&dir, "boot-once-dry", "z21", 0);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    assert!(!dir.join("args").exists(), "a dry run reached devlab");
}

#[test]
fn the_way_back_of_a_fastboot_device_reboots_even_when_the_kept_kernel_runs_from_memory() {
    // a kernel booted from memory is gone at the next reboot: "kept" names the image the way back returns to, so
    // that the running kernel carries its name must not make the way back skip itself
    let dir = tmp("back-fastboot");
    declare_fastboot_boot(&dir, 4);
    let mut m = fs::read_to_string(dir.join("device.toml")).unwrap();
    m = m.replace("boot = \"fastboot\"", "boot = \"fastboot\"\nkept = \"z21\"");
    fs::write(dir.join("device.toml"), m).unwrap();
    fs::write(dir.join("uname-release"), "6.1.0-z21\n").unwrap();
    let o = devlab_calls(&dir, "back", "", 0);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    assert!(
        fs::read_to_string(dir.join("rebooted"))
            .unwrap()
            .contains("systemctl reboot"),
        "the way back skipped its reboot: {}",
        said(&o)
    );
}

#[test]
fn the_gadget_may_come_up_within_device_wait_and_the_step_waits_that_long_for_it() {
    // device_wait = 2: the gadget shows 1 s after fastboot returns; one try with no wait would have failed
    let dir = tmp("boot-fastboot-late");
    declare_fastboot_boot(&dir, 2);
    let root = usb_tree(&dir, None);
    let late = root.join("1-2");
    let t = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(1000));
        fs::create_dir_all(&late).unwrap();
        fs::write(late.join("idVendor"), "1d6b\n").unwrap();
        fs::write(late.join("idProduct"), "0104\n").unwrap();
        fs::write(late.join("serial"), "s\n").unwrap();
    });
    let o = devlab_calls(&dir, "boot-once", "z21", 0);
    t.join().unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
}

/// sha256 of the three bytes "abc"
const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

/// The fastboot device with its image on disk and the checksum it declares.
fn fastboot_with_image(name: &str, bytes: &str, sha256: &str) -> PathBuf {
    let dir = tmp(name);
    declare_fastboot_boot(&dir, 4);
    let m = fs::read_to_string(dir.join("device.toml")).unwrap();
    fs::write(
        dir.join("device.toml"),
        m.replace(
            "boot-z21.img\"",
            &format!("boot-z21.img\"\nsha256 = \"{sha256}\""),
        ),
    )
    .unwrap();
    let img = dir.join("build/images/images-z21/boot-z21.img");
    fs::create_dir_all(img.parent().unwrap()).unwrap();
    fs::write(img, bytes).unwrap();
    dir
}

#[test]
fn an_image_whose_checksum_is_not_the_declared_one_is_not_booted_and_both_are_named() {
    let dir = fastboot_with_image("image-sum-bad", "abd", ABC);
    usb_tree(&dir, Some(("1d6b", "0104")));
    let o = devlab_calls(&dir, "boot-once", "z21", 0);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(1), "{t}");
    assert!(
        t.contains(ABC)
            && t.contains("a52d159f262b2c6ddb724a61840befc36eb30c88877a4030b65cbe86298449c9"),
        "{t}"
    );
    let args = fs::read_to_string(dir.join("args")).unwrap_or_default();
    assert!(
        !args.contains("fastboot boot"),
        "a wrong image reached fastboot: {args}"
    );
    // the checksum that matches boots it
    let dir = fastboot_with_image("image-sum-ok", "abc", ABC);
    usb_tree(&dir, Some(("1d6b", "0104")));
    let o = devlab_calls(&dir, "boot-once", "z21", 0);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    assert!(fs::read_to_string(dir.join("args"))
        .unwrap()
        .contains("fastboot boot now"));
}

#[test]
fn the_gadget_is_the_one_with_the_declared_serial_not_another_phone_with_the_same_id() {
    let dir = tmp("gadget-serial-other");
    declare_fastboot_boot(&dir, 1);
    let root = usb_tree(&dir, Some(("1d6b", "0104")));
    fs::write(root.join("1-2/serial"), "someone-else\n").unwrap();
    let o = devlab_calls(&dir, "boot-once", "z21", 0);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(1), "{t}");
    assert!(t.contains("1d6b:0104") && t.contains("serial \"s\""), "{t}");
}

#[test]
fn a_declared_checksum_with_no_image_on_disk_stops_the_step_and_a_dry_run_reads_nothing() {
    let dir = fastboot_with_image("image-sum-missing", "abc", ABC);
    fs::remove_file(dir.join("build/images/images-z21/boot-z21.img")).unwrap();
    usb_tree(&dir, Some(("1d6b", "0104")));
    let o = devlab_calls(&dir, "boot-once", "z21", 0);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(1), "{t}");
    assert!(t.contains("cannot read the image"), "{t}");
    let args = fs::read_to_string(dir.join("args")).unwrap_or_default();
    assert!(!args.contains("fastboot boot"), "{args}");
    // a wrong image in a dry run: nothing is read or run
    let dir = fastboot_with_image("image-sum-dry", "abd", ABC);
    let o = devlab_calls(&dir, "boot-once-dry", "z21", 0);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    assert!(!dir.join("args").exists());
}

#[test]
fn the_serial_is_found_among_devices_of_one_id_and_is_compared_as_written() {
    // two phones show 1d6b:0104: the one with the serial is enough, wherever it sits
    let dir = tmp("gadget-two");
    declare_fastboot_boot(&dir, 1);
    let root = usb_tree(&dir, Some(("1d6b", "0104")));
    fs::write(root.join("1-2/serial"), "someone-else\n").unwrap();
    fs::create_dir_all(root.join("1-3")).unwrap();
    fs::write(root.join("1-3/idVendor"), "1d6b\n").unwrap();
    fs::write(root.join("1-3/idProduct"), "0104\n").unwrap();
    fs::write(root.join("1-3/serial"), "s\n").unwrap();
    let o = devlab_calls(&dir, "boot-once", "z21", 0);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    // the serial in another case is another serial
    let dir = tmp("gadget-case");
    declare_fastboot_boot(&dir, 1);
    let root = usb_tree(&dir, Some(("1d6b", "0104")));
    fs::write(root.join("1-2/serial"), "S\n").unwrap();
    assert_eq!(
        devlab_calls(&dir, "boot-once", "z21", 0).status.code(),
        Some(1)
    );
    // a device with no serial file is no match for a declared serial
    let dir = tmp("gadget-noserial");
    declare_fastboot_boot(&dir, 1);
    let root = usb_tree(&dir, Some(("1d6b", "0104")));
    fs::remove_file(root.join("1-2/serial")).unwrap();
    assert_eq!(
        devlab_calls(&dir, "boot-once", "z21", 0).status.code(),
        Some(1)
    );
}

#[test]
fn wait_for_usb_still_goes_by_vendor_and_product_alone() {
    let dir = tmp("usb-no-serial");
    let bus = usb_tree(&dir, Some(("18d1", "4ee0")));
    fs::remove_file(bus.join("1-2/serial")).unwrap();
    let o = usb(&dir, &bus);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
}

#[test]
fn boot_once_on_a_fastboot_device_leaves_a_phone_already_in_fastboot_alone() {
    // a phone already in fastboot is not rebooted
    let dir = tmp("boot-fastboot-already");
    declare_fastboot_boot(&dir, 4);
    usb_tree(&dir, Some(("1d6b", "0104")));
    let o = devlab_calls(&dir, "boot-once", "z21", 0);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    assert!(!dir.join("rebooted").exists(), "{}", said(&o));
}

#[test]
fn a_phone_that_never_reaches_fastboot_ends_the_step_naming_it_and_boots_nothing() {
    let dir = tmp("boot-fastboot-never");
    declare_fastboot_boot(&dir, 1);
    let to = declare_to_bootloader(&dir, None);
    usb_tree(&dir, Some(("1d6b", "0104")));
    fs::write(dir.join("never-in-fastboot"), "").unwrap();
    let o = devlab_calls(&dir, "boot-once", "z21", 0);
    let t = said(&o);
    let _ = fs::remove_file(&to);
    assert_eq!(o.status.code(), Some(1), "{t}");
    assert!(
        t.contains("did not reach fastboot") && t.contains("restart2 bootloader"),
        "{t}"
    );
    let args = fs::read_to_string(dir.join("args")).unwrap();
    assert!(!args.contains("fastboot boot now"), "{args}");
}

#[test]
fn a_fastboot_state_devlab_cannot_read_reboots_nothing_and_ends_the_step() {
    let dir = tmp("boot-fastboot-state-fails");
    declare_fastboot_boot(&dir, 4);
    usb_tree(&dir, Some(("1d6b", "0104")));
    fs::write(dir.join("state-fails"), "").unwrap();
    let o = devlab_calls(&dir, "boot-once", "z21", 0);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(1), "{t}");
    assert!(
        !dir.join("rebooted").exists(),
        "the phone was rebooted on a guess: {t}"
    );
    let args = fs::read_to_string(dir.join("args")).unwrap();
    assert!(
        !args.contains("phone root") && !args.contains("fastboot boot now"),
        "{args}"
    );
}

fn declare_pre_boot(dir: &Path, gates: &[String]) {
    let list = gates
        .iter()
        .map(|g| format!("{g:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    let m = fs::read_to_string(dir.join("device.toml"))
        .unwrap()
        .replace(
            "boot = \"fastboot\"",
            &format!("boot = \"fastboot\"\npre_boot = [{list}]"),
        );
    fs::write(dir.join("device.toml"), m).unwrap();
}

#[test]
fn pre_boot_gates_run_in_order_as_argv_before_fastboot_boot_and_all_must_pass() {
    let dir = tmp("pre-boot-pass");
    declare_fastboot_boot(&dir, 4);
    usb_tree(&dir, Some(("1d6b", "0104")));
    let (a, b) = (dir.join("gate-a"), dir.join("x;y"));
    // argv, not a shell string: `;` is no operator (a file literally named x;y is made), and the second gate sees the first's file
    declare_pre_boot(
        &dir,
        &[
            format!("touch {}", a.display()),
            format!("test -e {}", a.display()),
            format!("touch {}", b.display()),
        ],
    );
    let o = devlab_calls(&dir, "boot-once", "z21", 0);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    assert!(a.exists() && b.exists(), "the gates ran as argv");
    let args = fs::read_to_string(dir.join("args")).unwrap();
    assert!(args.contains("fastboot boot now"), "{args}");
}

#[test]
fn a_failing_pre_boot_gate_boots_nothing_and_names_itself_and_its_output() {
    let dir = tmp("pre-boot-fail");
    declare_fastboot_boot(&dir, 4);
    usb_tree(&dir, Some(("1d6b", "0104")));
    let after = dir.join("after-the-failure");
    declare_pre_boot(
        &dir,
        &[
            format!("ls {}/no-such-gate-file", dir.display()),
            format!("touch {}", after.display()),
        ],
    );
    let o = devlab_calls(&dir, "boot-once", "z21", 0);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(1), "{t}");
    assert!(
        t.contains("pre_boot") && t.contains("no-such-gate-file"),
        "{t}"
    );
    assert!(!after.exists(), "a gate ran after one had failed");
    let args = fs::read_to_string(dir.join("args")).unwrap_or_default();
    assert!(
        !args.contains("fastboot"),
        "nothing reached the phone: {args}"
    );
}

#[test]
fn the_dry_run_lists_the_pre_boot_gates_and_runs_none() {
    let dir = tmp("pre-boot-dry");
    declare_fastboot_boot(&dir, 4);
    let made = dir.join("made-in-a-dry-run");
    declare_pre_boot(&dir, &[format!("touch {}", made.display())]);
    let o = devlab_calls(&dir, "boot-once-dry", "z21", 0);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(0), "{t}");
    assert!(
        t.contains("pre_boot") && t.contains(&format!("touch {}", made.display())),
        "{t}"
    );
    assert!(!made.exists(), "a dry run ran a gate");
}

/// The stand-in helper `declare_to_bootloader` writes (REBOOTED is where it logs)
const HELPER: &str = "#!/bin/sh\necho \"restart2 $*\" >> REBOOTED\n";

/// The device also declares how the lab takes its Linux to the bootloader: a helper pushed to a file under /tmp/ and
/// run there as root. The stand-in helper only writes a line (the host is never rebooted); it is the file on disk
/// whose sha256 the manifest declares. Returns where the phone gets it.
fn declare_to_bootloader(dir: &Path, sha_of: Option<&str>) -> String {
    use sha2::{Digest, Sha256};
    let body = HELPER.replace("REBOOTED", &dir.join("rebooted").display().to_string());
    let sha: String = Sha256::digest(body.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let tool = dir.join("build/images/restart2");
    fs::create_dir_all(tool.parent().unwrap()).unwrap();
    fs::write(&tool, body).unwrap();
    let to = format!(
        "/tmp/{}-restart2",
        dir.file_name().unwrap().to_string_lossy()
    );
    let mut m = fs::read_to_string(dir.join("device.toml")).unwrap();
    m.push_str(&format!(
        "[images.restart2]\npath = \"restart2\"\nsha256 = \"{}\"\n[boot.to_bootloader]\ntool = \"restart2\"\nto = \"{to}\"\nrun = \"{to} bootloader\"\n",
        sha_of.unwrap_or(&sha)
    ));
    fs::write(dir.join("device.toml"), m).unwrap();
    to
}

#[test]
fn a_phone_not_in_fastboot_gets_the_declared_helper_and_runs_it_as_root_never_busybox_reboot() {
    let dir = tmp("boot-fastboot-restart2");
    declare_fastboot_boot(&dir, 4);
    let to = declare_to_bootloader(&dir, None);
    usb_tree(&dir, Some(("1d6b", "0104")));
    fs::write(dir.join("not-in-fastboot"), "").unwrap();
    let o = devlab_calls(&dir, "boot-once", "z21", 0);
    let t = said(&o);
    let _ = fs::remove_file(&to);
    assert_eq!(o.status.code(), Some(0), "{t}");
    let rebooted = fs::read_to_string(dir.join("rebooted")).unwrap();
    assert!(rebooted.contains("restart2 bootloader"), "{rebooted}");
    assert!(!rebooted.contains("reboot bootloader"), "{rebooted}");
    let args = fs::read_to_string(dir.join("args")).unwrap();
    let lines: Vec<&str> = args.lines().collect();
    let at = |what: &str| lines.iter().position(|l| l.contains(what));
    assert!(
        at("phone put").unwrap() < at("phone root").unwrap()
            && at("phone root").unwrap() < at("fastboot boot now").unwrap(),
        "pushed, then run, then the boot: {args}"
    );
    assert!(args.contains("phone put") && args.contains(&to), "{args}");
}

#[test]
fn a_phone_not_in_fastboot_with_no_helper_declared_is_not_rebooted_and_the_step_names_the_table() {
    let dir = tmp("boot-fastboot-no-helper");
    declare_fastboot_boot(&dir, 4);
    usb_tree(&dir, Some(("1d6b", "0104")));
    fs::write(dir.join("not-in-fastboot"), "").unwrap();
    let o = devlab_calls(&dir, "boot-once", "z21", 0);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(1), "{t}");
    assert!(t.contains("[boot.to_bootloader]"), "{t}");
    assert!(!dir.join("rebooted").exists(), "{t}");
    let args = fs::read_to_string(dir.join("args")).unwrap();
    assert!(
        !args.contains("phone root")
            && !args.contains("phone put")
            && !args.contains("fastboot boot now"),
        "{args}"
    );
}

#[test]
fn a_helper_whose_checksum_is_not_the_declared_one_is_never_pushed() {
    let dir = tmp("boot-fastboot-bad-helper");
    declare_fastboot_boot(&dir, 4);
    let to = declare_to_bootloader(&dir, Some(ABC));
    usb_tree(&dir, Some(("1d6b", "0104")));
    fs::write(dir.join("not-in-fastboot"), "").unwrap();
    let o = devlab_calls(&dir, "boot-once", "z21", 0);
    let t = said(&o);
    assert_eq!(o.status.code(), Some(1), "{t}");
    assert!(t.contains(ABC), "{t}");
    let args = fs::read_to_string(dir.join("args")).unwrap();
    assert!(
        !args.contains("phone put") && !args.contains("phone root"),
        "{args}"
    );
    assert!(!Path::new(&to).exists(), "{t}");
}
