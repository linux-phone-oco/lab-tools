// SPDX-License-Identifier: MIT OR Apache-2.0
//! `keeper release NAME` finds the lease by its name from any folder: a keeper whose worktree is gone (so no
//! device.toml above it) still gives its lease back, and an unknown name exits non-zero naming the leases held.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Scratch(PathBuf);
impl Scratch {
    fn new(name: &str) -> Scratch {
        let d = std::env::temp_dir().join(format!(
            "devlab-keeper-release-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("home")).unwrap();
        std::fs::create_dir_all(d.join("nowhere")).unwrap();
        Scratch(d)
    }
    fn lease(&self, codename: &str, holder: &str) {
        let l = self
            .0
            .join("home/.local/state/devlab")
            .join(codename)
            .join("keeper.lease");
        std::fs::create_dir_all(&l).unwrap();
        std::fs::write(l.join("holder"), format!("{holder}\n")).unwrap();
    }
    fn held(&self, codename: &str) -> bool {
        self.0
            .join("home/.local/state/devlab")
            .join(codename)
            .join("keeper.lease")
            .exists()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn devlab(s: &Scratch, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lpoco-lab"))
        .args(args)
        .current_dir(Path::new(&s.0).join("nowhere"))
        .env_clear()
        .env("HOME", s.0.join("home"))
        .env("PATH", "/usr/bin:/bin")
        .output()
        .unwrap()
}

fn said(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

#[test]
fn a_keeper_whose_worktree_is_gone_gives_its_lease_back_by_name_from_any_folder() {
    let s = Scratch::new("byname");
    s.lease("google-walleye", "walleye-keeper");
    s.lease("xiaomi-surya", "surya-keeper");
    let o = devlab(&s, &["keeper", "release", "walleye-keeper"]);
    assert!(o.status.success(), "{}", said(&o));
    assert!(
        said(&o).contains("walleye-keeper") && said(&o).contains("google-walleye"),
        "{}",
        said(&o)
    );
    assert!(!s.held("google-walleye"), "the lease is still there");
    assert!(s.held("xiaomi-surya"), "another keeper's lease was taken");
}

#[test]
fn an_unknown_name_exits_non_zero_naming_the_leases_held_and_changes_nothing() {
    let s = Scratch::new("unknown");
    s.lease("google-walleye", "walleye-keeper");
    let o = devlab(&s, &["keeper", "release", "nobody-keeper"]);
    assert!(!o.status.success(), "{}", said(&o));
    assert!(
        said(&o).contains("google-walleye") && said(&o).contains("walleye-keeper"),
        "{}",
        said(&o)
    );
    assert!(s.held("google-walleye"));
    let none = Scratch::new("none");
    let o = devlab(&none, &["keeper", "release", "nobody-keeper"]);
    assert!(
        !o.status.success() && said(&o).contains("no lease is held"),
        "{}",
        said(&o)
    );
}
