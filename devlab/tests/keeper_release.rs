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
}

impl Scratch {
    fn root(&self) -> PathBuf {
        self.0.join("home/.local/state/devlab")
    }
    /// A lease whose holder was last renewed `secs` ago.
    fn old_lease(&self, codename: &str, holder: &str, secs: u64) {
        self.lease(codename, holder);
        let f = std::fs::File::options()
            .write(true)
            .open(self.root().join(codename).join("keeper.lease/holder"))
            .unwrap();
        f.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(secs))
            .unwrap();
    }
}

#[test]
fn a_release_when_no_lease_is_held_anywhere_is_done_and_so_is_a_second_one() {
    let s = Scratch::new("nobody");
    let o = devlab(&s, &["keeper", "release", "walleye-keeper"]);
    assert!(o.status.success(), "{}", said(&o));
    assert!(said(&o).contains("nobody held a lease"), "{}", said(&o));
    s.lease("google-walleye", "walleye-keeper");
    assert!(devlab(&s, &["keeper", "release", "walleye-keeper"])
        .status
        .success());
    let again = devlab(&s, &["keeper", "release", "walleye-keeper"]);
    assert!(again.status.success(), "{}", said(&again));
    assert!(
        said(&again).contains("nobody held a lease"),
        "{}",
        said(&again)
    );
}

#[test]
fn when_others_hold_leases_it_fails_on_stderr_naming_each_and_marking_a_lapsed_one() {
    let s = Scratch::new("others");
    s.lease("google-walleye", "walleye-keeper");
    s.old_lease("xiaomi-surya", "surya-keeper", 3 * 3600);
    let o = devlab(&s, &["keeper", "release", "nobody-keeper"]);
    assert_eq!(o.status.code(), Some(1), "{}", said(&o));
    let err = String::from_utf8_lossy(&o.stderr).to_string();
    assert!(err.contains("google-walleye: walleye-keeper"), "{err}");
    assert!(err.contains("xiaomi-surya: surya-keeper (lapsed"), "{err}");
    assert!(!err.contains("walleye-keeper (lapsed"), "{err}");
    assert!(s.held("google-walleye") && s.held("xiaomi-surya"));
}

#[test]
fn a_name_that_is_only_the_start_of_a_holder_releases_nothing() {
    let s = Scratch::new("prefix");
    s.lease("google-walleye", "walleye-keeper");
    let o = devlab(&s, &["keeper", "release", "walleye"]);
    assert!(!o.status.success(), "{}", said(&o));
    assert!(s.held("google-walleye"), "a prefix released a lease");
}

#[test]
fn a_lease_with_a_blank_holder_is_skipped_not_released_and_not_listed() {
    let s = Scratch::new("blank");
    s.lease("google-walleye", "");
    let o = devlab(&s, &["keeper", "release", "walleye-keeper"]);
    assert!(o.status.success(), "{}", said(&o));
    assert!(s.held("google-walleye"), "a lease half written was removed");
}

#[test]
fn a_lease_that_cannot_be_given_back_is_reported_beside_the_one_that_was() {
    use std::os::unix::fs::PermissionsExt;
    if Command::new("id")
        .arg("-u")
        .output()
        .is_ok_and(|o| o.stdout == b"0\n")
    {
        return; // root ignores a read-only folder
    }
    let s = Scratch::new("partial");
    s.lease("google-walleye", "walleye-keeper");
    s.lease("xiaomi-surya", "walleye-keeper");
    let stuck = s.root().join("xiaomi-surya");
    std::fs::set_permissions(&stuck, std::fs::Permissions::from_mode(0o555)).unwrap();
    let o = devlab(&s, &["keeper", "release", "walleye-keeper"]);
    std::fs::set_permissions(&stuck, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(o.status.code(), Some(1), "{}", said(&o));
    let err = said(&o);
    assert!(err.contains("released: google-walleye"), "{err}");
    assert!(err.contains("failed: xiaomi-surya"), "{err}");
    assert!(!s.held("google-walleye") && s.held("xiaomi-surya"));
}

#[test]
fn a_state_root_that_cannot_be_read_is_an_error_not_nothing_held() {
    let s = Scratch::new("unreadable");
    std::fs::create_dir_all(s.root().parent().unwrap()).unwrap();
    std::fs::write(s.root(), "not a folder").unwrap();
    let o = devlab(&s, &["keeper", "release", "walleye-keeper"]);
    assert_eq!(o.status.code(), Some(1), "{}", said(&o));
    assert!(said(&o).contains("cannot read the leases"), "{}", said(&o));
}

#[test]
fn the_help_says_release_ignores_device() {
    let s = Scratch::new("help");
    let o = devlab(&s, &["keeper", "release", "--help"]);
    assert!(said(&o).contains("ignores --device"), "{}", said(&o));
}
