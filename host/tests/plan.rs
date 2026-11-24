// SPDX-License-Identifier: MIT OR Apache-2.0
//! The plan changes nothing (decision 2026-11-17-lpoco-host): from a device's `[host]` it names the udev rule and
//! the modules file and the exact root commands; from the lab's `[tools]` it names each tool at its pin, what is
//! found, and the command that would fix it. The root prefix is a stub that records its calls, the tools are stubs.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lpoco-host-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn exe(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

struct Lab {
    dir: PathBuf,
    calls: PathBuf,
}

/// A lab: a device with walleye's `[host]` table, a lab.toml, a recording root stub and recording cargo/git
/// stubs with an older `land` on PATH. Every place is under `dir`.
fn lab(name: &str, lab_toml_tools: &str) -> Lab {
    let dir = tmp(name);
    let dev = dir.join("devs/google-walleye-lab");
    fs::create_dir_all(&dev).unwrap();
    fs::write(
        dev.join("device.toml"),
        r#"[device]
codename = "google-walleye"
[fastboot]
serial = "FA81N1A00790"
[host]
watch = true
modules = ["usb_storage", "cdc_acm"]
[[host.gadget]]
serial = "walleye-first-boot"
access = ["block", "tty"]
"#,
    )
    .unwrap();
    fs::write(
        dir.join("lab.toml"),
        format!(
            "devices = \"{}/home/Projects/linux-phone-oco/*/device.toml\"\n{lab_toml_tools}",
            dir.display()
        ),
    )
    .unwrap();
    let bin = dir.join("stubs");
    fs::create_dir_all(&bin).unwrap();
    let calls = dir.join("calls");
    let rec = format!("echo \"$0 $@\" >> {}", calls.display());
    exe(&bin.join("root"), &rec);
    exe(&bin.join("cargo"), &rec);
    exe(&bin.join("git"), &rec);
    exe(&bin.join("systemctl"), &rec);
    exe(&bin.join("land"), "echo land 0.1.0");
    fs::create_dir_all(dir.join("home/Projects")).unwrap();
    // the watcher units name devices at HOME/Projects/linux-phone-oco/NAME-lab
    std::os::unix::fs::symlink(dir.join("devs"), dir.join("home/Projects/linux-phone-oco"))
        .unwrap();
    Lab { dir, calls }
}

/// What the stubs recorded other than the host reading its declarations through git (rev-parse, fetch, show, rev-list).
fn changes(l: &Lab) -> Option<String> {
    let all = fs::read_to_string(&l.calls).ok()?;
    let kept: Vec<&str> = all.lines().filter(|c| !is_read_only_git(c)).collect();
    (!kept.is_empty()).then(|| kept.join("\n"))
}

