// SPDX-License-Identifier: MIT OR Apache-2.0
//! The boundary of osfact (decision 2026-11-27-osfact-own-rust-in-lab-tools): it depends on no lpoco crate, in
//! any section - normal, build or dev - so it builds and its tests run with no lpoco crate in its tree, and it can
//! leave for its own repository by moving its folder. lab-tools depends on osfact, never the reverse.

use std::process::Command;

#[test]
fn no_lpoco_crate_in_the_tree_of_osfact() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo)
        .args([
            "tree",
            "--package",
            "osfact",
            "--edges",
            "normal,build,dev",
            "--prefix",
            "none",
            "--format",
            "{p}",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo tree runs");
    assert!(
        out.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let tree = String::from_utf8_lossy(&out.stdout);
    assert!(
        tree.lines().any(|l| l.starts_with("osfact ")),
        "cargo tree did not list osfact itself:\n{tree}"
    );
    let lpoco: Vec<&str> = tree
        .lines()
        .filter(|l| l.starts_with("lpoco") || l.starts_with("devlab"))
        .collect();
    assert!(
        lpoco.is_empty(),
        "osfact depends on lpoco crates: {lpoco:?}"
    );
}
