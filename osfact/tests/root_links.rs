// SPDX-License-Identifier: MIT OR Apache-2.0
//! The proof of item osfact-root-applies-package-scripts: a root made without running the packages' install scripts
//! gets the links such a script would make, from the list the package ships, and a root lacking a required path is
//! refused naming it.
//!
//! Where each expected value comes from (a check names its source):
//! tests/fixtures/apk/busybox-paths: /etc/busybox-paths.d/busybox as the package installed it in a real tree, read
//!   2026-11-28 from the root osfact built (304 lines, `sbin/init` at line 102, `bin/sh` at 229);
//! the link target `/bin/busybox` and the command `busybox --install -s`: that package's post-install script, read from
//!   the same tree's lib/apk/db/scripts.tar.gz on 2026-11-28;
//! the profile: tests/fixtures/distros/alpine.toml (its `link_list` and `required_paths`).
//! The ignored test builds the real root: `cargo test -p osfact --test root_links -- --ignored`.

use osfact::root_links::{apply, require, LinkList};
use std::path::{Path, PathBuf};

const LIST: &str = include_str!("fixtures/apk/busybox-paths");

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("osfact-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

fn tree_with_the_list(dir: &Path) {
    std::fs::create_dir_all(dir.join("etc/busybox-paths.d")).expect("dirs");
    std::fs::write(dir.join("etc/busybox-paths.d/busybox"), LIST).expect("list");
}

fn spec() -> LinkList {
    LinkList {
        list: "etc/busybox-paths.d/busybox".into(),
        target: "/bin/busybox".into(),
    }
}

#[test]
fn every_applet_the_package_lists_becomes_a_link_to_the_target() {
    let tree = scratch("links");
    tree_with_the_list(&tree);
    let made = apply(&tree, &spec()).expect("links made");
    let listed: Vec<&str> = LIST.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(listed.len(), 304, "the captured list changed");
    assert_eq!(made, listed.len());
    for path in listed {
        let link = std::fs::read_link(tree.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert_eq!(link, Path::new("/bin/busybox"), "{path}");
    }
    let _ = std::fs::remove_dir_all(&tree);
}

#[test]
fn a_path_the_install_already_made_is_left_alone() {
    let tree = scratch("links-kept");
    tree_with_the_list(&tree);
    std::fs::create_dir_all(tree.join("bin")).expect("bin");
    std::fs::write(tree.join("bin/sh"), "a real file").expect("file");
    apply(&tree, &spec()).expect("links made");
    assert_eq!(
        std::fs::read_to_string(tree.join("bin/sh")).expect("kept"),
        "a real file"
    );
    let _ = std::fs::remove_dir_all(&tree);
}

#[test]
fn a_root_without_its_init_is_refused_naming_it() {
    let tree = scratch("links-required");
    tree_with_the_list(&tree);
    let refused = require(&tree, &["sbin/init".to_string()]).expect_err("no init yet");
    assert!(refused.contains("/sbin/init"), "{refused}");
    apply(&tree, &spec()).expect("links made");
    require(&tree, &["sbin/init".to_string()]).expect("init is there now");
    let _ = std::fs::remove_dir_all(&tree);
}

#[test]
fn a_missing_list_is_refused_naming_it() {
    let tree = scratch("links-nolist");
    let refused = apply(&tree, &spec()).expect_err("no list");
    assert!(refused.contains("etc/busybox-paths.d/busybox"), "{refused}");
    let _ = std::fs::remove_dir_all(&tree);
}
