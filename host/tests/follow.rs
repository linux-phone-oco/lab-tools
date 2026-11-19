// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco host follow` (decision 2026-11-19-host-follows-releases (b)): a user timer sees a newer family release,
//! waits for a window where every phone's keeper lease is free, installs it, runs the new host's check, steps back
//! on a failing check or a release that needs root, and lands the lab.toml pin bump. The lab is a real git
//! repository (a bare origin and a clone) so the landed pin can be read back from main; curl, root, cargo, systemctl
//! and `land` are stubs; the installed `lpoco-host` of a release is a script whose answers the test sets.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const FAMILY: &[&str] = &[
    "lpoco",
    "lpoco-lab",
    "lpoco-bringup",
    "lpoco-health",
    "lpoco-host",
    "lpoco-tone",
    "lpoco-attended",
];
const SUFFIX: &str = "x86_64-unknown-linux-gnu";
const NO_ROOT: &str = "root commands, through `sudo`:\n    none\n";

struct Lab {
    dir: PathBuf,
}

fn exe(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

fn which(p: &str) -> PathBuf {
    ["/usr/bin", "/bin"]
        .iter()
        .map(|d| Path::new(d).join(p))
        .find(|f| f.exists())
        .unwrap()
}

/// A lab pinned at lpoco 0.2.0 with two phones (xiaomi-surya, google-walleye), whose lab.toml lives in a clone of a
/// bare origin; release 0.2.1 is published (every program, the host one answering as `host_check`/`host_plan`).
fn lab(name: &str, follow_toml: &str) -> Lab {
    let dir = std::env::temp_dir().join(format!("lpoco-follow-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let home = dir.join("home");
    let stubs = dir.join("stubs");
    fs::create_dir_all(&stubs).unwrap();
    for (repo, code) in [
        ("surya-lab", "xiaomi-surya"),
        ("google-walleye-lab", "google-walleye"),
    ] {
        let d = home.join("Projects/linux-phone-oco").join(repo);
        fs::create_dir_all(&d).unwrap();
        fs::write(
            d.join("device.toml"),
            format!("[device]\ncodename = \"{code}\"\n[host]\nmodules = [\"cdc_acm\"]\n"),
        )
        .unwrap();
    }
    // the lab's own repository: a bare origin and the clone lab.toml is read from
    let origin = dir.join("origin.git");
    git(
        &dir.join(""),
        &[
            "init",
            "-q",
            "--bare",
            "-b",
            "main",
            origin.to_str().unwrap(),
        ]
        .map(|s| s)[..],
    )
    .to_string();
    let clone = home.join("Projects/lab-machine");
    fs::create_dir_all(&clone).unwrap();
    git(&clone, &["init", "-q", "-b", "main"]);
    git(
        &clone,
        &["remote", "add", "origin", origin.to_str().unwrap()],
    );
    fs::write(
        clone.join("lab.toml"),
        format!(
            "devices = \"{}/Projects/linux-phone-oco/[gs]*-lab/device.toml\"\n{follow_toml}[tools]\n# the family\nlpoco = \"0.2.0\"\n",
            home.display()
        ),
    )
    .unwrap();
    git(&clone, &["add", "."]);
    git(&clone, &["commit", "-q", "-m", "lab"]);
    git(&clone, &["push", "-q", "-u", "origin", "main"]);
    let rec = format!("echo \"$0 $@\" >> {}", dir.join("calls").display());
    for s in ["root", "cargo", "systemctl"] {
        exe(&stubs.join(s), &rec);
    }
    // landing is the pin's branch pushed to main of the bare origin
    exe(
        &stubs.join("land"),
        &format!("{rec}\ngit push -q origin HEAD:main"),
    );
    exe(
        &stubs.join("curl"),
        &format!(
            "echo \"$0 $@\" >> {}\nwhile [ $# -gt 0 ]; do case \"$1\" in --output) out=\"$2\"; shift;; --*) ;; *) url=\"$1\";; esac; shift; done\nsrc=\"${{url#file://}}\"\n[ -f \"$src\" ] || {{ printf 404; exit 22; }}\nprintf 200\ncp \"$src\" \"$out\"",
            dir.join("calls").display()
        ),
    );
    for p in ["cp", "git", "date", "mkdir", "touch", "cat", "sh"] {
        std::os::unix::fs::symlink(which(p), stubs.join(p)).unwrap();
    }
    let l = Lab { dir };
    publish_release(&l, "0.2.1", 0, NO_ROOT);
    l
}

/// The release folder of one version: every program an asset that says its name, lpoco-host one that answers
/// `check` with `check_exit` and `plan` with `plan_text`.
fn publish_release(l: &Lab, version: &str, check_exit: i32, plan_text: &str) {
    for p in FAMILY {
        let tag = l.dir.join("release").join(format!("{p}-v{version}"));
        fs::create_dir_all(&tag).unwrap();
        let file = tag.join(format!("{p}-{SUFFIX}"));
        let body = if *p == "lpoco-host" {
            format!(
                "#!/bin/sh\necho \"lpoco-host {version} $@\" >> {calls}\ncase \"$1\" in check) exit {check_exit};; plan) printf '%s\\n' '{plan_text}';; esac\n",
                calls = l.dir.join("calls").display()
            )
        } else {
            format!("#!/bin/sh\necho {p} {version} asset\n")
        };
        fs::write(&file, body).unwrap();
        let sum = Command::new("sha256sum").arg(&file).output().unwrap();
        let hash = String::from_utf8(sum.stdout).unwrap();
        fs::write(
            tag.join(format!("{p}-{SUFFIX}.sha256")),
            format!(
                "{}  {p}-{SUFFIX}\n",
                hash.split_whitespace().next().unwrap()
            ),
        )
        .unwrap();
    }
}

/// The releases list the host reads: GitHub's, newest first, other programs' tags among the family's.
fn releases(l: &Lab, versions: &[&str]) {
    let items: Vec<String> = versions
        .iter()
        .flat_map(|v| {
            [
                format!("{{\"tag_name\":\"lpoco-lab-v{v}\"}}"),
                format!("{{\"tag_name\":\"lpoco-v{v}\"}}"),
            ]
        })
        .collect();
    fs::write(
        l.dir.join("releases.json"),
        format!("[{}]", items.join(",")),
    )
    .unwrap();
}

fn install_old(l: &Lab) {
    install_old_host(l, 0, NO_ROOT);
}

/// The programs of 0.2.0; the host one answers `check` and `plan` as the test says (it is the baseline the new one is judged against).
fn install_old_host(l: &Lab, check_exit: i32, plan_text: &str) {
    let bin = l.dir.join("home/.local/bin");
    fs::create_dir_all(&bin).unwrap();
    for p in FAMILY.iter().chain(&["devlab", "tone"]) {
        exe(&bin.join(p), &format!("echo {p} 0.2.0 old"));
    }
    exe(
        &bin.join("lpoco-host"),
        &format!(
            "echo old\ncase \"$1\" in check) exit {check_exit};; plan) printf '%s\\n' '{plan_text}';; esac"
        ),
    );
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
        .env(
            "LPOCO_LAB",
            l.dir.join("home/Projects/lab-machine/lab.toml"),
        )
        .env("LPOCO_ROOT", l.dir.join("stubs/root"))
        .env("LPOCO_FS_ROOT", l.dir.join("fs"))
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

fn installed(l: &Lab, p: &str) -> String {
    fs::read_to_string(l.dir.join("home/.local/bin").join(p)).unwrap_or_default()
}

/// What lab.toml says on main of the origin: the pin and who landed it.
fn landed_pin(l: &Lab) -> (String, String) {
    let origin = l.dir.join("origin.git");
    let toml = git(&origin, &["show", "main:lab.toml"]);
    let pin = toml
        .lines()
        .find(|x| x.starts_with("lpoco ="))
        .unwrap()
        .to_string();
    (pin, git(&origin, &["log", "-1", "--format=%an"]))
}

fn lease(l: &Lab, codename: &str, holder: &str, age: &str) {
    let d = l
        .dir
        .join("home/.local/state/devlab")
        .join(codename)
        .join("keeper.lease");
    fs::create_dir_all(&d).unwrap();
    fs::write(d.join("holder"), format!("{holder}\n")).unwrap();
    if age != "now" {
        let ok = Command::new("touch")
            .args(["-d", age])
            .arg(d.join("holder"))
            .status()
            .unwrap();
        assert!(ok.success());
    }
}

#[test]
fn a_host_at_the_newest_release_is_left_alone() {
    let l = lab("current", "");
    releases(&l, &["0.2.0"]);
    install_old(&l);
    let o = follow(&l, &["now"]);
    assert!(o.status.success(), "{}", text(&o));
    assert!(text(&o).contains("up to date"), "{}", text(&o));
    assert!(installed(&l, "lpoco").contains("old"));
    assert_eq!(landed_pin(&l).0, "lpoco = \"0.2.0\"");
}

#[test]
fn a_newer_release_with_both_leases_free_is_installed_checked_and_its_pin_landed() {
    let l = lab("newer", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    // a lease that lapsed long ago holds nothing
    lease(&l, "xiaomi-surya", "surya-keeper", "3 hours ago");
    let o = follow(&l, &["now"]);
    assert!(o.status.success(), "{}", text(&o));
    for p in FAMILY {
        assert!(
            installed(&l, p).contains("0.2.1"),
            "{p}: {}",
            installed(&l, p)
        );
    }
    // the second names of the programs follow too
    assert_eq!(installed(&l, "devlab"), installed(&l, "lpoco-lab"));
    // the new host judged the declarations, with the pin bumped, and was asked what root would be needed for
    let c = calls(&l);
    assert!(c.contains("lpoco-host 0.2.1 check"), "{c}");
    assert!(c.contains("lpoco-host 0.2.1 plan"), "{c}");
    let (pin, who) = landed_pin(&l);
    assert_eq!(
        pin,
        "lpoco = \"0.2.1\"",
        "the pin was not landed: {}",
        text(&o)
    );
    assert_eq!(who, "lab-host-follow");
    assert!(c.contains("land"), "{c}");
    // the rest of lab.toml, its comment included, is as it was
    let toml = git(&l.dir.join("origin.git"), &["show", "main:lab.toml"]);
    assert!(toml.contains("# the family"), "{toml}");
}

#[test]
fn a_held_lease_makes_it_wait_and_nothing_is_changed() {
    let l = lab("held", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    lease(&l, "xiaomi-surya", "surya-keeper", "now");
    let o = follow(&l, &["now"]);
    let s = text(&o);
    assert!(o.status.success(), "{s}");
    assert!(
        s.contains("waits") && s.contains("xiaomi-surya") && s.contains("surya-keeper"),
        "{s}"
    );
    assert!(installed(&l, "lpoco").contains("old"));
    assert_eq!(landed_pin(&l).0, "lpoco = \"0.2.0\"");
    assert!(
        !calls(&l).contains(SUFFIX),
        "a program was fetched while a keeper holds a phone: {}",
        calls(&l)
    );
}

#[test]
fn a_failing_check_puts_the_previous_binaries_back_and_lands_no_pin() {
    let l = lab("badcheck", "");
    publish_release(&l, "0.2.1", 1, NO_ROOT);
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    let o = follow(&l, &["now"]);
    let s = text(&o);
    assert!(!o.status.success(), "{s}");
    assert!(s.contains("check failed") && s.contains("0.2.0"), "{s}");
    for p in FAMILY.iter().chain(&["devlab", "tone"]) {
        assert!(
            installed(&l, p).contains("old"),
            "{p} was not put back: {}",
            installed(&l, p)
        );
    }
    assert_eq!(landed_pin(&l).0, "lpoco = \"0.2.0\"");
    assert!(!calls(&l).contains("land"), "{}", calls(&l));
}

#[test]
fn a_release_that_needs_root_is_not_applied_and_says_so() {
    let l = lab("root", "");
    publish_release(&l, "0.2.1", 0, "root commands, through `sudo`:\n    install -m 0644 x /etc/udev/rules.d/70-lpoco-new.rules\n");
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    let o = follow(&l, &["now"]);
    let s = text(&o);
    assert!(!o.status.success(), "{s}");
    assert!(s.contains("needs root") && s.contains("0.2.1"), "{s}");
    assert!(
        installed(&l, "lpoco").contains("old"),
        "the release stayed installed"
    );
    assert_eq!(landed_pin(&l).0, "lpoco = \"0.2.0\"");
}

#[test]
fn without_now_it_only_says_what_it_would_do() {
    let l = lab("dry", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    let o = follow(&l, &[]);
    let s = text(&o);
    assert!(o.status.success(), "{s}");
    assert!(
        s.contains("0.2.0") && s.contains("0.2.1") && s.contains("follow now"),
        "{s}"
    );
    assert!(installed(&l, "lpoco").contains("old"));
    assert!(!calls(&l).contains(SUFFIX), "{}", calls(&l));
}

#[test]
fn a_declared_follow_is_planned_as_a_user_timer_and_prepare_writes_and_enables_it() {
    let l = lab("timer", "[follow]\nevery = \"15min\"\n\n");
    let plan = Command::new(env!("CARGO_BIN_EXE_lpoco-host"))
        .arg("plan")
        .env_clear()
        .env("HOME", l.dir.join("home"))
        .env("PATH", l.dir.join("stubs"))
        .env(
            "LPOCO_LAB",
            l.dir.join("home/Projects/lab-machine/lab.toml"),
        )
        .env("LPOCO_FS_ROOT", l.dir.join("fs"))
        .output()
        .unwrap();
    let s = text(&plan);
    assert!(
        s.contains("lpoco-host-follow.timer") && s.contains("OnUnitActiveSec=15min"),
        "{s}"
    );
    assert!(!l
        .dir
        .join("home/.config/systemd/user/lpoco-host-follow.timer")
        .exists());
    let prep = Command::new(env!("CARGO_BIN_EXE_lpoco-host"))
        .args(["prepare", "now"])
        .env_clear()
        .env("HOME", l.dir.join("home"))
        .env("PATH", l.dir.join("stubs"))
        .env(
            "LPOCO_LAB",
            l.dir.join("home/Projects/lab-machine/lab.toml"),
        )
        .env("LPOCO_ROOT", l.dir.join("stubs/root"))
        .env("LPOCO_FS_ROOT", l.dir.join("fs"))
        .env(
            "LPOCO_RELEASE_BASE",
            format!("file://{}/release", l.dir.display()),
        )
        .output()
        .unwrap();
    let unit = l.dir.join("home/.config/systemd/user");
    let timer = fs::read_to_string(unit.join("lpoco-host-follow.timer")).unwrap_or_default();
    let service = fs::read_to_string(unit.join("lpoco-host-follow.service")).unwrap_or_default();
    assert!(
        timer.contains("OnUnitActiveSec=15min"),
        "{timer}\n{}",
        text(&prep)
    );
    assert!(
        service.contains("ExecStart=%h/.local/bin/lpoco-host follow now"),
        "{service}"
    );
    assert!(
        calls(&l).contains("systemctl --user enable --now lpoco-host-follow.timer"),
        "{}",
        calls(&l)
    );
}

#[test]
fn a_host_whose_own_check_fails_is_not_followed_until_it_is_fixed() {
    let l = lab("badbase", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old_host(&l, 1, NO_ROOT);
    let o = follow(&l, &["now"]);
    let s = text(&o);
    assert!(s.contains("waits") && s.contains("own check fails"), "{s}");
    assert!(
        !calls(&l).contains(SUFFIX),
        "a program was fetched: {}",
        calls(&l)
    );
    assert!(installed(&l, "lpoco").contains("old"));
    assert_eq!(landed_pin(&l).0, "lpoco = \"0.2.0\"");
}

#[test]
fn a_root_command_the_host_already_lacks_is_not_blamed_on_the_release() {
    let l = lab("pending", "");
    let pending = "root commands, through `sudo`:\n    install -m 0644 x /etc/udev/rules.d/70-lpoco-pending.rules\n";
    publish_release(&l, "0.2.1", 0, pending);
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old_host(&l, 0, pending);
    let o = follow(&l, &["now"]);
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(landed_pin(&l).0, "lpoco = \"0.2.1\"");
}

#[test]
fn a_plan_that_does_not_answer_is_not_taken_for_no_root() {
    let l = lab("silent", "");
    publish_release(&l, "0.2.1", 0, "nothing of the kind");
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    let o = follow(&l, &["now"]);
    assert!(!o.status.success(), "{}", text(&o));
    assert!(installed(&l, "lpoco").contains("old"));
    assert_eq!(landed_pin(&l).0, "lpoco = \"0.2.0\"");
}

#[test]
fn a_pin_that_cannot_land_puts_the_programs_back() {
    let l = lab("noland", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    exe(&l.dir.join("stubs/land"), "exit 1");
    let o = follow(&l, &["now"]);
    let s = text(&o);
    assert!(!o.status.success(), "{s}");
    assert!(s.contains("pin was not landed"), "{s}");
    for p in FAMILY {
        assert!(
            installed(&l, p).contains("old"),
            "{p} stayed at the new version"
        );
    }
    assert_eq!(landed_pin(&l).0, "lpoco = \"0.2.0\"");
}

#[test]
fn a_lab_that_cannot_land_a_pin_is_refused_before_any_program_is_fetched() {
    let l = lab("noorigin", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    fs::remove_file(l.dir.join("stubs/land")).unwrap();
    let o = follow(&l, &["now"]);
    assert!(!o.status.success(), "{}", text(&o));
    assert!(text(&o).contains("land"), "{}", text(&o));
    assert!(!calls(&l).contains(SUFFIX), "{}", calls(&l));
}

#[test]
fn the_enabled_watchers_are_restarted_after_a_followed_release() {
    let l = lab("watch", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    let wants = l.dir.join("home/.config/systemd/user/default.target.wants");
    fs::create_dir_all(&wants).unwrap();
    fs::write(wants.join("devlab-watch@surya-lab.service"), "").unwrap();
    let o = follow(&l, &["now"]);
    assert!(o.status.success(), "{}", text(&o));
    assert!(
        calls(&l).contains("systemctl --user restart devlab-watch@surya-lab.service"),
        "{}",
        calls(&l)
    );
}

fn lock_file(l: &Lab) -> PathBuf {
    l.dir.join("home/.local/state/lpoco-host/lock")
}

/// The lock held the way a run at work holds it: an flock on the file, naming this process.
fn lock_held(l: &Lab) -> fs::File {
    fs::create_dir_all(lock_file(l).parent().unwrap()).unwrap();
    let mut f = fs::File::create(lock_file(l)).unwrap();
    f.try_lock().unwrap();
    use std::io::Write;
    writeln!(f, "{}", std::process::id()).unwrap();
    f
}

/// Whether nobody holds the lock now.
fn lock_is_free(l: &Lab) -> bool {
    match fs::File::create(lock_file(l)).unwrap().try_lock() {
        Ok(()) => true,
        Err(_) => false,
    }
}

#[test]
fn a_run_that_holds_the_lock_makes_follow_wait_and_nothing_is_changed() {
    let l = lab("locked", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    let _held = lock_held(&l);
    let o = follow(&l, &["now"]);
    let s = text(&o);
    assert!(o.status.success(), "{s}");
    assert!(
        s.contains("waits") && s.contains(&std::process::id().to_string()),
        "{s}"
    );
    assert!(installed(&l, "lpoco").contains("old"));
    assert!(
        !calls(&l).contains(SUFFIX),
        "a program was fetched under another run's lock: {}",
        calls(&l)
    );
}

#[test]
fn a_lock_file_left_by_a_run_that_is_gone_holds_nothing_and_follow_gives_it_back() {
    let l = lab("stale-lock", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    // a file naming a process that is not there, as a kill or a power cut leaves it
    fs::create_dir_all(lock_file(&l).parent().unwrap()).unwrap();
    fs::write(lock_file(&l), "999999999\n").unwrap();
    let o = follow(&l, &["now"]);
    assert!(o.status.success(), "{}", text(&o));
    assert!(installed(&l, "lpoco").contains("0.2.1"), "{}", text(&o));
    assert!(lock_is_free(&l), "follow still holds its lock");
}

#[test]
fn a_failed_follow_gives_the_lock_back_too() {
    let l = lab("lock-after-failure", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    publish_release(&l, "0.2.1", 1, NO_ROOT);
    install_old(&l);
    let o = follow(&l, &["now"]);
    assert!(!o.status.success(), "{}", text(&o));
    assert!(lock_is_free(&l), "a failed follow still holds its lock");
}

#[test]
fn an_unreachable_lock_is_an_error_and_not_a_wait() {
    let l = lab("lock-unwritable", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    // the lock's place is a folder: it cannot be opened as a file
    fs::create_dir_all(lock_file(&l)).unwrap();
    let o = follow(&l, &["now"]);
    assert!(!o.status.success(), "{}", text(&o));
    assert!(text(&o).contains("cannot take the lock"), "{}", text(&o));
}

fn prepare(l: &Lab) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lpoco-host"))
        .args(["prepare", "now"])
        .env_clear()
        .env("HOME", l.dir.join("home"))
        .env("PATH", l.dir.join("stubs"))
        .env(
            "LPOCO_LAB",
            l.dir.join("home/Projects/lab-machine/lab.toml"),
        )
        .env("LPOCO_ROOT", l.dir.join("stubs/root"))
        .env("LPOCO_FS_ROOT", l.dir.join("fs"))
        .output()
        .unwrap()
}

#[test]
fn prepare_refuses_while_another_run_holds_the_lock_and_gives_it_back_when_done() {
    let l = lab("prepare-locked", "");
    {
        let _held = lock_held(&l);
        let o = prepare(&l);
        assert!(!o.status.success(), "{}", text(&o));
        assert!(
            text(&o).contains(&format!("(pid {}) holds the lock", std::process::id())),
            "{}",
            text(&o)
        );
        assert!(
            !calls(&l).contains("root"),
            "prepare ran root under another run's lock: {}",
            calls(&l)
        );
    }
    let o = prepare(&l);
    assert!(
        text(&o).contains("read:") || o.status.success(),
        "{}",
        text(&o)
    );
    assert!(!text(&o).contains("holds the lock"), "{}", text(&o));
    assert!(lock_is_free(&l), "prepare still holds its lock");
}

/// A lab.toml that is not at the default place must be the one the timer's service reads, or the timer follows a
/// different lab than the one it was prepared from.
#[test]
fn the_service_reads_the_lab_it_was_prepared_from_when_that_is_not_the_default_one() {
    let l = lab("servicelab", "[follow]\nevery = \"15min\"\n\n");
    let other = l.dir.join("other lab%1/lab.toml");
    fs::create_dir_all(other.parent().unwrap()).unwrap();
    fs::copy(l.dir.join("home/Projects/lab-machine/lab.toml"), &other).unwrap();
    let plan_with = |lab: &Path| {
        let o = Command::new(env!("CARGO_BIN_EXE_lpoco-host"))
            .arg("plan")
            .env_clear()
            .env("HOME", l.dir.join("home"))
            .env("PATH", l.dir.join("stubs"))
            .env("LPOCO_LAB", lab)
            .env("LPOCO_FS_ROOT", l.dir.join("fs"))
            .output()
            .unwrap();
        text(&o)
    };
    let s = plan_with(&other);
    let want = format!(
        "ExecStart=%h/.local/bin/lpoco-host follow now --lab \"{}\"",
        other.display().to_string().replace('%', "%%")
    );
    assert!(s.contains(&want), "{want}\n{s}");
    let d = plan_with(&l.dir.join("home/Projects/lab-machine/lab.toml"));
    assert!(
        d.contains("ExecStart=%h/.local/bin/lpoco-host follow now\n") && !d.contains("--lab"),
        "{d}"
    );
}

/// A relative lab path is made absolute: the user service does not start where `prepare` was run. A `$` and a quote
/// in the path reach the unit escaped; the clones service reads the same lab.
#[test]
fn a_relative_lab_path_reaches_the_units_absolute_and_escaped_for_both_services() {
    let l = lab(
        "relative",
        "[follow]\nevery = \"15min\"\n\n[clones]\nfolders = [\"~/a\"]\nevery = \"1h\"\n\n",
    );
    let dir = l.dir.join("here/a$b\"c");
    fs::create_dir_all(&dir).unwrap();
    fs::copy(
        l.dir.join("home/Projects/lab-machine/lab.toml"),
        dir.join("lab.toml"),
    )
    .unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_lpoco-host"))
        .args(["plan", "--lab", "lab.toml"])
        .current_dir(&dir)
        .env_clear()
        .env("HOME", l.dir.join("home"))
        .env("PATH", l.dir.join("stubs"))
        .env("LPOCO_FS_ROOT", l.dir.join("fs"))
        .output()
        .unwrap();
    let s = text(&o);
    let abs = format!("{}/lab.toml", dir.display())
        .replace('$', "$$")
        .replace('"', "\\\"");
    for verb in ["follow", "clones"] {
        let want = format!("lpoco-host {verb} now --lab \"{abs}\"");
        assert!(s.contains(&want), "{want}\n{s}");
    }
}
