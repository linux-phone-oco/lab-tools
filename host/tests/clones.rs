// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco host clones` (backlog host-clones-fresh): a user timer fast-forwards every shared clone under the declared
//! folders that is clean and on main, and leaves a dirty, detached, off-main or diverged one alone, naming it. The
//! clones are real git repositories against a real bare origin; systemctl is a stub that records its calls.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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

struct Lab {
    dir: PathBuf,
}

impl Lab {
    fn shared(&self) -> PathBuf {
        self.dir.join("home/Projects/shared")
    }
    fn origin(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.git"))
    }
    /// Another session lands a commit on the origin's main.
    fn land(&self, name: &str, file: &str) {
        let w = self.dir.join(format!("work-{name}"));
        if !w.exists() {
            git(
                &self.dir,
                &[
                    "clone",
                    "-q",
                    self.origin(name).to_str().unwrap(),
                    w.to_str().unwrap(),
                ],
            );
        }
        fs::write(w.join(file), file).unwrap();
        git(&w, &["add", "."]);
        git(&w, &["commit", "-q", "-m", file]);
        git(&w, &["push", "-q", "origin", "HEAD:main"]);
    }
    fn head(&self, name: &str) -> String {
        git(&self.shared().join(name), &["rev-parse", "HEAD"])
    }
    fn origin_main(&self, name: &str) -> String {
        git(&self.origin(name), &["rev-parse", "main"])
    }
    fn host(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_lpoco-host"))
            .args(args)
            .env_clear()
            .env("HOME", self.dir.join("home"))
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.dir.join("stubs").display()),
            )
            .env(
                "LPOCO_LAB",
                self.dir.join("home/Projects/lab-machine/lab.toml"),
            )
            .env("LPOCO_FS_ROOT", self.dir.join("fs"))
            .output()
            .unwrap()
    }
    fn calls(&self) -> String {
        fs::read_to_string(self.dir.join("calls")).unwrap_or_default()
    }
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

