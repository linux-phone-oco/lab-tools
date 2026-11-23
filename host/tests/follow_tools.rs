// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco host follow` installs a declared tool whose lab.toml pin moved (backlog host-installs-moved-tool-pins):
//! the family is current, `[tools.rhizoid]` names a newer tag than the installed binary's stamp, and follow - under
//! the leases of every phone, judged by the host's own check - installs the asset at its pin, or leaves it while a
//! keeper holds a lease, or puts the old binary back when the check fails. The release is a folder
//! (`LPOCO_GITHUB=file://DIR`), curl a stub that copies from it.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const REPO: &str = "linux-phone-oco/rhizoid";
const OLD: &str = "v0.3.6";
const NEW: &str = "v0.3.8";
const ASSET: &str = "rhizoid-x86_64-unknown-linux-gnu";

struct Lab {
    dir: PathBuf,
}

fn exe(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn which(p: &str) -> PathBuf {
    ["/usr/bin", "/bin"]
        .iter()
        .map(|d| Path::new(d).join(p))
        .find(|f| f.exists())
        .unwrap()
}

/// A lab whose family is current (lpoco 0.2.0 is pinned, installed and the newest release) and whose
/// `[tools.rhizoid]` pins `NEW` while `OLD` is installed and stamped; one phone; the host's check answers `check_exit`.
fn lab(name: &str, check_exit: i32) -> Lab {
    let dir =
        std::env::temp_dir().join(format!("lpoco-follow-tools-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let home = dir.join("home");
    let stubs = dir.join("stubs");
    let bin = home.join(".local/bin");
    fs::create_dir_all(&stubs).unwrap();
    fs::create_dir_all(&bin).unwrap();
    let phone = home.join("Projects/linux-phone-oco/surya-lab");
    fs::create_dir_all(&phone).unwrap();
    fs::write(
        phone.join("device.toml"),
        "[device]\ncodename = \"xiaomi-surya\"\n[host]\nmodules = [\"cdc_acm\"]\n",
    )
    .unwrap();
    fs::write(
        dir.join("lab.toml"),
        format!(
            "devices = \"{}/Projects/linux-phone-oco/[gs]*-lab/device.toml\"\n[tools]\nlpoco = \"0.2.0\"\n[tools.rhizoid]\nrepository = \"{REPO}\"\ntag = \"{NEW}\"\nasset = \"{ASSET}\"\n",
            home.display()
        ),
    )
    .unwrap();
    let calls = dir.join("calls");
    let rec = format!("echo \"$0 $@\" >> {}", calls.display());
    for s in ["root", "cargo", "systemctl", "land", "git"] {
        exe(&stubs.join(s), &rec);
    }
    exe(
        &stubs.join("curl"),
        &format!(
            "echo \"$0 $@\" >> {0}\nfor f in \"$HOME\"/.local/state/devlab/*/keeper.lease/holder; do [ -f \"$f\" ] && echo \"lease-held $(cat \"$f\")\" >> {0}; done\nwhile [ $# -gt 0 ]; do case \"$1\" in --output) out=\"$2\"; shift;; --*) ;; *) url=\"$1\";; esac; shift; done\nsrc=\"${{url#file://}}\"\n[ -f \"$src\" ] || {{ printf 404; exit 22; }}\nprintf 200\ncp \"$src\" \"$out\"",
            calls.display()
        ),
    );
    for p in ["cp", "date", "mkdir", "touch", "cat", "sh", "grep"] {
        std::os::unix::fs::symlink(which(p), stubs.join(p)).unwrap();
    }
    for p in [
        "lpoco",
        "lpoco-lab",
        "lpoco-bringup",
        "lpoco-health",
        "lpoco-tone",
        "devlab",
        "tone",
    ] {
        exe(&bin.join(p), &format!("echo {p} 0.2.0"));
    }
    exe(
        &bin.join("lpoco-host"),
        &format!(
            "case \"$1\" in check) exit {check_exit};; plan) printf '%s\\n' 'root commands, through `sudo`:' '    none';; esac"
        ),
    );
    exe(&bin.join("rhizoid"), "echo rhizoid old");
    fs::create_dir_all(home.join(".local/share/lpoco-host")).unwrap();
    fs::write(
        home.join(".local/share/lpoco-host/rev-rhizoid"),
        format!("{OLD}\n"),
    )
    .unwrap();
    fs::write(
        dir.join("releases.json"),
        "[{\"tag_name\":\"lpoco-v0.2.0\"}]",
    )
    .unwrap();
    let folder = dir
        .join("github")
        .join(REPO)
        .join("releases/download")
        .join(NEW);
    fs::create_dir_all(&folder).unwrap();
    let file = folder.join(ASSET);
    fs::write(&file, "#!/bin/sh\necho rhizoid new\n").unwrap();
    let sum = Command::new("sha256sum").arg(&file).output().unwrap();
    let hash = String::from_utf8(sum.stdout).unwrap();
    fs::write(
        folder.join(format!("{ASSET}.sha256")),
        format!("{}  {ASSET}\n", hash.split_whitespace().next().unwrap()),
    )
    .unwrap();
    Lab { dir }
}

fn follow(l: &Lab, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lpoco-host"))
        .arg("follow")
        .args(args)
        .env_clear()
        .env("HOME", l.dir.join("home"))
        .env(
            "PATH",
            format!(
                "{}:{}",
                l.dir.join("stubs").display(),
                l.dir.join("home/.local/bin").display()
            ),
        )
        .env("LPOCO_LAB", l.dir.join("lab.toml"))
        .env("LPOCO_ROOT", l.dir.join("stubs/root"))
        .env("LPOCO_FS_ROOT", l.dir.join("fs"))
        .env("LPOCO_GITHUB", format!("file://{}/github", l.dir.display()))
        .env(
            "LPOCO_RELEASE_BASE",
            format!("file://{}/release", l.dir.display()),
        )
        .env(
            "LPOCO_RELEASES",
            format!("file://{}/releases.json", l.dir.display()),
        )
        .output()
        .unwrap()
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn calls(l: &Lab) -> String {
    fs::read_to_string(l.dir.join("calls")).unwrap_or_default()
}

fn rhizoid(l: &Lab) -> String {
    fs::read_to_string(l.dir.join("home/.local/bin/rhizoid")).unwrap()
}

fn stamp(l: &Lab) -> String {
    fs::read_to_string(l.dir.join("home/.local/share/lpoco-host/rev-rhizoid"))
        .unwrap()
        .trim()
        .to_string()
}

fn lease(l: &Lab, holder: &str) {
    let d = l
        .dir
        .join("home/.local/state/devlab/xiaomi-surya/keeper.lease");
    fs::create_dir_all(&d).unwrap();
    fs::write(d.join("holder"), format!("{holder}\n")).unwrap();
}

#[test]
fn a_tool_pin_newer_than_the_installed_binary_is_installed_with_the_leases_held_and_stamped() {
    let l = lab("moved", 0);
    let o = follow(&l, &["now"]);
    let s = text(&o);
    assert!(o.status.success(), "{s}");
    assert!(rhizoid(&l).contains("rhizoid new"), "{s}");
    assert_eq!(stamp(&l), NEW);
    assert!(s.contains("followed: rhizoid") && s.contains(NEW), "{s}");
    // the asset was fetched while the phone's lease was ours, and the family was left alone
    assert!(
        calls(&l).contains("lease-held lpoco-host-follow"),
        "{}",
        calls(&l)
    );
    let lpoco = fs::read_to_string(l.dir.join("home/.local/bin/lpoco")).unwrap();
    assert!(lpoco.contains("0.2.0"), "{lpoco}");
    assert!(!l
        .dir
        .join("home/.local/state/devlab/xiaomi-surya/keeper.lease")
        .exists());
}

#[test]
fn a_held_lease_leaves_the_old_tool_and_fetches_nothing() {
    let l = lab("held", 0);
    lease(&l, "surya keeper");
    let o = follow(&l, &["now"]);
    let s = text(&o);
    assert!(o.status.success(), "{s}");
    assert!(s.contains("waits: surya keeper keeps xiaomi-surya"), "{s}");
    assert!(rhizoid(&l).contains("rhizoid old"));
    assert_eq!(stamp(&l), OLD);
    assert!(!calls(&l).contains(ASSET), "{}", calls(&l));
}

#[test]
fn without_now_it_says_what_it_would_install_and_changes_nothing() {
    let l = lab("said", 0);
    let o = follow(&l, &[]);
    let s = text(&o);
    assert!(o.status.success(), "{s}");
    assert!(
        s.contains("rhizoid") && s.contains(NEW) && s.contains(OLD),
        "{s}"
    );
    assert!(rhizoid(&l).contains("rhizoid old"));
    assert!(!calls(&l).contains(ASSET), "{}", calls(&l));
}

#[test]
fn a_tool_at_its_pin_is_up_to_date_and_nothing_is_fetched() {
    let l = lab("current", 0);
    fs::write(
        l.dir.join("home/.local/share/lpoco-host/rev-rhizoid"),
        format!("{NEW}\n"),
    )
    .unwrap();
    let o = follow(&l, &["now"]);
    let s = text(&o);
    assert!(o.status.success(), "{s}");
    assert!(s.contains("up to date"), "{s}");
    assert!(!calls(&l).contains("curl lpoco"), "{}", calls(&l));
    assert!(rhizoid(&l).contains("rhizoid old"));
}

#[test]
fn a_failing_check_after_the_install_puts_the_old_tool_and_its_stamp_back() {
    let l = lab("failing", 0);
    // the check passes before (baseline), fails once the new tool stands: the host answers by what rhizoid is installed
    exe(
        &l.dir.join("home/.local/bin/lpoco-host"),
        &format!(
            "case \"$1\" in check) grep -q new {} && exit 1; exit 0;; plan) printf '%s\\n' 'root commands, through `sudo`:' '    none';; esac",
            l.dir.join("home/.local/bin/rhizoid").display()
        ),
    );
    let o = follow(&l, &["now"]);
    let s = text(&o);
    assert!(!o.status.success(), "{s}");
    assert!(s.contains("back") && s.contains("rhizoid"), "{s}");
    assert!(rhizoid(&l).contains("rhizoid old"), "{s}");
    assert_eq!(stamp(&l), OLD);
    assert!(!l
        .dir
        .join("home/.local/state/devlab/xiaomi-surya/keeper.lease")
        .exists());
}

#[test]
fn a_tool_that_was_never_installed_is_left_to_prepare() {
    let l = lab("absent", 0);
    fs::remove_file(l.dir.join("home/.local/bin/rhizoid")).unwrap();
    fs::remove_file(l.dir.join("home/.local/share/lpoco-host/rev-rhizoid")).unwrap();
    let o = follow(&l, &["now"]);
    let s = text(&o);
    assert!(o.status.success(), "{s}");
    assert!(s.contains("up to date"), "{s}");
    assert!(!l.dir.join("home/.local/bin/rhizoid").exists());
}

/// A host that judges like the real one: a tool off its pin is a `tool NAME: FAIL` line and exit 1.
fn honest_host(l: &Lab) {
    exe(
        &l.dir.join("home/.local/bin/lpoco-host"),
        &format!(
            "case \"$1\" in check) grep -q {NEW} {} && exit 0; echo 'tool rhizoid: FAIL, pinned {NEW} found {OLD}'; exit 1;; plan) printf '%s\\n' 'root commands, through `sudo`:' '    none';; esac",
            l.dir.join("home/.local/share/lpoco-host/rev-rhizoid").display()
        ),
    );
}

#[test]
fn the_baseline_does_not_hold_the_moved_tool_against_the_host() {
    let l = lab("honest", 0);
    honest_host(&l);
    let o = follow(&l, &["now"]);
    let s = text(&o);
    assert!(o.status.success(), "{s}");
    assert!(rhizoid(&l).contains("rhizoid new"), "{s}");
    assert_eq!(stamp(&l), NEW);
}

#[test]
fn a_tool_on_the_path_with_no_stamp_is_off_its_pin_and_is_installed() {
    let l = lab("unstamped", 0);
    fs::remove_file(l.dir.join("home/.local/share/lpoco-host/rev-rhizoid")).unwrap();
    let o = follow(&l, &["now"]);
    let s = text(&o);
    assert!(o.status.success(), "{s}");
    assert!(rhizoid(&l).contains("rhizoid new"), "{s}");
    assert_eq!(stamp(&l), NEW);
}