fn host(l: &Lab, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lpoco-host"))
        .args(args)
        .env_clear()
        .env("HOME", l.dir.join("home"))
        .env("PATH", l.dir.join("stubs"))
        .env("LPOCO_LAB", l.dir.join("lab.toml"))
        .env("LPOCO_ROOT", l.dir.join("stubs/root"))
        .env("LPOCO_FS_ROOT", l.dir.join("fs"))
        .output()
        .unwrap()
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn plan_grants_gadget_serial_and_changes_nothing() {
    let l = lab("grant", "");
    let o = host(&l, &["plan"]);
    let s = out(&o);
    assert!(
        o.status.success(),
        "{s}{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(
        s.contains("/etc/udev/rules.d/70-lpoco-google-walleye.rules"),
        "{s}"
    );
    for kind in ["block", "tty"] {
        assert!(
            s.contains(&format!(
                "ACTION!=\"remove\", SUBSYSTEM==\"{kind}\", ATTRS{{serial}}==\"walleye-first-boot\", TAG+=\"uaccess\""
            )),
            "{s}"
        );
    }
    assert!(s.contains("/etc/modules-load.d/lpoco.conf"), "{s}");
    assert!(s.contains("usb_storage") && s.contains("cdc_acm"), "{s}");
    assert!(
        s.contains("udevadm trigger --action=change --attr-match=serial=walleye-first-boot"),
        "{s}"
    );
    assert!(changes(&l).is_none(), "the plan called the root prefix");
    // the default verb is the plan
    assert_eq!(out(&host(&l, &[])), s);
    // prepare without `now` refuses and calls nothing
    let o = host(&l, &["prepare"]);
    assert!(!o.status.success());
    assert!(
        changes(&l).is_none(),
        "prepare without now called the root prefix"
    );
}

#[test]
fn plan_lists_host_tools_at_pins_and_installs_nothing() {
    let l = lab(
        "tools",
        r#"[tools]
lpoco = "0.2.0"
[tools.land]
git = "https://github.com/grenudi/lab-machine"
path = "bin/land"
rev = "abc123"
version = "0.3.0"
[tools.oco-work]
git = "https://github.com/grenudi/lab-machine"
path = "bin/oco-work"
rev = "abc123"
version = "1.0.0"
[tools.agent-env]
git = "https://github.com/grenudi/ideal-agent"
path = "scripts/agent-env"
rev = "def456"
version = "2.0.0"
"#,
    );
    let o = host(&l, &["plan"]);
    let s = out(&o);
    assert!(
        o.status.success(),
        "{s}{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let line = |name: &str| {
        s.lines()
            .find(|x| x.trim_start().starts_with(name))
            .unwrap_or("")
            .to_string()
    };
    let land = line("land");
    assert!(
        land.contains("abc123") && land.contains("on PATH"),
        "a script is pinned by rev: {s}"
    );
    assert!(s.contains("git clone"), "{s}");
    assert!(s.contains("oco-work") && s.contains("agent-env"), "{s}");
    assert!(
        s.contains("cargo install --locked --git https://github.com/linux-phone-oco/lab-tools --tag lpoco-v0.2.0"),
        "{s}"
    );
    for p in ["lpoco-lab", "lpoco-bringup", "lpoco-health", "lpoco-host"] {
        assert!(s.contains(&format!("--tag {p}-v0.2.0 {p}")), "{p}: {s}");
    }
    assert!(changes(&l).is_none(), "the plan ran a stub");
    assert!(
        !l.dir.join("home/.local").exists(),
        "the plan wrote into ~/.local"
    );
}

#[test]
fn prepare_now_runs_the_printed_root_commands_and_only_them() {
    let l = lab("now", "");
    let o = host(&l, &["prepare", "now"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let calls = changes(&l).unwrap();
    for want in [
        "install -D -m 0644",
        "70-lpoco-google-walleye.rules",
        "udevadm control --reload",
        "udevadm trigger --action=change --attr-match=serial=walleye-first-boot",
    ] {
        assert!(calls.contains(want), "{want}: {calls}");
    }
    // what the plan printed is what ran
    let plan = out(&host(&l, &["plan"]));
    for line in calls
        .lines()
        .filter(|c| !c.contains("modprobe") && !c.contains("daemon-reload"))
    {
        let cmd = line.split_once(' ').unwrap().1;
        assert!(
            plan.contains(cmd),
            "{cmd} was run but not in the plan:\n{plan}"
        );
    }
}

#[test]
fn a_manifest_that_would_inject_into_root_commands_is_refused() {
    let l = lab("inject", "");
    fs::write(
        l.dir.join("devs/google-walleye-lab/device.toml"),
        "[device]\ncodename = \"../x\"\n[host]\nmodules = [\"-r\"]\n",
    )
    .unwrap();
    let o = host(&l, &["plan"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("codename"),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(changes(&l).is_none());
}

#[test]
fn the_wide_rule_is_named_and_never_touched_and_an_old_devlab_is_off_the_pin() {
    let l = lab("kept", "[tools]\nlpoco = \"0.2.0\"\n");
    fs::create_dir_all(l.dir.join("fs/etc/udev/rules.d")).unwrap();
    fs::write(l.dir.join("fs/etc/udev/rules.d/51-android.rules"), "x\n").unwrap();
    exe(&l.dir.join("stubs/devlab"), "echo devlab 0.1.0");
    let s = out(&host(&l, &["plan"]));
    assert!(s.contains("51-android.rules is a wide rule"), "{s}");
    assert!(
        s.lines()
            .any(|x| x.contains("devlab") && x.contains("0.1.0")),
        "{s}"
    );
    assert!(s.contains("off the pin"), "{s}");
    assert!(
        !s.contains("rm ") && !s.contains("51-android.rules ("),
        "{s}"
    );
}

const SCRIPT_TOOLS: &str = r#"[tools.land]
git = "https://github.com/grenudi/lab-machine"
path = "bin/land"
rev = "abc123"
version = "0.1.0"
"#;

#[test]
fn a_script_tool_is_judged_by_its_rev_stamp_not_by_version() {
    // a script prints no --version of its own: `land` here prints nothing useful
    let l = lab("rev", SCRIPT_TOOLS);
    exe(&l.dir.join("stubs/land"), "echo usage: land BRANCH");
    let s = out(&host(&l, &["plan"]));
    let land = s
        .lines()
        .find(|x| x.trim_start().starts_with("land"))
        .unwrap_or("");
    assert!(
        land.contains("on PATH") && land.contains("rev not known"),
        "a script on PATH without a stamp is named as found: {s}"
    );
    assert!(!land.contains("not found"), "{s}");
    // the install wrote its stamp: at the pinned rev there is nothing to do
    let stamps = l.dir.join("home/.local/share/lpoco-host");
    fs::create_dir_all(&stamps).unwrap();
    fs::write(stamps.join("rev-land"), "abc123\n").unwrap();
    let s = out(&host(&l, &["plan"]));
    let land = s
        .lines()
        .find(|x| x.trim_start().starts_with("land"))
        .unwrap_or("");
    assert!(
        land.contains("abc123") && land.contains("nothing to do"),
        "{s}"
    );
    // at another rev it is off the pin and the clone and copy are printed
    fs::write(stamps.join("rev-land"), "old999\n").unwrap();
    let s = out(&host(&l, &["plan"]));
    assert!(s.contains("old999") && s.contains("git clone"), "{s}");
}

#[test]
fn prepare_now_stamps_the_rev_of_an_installed_script() {
    let l = lab("stamp", SCRIPT_TOOLS);
    // the git stub makes the clone folder, with the script in it
    exe(
        &l.dir.join("stubs/git"),
        &format!(
            "echo \"$0 $@\" >> {}\ncase \"$1\" in clone) /bin/mkdir -p \"$4/bin\"; echo x > \"$4/bin/land\";; esac",
            l.calls.display()
        ),
    );
    let o = host(&l, &["prepare", "now"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let stamp = fs::read_to_string(l.dir.join("home/.local/share/lpoco-host/rev-land")).unwrap();
    assert_eq!(stamp.trim(), "abc123");
}

#[test]
fn devices_accepts_a_list_of_globs() {
    let l = lab("list", "");
    let dev = l.dir.join("devs/other-lab");
    fs::create_dir_all(&dev).unwrap();
    fs::write(
        dev.join("device.toml"),
        "[device]\ncodename = \"other\"\n[host]\nmodules = [\"loop\"]\n",
    )
    .unwrap();
    fs::write(
        l.dir.join("lab.toml"),
        format!(
            "devices = [\"{d}/devs/g*-lab/device.toml\", \"{d}/devs/o*-lab/device.toml\"]\n",
            d = l.dir.display()
        ),
    )
    .unwrap();
    let o = host(&l, &["plan"]);
    let s = out(&o);
    assert!(
        o.status.success(),
        "{s}{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(s.contains("google-walleye") && s.contains("other"), "{s}");
}

#[test]
fn the_off_the_pin_advice_says_reinstall_into_local() {
    let l = lab("advice", "[tools]\nlpoco = \"0.2.0\"\n");
    exe(&l.dir.join("stubs/devlab"), "echo devlab 0.1.0");
    let s = out(&host(&l, &["plan"]));
    assert!(
        s.contains("~/.local") && s.contains("remove any copy that shadows"),
        "{s}"
    );
    assert!(!s.contains("~/.cargo/bin before"), "{s}");
}

#[test]
fn a_stamp_without_its_script_is_not_a_found_tool() {
    let l = lab("orphan", SCRIPT_TOOLS);
    let stamps = l.dir.join("home/.local/share/lpoco-host");
    fs::create_dir_all(&stamps).unwrap();
    fs::write(stamps.join("rev-land"), "abc123\n").unwrap();
    let _ = fs::remove_file(l.dir.join("stubs/land"));
    let s = out(&host(&l, &["plan"]));
    assert!(s.contains("git clone"), "{s}");
}

/// Real git, a bare origin and a clone that is one commit behind it.
fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn reads_each_device_from_origin_main_and_names_how_far_behind() {
    let l = lab("origin-main", "");
    let devs = l.dir.join("devs");
    fs::remove_dir_all(devs.join("google-walleye-lab")).unwrap();
    let origin = l.dir.join("origin.git");
    let work = l.dir.join("work");
    for d in [&origin, &work] {
        fs::create_dir_all(d).unwrap();
    }
    git(&origin, &["init", "-q", "--bare", "-b", "main"]);
    git(&work, &["init", "-q", "-b", "main"]);
    fs::write(
        work.join("device.toml"),
        "[device]\ncodename = \"google-walleye\"\n",
    )
    .unwrap();
    git(&work, &["add", "."]);
    git(&work, &["commit", "-q", "-m", "no host yet"]);
    git(
        &work,
        &["remote", "add", "origin", origin.to_str().unwrap()],
    );
    git(&work, &["push", "-q", "origin", "main"]);
    git(
        &devs,
        &[
            "clone",
            "-q",
            origin.to_str().unwrap(),
            "google-walleye-lab",
        ],
    );
    fs::write(
        work.join("device.toml"),
        "[device]\ncodename = \"google-walleye\"\n[host]\nmodules = [\"cdc_acm\"]\n[[host.gadget]]\nserial = \"walleye-first-boot\"\naccess = [\"tty\"]\n",
    )
    .unwrap();
    git(&work, &["commit", "-q", "-am", "declare host"]);
    git(&work, &["push", "-q", "origin", "main"]);
    fs::remove_file(l.dir.join("stubs/git")).unwrap();
    let real_git = String::from_utf8(
        Command::new("sh")
            .args(["-c", "command -v git"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let git_dir = Path::new(real_git.trim()).parent().unwrap().to_path_buf();
    let o = Command::new(env!("CARGO_BIN_EXE_lpoco-host"))
        .arg("plan")
        .env_clear()
        .env("HOME", l.dir.join("home"))
        .env(
            "PATH",
            format!("{}:{}", l.dir.join("stubs").display(), git_dir.display()),
        )
        .env("LPOCO_LAB", l.dir.join("lab.toml"))
        .env("LPOCO_ROOT", l.dir.join("stubs/root"))
        .env("LPOCO_FS_ROOT", l.dir.join("fs"))
        .output()
        .unwrap();
    let s = out(&o);
    assert!(
        o.status.success(),
        "{s}{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(s.contains("70-lpoco-google-walleye.rules"), "{s}");
    assert!(s.contains("origin/main") && s.contains("1 behind"), "{s}");
}

/// A recorded `git -C DIR [-c K=V]... SUB ...` whose SUB only reads: the subcommand itself, not a word anywhere in the line.
fn is_read_only_git(call: &str) -> bool {
    let mut t = call.split_whitespace();
    if !t.next().is_some_and(|p| p.ends_with("/git")) || t.next() != Some("-C") {
        return false;
    }
    t.next(); // the directory
    let mut t = t.peekable();
    while t.peek() == Some(&"-c") {
        t.nth(1);
    }
    t.next()
        .is_some_and(|s| ["rev-parse", "fetch", "show", "rev-list", "ls-tree", "log"].contains(&s))
}

#[test]
fn reads_lab_toml_from_origin_main_and_ignores_a_working_file_that_differs() {
    let l = lab("lab-origin-main", "");
    let origin = l.dir.join("lab-origin.git");
    let work = l.dir.join("lab-work");
    let clone = l.dir.join("lab-clone");
    for d in [&origin, &work] {
        fs::create_dir_all(d).unwrap();
    }
    let devs_glob = format!("devices = \"{}/devs/*/device.toml\"\n", l.dir.display());
    git(&origin, &["init", "-q", "--bare", "-b", "main"]);
    git(&work, &["init", "-q", "-b", "main"]);
    fs::write(
        work.join("lab.toml"),
        format!("{devs_glob}[tools]\nlpoco = \"0.4.0\"\n"),
    )
    .unwrap();
    git(&work, &["add", "."]);
    git(&work, &["commit", "-q", "-m", "pin 0.4.0"]);
    git(
        &work,
        &["remote", "add", "origin", origin.to_str().unwrap()],
    );
    git(&work, &["push", "-q", "origin", "main"]);
    git(
        &l.dir,
        &["clone", "-q", origin.to_str().unwrap(), "lab-clone"],
    );
    fs::write(
        work.join("lab.toml"),
        format!("{devs_glob}[tools]\nlpoco = \"0.4.1\"\n"),
    )
    .unwrap();
    git(&work, &["commit", "-q", "-am", "pin 0.4.1"]);
    git(&work, &["push", "-q", "origin", "main"]);
    let real_git = String::from_utf8(
        Command::new("sh")
            .args(["-c", "command -v git"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let git_dir = Path::new(real_git.trim()).parent().unwrap().to_path_buf();
    fs::remove_file(l.dir.join("stubs/git")).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_lpoco-host"))
        .arg("plan")
        .env_clear()
        .env("HOME", l.dir.join("home"))
        .env(
            "PATH",
            format!("{}:{}", l.dir.join("stubs").display(), git_dir.display()),
        )
        .env("LPOCO_LAB", clone.join("lab.toml"))
        .env("LPOCO_ROOT", l.dir.join("stubs/root"))
        .env("LPOCO_FS_ROOT", l.dir.join("fs"))
        .output()
        .unwrap();
    let s = out(&o);
    assert!(
        o.status.success(),
        "{s}{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(
        s.contains("--tag lpoco-v0.4.1 "),
        "pins are origin/main's, not the working file's: {s}"
    );
    assert!(!s.contains("lpoco-v0.4.0"), "{s}");
    let read = s
        .lines()
        .find(|x| x.starts_with("read:") && x.contains("lab.toml"))
        .unwrap_or("");
    assert!(
        read.contains("from origin/main")
            && read.contains("1 behind")
            && read.contains("differs and is ignored"),
        "the plan names the rev it read: {s}"
    );
}

fn host_as(l: &Lab, user: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lpoco-host"))
        .args(args)
        .env_clear()
        .env("HOME", l.dir.join("home"))
        .env("USER", user)
        .env("PATH", l.dir.join("stubs"))
        .env("LPOCO_LAB", l.dir.join("lab.toml"))
        .env("LPOCO_ROOT", l.dir.join("stubs/root"))
        .env("LPOCO_FS_ROOT", l.dir.join("fs"))
        .output()
        .unwrap()
}

#[test]
fn the_gadget_group_from_lab_toml_goes_into_each_rule_line_beside_uaccess() {
    let l = lab("group-rule", "gadget_group = \"users\"\n");
    let s = out(&host(&l, &["plan"]));
    for kind in ["block", "tty"] {
        assert!(
            s.contains(&format!(
                "ACTION!=\"remove\", SUBSYSTEM==\"{kind}\", ATTRS{{serial}}==\"walleye-first-boot\", TAG+=\"uaccess\", GROUP=\"users\", MODE=\"0660\""
            )),
            "{s}"
        );
    }
}

#[test]
fn check_says_which_group_and_that_the_lab_user_is_in_it() {
    let l = lab("group-check", "gadget_group = \"users\"\n");
    fs::create_dir_all(l.dir.join("fs/etc")).unwrap();
    fs::write(
        l.dir.join("fs/etc/group"),
        "uucp:x:14:\nusers:x:982:alice,mll\n",
    )
    .unwrap();
    let s = out(&host_as(&l, "mll", &["check"]));
    assert!(s.contains("gadget group users: ok, mll is in it"), "{s}");
    let o = host_as(&l, "bob", &["check"]);
    let s = out(&o);
    assert!(
        s.contains("gadget group users: FAIL, bob is not in it"),
        "{s}"
    );
    assert!(!o.status.success(), "{s}");
    fs::write(l.dir.join("fs/etc/group"), "uucp:x:14:\n").unwrap();
    let s = out(&host_as(&l, "mll", &["check"]));
    assert!(s.contains("gadget group users: FAIL, no such group"), "{s}");
}

#[test]
fn a_gadget_group_that_would_inject_into_a_rule_is_refused() {
    let l = lab("group-inject", "gadget_group = \"users\\\", RUN+=\\\"x\"\n");
    let o = host(&l, &["plan"]);
    assert!(!o.status.success());
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("gadget_group"),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn check_counts_the_primary_group_and_refuses_an_unset_user() {
    let l = lab("group-primary", "gadget_group = \"users\"\n");
    fs::create_dir_all(l.dir.join("fs/etc")).unwrap();
    fs::write(l.dir.join("fs/etc/group"), "users:x:982:\n").unwrap();
    fs::write(
        l.dir.join("fs/etc/passwd"),
        "carol:x:1001:982::/home/carol:/bin/sh\n",
    )
    .unwrap();
    let s = out(&host_as(&l, "carol", &["check"]));
    assert!(s.contains("gadget group users: ok, carol is in it"), "{s}");
    let o = host_as(&l, "", &["check"]);
    let s = out(&o);
    assert!(s.contains("FAIL, USER is not set"), "{s}");
    assert!(!o.status.success());
}

const BANNER_LOG: &str = "\
Nov 18 10:06:54 lab kernel: usb 1-2: new high-speed USB device number 7 using xhci_hcd
Nov 18 10:06:55 lab kernel: usb 1-2: New USB device found, idVendor=1d6b, idProduct=0104, bcdDevice= 4.19
Nov 18 10:06:55 lab kernel: usb 1-2: New USB device strings: Mfr=1, Product=2, SerialNumber=3
Nov 18 10:06:55 lab kernel: usb 1-2: SerialNumber: walleye-banner
Nov 18 10:07:10 lab kernel: usb 1-3: New USB device found, idVendor=046d, idProduct=c077, bcdDevice= 1.00
Nov 18 10:07:10 lab kernel: usb 1-3: SerialNumber: some-mouse
";

#[test]
fn check_names_a_gadget_that_enumerated_with_a_serial_no_device_declares() {
    let l = lab("undeclared-serial", "");
    fs::create_dir_all(l.dir.join("fs")).unwrap();
    fs::write(l.dir.join("fs/kernel.log"), BANNER_LOG).unwrap();
    let o = host_as(&l, "mll", &["check"]);
    let s = out(&o);
    assert!(
        s.contains("usb 1-2")
            && s.contains("walleye-banner")
            && s.contains("walleye-first-boot")
            && s.contains("Nov 18 10:06:55")
            && s.contains("undeclared gadget serial"),
        "{s}"
    );
    assert!(
        !s.contains("some-mouse"),
        "only 1d6b gadgets are judged: {s}"
    );
    assert!(!o.status.success(), "{s}");
}

#[test]
fn check_does_not_report_a_gadget_whose_serial_a_device_declares() {
    let l = lab("declared-serial", "");
    fs::create_dir_all(l.dir.join("fs")).unwrap();
    fs::write(
        l.dir.join("fs/kernel.log"),
        BANNER_LOG.replace("walleye-banner", "walleye-first-boot"),
    )
    .unwrap();
    let s = out(&host_as(&l, "mll", &["check"]));
    assert!(!s.contains("undeclared gadget serial"), "{s}");
    assert!(s.contains("kernel log: ok"), "{s}");
}

/// Watcher units (lpoco-host part 2): one `devlab-watch@` user unit per device found, no root, replacing
/// lab-machine's setup/devlab-watch.sh.
#[test]
fn plan_lists_a_watcher_unit_per_device_and_changes_nothing() {
    let l = lab("watch-plan", "");
    let o = host(&l, &["plan"]);
    let s = out(&o);
    assert!(
        o.status.success(),
        "{s}{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(s.contains("watchers"), "{s}");
    assert!(s.contains("devlab-watch@.service"), "{s}");
    assert!(
        s.contains("systemctl --user enable --now devlab-watch@google-walleye-lab.service"),
        "{s}"
    );
    assert!(
        s.contains("setup/devlab-watch.sh"),
        "names what it replaces: {s}"
    );
    assert!(changes(&l).is_none(), "the plan ran a stub");
    assert!(
        !l.dir.join("home/.config").exists(),
        "the plan wrote the unit"
    );
}

#[test]
fn prepare_now_installs_the_unit_and_enables_each_watcher_without_root() {
    let l = lab("watch-now", "");
    let o = host(&l, &["prepare", "now"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let unit = l
        .dir
        .join("home/.config/systemd/user/devlab-watch@.service");
    let text = fs::read_to_string(&unit).unwrap();
    assert!(
        text.contains("devlab --device %h/Projects/linux-phone-oco/%i/device.toml watch"),
        "{text}"
    );
    let calls = changes(&l).unwrap();
    for want in [
        "systemctl --user daemon-reload",
        "systemctl --user enable --now devlab-watch@google-walleye-lab.service",
    ] {
        assert!(calls.contains(want), "{want}: {calls}");
    }
    assert!(
        !calls
            .lines()
            .any(|c| c.contains("/root") && c.contains("systemctl")),
        "a user unit needs no root: {calls}"
    );
    // already enabled and as declared: nothing more to do
    let wants = l.dir.join("home/.config/systemd/user/default.target.wants");
    fs::create_dir_all(&wants).unwrap();
    fs::write(wants.join("devlab-watch@google-walleye-lab.service"), "").unwrap();
    fs::remove_file(&l.calls).unwrap();
    let p = out(&host(&l, &["plan"]));
    assert!(!p.contains("systemctl --user"), "{p}");
}

#[test]
fn a_replaced_binary_restarts_the_enabled_watchers() {
    let l = lab("watch-restart", "[tools]\nlpoco = \"0.2.0\"\n");
    let user = l.dir.join("home/.config/systemd/user");
    fs::create_dir_all(user.join("default.target.wants")).unwrap();
    fs::write(user.join("devlab-watch@.service"), "").unwrap();
    fs::write(
        user.join("default.target.wants/devlab-watch@google-walleye-lab.service"),
        "",
    )
    .unwrap();
    let s = out(&host(&l, &["plan"]));
    assert!(
        s.contains("systemctl --user restart devlab-watch@google-walleye-lab.service"),
        "{s}"
    );
}

#[test]
fn worktrees_and_templates_are_not_watched() {
    let l = lab("watch-skip", "");
    for n in ["google-walleye-lab-pr-wt", "device-template"] {
        let d = l.dir.join("devs").join(n);
        fs::create_dir_all(&d).unwrap();
        fs::write(
            d.join("device.toml"),
            format!("[device]\ncodename = \"{n}\"\n"),
        )
        .unwrap();
    }
    let s = out(&host(&l, &["plan"]));
    assert!(s.contains("devlab-watch@google-walleye-lab.service"), "{s}");
    for n in ["google-walleye-lab-pr-wt", "device-template"] {
        assert!(!s.contains(&format!("devlab-watch@{n}")), "{n}: {s}");
    }
}

/// Watching is the device's own choice (`[host] watch = true`), like its gadgets: a device that does not ask gets no watcher.
fn declare(l: &Lab, folder: &str, host_table: &str) {
    let d = l.dir.join("devs").join(folder);
    fs::create_dir_all(&d).unwrap();
    fs::write(
        d.join("device.toml"),
        format!("[device]\ncodename = \"{folder}\"\n{host_table}"),
    )
    .unwrap();
}

#[test]
fn only_devices_that_ask_are_watched() {
    let l = lab("watch-asks", "");
    declare(&l, "asks-lab", "[host]\nwatch = true\n");
    declare(&l, "silent-lab", "[host]\nmodules = [\"cdc_acm\"]\n");
    declare(&l, "no-host-lab", "");
    declare(&l, "refuses-lab", "[host]\nwatch = false\n");
    let s = out(&host(&l, &["plan"]));
    for n in ["google-walleye-lab", "asks-lab"] {
        assert!(
            s.contains(&format!(
                "systemctl --user enable --now devlab-watch@{n}.service"
            )),
            "{n}: {s}"
        );
    }
    for n in ["silent-lab", "no-host-lab", "refuses-lab"] {
        assert!(!s.contains(&format!("devlab-watch@{n}")), "{n}: {s}");
    }
}

#[test]
fn an_enabled_watcher_of_a_device_that_does_not_ask_is_named_never_stopped() {
    let l = lab("watch-unasked", "");
    declare(&l, "quiet-lab", "[host]\nmodules = [\"cdc_acm\"]\n");
    let wants = l.dir.join("home/.config/systemd/user/default.target.wants");
    fs::create_dir_all(&wants).unwrap();
    fs::write(wants.join("devlab-watch@quiet-lab.service"), "").unwrap();
    let plan = out(&host(&l, &["plan"]));
    assert!(
        plan.contains("devlab-watch@quiet-lab.service") && plan.contains("no device that asks"),
        "named in the plan: {plan}"
    );
    let o = host(&l, &["prepare", "now"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let calls = changes(&l).unwrap_or_default();
    for verb in ["stop", "disable", "mask"] {
        assert!(
            !calls.contains(&format!("systemctl --user {verb}")),
            "{verb}: {calls}"
        );
    }
    assert!(wants.join("devlab-watch@quiet-lab.service").exists());
}

#[test]
fn a_foreign_rule_on_our_serial_or_wide_is_named_and_never_touched() {
    let l = lab("foreign", "[tools]\nlpoco = \"0.2.0\"\n");
    let rules = l.dir.join("fs/etc/udev/rules.d");
    fs::create_dir_all(&rules).unwrap();
    fs::write(
        rules.join("99-mine.rules"),
        "SUBSYSTEM==\"tty\", ATTRS{serial}==\"walleye-first-boot\", MODE=\"0600\"\n",
    )
    .unwrap();
    fs::write(
        rules.join("60-wide.rules"),
        "SUBSYSTEM==\"usb\", MODE=\"0666\"\n",
    )
    .unwrap();
    fs::write(rules.join("51-android.rules"), "MODE=\"0666\"\n").unwrap();
    fs::write(
        rules.join("70-lpoco-google-walleye.rules"),
        "ATTRS{serial}==\"walleye-first-boot\"\n",
    )
    .unwrap();
    fs::write(
        rules.join("55-comment.rules"),
        "# MODE=\"0666\" walleye-first-boot\nATTR{idProduct}==\"0666\"\n",
    )
    .unwrap();
    fs::write(rules.join("50-quiet.rules"), "# nothing of ours\n").unwrap();
    let s = out(&host(&l, &["plan"]));
    assert!(
        s.lines().any(|x| x.contains("foreign")
            && x.contains("99-mine.rules")
            && x.contains("walleye-first-boot")),
        "{s}"
    );
    assert!(
        s.lines()
            .any(|x| x.contains("foreign") && x.contains("60-wide.rules") && x.contains("0666")),
        "{s}"
    );
    assert!(!s.contains("50-quiet.rules"), "{s}");
    assert!(!s.contains("51-android.rules is not ours"), "{s}");
    assert!(
        !s.contains("70-lpoco-google-walleye.rules is not ours"),
        "{s}"
    );
    assert!(!s.contains("55-comment.rules"), "{s}");
    assert!(changes(&l).is_none());
}

/// Host commands (lpoco-host part 4): a device names the programs it needs on the host's PATH; the plan and check
/// say which are found and where, a missing one fails check, and nothing is ever installed for it.
#[test]
fn a_missing_host_command_is_named_by_plan_and_fails_check_and_nothing_is_installed() {
    let l = lab("commands", "");
    declare(
        &l,
        "cmd-lab",
        "[host]\ncommands = [\"git\", \"no-such-command\"]\n",
    );
    let s = out(&host(&l, &["plan"]));
    assert!(
        s.lines()
            .any(|x| x.contains("command git") && x.contains("stubs/git")),
        "{s}"
    );
    assert!(
        s.lines()
            .any(|x| x.contains("command no-such-command") && x.contains("missing")),
        "{s}"
    );
    assert!(changes(&l).is_none(), "the plan changed something");
    let o = host_as(&l, "mll", &["check"]);
    let c = out(&o);
    assert!(
        c.lines()
            .any(|x| x.contains("no-such-command") && x.contains("FAIL")),
        "{c}"
    );
    assert!(
        !c.lines()
            .any(|x| x.contains("command git") && x.contains("FAIL")),
        "{c}"
    );
    assert!(!o.status.success(), "{c}");
    // a command that would inject into a line is refused when the device is read
    declare(&l, "bad-lab", "[host]\ncommands = [\"a b;rm\"]\n");
    let bad = host(&l, &["plan"]);
    assert!(!bad.status.success());
    assert!(
        String::from_utf8_lossy(&bad.stderr).contains("command \"a b;rm\""),
        "{}",
        String::from_utf8_lossy(&bad.stderr)
    );
}

#[test]
fn check_passes_with_only_found_commands_and_a_second_device_is_named_too() {
    let l = lab("commands-ok", "");
    declare(&l, "one-lab", "[host]\ncommands = [\"git\"]\n");
    declare(&l, "two-lab", "[host]\ncommands = [\"git\"]\n");
    let o = host_as(&l, "mll", &["check"]);
    let c = out(&o);
    let line = c.lines().find(|x| x.contains("command git")).expect(&c);
    assert!(
        line.contains("ok") && line.contains("one-lab") && line.contains("two-lab"),
        "{c}"
    );
    assert_eq!(c.matches("command git").count(), 1, "{c}");
    assert!(o.status.success(), "{c}");
    // a file on PATH that is not executable is not a command
    fs::write(l.dir.join("stubs/plain"), "x").unwrap();
    declare(&l, "three-lab", "[host]\ncommands = [\"plain\"]\n");
    let c = out(&host_as(&l, "mll", &["check"]));
    assert!(
        c.lines()
            .any(|x| x.contains("command plain") && x.contains("FAIL")),
        "{c}"
    );
}

/// Cross targets (lpoco-host part 3): a device names the Rust targets its tools are built for; the plan and check
/// say which have a standard library on the host (the PATH rustc's, or the lab's own devlab-cross toolchain), a
/// missing one fails check, and nothing is installed for it.
#[test]
fn a_missing_cross_target_is_named_by_plan_and_fails_check_and_nothing_is_installed() {
    let l = lab("targets", "");
    declare(
        &l,
        "tgt-lab",
        "[host]\ntargets = [\"have-musl\", \"gone-musl\", \"kept-musl\"]\n",
    );
    let libs = l.dir.join("libs");
    fs::create_dir_all(libs.join("have-musl")).unwrap();
    fs::write(libs.join("have-musl/libstd-abc.rlib"), "").unwrap();
    fs::create_dir_all(libs.join("none")).unwrap();
    exe(
        &l.dir.join("stubs/rustc"),
        &format!(
            "case \"$*\" in *have-musl*) echo {}/have-musl;; *) echo {}/none;; esac",
            libs.display(),
            libs.display()
        ),
    );
    // the lab's own toolchain beside the system's holds kept-musl
    fs::create_dir_all(
        l.dir.join(
            "home/.local/share/devlab-cross/rustup/toolchains/stable/lib/rustlib/kept-musl/lib",
        ),
    )
    .unwrap();
    fs::write(
        l.dir.join("home/.local/share/devlab-cross/rustup/toolchains/stable/lib/rustlib/kept-musl/lib/libstd-x.rlib"),
        "",
    )
    .unwrap();
    let s = out(&host(&l, &["plan"]));
    let line = |t: &str| {
        s.lines()
            .find(|x| x.contains(&format!("target {t}")))
            .map(str::to_string)
    };
    assert!(line("have-musl").is_some_and(|x| x.contains("ok")), "{s}");
    assert!(line("kept-musl").is_some_and(|x| x.contains("ok")), "{s}");
    let gone = line("gone-musl").expect(&s);
    assert!(gone.contains("missing") && gone.contains("tgt-lab"), "{s}");
    assert!(changes(&l).is_none(), "the plan changed something");
    let o = host_as(&l, "mll", &["check"]);
    let c = out(&o);
    assert!(
        c.lines()
            .any(|x| x.contains("target gone-musl") && x.contains("FAIL")),
        "{c}"
    );
    assert!(
        !c.lines()
            .any(|x| x.contains("target have-musl") && x.contains("FAIL")),
        "{c}"
    );
    assert!(!o.status.success(), "{c}");
    // a target that would inject into a line is refused when the device is read
    declare(&l, "bad-lab", "[host]\ntargets = [\"a b;rm\"]\n");
    let bad = host(&l, &["plan"]);
    assert!(!bad.status.success());
    assert!(
        String::from_utf8_lossy(&bad.stderr).contains("target \"a b;rm\""),
        "{}",
        String::from_utf8_lossy(&bad.stderr)
    );
}

#[test]
fn a_target_is_found_once_for_two_devices_and_dot_or_an_empty_libdir_or_a_broken_rustc_is_not_a_library(
) {
    let l = lab("targets-more", "");
    declare(
        &l,
        "one-lab",
        "[host]\ntargets = [\"t-musl\", \"t-musl\", \".\"]\n",
    );
    declare(&l, "two-lab", "[host]\ntargets = [\"t-musl\"]\n");
    // a libdir that exists but holds no standard library, and a rustc that cannot run
    fs::create_dir_all(l.dir.join("libs/t-musl")).unwrap();
    exe(
        &l.dir.join("stubs/rustc"),
        &format!("echo {}/libs/t-musl", l.dir.display()),
    );
    let s = out(&host(&l, &["plan"]));
    assert_eq!(s.matches("target t-musl").count(), 1, "{s}");
    let t = s.lines().find(|x| x.contains("target t-musl")).unwrap();
    assert!(
        t.contains("missing") && t.contains("one-lab") && t.contains("two-lab"),
        "{s}"
    );
    assert!(
        s.lines()
            .any(|x| x.contains("target .") && x.contains("missing")),
        "{s}"
    );
    exe(&l.dir.join("stubs/rustc"), "exit 1");
    let o = host_as(&l, "mll", &["check"]);
    assert!(!o.status.success());
    assert!(changes(&l).is_none(), "check changed something");
}

#[test]
fn a_gadgets_usb_network_gets_a_stable_name_by_its_serial_and_nothing_changes_without_the_verb() {
    let l = lab("ncm", "");
    declare(
        &l,
        "google-walleye-lab",
        "[host]\n[[host.gadget]]\nserial = \"walleye-first-boot\"\naccess = [\"tty\"]\nnet_name = \"lpoco-walleye\"\n",
    );
    // a second gadget that does not name its network gets no net line
    declare(
        &l,
        "xiaomi-surya-lab",
        "[host]\n[[host.gadget]]\nserial = \"surya-gadget\"\naccess = [\"tty\"]\n",
    );
    let o = host(&l, &["plan"]);
    let s = out(&o);
    assert!(
        o.status.success(),
        "{s}{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(
        s.contains("ACTION==\"add\", SUBSYSTEM==\"net\", ATTRS{serial}==\"walleye-first-boot\", NAME=\"lpoco-walleye\""),
        "{s}"
    );
    assert_eq!(s.matches("SUBSYSTEM==\"net\"").count(), 1, "{s}");
    assert!(changes(&l).is_none(), "the plan called the root prefix");
    // a name that would inject into a rule is refused
    declare(
        &l,
        "google-walleye-lab",
        "[host]\n[[host.gadget]]\nserial = \"w\"\naccess = [\"tty\"]\nnet_name = \"x\\\", RUN+=\\\"y\"\n",
    );
    let o = host(&l, &["plan"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("net_name"));
    declare(
        &l,
        "google-walleye-lab",
        "[host]\n[[host.gadget]]\nserial = \"w\"\naccess = [\"tty\"]\nnet_name = \".\"\n",
    );
    assert_eq!(host(&l, &["plan"]).status.code(), Some(2));
}

fn net_gadget(l: &Lab, extra: &str) {
    declare(
        l,
        "google-walleye-lab",
        &format!(
            "[host]\n[[host.gadget]]\nserial = \"walleye-first-boot\"\naccess = [\"tty\"]\nnet_name = \"lpoco-walleye\"\n{extra}"
        ),
    );
}

#[test]
fn a_named_usb_network_gets_the_hosts_link_config_in_the_format_of_the_manager_it_runs_and_only_with_the_verb(
) {
    // NetworkManager runs: a keyfile NM will read (mode 0600), reloaded with nmcli
    let l = lab("link-nm", "");
    net_gadget(&l, "net_address = \"10.55.0.1/24\"\n");
    fs::create_dir_all(l.dir.join("fs/run/NetworkManager")).unwrap();
    let s = out(&host(&l, &["plan"]));
    assert!(
        s.contains("/etc/NetworkManager/system-connections/lpoco-walleye.nmconnection"),
        "{s}"
    );
    assert!(s.contains("interface-name=lpoco-walleye"), "{s}");
    assert!(s.contains("address1=10.55.0.1/24"), "{s}");
    assert!(s.contains("install -D -m 0600"), "{s}");
    assert!(s.contains("nmcli connection reload"), "{s}");
    assert!(!s.contains("/etc/systemd/network"), "{s}");
    assert!(changes(&l).is_none(), "the plan called the root prefix");

    // only networkd runs: a .network file matching the name
    let l = lab("link-networkd", "");
    net_gadget(&l, "net_address = \"10.55.0.1/24\"\n");
    fs::create_dir_all(l.dir.join("fs/run/systemd/netif")).unwrap();
    let s = out(&host(&l, &["plan"]));
    assert!(
        s.contains("/etc/systemd/network/70-lpoco-walleye.network"),
        "{s}"
    );
    assert!(s.contains("Name=lpoco-walleye"), "{s}");
    assert!(s.contains("Address=10.55.0.1/24"), "{s}");
    assert!(s.contains("networkctl reload"), "{s}");
    assert!(!s.contains("NetworkManager/system-connections"), "{s}");
    assert!(changes(&l).is_none());

    // neither runs: named, no file planned, not an error
    let l = lab("link-none", "");
    net_gadget(&l, "net_address = \"10.55.0.1/24\"\n");
    let o = host(&l, &["plan"]);
    let s = out(&o);
    assert!(o.status.success(), "{s}");
    assert!(s.contains("no NetworkManager or systemd-networkd"), "{s}");
    assert!(
        !s.contains(".nmconnection") && !s.contains(".network"),
        "{s}"
    );

    // no address declared: the stable name only, no link file
    let l = lab("link-noaddr", "");
    net_gadget(&l, "");
    fs::create_dir_all(l.dir.join("fs/run/NetworkManager")).unwrap();
    let s = out(&host(&l, &["plan"]));
    assert!(!s.contains(".nmconnection"), "{s}");

    // prepare now stages the keyfile with its own mode
    let l = lab("link-prepare", "");
    net_gadget(&l, "net_address = \"10.55.0.1/24\"\n");
    fs::create_dir_all(l.dir.join("fs/run/NetworkManager")).unwrap();
    let o = host(&l, &["prepare", "now"]);
    assert!(
        o.status.success(),
        "{}{}",
        out(&o),
        String::from_utf8_lossy(&o.stderr)
    );
    let c = changes(&l).unwrap();
    assert!(
        c.contains("install -D -m 0600") && c.contains("lpoco-walleye.nmconnection"),
        "{c}"
    );
    assert!(c.contains("nmcli connection reload"), "{c}");

    // an address that is not an IPv4 CIDR, or one without a net_name, is refused
    for bad in [
        "net_address = \"10.55.0.1\"\n",
        "net_address = \"x/24\"\n",
        "net_address = \"10.55.0.1/33\"\n",
        "net_address = \"10.55.0.1/24\\nexec=y\"\n",
    ] {
        let l = lab("link-bad", "");
        net_gadget(&l, bad);
        let o = host(&l, &["plan"]);
        assert_eq!(o.status.code(), Some(2), "{bad}");
        assert!(
            String::from_utf8_lossy(&o.stderr).contains("net_address"),
            "{bad}"
        );
    }
    let l = lab("link-noname", "");
    declare(&l, "google-walleye-lab", "[host]\n[[host.gadget]]\nserial = \"w\"\naccess = [\"tty\"]\nnet_address = \"10.55.0.1/24\"\n");
    assert_eq!(host(&l, &["plan"]).status.code(), Some(2));
}

#[test]
fn one_net_name_declared_twice_by_a_device_is_refused() {
    let l = lab("link-twice", "");
    declare(&l, "google-walleye-lab", "[host]\n[[host.gadget]]\nserial = \"a\"\naccess = [\"tty\"]\nnet_name = \"lpoco-x\"\n[[host.gadget]]\nserial = \"b\"\naccess = [\"tty\"]\nnet_name = \"lpoco-x\"\n");
    let o = host(&l, &["plan"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("declared twice"));
}

#[test]
fn an_enabled_but_stopped_watcher_is_started_and_a_running_one_is_left() {
    let l = lab("watch-stopped", "");
    let user = l.dir.join("home/.config/systemd/user");
    fs::create_dir_all(user.join("default.target.wants")).unwrap();
    // as declared: the template is what prepare would write, the instance has its wants link
    fs::write(
        user.join("devlab-watch@.service"),
        lpoco_host::watch_unit_text(),
    )
    .unwrap();
    fs::write(
        user.join("default.target.wants/devlab-watch@google-walleye-lab.service"),
        "",
    )
    .unwrap();
    // systemctl says the instance is not active (is-active exits 3); other calls are recorded
    exe(
        &l.dir.join("stubs/systemctl"),
        &format!(
            "case \"$*\" in *is-active*) exit 3;; esac; echo \"$0 $@\" >> {}",
            l.calls.display()
        ),
    );
    let s = out(&host(&l, &["plan"]));
    assert!(
        s.contains("systemctl --user start devlab-watch@google-walleye-lab.service"),
        "{s}"
    );
    assert!(
        changes(&l).is_none(),
        "plan changed something: {:?}",
        changes(&l)
    );
    // running: nothing to do
    exe(&l.dir.join("stubs/systemctl"), "exit 0");
    let s = out(&host(&l, &["plan"]));
    assert!(!s.contains("systemctl --user"), "{s}");
}

#[test]
fn a_script_running_while_prepare_replaces_it_finishes_with_its_old_text() {
    let l = lab("atomic", SCRIPT_TOOLS);
    exe(
        &l.dir.join("stubs/git"),
        &format!(
            "echo \"$0 $@\" >> {}\ncase \"$1\" in clone) /bin/mkdir -p \"$4/bin\"; printf '#!/bin/sh\\necho new\\n' > \"$4/bin/land\";; esac",
            l.calls.display()
        ),
    );
    // an installed script a shell reads as it goes: it waits, then prints its last line
    let bin = l.dir.join("home/.local/bin");
    fs::create_dir_all(&bin).unwrap();
    let started = l.dir.join("started");
    let done = l.dir.join("done");
    exe(
        &bin.join("land"),
        &format!(
            "/bin/touch {s}\nwhile [ ! -e {d} ]; do /bin/sleep 0.05; done\necho old-finished",
            s = started.display(),
            d = done.display()
        ),
    );
    // bash reads a script as it runs (and not exec of a file just written: that can be busy)
    let mut running = Command::new("/bin/bash")
        .arg(bin.join("land"))
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !started.exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "the old script never started"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let _guard = Kill(done.clone());
    let o = host(&l, &["prepare", "now"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    // the new text is in place, the running one has not been written under
    assert!(fs::read_to_string(bin.join("land"))
        .unwrap()
        .contains("echo new"));
    fs::write(&done, "").unwrap();
    let mut text = String::new();
    std::io::Read::read_to_string(running.stdout.as_mut().unwrap(), &mut text).unwrap();
    running.wait().unwrap();
    assert_eq!(text.trim(), "old-finished");
    // no temp file is left beside the tool
    let left: Vec<_> = fs::read_dir(&bin)
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(left.len(), 1, "{left:?}");
}

/// Lets a waiting old script end if the test fails before it does.
struct Kill(PathBuf);
impl Drop for Kill {
    fn drop(&mut self) {
        let _ = fs::write(&self.0, "");
    }
}

/// Makes the udev rules directory of the lab's fake root unreadable, as /etc/polkit-1/rules.d and NetworkManager's
/// directory are to the lab user; gives it back when dropped so the temp folder can be removed.
struct Closed(PathBuf);

impl Closed {
    fn new(l: &Lab) -> Closed {
        let d = l.dir.join("fs/etc/udev/rules.d");
        fs::create_dir_all(&d).unwrap();
        fs::set_permissions(&d, fs::Permissions::from_mode(0o000)).unwrap();
        Closed(d)
    }
}

impl Drop for Closed {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o755));
    }
}

#[test]
fn a_root_file_the_lab_user_cannot_read_is_judged_by_the_hash_recorded_at_its_last_install() {
    // (3) no record: named unreadable and not known, never new
    let l = lab("unreadable-unknown", "");
    let _closed = Closed::new(&l);
    let s = out(&host(&l, &["plan"]));
    let rule = s
        .lines()
        .find(|x| x.contains("70-lpoco-google-walleye.rules"))
        .unwrap_or_else(|| panic!("{s}"));
    assert!(
        rule.contains("unreadable") && rule.contains("not known yet") && !rule.contains("(new)"),
        "{s}"
    );

    // (4) prepare records the hash after the root install it runs (the folder is readable at that time)
    let l = lab("unreadable-recorded", "");
    let o = host(&l, &["prepare", "now"]);
    assert!(
        o.status.success(),
        "{}{}",
        out(&o),
        String::from_utf8_lossy(&o.stderr)
    );
    let c = changes(&l).unwrap();
    assert!(c.contains("70-lpoco-google-walleye.rules"), "{c}");
    let closed = Closed::new(&l);

    // (1) the same declaration, the folder now closed: up to date, 0 root commands
    let s = out(&host(&l, &["plan"]));
    assert!(
        s.contains("70-lpoco-google-walleye.rules (as declared, unreadable: recorded at install)"),
        "{s}"
    );
    assert_eq!(
        s.matches("70-lpoco-google-walleye.rules").count(),
        1,
        "no root command names it: {s}"
    );
    assert!(!s.contains("udevadm control --reload"), "{s}");

    // (2) the declaration changes: a root install again
    let dev = l.dir.join("devs/google-walleye-lab/device.toml");
    let t = fs::read_to_string(&dev).unwrap();
    fs::write(&dev, t.replace("walleye-first-boot", "walleye-second-boot")).unwrap();
    let s = out(&host(&l, &["plan"]));
    assert!(
        s.contains("install -D -m 0644") && s.contains("70-lpoco-google-walleye.rules"),
        "{s}"
    );
    drop(closed);
}

/// The polkit rule's folder closed to the lab user, as /etc/polkit-1/rules.d is; given back on drop.
struct ClosedPolkit(PathBuf);

impl ClosedPolkit {
    fn new(l: &Lab) -> ClosedPolkit {
        let d = l.dir.join("fs/etc/polkit-1/rules.d");
        fs::create_dir_all(&d).unwrap();
        fs::set_permissions(&d, fs::Permissions::from_mode(0o000)).unwrap();
        ClosedPolkit(d)
    }
}

impl Drop for ClosedPolkit {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o755));
    }
}

/// A lab whose phone is on USB (`ip` stubs) and whose `systemctl` stub starts and stops the share unit the way
/// the real one does (state under the fake root's /run), or fails to start it.
fn lab_with_share(name: &str, start_works: bool) -> Lab {
    let l = lab(name, "");
    let dev = l.dir.join("devs/google-walleye-lab/device.toml");
    let mut t = fs::read_to_string(&dev).unwrap();
    t.push_str("[connection]\nuser = \"u\"\naddresses = [\"172.16.42.1\"]\n");
    fs::write(&dev, t).unwrap();
    let bin = l.dir.join("stubs");
    exe(
        &bin.join("ip"),
        r#"case "$*" in
"-o -4 addr") echo "7: usb0    inet 172.16.42.2/24 brd 172.16.42.255 scope global usb0" ;;
"route show default") echo "default via 10.0.0.1 dev eth0" ;;
esac"#,
    );
    let run = l.dir.join("fs/run/lpoco-share/active");
    let start = if start_works {
        {
            fs::create_dir_all(&run).unwrap();
            format!(": > {}/usb0--eth0", run.display())
        }
    } else {
        "echo refused >&2; exit 1".to_string()
    };
    exe(
        &bin.join("systemctl"),
        &format!(
            "case \"$1\" in\nstart) {start} ;;\nis-active) test -e {r}/usb0--eth0 ;;\nstop) /usr/bin/rm -f {r}/usb0--eth0 ;;\nesac",
            r = run.display()
        ),
    );
    l
}

#[test]
fn a_passing_share_check_records_the_rule_it_proved_and_a_failing_one_records_nothing() {
    // the check passes: the rule in the closed folder is recorded, so the next plan asks no root for it
    let l = lab_with_share("record-from-check", true);
    let closed = ClosedPolkit::new(&l);
    let before = out(&host_as(&l, "lab", &["plan"]));
    assert!(
        before.contains("50-lpoco.rules (unreadable, not known yet"),
        "{before}"
    );
    let c = host_as(&l, "lab", &["check"]);
    let cs = out(&c);
    assert!(cs.contains("started and stopped by the lab user"), "{cs}");
    let after = out(&host_as(&l, "lab", &["plan"]));
    assert!(
        after.contains("50-lpoco.rules (as declared, unreadable: recorded at install)"),
        "{after}"
    );
    assert!(before.contains("stage/50-lpoco.rules"), "{before}");
    assert!(
        !after.contains("stage/50-lpoco.rules"),
        "no root command installs it: {after}"
    );
    drop(closed);

    // the check fails: nothing is recorded, the rule stays not known
    let l = lab_with_share("record-not-from-failed-check", false);
    let _closed = ClosedPolkit::new(&l);
    let c = out(&host_as(&l, "lab", &["check"]));
    assert!(c.contains("share: FAIL"), "{c}");
    let after = out(&host_as(&l, "lab", &["plan"]));
    assert!(
        after.contains("50-lpoco.rules (unreadable, not known yet"),
        "{after}"
    );
}

#[test]
fn a_passing_share_check_does_not_vouch_for_a_rule_recorded_with_other_content() {
    let l = lab_with_share("record-stale", true);
    let state = l.dir.join("home/.local/state/lpoco-host");
    fs::create_dir_all(&state).unwrap();
    fs::write(
        state.join("installed"),
        "0000000000000000000000000000000000000000 /etc/polkit-1/rules.d/50-lpoco.rules\n",
    )
    .unwrap();
    let _closed = ClosedPolkit::new(&l);
    let c = out(&host_as(&l, "lab", &["check"]));
    assert!(c.contains("started and stopped by the lab user"), "{c}");
    let after = out(&host_as(&l, "lab", &["plan"]));
    assert!(
        after.contains("stage/50-lpoco.rules"),
        "still installed: {after}"
    );
}

#[test]
fn a_misspelled_table_in_a_device_manifest_is_refused_by_host_too() {
    let e = lpoco_host::parse_device(
        std::path::Path::new("device.toml"),
        "[device]\ncodename = \"p\"\n[[flash.overlays]]\nfrom = \"a\"\nto = \"b\"\n",
    )
    .unwrap_err();
    assert!(e.contains("overlays"), "{e}");
}

fn follow_stamp(l: &Lab, age_seconds: u64) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let dir = l.dir.join("home/.cache/lpoco-host/follow");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("ok"), format!("{}\n", now - age_seconds)).unwrap();
}

#[test]
fn check_fails_when_follows_last_success_is_older_than_three_rounds_of_its_timer() {
    let l = lab("follow-stale", "[follow]\nevery = \"15min\"\n");
    follow_stamp(&l, 3 * 900 + 60);
    let o = host_as(&l, "mll", &["check"]);
    let s = out(&o);
    assert!(s.contains("follow: FAIL, its last success was"), "{s}");
    assert!(!o.status.success(), "{s}");
    follow_stamp(&l, 2 * 900);
    let s = out(&host_as(&l, "mll", &["check"]));
    assert!(s.contains("follow: ok, its last success was"), "{s}");
}

#[test]
fn check_says_follow_is_not_measured_before_its_first_success_and_says_nothing_without_a_follow() {
    let l = lab("follow-unmeasured", "[follow]\nevery = \"15min\"\n");
    let s = out(&host_as(&l, "mll", &["check"]));
    assert!(
        s.contains("follow: not measured, no follow has finished here yet"),
        "{s}"
    );
    let l = lab("follow-none", "");
    let s = out(&host_as(&l, "mll", &["check"]));
    assert!(!s.contains("follow:"), "{s}");
}

/// An enabled watcher instance of walleye's lab, the template as prepare would write it.
fn enabled_watcher(l: &Lab) {
    let user = l.dir.join("home/.config/systemd/user");
    fs::create_dir_all(user.join("default.target.wants")).unwrap();
    fs::write(
        user.join("devlab-watch@.service"),
        lpoco_host::watch_unit_text(),
    )
    .unwrap();
    fs::write(
        user.join("default.target.wants/devlab-watch@google-walleye-lab.service"),
        "",
    )
    .unwrap();
}

#[test]
fn a_watcher_whose_state_cannot_be_read_is_said_not_measured_not_taken_for_running() {
    // source: `systemctl --user is-active` exits 3 for an inactive unit (systemctl(1)); any other failure - no user
    // bus, no systemctl - tells nothing (practice 12: not known yet, never silently fine)
    let l = lab("watch-unknown", "");
    enabled_watcher(&l);
    exe(
        &l.dir.join("stubs/systemctl"),
        "case \"$*\" in *is-active*) echo 'Failed to connect to user scope bus' >&2; exit 1;; esac",
    );
    let s = out(&host(&l, &["plan"]));
    assert!(
        s.contains("devlab-watch@google-walleye-lab.service: not measured"),
        "{s}"
    );
    assert!(!s.contains("systemctl --user start"), "{s}");
    // no systemctl at all
    fs::remove_file(l.dir.join("stubs/systemctl")).unwrap();
    let s = out(&host(&l, &["plan"]));
    assert!(
        s.contains("devlab-watch@google-walleye-lab.service: not measured"),
        "{s}"
    );
}

#[test]
fn without_a_gadget_group_the_lab_users_own_primary_group_is_the_default() {
    // headless (no seat session, so no uaccess ACL): the node still reaches the lab user. Source of the lookup:
    // /etc/passwd's fourth field is the primary gid, /etc/group names it (passwd(5), group(5)).
    let l = lab("group-default", "");
    fs::create_dir_all(l.dir.join("fs/etc")).unwrap();
    fs::write(l.dir.join("fs/etc/group"), "users:x:982:\nlab:x:1001:\n").unwrap();
    fs::write(
        l.dir.join("fs/etc/passwd"),
        "carol:x:1001:1001::/home/carol:/bin/sh\n",
    )
    .unwrap();
    let s = out(&host_as(&l, "carol", &["plan"]));
    assert!(
        s.contains("TAG+=\"uaccess\", GROUP=\"lab\", MODE=\"0660\""),
        "{s}"
    );
}

#[test]
fn a_declared_gadget_group_beats_the_primary_group_default() {
    let l = lab("group-declared", "gadget_group = \"users\"\n");
    fs::create_dir_all(l.dir.join("fs/etc")).unwrap();
    fs::write(
        l.dir.join("fs/etc/group"),
        "users:x:982:carol\nlab:x:1001:\n",
    )
    .unwrap();
    fs::write(
        l.dir.join("fs/etc/passwd"),
        "carol:x:1001:1001::/home/carol:/bin/sh\n",
    )
    .unwrap();
    let s = out(&host_as(&l, "carol", &["plan"]));
    assert!(s.contains("GROUP=\"users\", MODE=\"0660\""), "{s}");
    assert!(!s.contains("GROUP=\"lab\""), "{s}");
}

#[test]
fn a_user_whose_primary_group_is_not_known_gets_no_group_default_and_it_is_said() {
    // not measured, not guessed: no passwd line, no group
    let l = lab("group-unknown", "");
    let s = out(&host_as(&l, "nobody-here", &["plan"]));
    assert!(!s.contains("GROUP="), "{s}");
    assert!(s.contains("gadget group: none declared"), "{s}");
}

#[test]
fn targets_names_each_declared_cross_target_once_one_a_line_and_changes_nothing() {
    // what health/build.sh asks for: the targets the declarations name, not a list kept in the script
    let l = lab("targets-verb", "");
    declare(&l, "a-lab", "[host]\ntargets = [\"t-musl\", \"u-musl\"]\n");
    declare(&l, "b-lab", "[host]\ntargets = [\"t-musl\"]\n");
    let o = host(&l, &["targets"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(out(&o), "t-musl\nu-musl\n");
    assert!(changes(&l).is_none());
}
