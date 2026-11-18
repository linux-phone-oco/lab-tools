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
            "devices = \"{}/devs/*/device.toml\"\n{lab_toml_tools}",
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
    exe(&bin.join("land"), "echo land 0.1.0");
    fs::create_dir_all(dir.join("home")).unwrap();
    Lab { dir, calls }
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
    assert!(!l.calls.exists(), "the plan called the root prefix");
    // the default verb is the plan
    assert_eq!(out(&host(&l, &[])), s);
    // prepare without `now` refuses and calls nothing
    let o = host(&l, &["prepare"]);
    assert!(!o.status.success());
    assert!(
        !l.calls.exists(),
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
    assert!(!l.calls.exists(), "the plan ran a stub");
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
    let calls = fs::read_to_string(&l.calls).unwrap();
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
    for line in calls.lines().filter(|c| !c.contains("modprobe")) {
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
    assert!(!l.calls.exists());
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