/// Seven clones of seven origins under ~/Projects/shared (plus a worktree and a plain folder, which are not clones):
/// fresh, behind, dirty, off main, detached, diverged (commons places), private (behind, no AGENTS.md) - all but `fresh` have a commit waiting on origin.
fn lab(name: &str, clones_toml: &str) -> Lab {
    let dir = std::env::temp_dir().join(format!("lpoco-clones-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let l = Lab { dir: dir.clone() };
    fs::create_dir_all(l.shared()).unwrap();
    fs::create_dir_all(dir.join("stubs")).unwrap();
    let stub = dir.join("stubs/systemctl");
    fs::write(
        &stub,
        format!(
            "#!/bin/sh\necho \"$0 $@\" >> {}\n",
            dir.join("calls").display()
        ),
    )
    .unwrap();
    fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
    for n in [
        "fresh", "behind", "dirty", "offmain", "detached", "diverged", "private",
    ] {
        git(
            &dir,
            &[
                "init",
                "-q",
                "--bare",
                "-b",
                "main",
                l.origin(n).to_str().unwrap(),
            ],
        );
        // a commons place has an AGENTS.md at its top; "private" is an author's own repository without one
        if n != "private" {
            l.land(n, "AGENTS.md");
        }
        l.land(n, "first");
        git(
            &dir,
            &[
                "clone",
                "-q",
                l.origin(n).to_str().unwrap(),
                l.shared().join(n).to_str().unwrap(),
            ],
        );
    }
    for n in [
        "behind", "dirty", "offmain", "detached", "diverged", "private",
    ] {
        l.land(n, "second");
    }
    fs::write(l.shared().join("dirty/first"), "edited").unwrap();
    git(
        &l.shared().join("offmain"),
        &["checkout", "-q", "-b", "topic"],
    );
    git(
        &l.shared().join("detached"),
        &["checkout", "-q", "--detach"],
    );
    let d = l.shared().join("diverged");
    fs::write(d.join("mine"), "mine").unwrap();
    git(&d, &["add", "."]);
    git(&d, &["commit", "-q", "-m", "mine"]);
    // a worktree of one clone and a plain folder: neither is a shared clone
    git(
        &l.shared().join("fresh"),
        &[
            "worktree",
            "add",
            "-q",
            "--detach",
            l.shared().join("wt-x").to_str().unwrap(),
        ],
    );
    fs::create_dir_all(l.shared().join("notes")).unwrap();
    // the lab's own declaration
    let lab_dir = dir.join("home/Projects/lab-machine");
    fs::create_dir_all(&lab_dir).unwrap();
    fs::write(
        lab_dir.join("lab.toml"),
        format!(
            "devices = \"{}/none/[x]*/device.toml\"\n{clones_toml}",
            dir.display()
        ),
    )
    .unwrap();
    l
}

const DECLARED: &str = "[clones]\nfolders = [\"~/Projects/shared\"]\nevery = \"15min\"\n";

#[test]
fn a_clean_clone_on_main_is_fast_forwarded_and_the_others_are_left_alone_and_named() {
    let l = lab("run", DECLARED);
    let (dirty, off, det, div) = (
        l.head("dirty"),
        l.head("offmain"),
        l.head("detached"),
        l.head("diverged"),
    );
    let o = l.host(&["clones", "now"]);
    let s = text(&o);
    assert_eq!(l.head("behind"), l.origin_main("behind"), "{s}");
    assert!(s.contains("behind: updated 1 commit"), "{s}");
    assert!(s.contains("fresh: up to date"), "{s}");
    for (n, before, why) in [
        ("dirty", dirty, "uncommitted changes"),
        ("offmain", off, "on topic, not main"),
        ("detached", det, "detached"),
        ("diverged", div, "diverged"),
    ] {
        assert_eq!(l.head(n), before, "{n} moved: {s}");
        assert!(s.contains(&format!("{n}: left alone ({why}")), "{n}: {s}");
    }
    assert!(!s.contains("wt-x") && !s.contains("notes"), "{s}");
    assert_eq!(
        o.status.code(),
        Some(0),
        "a clone left alone is named, not a failed unit: {s}"
    );
}

#[test]
fn without_now_nothing_moves_and_it_says_what_it_would_do() {
    let l = lab("dry", DECLARED);
    let before = l.head("behind");
    let s = text(&l.host(&["clones"]));
    assert_eq!(l.head("behind"), before, "{s}");
    assert!(s.contains("behind: would update 1 commit"), "{s}");
}

#[test]
fn a_clone_whose_origin_is_gone_is_named_and_the_rest_still_run() {
    let l = lab("gone", DECLARED);
    fs::remove_dir_all(l.origin("dirty")).unwrap();
    fs::remove_dir_all(l.origin("fresh")).unwrap();
    let s = text(&l.host(&["clones", "now"]));
    assert!(s.contains("fresh: left alone (fetch failed"), "{s}");
    assert_eq!(l.head("behind"), l.origin_main("behind"), "{s}");
}

#[test]
fn nothing_declared_is_said_and_nothing_is_touched() {
    let l = lab("none", "");
    let before = l.head("behind");
    let o = l.host(&["clones", "now"]);
    assert_eq!(l.head("behind"), before);
    assert!(text(&o).contains("no [clones] declared"), "{}", text(&o));
    assert_eq!(o.status.code(), Some(0));
}

#[test]
fn a_declared_clones_job_is_planned_as_a_user_timer_and_prepare_writes_and_enables_it() {
    let l = lab("timer", DECLARED);
    let plan = text(&l.host(&["plan"]));
    assert!(
        plan.contains("lpoco-host-clones.timer") && plan.contains("OnUnitActiveSec=15min"),
        "{plan}"
    );
    let units = l.dir.join("home/.config/systemd/user");
    assert!(!units.join("lpoco-host-clones.timer").exists());
    let prep = text(&l.host(&["prepare", "now"]));
    let service = fs::read_to_string(units.join("lpoco-host-clones.service")).unwrap_or_default();
    assert!(
        service.contains("ExecStart=%h/.local/bin/lpoco-host clones now"),
        "{service}\n{prep}"
    );
    assert!(
        fs::read_to_string(units.join("lpoco-host-clones.timer"))
            .unwrap_or_default()
            .contains("OnUnitActiveSec=15min"),
        "{prep}"
    );
    assert!(
        l.calls()
            .contains("systemctl --user enable --now lpoco-host-clones.timer"),
        "{}\n{prep}",
        l.calls()
    );
}

#[test]
fn a_folder_or_interval_that_would_reach_a_unit_file_is_refused() {
    for bad in [
        "[clones]\nfolders = [\"~/a\"]\nevery = \"1h\\nExecStart=x\"\n",
        "[clones]\nfolders = []\nevery = \"1h\"\n",
        "[clones]\nfolders = [\"/etc\"]\nevery = \"1h\"\n",
    ] {
        let l = lab("bad", bad);
        let o = l.host(&["clones", "now"]);
        assert_eq!(o.status.code(), Some(2), "{bad}: {}", text(&o));
        assert!(text(&o).contains("[clones]"), "{bad}: {}", text(&o));
    }
}

#[test]
fn a_clone_without_a_fetch_refspec_is_still_brought_to_origin_main() {
    let l = lab("refspec", DECLARED);
    let b = l.shared().join("behind");
    git(&b, &["config", "--unset-all", "remote.origin.fetch"]);
    let s = text(&l.host(&["clones", "now"]));
    assert_eq!(l.head("behind"), l.origin_main("behind"), "{s}");
    assert!(s.contains("behind: updated 1 commit"), "{s}");
}

#[test]
fn an_untracked_file_in_the_way_stops_the_fast_forward_and_is_left_intact() {
    let l = lab("untracked", DECLARED);
    let b = l.shared().join("behind");
    fs::write(b.join("second"), "mine, not yet added").unwrap();
    let before = l.head("behind");
    let s = text(&l.host(&["clones", "now"]));
    assert_eq!(l.head("behind"), before, "{s}");
    assert_eq!(
        fs::read_to_string(b.join("second")).unwrap(),
        "mine, not yet added"
    );
    assert!(s.contains("behind: left alone (fast-forward failed"), "{s}");
}

#[test]
fn a_declared_folder_that_is_not_there_is_named_and_the_unit_fails() {
    let l = lab(
        "missing",
        "[clones]\nfolders = [\"~/Projects/shared\", \"~/Projects/typo\"]\nevery = \"15min\"\n",
    );
    let o = l.host(&["clones", "now"]);
    let s = text(&o);
    assert!(s.contains("typo: cannot be read"), "{s}");
    assert_eq!(
        l.head("behind"),
        l.origin_main("behind"),
        "the other folder still runs: {s}"
    );
    assert_eq!(o.status.code(), Some(1), "{s}");
}

#[test]
fn a_run_whose_every_fetch_fails_fails_the_unit() {
    let l = lab("nofetch", DECLARED);
    for n in [
        "fresh", "behind", "dirty", "offmain", "detached", "diverged",
    ] {
        fs::remove_dir_all(l.origin(n)).unwrap();
    }
    let o = l.host(&["clones", "now"]);
    let s = text(&o);
    assert!(s.contains("behind: left alone (fetch failed"), "{s}");
    assert_eq!(o.status.code(), Some(1), "{s}");
}

#[test]
fn a_clone_without_an_agents_md_is_not_a_commons_place_and_is_left_where_it_is() {
    let l = lab("private", DECLARED);
    let before = l.head("private");
    // behind and clean on main: only the missing AGENTS.md keeps it still
    for args in [&["clones", "now"][..], &["clones"][..]] {
        let s = text(&l.host(args));
        assert_eq!(l.head("private"), before, "{s}");
        assert!(s.contains("private: not a commons place, left"), "{s}");
        assert!(
            !s.contains("private: updated") && !s.contains("private: would update"),
            "{s}"
        );
    }
    // the commons place beside it still moves
    assert_eq!(l.head("behind"), l.origin_main("behind"));
}
