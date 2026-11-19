// SPDX-License-Identifier: MIT OR Apache-2.0
//! lpoco-lab and devlab are one program under two names (backlog lpoco-rename-lab-tone, proof
//! lpoco-lab-and-devlab-are-one-program): the same code answers to both, and `lpoco list` shows the
//! family under one version.

use std::path::PathBuf;
use std::process::Command;

fn out(bin: &str, args: &[&str]) -> String {
    let o = Command::new(bin).args(args).output().unwrap();
    assert!(o.status.success(), "{bin} {args:?}: {:?}", o);
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn lpoco_lab_and_devlab_print_the_same() {
    let lab = env!("CARGO_BIN_EXE_lpoco-lab");
    let old = env!("CARGO_BIN_EXE_devlab");
    for args in [&["phone", "find", "--help"][..], &["--version"][..]] {
        assert_eq!(out(lab, args), out(old, args), "{args:?}");
    }
    assert!(out(lab, &["--version"])
        .trim()
        .ends_with(env!("CARGO_PKG_VERSION")));
}

#[test]
fn lpoco_list_shows_the_family_under_one_version() {
    // the family's binaries are built beside each other in the workspace's target folder
    let dir: PathBuf = PathBuf::from(env!("CARGO_BIN_EXE_lpoco-lab"))
        .parent()
        .unwrap()
        .to_path_buf();
    // a clean `cargo test` builds only what each test needs: build every program of the family first
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let built = Command::new(cargo)
        .args(["build", "--workspace", "--bins"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()
        .unwrap();
    assert!(built.success(), "cargo build --workspace --bins failed");
    let lpoco = dir.join("lpoco");
    assert!(
        lpoco.exists(),
        "build the workspace first (cargo test --workspace): {} is missing",
        lpoco.display()
    );
    let listing = Command::new(&lpoco)
        .arg("list")
        .env("PATH", &dir)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&listing.stdout).into_owned();
    let v = env!("CARGO_PKG_VERSION");
    for name in ["lab", "health", "tone", "bringup"] {
        let line = text
            .lines()
            .find(|l| l.split_whitespace().next() == Some(name))
            .unwrap_or_else(|| panic!("no {name} in:\n{text}"));
        assert_eq!(line.split_whitespace().nth(1), Some(v), "{line}");
        assert!(!line.contains("differs"), "{line}");
    }
}
