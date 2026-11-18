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
        .is_some_and(|s| ["rev-parse", "fetch", "show", "rev-list"].contains(&s))
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
        plan.contains("devlab-watch@quiet-lab.service") && plan.contains("does not ask"),
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
