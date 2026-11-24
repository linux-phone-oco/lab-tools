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
    "lpoco-layer",
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
            "echo \"$0 $@\" >> {0}\n[ -d \"$HOME/.cache/lpoco-host/follow/journal\" ] && echo journal-present >> {0}\nfor f in \"$HOME\"/.local/state/devlab/*/keeper.lease/holder; do [ -f \"$f\" ] && echo \"lease-held $(cat \"$f\")\" >> {0}; done\nwhile [ $# -gt 0 ]; do case \"$1\" in --output) out=\"$2\"; shift;; --*) ;; *) url=\"$1\";; esac; shift; done\nsrc=\"${{url#file://}}\"\n[ -f \"$src\" ] || {{ printf 404; exit 22; }}\nprintf 200\ncp \"$src\" \"$out\"",
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
                "#!/bin/sh\necho \"lpoco-host {version} $@\" >> {calls}\ncase \"$1\" in check) cat {out} 2>/dev/null; exit {check_exit};; plan) printf '%s\\n' '{plan_text}';; esac\n",
                calls = l.dir.join("calls").display(),
                out = l.dir.join("check.out").display()
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
            "echo old\ncase \"$1\" in check) cat {out} 2>/dev/null; exit {check_exit};; plan) printf '%s\\n' '{plan_text}';; esac",
            out = l.dir.join("check.out").display()
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

/// The programs of a host whose lpoco is older than the pin say so by `--version`.
fn drift(l: &Lab, installed_version: &str) {
    exe(
        &l.dir.join("home/.local/bin/lpoco"),
        &format!("echo lpoco {installed_version}"),
    );
}

#[test]
fn programs_older_than_the_pinned_newest_release_are_installed_and_the_pin_stays() {
    let l = lab("drifted", "");
    publish_release(&l, "0.2.0", 0, NO_ROOT);
    releases(&l, &["0.2.0"]);
    install_old(&l);
    drift(&l, "0.1.9");
    let o = follow(&l, &["now"]);
    assert!(o.status.success(), "{}", text(&o));
    assert!(!text(&o).contains("up to date"), "{}", text(&o));
    assert!(installed(&l, "lpoco").contains("asset"), "{}", text(&o));
    assert!(calls(&l).contains("check"), "{}", calls(&l));
    assert_eq!(landed_pin(&l).0, "lpoco = \"0.2.0\"");
    assert!(!journal(&l).exists());
}

#[test]
fn drifted_programs_are_only_announced_without_now_and_a_failing_check_puts_them_back() {
    let l = lab("drifted-plain", "");
    releases(&l, &["0.2.0"]);
    install_old(&l);
    drift(&l, "0.1.9");
    let o = follow(&l, &[]);
    assert!(o.status.success(), "{}", text(&o));
    assert!(text(&o).contains("0.1.9"), "{}", text(&o));
    assert!(installed(&l, "lpoco").contains("0.1.9"));

    let l = lab("drifted-fail", "");
    releases(&l, &["0.2.0"]);
    install_old(&l);
    drift(&l, "0.1.9");
    publish_release(&l, "0.2.0", 1, NO_ROOT);
    let o = follow(&l, &["now"]);
    assert!(!o.status.success(), "{}", text(&o));
    assert!(text(&o).contains("lpoco 0.1.9 -> 0.2.0"), "{}", text(&o));
    assert!(
        text(&o).contains("programs of 0.1.9 are back"),
        "{}",
        text(&o)
    );
    assert!(!journal(&l).exists());
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

fn leases_left(l: &Lab) -> Vec<String> {
    let root = l.dir.join("home/.local/state/devlab");
    let mut left: Vec<String> = fs::read_dir(&root)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join("keeper.lease").exists())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    left
}

#[test]
fn every_phones_lease_is_held_by_the_follow_while_it_installs_and_given_back_after() {
    let l = lab("holds", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    // a lapsed lease is taken over, not waited for
    lease(&l, "xiaomi-surya", "surya-keeper", "3 hours ago");
    let o = follow(&l, &["now"]);
    assert!(o.status.success(), "{}", text(&o));
    // every fetch of a program (the journal is on disk then) saw the follow's lease, never the lapsed keeper's
    let c = calls(&l);
    let lines: Vec<&str> = c.lines().collect();
    let installing: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i] == "journal-present")
        .collect();
    assert!(!installing.is_empty(), "{c}");
    for i in installing {
        assert_eq!(lines[i + 1], "lease-held lpoco-host-follow", "{c}");
        assert_eq!(lines[i + 2], "lease-held lpoco-host-follow", "{c}");
    }
    assert_eq!(leases_left(&l), Vec::<String>::new(), "{}", text(&o));
}

#[test]
fn a_failing_check_gives_the_leases_back_too() {
    let l = lab("holds-failing", "");
    publish_release(&l, "0.2.1", 1, NO_ROOT);
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    let o = follow(&l, &["now"]);
    assert!(!o.status.success(), "{}", text(&o));
    assert!(
        calls(&l).contains("lease-held lpoco-host-follow"),
        "{}",
        calls(&l)
    );
    assert_eq!(leases_left(&l), Vec::<String>::new(), "{}", text(&o));
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

fn journal(l: &Lab) -> PathBuf {
    l.dir.join("home/.cache/lpoco-host/follow/journal")
}

#[test]
fn the_programs_to_step_back_to_are_kept_on_disk_during_an_install_and_gone_after_it() {
    let l = lab("journal-during", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    let o = follow(&l, &["now"]);
    assert!(o.status.success(), "{}", text(&o));
    let c = calls(&l);
    assert!(
        c.contains("journal-present"),
        "no journal stood while the programs were fetched: {c}"
    );
    assert!(!journal(&l).exists(), "the journal outlived the follow");
}

#[test]
fn a_failed_follow_leaves_no_journal_either() {
    let l = lab("journal-failed", "");
    publish_release(&l, "0.2.1", 1, NO_ROOT);
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    let o = follow(&l, &["now"]);
    assert!(!o.status.success(), "{}", text(&o));
    assert!(!journal(&l).exists());
}

#[test]
fn a_follow_killed_mid_install_is_stepped_back_by_the_next_run_before_it_does_anything_else() {
    let l = lab("journal-killed", "");
    releases(&l, &["0.2.0"]);
    install_old(&l);
    // what a kill left: the journal of the programs of 0.2.0, and a bin holding two versions
    let j = journal(&l);
    fs::create_dir_all(&j).unwrap();
    for p in FAMILY.iter().chain(&["devlab", "tone"]) {
        fs::write(j.join(p), format!("#!/bin/sh\necho {p} 0.2.0 kept\n")).unwrap();
    }
    fs::write(j.join("lpoco-tone.absent"), "").unwrap();
    fs::write(j.join("TARGET"), "0.2.1\n").unwrap();
    fs::remove_file(j.join("lpoco-tone")).unwrap();
    let bin = l.dir.join("home/.local/bin");
    exe(&bin.join("lpoco"), "echo lpoco 0.2.1 half");
    exe(&bin.join("lpoco-tone"), "echo half-written");
    let o = follow(&l, &["now"]);
    let s = text(&o);
    assert!(o.status.success(), "{s}");
    assert!(s.contains("killed") && s.contains("put back"), "{s}");
    for p in FAMILY
        .iter()
        .filter(|p| **p != "lpoco-tone")
        .chain(&["devlab", "tone"])
    {
        assert!(
            installed(&l, p).contains("0.2.0 kept"),
            "{p}: {}",
            installed(&l, p)
        );
    }
    assert!(
        installed(&l, "lpoco").contains("0.2.0 kept"),
        "{}",
        installed(&l, "lpoco")
    );
    assert!(
        !bin.join("lpoco-tone").exists(),
        "a program that was not there before is not left behind"
    );
    assert!(!j.exists(), "the journal stays after it was used");
}

#[test]
fn a_journal_that_is_not_whole_is_an_error_and_changes_nothing() {
    let l = lab("journal-torn", "");
    releases(&l, &["0.2.0"]);
    install_old(&l);
    let j = journal(&l);
    fs::create_dir_all(&j).unwrap();
    // programs but no TARGET: it was not written whole
    fs::write(j.join("lpoco"), "#!/bin/sh\necho kept\n").unwrap();
    let o = follow(&l, &["now"]);
    assert!(!o.status.success(), "{}", text(&o));
    assert!(
        installed(&l, "lpoco").contains("old"),
        "{}",
        installed(&l, "lpoco")
    );
    assert!(j.exists());
}

#[test]
fn a_follow_killed_after_its_pin_landed_keeps_the_new_programs_and_drops_the_journal() {
    let l = lab("journal-landed", "");
    releases(&l, &["0.2.0"]);
    install_old(&l);
    let j = journal(&l);
    fs::create_dir_all(&j).unwrap();
    for p in FAMILY.iter().chain(&["devlab", "tone"]) {
        fs::write(j.join(p), "#!/bin/sh\necho older\n").unwrap();
    }
    // the pin is at 0.2.0: that was the release being installed
    fs::write(j.join("TARGET"), "0.2.0\n").unwrap();
    let o = follow(&l, &["now"]);
    let s = text(&o);
    assert!(o.status.success(), "{s}");
    assert!(s.contains("pin landed"), "{s}");
    assert!(
        installed(&l, "lpoco").contains("old"),
        "{}",
        installed(&l, "lpoco")
    );
    assert!(!installed(&l, "lpoco").contains("older"));
    assert!(!j.exists());
}

#[test]
fn a_journal_left_alone_by_follow_without_now() {
    let l = lab("journal-no-now", "");
    releases(&l, &["0.2.0"]);
    install_old(&l);
    let j = journal(&l);
    fs::create_dir_all(&j).unwrap();
    fs::write(j.join("TARGET"), "0.2.1\n").unwrap();
    fs::write(j.join("lpoco"), "#!/bin/sh\necho kept\n").unwrap();
    let o = follow(&l, &[]);
    assert!(o.status.success(), "{}", text(&o));
    assert!(j.exists());
    assert!(installed(&l, "lpoco").contains("old"));
}

const NOT_MEASURED: &str = "tool oco-work: not measured, its pin is not known yet\nshare: not measured, google-walleye: the phone is not attached\n";

#[test]
fn a_check_that_only_has_not_measured_lines_does_not_hold_a_release_back() {
    let l = lab("unmeasured", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    // an installed host that still exits 1 for what it could not measure, as 0.39 did
    install_old_host(&l, 1, NO_ROOT);
    fs::write(l.dir.join("check.out"), NOT_MEASURED).unwrap();
    let o = follow(&l, &["now"]);
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(landed_pin(&l).0, "lpoco = \"0.2.1\"");
}

#[test]
fn a_new_host_that_only_could_not_measure_is_kept() {
    let l = lab("newunmeasured", "");
    publish_release(&l, "0.2.1", 1, NO_ROOT);
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    fs::write(l.dir.join("check.out"), NOT_MEASURED).unwrap();
    let o = follow(&l, &["now"]);
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(landed_pin(&l).0, "lpoco = \"0.2.1\"");
}

#[test]
fn a_fail_line_is_what_a_waiting_follow_names_not_the_last_lines_printed() {
    let l = lab("failnamed", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old_host(&l, 1, NO_ROOT);
    fs::write(
        l.dir.join("check.out"),
        format!("tool land: FAIL, pinned a found b\n{NOT_MEASURED}"),
    )
    .unwrap();
    let o = follow(&l, &["now"]);
    let s = text(&o);
    assert!(
        s.contains("own check fails") && s.contains("tool land: FAIL"),
        "{s}"
    );
    assert!(!s.contains("not measured"), "{s}");
    assert_eq!(landed_pin(&l).0, "lpoco = \"0.2.0\"");
}

#[test]
fn a_check_that_errors_is_a_failure_whatever_it_printed() {
    let l = lab("errored", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old_host(&l, 2, NO_ROOT);
    fs::write(l.dir.join("check.out"), NOT_MEASURED).unwrap();
    let o = follow(&l, &["now"]);
    assert!(text(&o).contains("own check fails"), "{}", text(&o));
    assert_eq!(landed_pin(&l).0, "lpoco = \"0.2.0\"");
}

fn follow_at(l: &Lab, clock: &str) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_lpoco-host"));
    c.arg("follow").arg("now");
    c.env_clear()
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
        .env("LPOCO_CLOCK", clock)
        .env(
            "LPOCO_RELEASE_BASE",
            format!("file://{}/release", l.dir.display()),
        )
        .env(
            "LPOCO_RELEASES",
            format!("file://{}/releases.json", l.dir.display()),
        );
    c.output().unwrap()
}

#[test]
fn outside_its_window_follow_waits_and_changes_nothing() {
    let l = lab(
        "outside",
        "[follow]\nevery = \"15min\"\nwindow = \"02:00-05:00\"\n\n",
    );
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    let o = follow_at(&l, "14:30");
    let s = text(&o);
    assert!(o.status.success(), "{s}");
    assert!(
        s.contains("waits") && s.contains("02:00-05:00") && s.contains("14:30"),
        "{s}"
    );
    assert!(installed(&l, "lpoco").contains("old"));
    assert_eq!(landed_pin(&l).0, "lpoco = \"0.2.0\"");
    assert!(
        !calls(&l).contains(SUFFIX),
        "a program was fetched outside the window: {}",
        calls(&l)
    );
}

#[test]
fn inside_its_window_follow_installs_and_a_window_across_midnight_counts() {
    for (name, window, clock) in [
        ("inside", "02:00-05:00", "03:10"),
        ("midnight", "22:00-04:00", "23:59"),
    ] {
        let l = lab(
            name,
            &format!("[follow]\nevery = \"15min\"\nwindow = \"{window}\"\n\n"),
        );
        releases(&l, &["0.2.1", "0.2.0"]);
        install_old(&l);
        let o = follow_at(&l, clock);
        assert!(o.status.success(), "{}", text(&o));
        assert!(
            installed(&l, "lpoco").contains("0.2.1"),
            "{name}: {}",
            text(&o)
        );
        assert_eq!(landed_pin(&l).0, "lpoco = \"0.2.1\"", "{name}");
    }
}

#[test]
fn a_window_that_is_not_two_clock_times_is_refused_where_lab_toml_is_read() {
    let l = lab(
        "badwindow",
        "[follow]\nevery = \"15min\"\nwindow = \"night\"\n\n",
    );
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    let o = follow_at(&l, "03:00");
    assert!(!o.status.success(), "{}", text(&o));
    assert!(text(&o).contains("window"), "{}", text(&o));
    assert!(installed(&l, "lpoco").contains("old"));
}

#[test]
fn a_killed_follow_is_stepped_back_at_any_hour_and_an_up_to_date_host_stays_quiet_outside_the_window(
) {
    let w = "[follow]\nevery = \"15min\"\nwindow = \"02:00-05:00\"\n\n";
    let l = lab("outside-killed", w);
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    let j = journal(&l);
    fs::create_dir_all(&j).unwrap();
    for p in FAMILY.iter().chain(&["devlab", "tone"]) {
        fs::write(j.join(p), format!("#!/bin/sh\necho {p} 0.2.0 kept\n")).unwrap();
    }
    fs::write(j.join("TARGET"), "0.2.1\n").unwrap();
    let o = follow_at(&l, "14:30");
    let s = text(&o);
    assert!(o.status.success(), "{s}");
    assert!(s.contains("put back"), "{s}");
    assert!(
        installed(&l, "lpoco").contains("0.2.0 kept"),
        "{}",
        installed(&l, "lpoco")
    );
    assert!(!j.exists());

    let up = lab("outside-uptodate", w);
    releases(&up, &["0.2.0"]);
    install_old(&up);
    let s = text(&follow_at(&up, "14:30"));
    assert!(s.contains("up to date") && !s.contains("waits"), "{s}");
}

#[test]
fn a_reinstall_killed_mid_install_is_stepped_back_not_taken_for_a_landed_pin() {
    let w = "[follow]\nevery = \"15min\"\nwindow = \"02:00-05:00\"\n\n";
    let l = lab("drift-killed", w);
    releases(&l, &["0.2.0"]);
    install_old(&l);
    // what a killed reinstall left: the journal of the programs of 0.1.9 (the pin is 0.2.0 and stays),
    // and a bin whose lpoco is already the new one
    let j = journal(&l);
    fs::create_dir_all(&j).unwrap();
    for p in FAMILY.iter().chain(&["devlab", "tone"]) {
        fs::write(j.join(p), format!("#!/bin/sh\necho {p} 0.1.9\n")).unwrap();
    }
    fs::write(j.join("TARGET"), "0.2.0-reinstall\n").unwrap();
    exe(&l.dir.join("home/.local/bin/lpoco"), "echo lpoco 0.2.0");
    let o = follow_at(&l, "14:30");
    let s = text(&o);
    assert!(o.status.success(), "{s}");
    assert!(
        s.contains("killed mid-install") && s.contains("put back"),
        "{s}"
    );
    assert!(!s.contains("pin landed"), "{s}");
    assert!(
        installed(&l, "lpoco").contains("0.1.9"),
        "{}",
        installed(&l, "lpoco")
    );
    assert!(!j.exists());
}

#[test]
fn the_edges_of_a_window_and_a_clock_that_is_not_a_time() {
    for (clock, installs) in [
        ("02:00", true),
        ("04:59", true),
        ("05:00", false),
        ("01:59", false),
    ] {
        let l = lab(
            "edge",
            "[follow]\nevery = \"15min\"\nwindow = \"02:00-05:00\"\n\n",
        );
        releases(&l, &["0.2.1", "0.2.0"]);
        install_old(&l);
        let o = follow_at(&l, clock);
        assert!(o.status.success(), "{clock}: {}", text(&o));
        assert_eq!(
            installed(&l, "lpoco").contains("0.2.1"),
            installs,
            "{clock}"
        );
    }
    let l = lab(
        "badclock",
        "[follow]\nevery = \"15min\"\nwindow = \"02:00-05:00\"\n\n",
    );
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    let o = follow_at(&l, "noon");
    assert!(
        !o.status.success() && text(&o).contains("HH:MM"),
        "{}",
        text(&o)
    );
    assert!(installed(&l, "lpoco").contains("old"));
}

#[test]
fn drifted_programs_wait_outside_the_window_and_for_a_keeper_and_change_nothing() {
    let w = "[follow]\nevery = \"15min\"\nwindow = \"02:00-05:00\"\n\n";
    let l = lab("drift-waits", w);
    releases(&l, &["0.2.0"]);
    publish_release(&l, "0.2.0", 0, NO_ROOT);
    install_old(&l);
    drift(&l, "0.1.9");
    let s = text(&follow_at(&l, "14:30"));
    assert!(s.contains("waits"), "{s}");
    assert!(installed(&l, "lpoco").contains("0.1.9") && !journal(&l).exists());

    let k = lab("drift-keeper", "");
    releases(&k, &["0.2.0"]);
    publish_release(&k, "0.2.0", 0, NO_ROOT);
    install_old(&k);
    drift(&k, "0.1.9");
    lease(&k, "xiaomi-surya", "surya-keeper", "now");
    let s = text(&follow(&k, &["now"]));
    assert!(s.contains("waits") && s.contains("surya-keeper"), "{s}");
    assert!(installed(&k, "lpoco").contains("0.1.9") && !journal(&k).exists());
}

#[test]
fn an_installed_version_that_cannot_be_read_is_said_not_judged_instead_of_up_to_date_alone() {
    let l = lab("unreadable-version", "");
    releases(&l, &["0.2.0"]);
    install_old(&l);
    let s = text(&follow(&l, &["now"]));
    assert!(s.contains("up to date") && s.contains("not judged"), "{s}");
}

#[test]
fn a_lease_a_killed_follow_left_does_not_hold_the_next_follow_back() {
    let l = lab("own-leftover", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    lease(&l, "xiaomi-surya", "lpoco-host-follow", "now");
    let o = follow(&l, &["now"]);
    assert!(o.status.success(), "{}", text(&o));
    assert!(installed(&l, "lpoco").contains("0.2.1"), "{}", text(&o));
    assert_eq!(leases_left(&l), Vec::<String>::new(), "{}", text(&o));
}

const EVERY: &str = "[follow]\nevery = \"15min\"\n\n";

/// What a failed try of a release leaves: the release and the second its run began.
fn tried(l: &Lab) -> PathBuf {
    l.dir.join("home/.cache/lpoco-host/follow/tried")
}

fn secs_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[test]
fn a_release_that_failed_is_not_tried_again_before_every_has_passed() {
    let l = lab("rate-wait", EVERY);
    publish_release(&l, "0.2.1", 1, NO_ROOT);
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    let first = follow(&l, &["now"]);
    assert!(!first.status.success(), "{}", text(&first));
    let touched = |l: &Lab| {
        calls(l)
            .lines()
            .filter(|c| !c.contains("releases.json"))
            .count()
    };
    let after_first = touched(&l);
    let second = follow(&l, &["now"]);
    let s = text(&second);
    assert!(second.status.success(), "{s}");
    assert!(
        s.contains("waits") && s.contains("0.2.1") && s.contains("15min"),
        "{s}"
    );
    assert_eq!(touched(&l), after_first, "the second run touched the host");
    assert!(
        installed(&l, "lpoco").contains("old"),
        "{}",
        installed(&l, "lpoco")
    );
}

#[test]
fn a_release_that_failed_is_tried_again_once_every_has_passed() {
    let l = lab("rate-again", EVERY);
    publish_release(&l, "0.2.1", 1, NO_ROOT);
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    let first = follow(&l, &["now"]);
    assert!(!first.status.success(), "{}", text(&first));
    fs::write(tried(&l), format!("0.2.1 {}\n", secs_now() - 16 * 60)).unwrap();
    let second = follow(&l, &["now"]);
    let s = text(&second);
    assert!(
        !second.status.success() && s.contains("check failed"),
        "tried again: {s}"
    );
}

#[test]
fn a_newer_release_than_the_one_that_failed_is_tried_at_once_and_a_follow_that_lands_forgets() {
    let l = lab("rate-newer", EVERY);
    publish_release(&l, "0.2.2", 0, NO_ROOT);
    releases(&l, &["0.2.2", "0.2.0"]);
    install_old(&l);
    fs::create_dir_all(tried(&l).parent().unwrap()).unwrap();
    fs::write(tried(&l), format!("0.2.1 {}\n", secs_now())).unwrap();
    let o = follow(&l, &["now"]);
    assert!(o.status.success(), "{}", text(&o));
    assert!(installed(&l, "lpoco").contains("0.2.2"), "{}", text(&o));
    assert!(!tried(&l).exists(), "a landed follow keeps no failed try");
}

#[test]
fn without_a_follow_table_there_is_no_every_and_so_no_rate_limit() {
    let l = lab("rate-none", "");
    publish_release(&l, "0.2.1", 1, NO_ROOT);
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    for _ in 0..2 {
        let o = follow(&l, &["now"]);
        assert!(text(&o).contains("check failed"), "{}", text(&o));
    }
}

#[test]
fn a_release_without_its_binaries_is_built_with_cargo() {
    let l = lab("noasset", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    // the release is listed but carries no asset: every download is a 404
    fs::remove_dir_all(l.dir.join("release")).unwrap();
    let o = follow(&l, &["now"]);
    let c = calls(&l);
    assert!(
        c.lines()
            .any(|x| x.contains("cargo") && x.contains("install")),
        "no cargo build for a release without assets: {c}\n{}",
        text(&o)
    );
}

/// A tool of another project pinned at `v0.3.8` while `v0.3.6` is installed: the host's check fails on it, and only
/// a follow can install the moved pin. Source: the check line is the one `lpoco-host check` prints (host/src/main.rs
/// `tool {}: FAIL, pinned {} found {}`).
const MOVED_TOOL: &str = "[tools.rhizoid]\nrepository = \"git-rhizoid/rhizoid\"\ntag = \"v0.3.8\"\nasset = \"rhizoid-x86_64-unknown-linux-gnu\"\n";
const MOVED_TOOL_FAIL: &str = "tool rhizoid: FAIL, pinned v0.3.8 found v0.3.6\n";

fn a_moved_tool_is_installed_off_its_pin(l: &Lab) {
    let bin = l.dir.join("home/.local/bin");
    exe(&bin.join("rhizoid"), "echo rhizoid old");
    let share = l.dir.join("home/.local/share/lpoco-host");
    fs::create_dir_all(&share).unwrap();
    fs::write(share.join("rev-rhizoid"), "v0.3.6\n").unwrap();
    fs::write(l.dir.join("check.out"), MOVED_TOOL_FAIL).unwrap();
}

#[test]
fn a_check_that_fails_only_on_a_tool_follow_installs_does_not_stop_a_family_release() {
    let l = lab("moved-tool-family", MOVED_TOOL);
    publish_release(&l, "0.2.1", 1, NO_ROOT);
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old_host(&l, 1, NO_ROOT);
    a_moved_tool_is_installed_off_its_pin(&l);
    let o = follow(&l, &["now"]);
    let s = text(&o);
    assert!(o.status.success() && !s.contains("waits"), "{s}");
    assert!(installed(&l, "lpoco").contains("0.2.1"), "{s}");
    assert_eq!(landed_pin(&l).0, "lpoco = \"0.2.1\"", "{s}");
}

#[test]
fn a_check_that_fails_on_another_thing_still_stops_it_whatever_tool_is_moved() {
    let l = lab("moved-tool-and-more", MOVED_TOOL);
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old_host(&l, 1, NO_ROOT);
    a_moved_tool_is_installed_off_its_pin(&l);
    fs::write(
        l.dir.join("check.out"),
        format!("{MOVED_TOOL_FAIL}share: FAIL, nothing\n"),
    )
    .unwrap();
    let s = text(&follow(&l, &["now"]));
    assert!(s.contains("waits") && s.contains("share: FAIL"), "{s}");
    assert!(installed(&l, "lpoco").contains("old"), "{s}");
}

#[test]
fn the_releases_request_carries_the_token_to_github_and_to_nobody_else() {
    // the stub curl copies the releases list for any url and writes the config it was given on stdin
    let l = lab("token", "");
    releases(&l, &["0.2.0"]);
    install_old(&l);
    let curl = l.dir.join("stubs/curl");
    exe(
        &curl,
        &format!(
            "echo \"$0 $@\" >> {calls}\ncat >> {cfg}\nwhile [ $# -gt 0 ]; do case \"$1\" in --output) out=\"$2\"; shift;; esac; shift; done\nprintf 200\ncp {rel} \"$out\"",
            calls = l.dir.join("calls").display(),
            cfg = l.dir.join("curl-config").display(),
            rel = l.dir.join("releases.json").display()
        ),
    );
    let run = |url: &str| {
        let _ = fs::remove_file(l.dir.join("curl-config"));
        let o = Command::new(env!("CARGO_BIN_EXE_lpoco-host"))
            .args(["follow"])
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
            .env("LPOCO_RELEASES", url)
            .env("GITHUB_TOKEN", "ghp_SECRET")
            .output()
            .unwrap();
        (
            text(&o),
            fs::read_to_string(l.dir.join("curl-config")).unwrap_or_default(),
        )
    };
    let (s, cfg) =
        run("https://api.github.com/repos/linux-phone-oco/lab-tools/releases?per_page=100");
    assert!(
        cfg.contains("Authorization: Bearer ghp_SECRET"),
        "{s}\n{cfg}"
    );
    assert!(
        !calls(&l).contains("ghp_SECRET"),
        "the token is on the command line: {}",
        calls(&l)
    );
    let (_, cfg) = run("https://example.org/releases.json");
    assert!(!cfg.contains("ghp_SECRET"), "{cfg}");
}

#[test]
fn a_token_that_is_not_a_github_token_is_refused_without_being_repeated() {
    let l = lab("badtoken", "");
    releases(&l, &["0.2.0"]);
    install_old(&l);
    let o = Command::new(env!("CARGO_BIN_EXE_lpoco-host"))
        .args(["follow"])
        .env_clear()
        .env("HOME", l.dir.join("home"))
        .env(
            "LPOCO_LAB",
            l.dir.join("home/Projects/lab-machine/lab.toml"),
        )
        .env("GITHUB_TOKEN", "ab\"cd")
        .output()
        .unwrap();
    let s = text(&o);
    assert!(
        !o.status.success() && s.contains("GITHUB_TOKEN") && !s.contains("ab\"cd"),
        "{s}"
    );
}

fn ok_stamp(l: &Lab) -> String {
    fs::read_to_string(l.dir.join("home/.cache/lpoco-host/follow/ok")).unwrap_or_default()
}

#[test]
fn a_follow_that_finishes_keeps_when_and_one_that_waits_does_not() {
    let l = lab("stamp", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    lease(&l, "xiaomi-surya", "keeper", "now");
    follow(&l, &["now"]);
    assert_eq!(ok_stamp(&l), "", "a wait is not a success");
    let l = lab("stamp-current", "");
    releases(&l, &["0.2.0"]);
    install_old(&l);
    follow(&l, &["now"]);
    assert!(
        ok_stamp(&l).trim().parse::<u64>().is_ok(),
        "{}",
        ok_stamp(&l)
    );
    let l = lab("stamp-landed", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old_host(&l, 0, NO_ROOT);
    let o = follow(&l, &["now"]);
    assert!(o.status.success(), "{}", text(&o));
    assert!(
        ok_stamp(&l).trim().parse::<u64>().is_ok(),
        "{}",
        ok_stamp(&l)
    );
}

#[test]
fn a_stale_follow_line_in_the_check_does_not_stop_the_follow_that_would_clear_it() {
    let l = lab("stale-line", "[follow]\nevery = \"15min\"\n");
    publish_release(&l, "0.2.1", 0, NO_ROOT);
    releases(&l, &["0.2.1", "0.2.0"]);
    fs::write(
        l.dir.join("check.out"),
        "follow: FAIL, its last success was 999 min ago, more than 3 rounds of every = 15min\n",
    )
    .unwrap();
    install_old_host(&l, 1, NO_ROOT);
    let o = follow(&l, &["now"]);
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(landed_pin(&l).0, "lpoco = \"0.2.1\"", "{}", text(&o));
}

#[test]
fn a_dry_run_keeps_no_success() {
    let l = lab("stamp-dry", "");
    releases(&l, &["0.2.0"]);
    install_old(&l);
    follow(&l, &[]);
    assert_eq!(ok_stamp(&l), "");
}

#[test]
fn with_no_token_given_follow_asks_gh_for_the_one_it_is_logged_in_with() {
    // the stub gh prints a token for `gh auth token`; the stub curl records the config it is given on stdin
    let l = lab("ghauth", "");
    releases(&l, &["0.2.0"]);
    install_old(&l);
    exe(
        &l.dir.join("stubs/gh"),
        "[ \"$*\" = \"auth token --hostname github.com --no-refresh\" ] && echo ghp_FROMGH",
    );
    exe(
        &l.dir.join("stubs/curl"),
        &format!(
            "echo \"$0 $@\" >> {calls}\ncat >> {cfg}\nwhile [ $# -gt 0 ]; do case \"$1\" in --output) out=\"$2\"; shift;; esac; shift; done\nprintf 200\ncp {rel} \"$out\"",
            calls = l.dir.join("calls").display(),
            cfg = l.dir.join("curl-config").display(),
            rel = l.dir.join("releases.json").display()
        ),
    );
    let o = Command::new(env!("CARGO_BIN_EXE_lpoco-host"))
        .args(["follow"])
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
        .env(
            "LPOCO_RELEASES",
            "https://api.github.com/repos/linux-phone-oco/lab-tools/releases?per_page=100",
        )
        .output()
        .unwrap();
    let cfg = fs::read_to_string(l.dir.join("curl-config")).unwrap_or_default();
    assert!(
        cfg.contains("Authorization: Bearer ghp_FROMGH"),
        "{}\n{cfg}",
        text(&o)
    );
    assert!(!calls(&l).contains("ghp_FROMGH"), "{}", calls(&l));
}

/// A lab whose gh stub leaves a marker when it runs and whose curl stub records its stdin config.
fn gh_lab(name: &str, gh_body: &str) -> Lab {
    let l = lab(name, "");
    releases(&l, &["0.2.0"]);
    install_old(&l);
    exe(
        &l.dir.join("stubs/gh"),
        &format!("echo ran >> {}\n{gh_body}", l.dir.join("gh-ran").display()),
    );
    exe(
        &l.dir.join("stubs/curl"),
        &format!(
            "cat >> {cfg}\nwhile [ $# -gt 0 ]; do case \"$1\" in --output) out=\"$2\"; shift;; esac; shift; done\nprintf 200\ncp {rel} \"$out\"",
            cfg = l.dir.join("curl-config").display(),
            rel = l.dir.join("releases.json").display()
        ),
    );
    l
}

fn follow_with(l: &Lab, url: &str, token: Option<&str>) -> String {
    follow_args(l, &[], url, token)
}

fn follow_args(l: &Lab, args: &[&str], url: &str, token: Option<&str>) -> String {
    let mut c = Command::new(env!("CARGO_BIN_EXE_lpoco-host"));
    c.arg("follow")
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
        .env("LPOCO_RELEASES", url);
    if let Some(t) = token {
        c.env("GITHUB_TOKEN", t);
    }
    text(&c.output().unwrap())
}

const API: &str = "https://api.github.com/repos/linux-phone-oco/lab-tools/releases?per_page=100";

#[test]
fn a_given_token_wins_and_gh_is_not_asked() {
    let l = gh_lab("ghwins", "echo ghp_FROMGH");
    follow_with(&l, API, Some("ghp_GIVEN"));
    let cfg = fs::read_to_string(l.dir.join("curl-config")).unwrap_or_default();
    assert!(
        cfg.contains("ghp_GIVEN") && !cfg.contains("ghp_FROMGH"),
        "{cfg}"
    );
    assert!(!l.dir.join("gh-ran").exists());
}

#[test]
fn gh_is_not_asked_for_a_url_that_is_not_githubs_api() {
    let l = gh_lab("ghnotapi", "echo ghp_FROMGH");
    follow_with(&l, "https://example.org/releases.json", None);
    assert!(!l.dir.join("gh-ran").exists());
    assert!(!fs::read_to_string(l.dir.join("curl-config"))
        .unwrap_or_default()
        .contains("ghp_"));
}

#[test]
fn a_gh_that_fails_or_prints_no_token_leaves_follow_anonymous_and_silent_of_it() {
    for (name, body) in [
        ("ghfail", "echo ghp_FROMGH; exit 1"),
        ("ghjunk", "echo 'not a token!'"),
    ] {
        let l = gh_lab(name, body);
        let s = follow_with(&l, API, None);
        assert!(l.dir.join("gh-ran").exists(), "{name}");
        assert!(
            !fs::read_to_string(l.dir.join("curl-config"))
                .unwrap_or_default()
                .contains("Bearer"),
            "{name}"
        );
        assert!(
            !s.contains("ghp_FROMGH") && !s.contains("not a token"),
            "{name}: {s}"
        );
    }
}

#[test]
fn a_401_for_the_token_gh_gave_is_retried_anonymous_and_a_given_token_is_not() {
    // source: GitHub answers 401 "Bad credentials" to a bearer token it does not know, whatever the endpoint;
    // curl --fail then exits 22 with the code on --write-out (host/src/main.rs fetch_with reads that code)
    let stub = |l: &Lab| {
        exe(
            &l.dir.join("stubs/curl"),
            &format!(
                "cfg=$(cat)\necho \"$cfg\" >> {cfg}\nwhile [ $# -gt 0 ]; do case \"$1\" in --output) out=\"$2\"; shift;; esac; shift; done\ncase \"$cfg\" in *Bearer*) printf 401; exit 22;; esac\nprintf 200\ncp {rel} \"$out\"",
                cfg = l.dir.join("curl-config").display(),
                rel = l.dir.join("releases.json").display()
            ),
        );
    };
    let l = gh_lab("gh401", "echo ghp_STALE");
    stub(&l);
    let said = follow_with(&l, API, None);
    assert!(
        !said.contains("HTTP 401") && !said.contains("cannot fetch"),
        "{said}"
    );
    let cfg = fs::read_to_string(l.dir.join("curl-config")).unwrap_or_default();
    assert!(
        cfg.contains("Bearer ghp_STALE"),
        "tried the gh token first: {cfg}"
    );

    let l = gh_lab("gh401given", "echo ghp_FROMGH");
    stub(&l);
    let said = follow_with(&l, API, Some("ghp_GIVEN"));
    assert!(
        said.contains("HTTP 401"),
        "a token the user gave is not dropped: {said}"
    );
}

#[test]
fn a_gh_that_hangs_is_given_up_on_and_follow_goes_on_anonymous() {
    // source: a `gh auth token` that waits (a keyring prompt, a broken credential helper) must not stop follow,
    // which runs from a timer: gh (with a helper that keeps the pipe) gets 10 seconds, then asks anonymous
    let l = gh_lab(
        "ghhang",
        "/usr/bin/sleep 20 & /usr/bin/sleep 60; echo ghp_LATE",
    );
    let started = std::time::Instant::now();
    let said = follow_with(&l, API, None);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(18),
        "follow waited for gh: {:?}",
        started.elapsed()
    );
    let cfg = fs::read_to_string(l.dir.join("curl-config")).unwrap_or_default();
    assert!(l.dir.join("gh-ran").exists(), "gh was asked");
    assert!(!cfg.contains("Bearer"), "{cfg} {said}");
}

/// Where a request for `url` went and what its stdin config carried: one line `URL<TAB>carried-a-token`.
fn token_log_curl(l: &Lab) {
    exe(
        &l.dir.join("stubs/curl"),
        &format!(
            "cfg=$(cat)\nwhile [ $# -gt 0 ]; do case \"$1\" in --output) out=\"$2\"; shift;; http*|file:*) url=\"$1\";; esac; shift; done\ncase \"$cfg\" in *Bearer*) t=token;; *) t=none;; esac\nprintf '%s\\t%s\\n' \"$url\" \"$t\" >> {log}\ncase \"$url\" in\n  *api.github.com/repos/*) cp {rel} \"$out\"; printf 200;;\n  */releases/download/*) src={dir}/release/${{url##*/download/}}; [ -f \"$src\" ] || {{ printf 404; exit 22; }}; cp \"$src\" \"$out\"; printf 200;;\n  file://*) src=${{url#file://}}; [ -f \"$src\" ] || {{ printf 404; exit 22; }}; cp \"$src\" \"$out\"; printf 200;;\n  *) printf 404; exit 22;;\nesac",
            log = l.dir.join("curl-log").display(),
            rel = l.dir.join("releases.json").display(),
            dir = l.dir.display()
        ),
    );
}

#[test]
fn a_token_goes_to_the_releases_list_and_never_to_an_asset_fetch() {
    // source: the asset urls follow builds are github.com/.../releases/download/TAG/FILE (host/src/lib.rs
    // family_tag, ASSET_TARGET); only api.github.com carries the token (fetch_with)
    let l = gh_lab("assetnotoken", "echo ghp_FROMGH");
    releases(&l, &["0.2.1", "0.2.0"]);
    token_log_curl(&l);
    let said = follow_args(&l, &["now"], API, Some("ghp_GIVEN"));
    let log = fs::read_to_string(l.dir.join("curl-log")).unwrap_or_default();
    assert!(
        log.lines()
            .any(|x| x.starts_with("https://api.github.com/") && x.ends_with("\ttoken")),
        "the releases list carries the token: {log}\n{said}"
    );
    let assets: Vec<&str> = log
        .lines()
        .filter(|x| x.contains("/releases/download/"))
        .collect();
    assert!(!assets.is_empty(), "no asset was fetched: {log}\n{said}");
    assert!(
        assets.iter().all(|x| x.ends_with("\tnone")),
        "an asset fetch carried a token: {assets:?}"
    );
}

#[test]
fn an_asset_fetch_with_no_token_given_does_not_ask_gh() {
    let l = gh_lab("assetnogh", "echo ghp_FROMGH");
    token_log_curl(&l);
    // the list comes from a url that is not the API: gh is never asked, and every fetch is anonymous
    follow_with(
        &l,
        "https://github.com/linux-phone-oco/lab-tools/releases.json",
        None,
    );
    let log = fs::read_to_string(l.dir.join("curl-log")).unwrap_or_default();
    assert!(log.contains("\tnone"), "a request was made: {log}");
    assert!(!log.contains("\ttoken"), "{log}");
    assert!(!l.dir.join("gh-ran").exists());
}

#[test]
fn a_url_that_only_looks_like_githubs_api_gets_no_token_and_no_gh() {
    for (n, url) in [
        (
            "dots",
            "https://api.github.com.evil.example/repos/linux-phone-oco/lab-tools/releases",
        ),
        (
            "userinfo",
            "https://api.github.com@evil.example/repos/linux-phone-oco/lab-tools/releases",
        ),
        (
            "plain",
            "http://api.github.com/repos/linux-phone-oco/lab-tools/releases",
        ),
        (
            "nosep",
            "https://api.github.comevil.example/repos/linux-phone-oco/lab-tools/releases",
        ),
    ] {
        let l = gh_lab(&format!("look-{n}"), "echo ghp_FROMGH");
        token_log_curl(&l);
        follow_with(&l, url, Some("ghp_GIVEN"));
        let log = fs::read_to_string(l.dir.join("curl-log")).unwrap_or_default();
        assert!(
            log.lines()
                .any(|x| x.starts_with(url) && x.ends_with("\tnone")),
            "{n}: the request was made without a token: {log}"
        );
        assert!(!log.contains("\ttoken"), "{n}: {log}");
        let l = gh_lab(&format!("lookgh-{n}"), "echo ghp_FROMGH");
        token_log_curl(&l);
        follow_with(&l, url, None);
        assert!(!l.dir.join("gh-ran").exists(), "{n}: gh was asked");
    }
}

#[test]
fn a_family_member_whose_release_is_gone_is_reported_and_skipped_and_the_rest_installs() {
    // lpoco-bringup-v0.2.1 is gone from the releases (no asset) and the build from its tag fails too
    let l = gh_lab("gonemember", "exit 1");
    releases(&l, &["0.2.1", "0.2.0"]);
    token_log_curl(&l);
    exe(
        &l.dir.join("stubs/cargo"),
        &format!(
            "echo \"$0 $@\" >> {}; exit 101",
            l.dir.join("calls").display()
        ),
    );
    fs::remove_dir_all(l.dir.join("release/lpoco-bringup-v0.2.1")).unwrap();
    let o = follow(&l, &["now"]);
    let s = text(&o);
    assert!(
        installed(&l, "lpoco").contains("0.2.1"),
        "the dispatcher is installed: {s}"
    );
    assert!(
        installed(&l, "lpoco-tone").contains("0.2.1"),
        "a member after the gone one is installed: {s}"
    );
    assert!(
        installed(&l, "lpoco-bringup").contains("0.2.0"),
        "the gone one stays as it was: {s}"
    );
    assert!(
        s.contains("lpoco-bringup") && s.contains("skipped") && s.contains("another version"),
        "the skip and the mixed versions are said: {s}"
    );
}

#[test]
fn a_skipped_member_that_the_new_check_names_off_its_pin_does_not_step_the_install_back() {
    // source: the line the real check prints for a family member behind (host/src/main.rs
    // `tool {}: FAIL, pinned {} found {}`); the skipped member is named by follow itself, so that line is not a failure
    let l = gh_lab("gonecheck", "exit 1");
    releases(&l, &["0.2.1", "0.2.0"]);
    // only the new lpoco-host prints the line: the old one, which judges before the install, is clean
    let asset = l
        .dir
        .join(format!("release/lpoco-host-v0.2.1/lpoco-host-{SUFFIX}"));
    fs::write(
        &asset,
        format!("#!/bin/sh\ncase \"$1\" in check) echo \"tool lpoco-bringup: FAIL, pinned 0.2.1 found 0.2.0\"; exit 1;; plan) printf '%s\\n' '{NO_ROOT}';; esac\n"),
    ).unwrap();
    let sum = Command::new("sha256sum").arg(&asset).output().unwrap();
    fs::write(
        asset.with_file_name(format!("lpoco-host-{SUFFIX}.sha256")),
        format!(
            "{}  lpoco-host-{SUFFIX}\n",
            String::from_utf8(sum.stdout)
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap()
        ),
    )
    .unwrap();
    token_log_curl(&l);
    exe(
        &l.dir.join("stubs/cargo"),
        &format!(
            "echo \"$0 $@\" >> {}; exit 101",
            l.dir.join("calls").display()
        ),
    );
    fs::remove_dir_all(l.dir.join("release/lpoco-bringup-v0.2.1")).unwrap();
    let o = follow(&l, &["now"]);
    let s = text(&o);
    assert!(o.status.success(), "{s}");
    assert!(installed(&l, "lpoco").contains("0.2.1"), "{s}");
    assert_eq!(landed_pin(&l).0, "lpoco = \"0.2.1\"", "{s}");
}

#[test]
fn follow_installs_lpoco_layer_with_the_rest_of_the_family() {
    // source: the dispatcher runs `lpoco layer` as the program lpoco-layer found on the host (README, the layer/ row),
    // so the host carries it at the family version like the other lab-side programs
    let l = lab("layerfamily", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    assert!(installed(&l, "lpoco-layer").contains("0.2.0"));
    let o = follow(&l, &["now"]);
    assert!(
        installed(&l, "lpoco-layer").contains("0.2.1"),
        "lpoco-layer was not followed: {}",
        text(&o)
    );
}

/// Publishes `member` at `version` as one more family asset (what and how: publish_release).
fn publish_member(l: &Lab, member: &str, version: &str) {
    let tag = l.dir.join("release").join(format!("{member}-v{version}"));
    fs::create_dir_all(&tag).unwrap();
    let file = tag.join(format!("{member}-{SUFFIX}"));
    fs::write(&file, format!("#!/bin/sh\necho {member} {version} asset\n")).unwrap();
    let sum = Command::new("sha256sum").arg(&file).output().unwrap();
    fs::write(
        tag.join(format!("{member}-{SUFFIX}.sha256")),
        format!(
            "{}  {member}-{SUFFIX}\n",
            String::from_utf8(sum.stdout)
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap()
        ),
    )
    .unwrap();
}

#[test]
fn a_release_that_adds_a_family_member_installs_it_before_the_new_host_judges() {
    // source: the new host lists its family with `lpoco-host family` (one name per line, FAMILY of host/src/lib.rs) and its
    // check names a member it lacks as `tool NAME: FAIL, pinned P found not found` (host/src/main.rs); the running
    // follow's own FAMILY does not hold the new member (2026-11-24, lpoco-layer: follow stepped back every round)
    let l = lab("newmember", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    publish_member(&l, "lpoco-newmember", "0.2.1");
    let asset = l
        .dir
        .join(format!("release/lpoco-host-v0.2.1/lpoco-host-{SUFFIX}"));
    let members: String = FAMILY
        .iter()
        .chain(&["lpoco-newmember"])
        .map(|m| format!("{m}\\n"))
        .collect();
    fs::write(
        &asset,
        format!(
            "#!/bin/sh\ncase \"$1\" in family) printf '{members}';; check) [ -x \"$HOME/.local/bin/lpoco-newmember\" ] || {{ echo \"tool lpoco-newmember: FAIL, pinned 0.2.1 found not found\"; exit 1; }};; plan) printf '%s\\n' '{NO_ROOT}';; esac\n"
        ),
    )
    .unwrap();
    let sum = Command::new("sha256sum").arg(&asset).output().unwrap();
    fs::write(
        asset.with_file_name(format!("lpoco-host-{SUFFIX}.sha256")),
        format!(
            "{}  lpoco-host-{SUFFIX}\n",
            String::from_utf8(sum.stdout)
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap()
        ),
    )
    .unwrap();
    let o = follow(&l, &["now"]);
    let s = text(&o);
    assert!(o.status.success(), "{s}");
    assert!(installed(&l, "lpoco-newmember").contains("0.2.1"), "{s}");
    assert_eq!(landed_pin(&l).0, "lpoco = \"0.2.1\"", "{s}");
}

#[test]
fn a_family_answer_that_is_not_one_word_names_is_refused_and_installs_nothing_extra() {
    // an answer is data from a program we just installed: a name is `lpoco-` and letters, digits, hyphens
    let l = lab("badmember", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    let asset = l
        .dir
        .join(format!("release/lpoco-host-v0.2.1/lpoco-host-{SUFFIX}"));
    fs::write(
        &asset,
        format!(
            "#!/bin/sh\ncase \"$1\" in family) printf 'lpoco-x/../../evil\\nlpoco-ok name\\nnot-lpoco\\n';; plan) printf '%s\\n' '{NO_ROOT}';; esac\n"
        ),
    )
    .unwrap();
    let sum = Command::new("sha256sum").arg(&asset).output().unwrap();
    fs::write(
        asset.with_file_name(format!("lpoco-host-{SUFFIX}.sha256")),
        format!(
            "{}  lpoco-host-{SUFFIX}\n",
            String::from_utf8(sum.stdout)
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap()
        ),
    )
    .unwrap();
    let o = follow(&l, &["now"]);
    let s = text(&o);
    assert!(
        !l.dir.join("home/.local/evil").exists() && !installed(&l, "evil").contains("asset"),
        "{s}"
    );
    assert!(
        s.contains("family") && s.contains("not a member name"),
        "{s}"
    );
}

#[test]
fn a_member_the_new_host_added_is_taken_away_again_when_the_install_steps_back() {
    let l = lab("newmemberback", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    publish_member(&l, "lpoco-newmember", "0.2.1");
    let asset = l
        .dir
        .join(format!("release/lpoco-host-v0.2.1/lpoco-host-{SUFFIX}"));
    let members: String = FAMILY
        .iter()
        .chain(&["lpoco-newmember"])
        .map(|m| format!("{m}\\n"))
        .collect();
    // the new host's check fails on something else: the install steps back
    fs::write(
        &asset,
        format!(
            "#!/bin/sh\ncase \"$1\" in family) printf '{members}';; check) echo \"udev: FAIL, broken\"; exit 1;; plan) printf '%s\\n' '{NO_ROOT}';; esac\n"
        ),
    )
    .unwrap();
    let sum = Command::new("sha256sum").arg(&asset).output().unwrap();
    fs::write(
        asset.with_file_name(format!("lpoco-host-{SUFFIX}.sha256")),
        format!(
            "{}  lpoco-host-{SUFFIX}\n",
            String::from_utf8(sum.stdout)
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap()
        ),
    )
    .unwrap();
    let o = follow(&l, &["now"]);
    let s = text(&o);
    assert!(s.contains("are back"), "{s}");
    assert!(
        !l.dir.join("home/.local/bin/lpoco-newmember").exists(),
        "the added member stayed: {s}"
    );
    assert!(installed(&l, "lpoco").contains("0.2.0"), "{s}");
}

/// follow with the family's tags read from git (LPOCO_TAGS) and no releases list given: the default is the API.
fn follow_by_tags(l: &Lab, tags: &str, releases: Option<&str>) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_lpoco-host"));
    c.arg("follow")
        .arg("now")
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
        .env("LPOCO_TAGS", tags);
    if let Some(r) = releases {
        c.env("LPOCO_RELEASES", r);
    }
    c.output().unwrap()
}

/// A git that answers `ls-remote` with what `git ls-remote --tags --refs URL` prints (git-ls-remote(1): the object name,
/// a tab, the ref) from the file `tags` beside the lab, or fails when there is none, and is the real git otherwise.
fn git_with_tags(l: &Lab) {
    let real = which("git");
    // stubs/git is a link to the real one: write a new file, not through the link
    let _ = fs::remove_file(l.dir.join("stubs/git"));
    exe(
        &l.dir.join("stubs/git"),
        &format!(
            "echo \"git $@\" >> {calls}\ncase \"$1\" in ls-remote) [ -f {tags} ] || exit 128; cat {tags};; *) exec {real} \"$@\";; esac",
            calls = l.dir.join("calls").display(),
            tags = l.dir.join("tags").display(),
            real = real.display()
        ),
    );
}

const TAGS_0_2_1: &str = "1111111111111111111111111111111111111111\trefs/tags/lpoco-bringup-v0.2.1\n2222222222222222222222222222222222222222\trefs/tags/lpoco-v0.2.0\n3333333333333333333333333333333333333333\trefs/tags/lpoco-v0.2.1\n4444444444444444444444444444444444444444\trefs/tags/lpoco-manifest-v0.9.0\n";

#[test]
fn a_newer_tag_from_git_installs_with_no_api_call() {
    let l = lab("tags-install", "");
    install_old(&l);
    git_with_tags(&l);
    fs::write(l.dir.join("tags"), TAGS_0_2_1).unwrap();
    token_log_curl(&l);
    let o = follow_by_tags(&l, "https://example.test/lab-tools.git", None);
    let s = text(&o);
    assert!(o.status.success(), "{s}");
    assert!(installed(&l, "lpoco").contains("0.2.1"), "{s}");
    let curl = fs::read_to_string(l.dir.join("curl-log")).unwrap_or_default();
    assert!(
        !curl.contains("api.github.com"),
        "the API was asked: {curl}"
    );
    let git = calls(&l);
    assert!(
        git.contains("git ls-remote --tags --refs https://example.test/lab-tools.git"),
        "{git}"
    );
    assert_eq!(landed_pin(&l).0, "lpoco = \"0.2.1\"", "{s}");
}

#[test]
fn a_tag_whose_asset_sum_is_wrong_is_refused_and_the_old_programs_stay() {
    let l = lab("tags-badsum", "");
    install_old(&l);
    git_with_tags(&l);
    fs::write(l.dir.join("tags"), TAGS_0_2_1).unwrap();
    token_log_curl(&l);
    fs::write(
        l.dir
            .join(format!("release/lpoco-v0.2.1/lpoco-{SUFFIX}.sha256")),
        format!("{}  lpoco-{SUFFIX}\n", "0".repeat(64)),
    )
    .unwrap();
    let o = follow_by_tags(&l, "https://example.test/lab-tools.git", None);
    let s = text(&o);
    assert!(s.contains("not installed"), "{s}");
    assert!(installed(&l, "lpoco").contains("0.2.0"), "{s}");
    assert_ne!(landed_pin(&l).0, "lpoco = \"0.2.1\"", "{s}");
}

#[test]
fn when_git_cannot_say_the_releases_list_is_the_fallback_and_is_said() {
    let l = lab("tags-fallback", "");
    releases(&l, &["0.2.1", "0.2.0"]);
    install_old(&l);
    git_with_tags(&l); // no tags file: ls-remote fails
    token_log_curl(&l);
    let o = follow_by_tags(
        &l,
        "https://example.test/lab-tools.git",
        Some(&format!("file://{}/releases.json", l.dir.display())),
    );
    let s = text(&o);
    assert!(o.status.success(), "{s}");
    assert!(
        s.contains("git ls-remote") && s.contains("releases list"),
        "{s}"
    );
    assert!(installed(&l, "lpoco").contains("0.2.1"), "{s}");
}
