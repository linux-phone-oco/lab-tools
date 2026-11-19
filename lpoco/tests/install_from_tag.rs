// SPDX-License-Identifier: MIT OR Apache-2.0
//! A lab host installs the family from the release tags with cargo, nothing else (proof
//! bringup-installs-from-tag, decision 2026-11-17-bringup-installs-from-tag): into a clean root,
//! `lpoco` from its tag and `lpoco-bringup` from its own (release-plz tags per package), then the
//! installed `lpoco` reports the tag's version, lists bringup, and bringup refuses a wrong pin.
//! Needs the network (GitHub for the tags, crates.io); it compiles from source, so it is slow.

use std::fs;
use std::path::{Path, PathBuf};

const URL: &str = "https://github.com/linux-phone-oco/lab-tools";
use std::process::{Command, Output};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn run(c: &mut Command) -> Output {
    let o = c.output().unwrap();
    assert!(
        o.status.success(),
        "{:?} failed: {}",
        c,
        String::from_utf8_lossy(&o.stderr)
    );
    o
}

/// The newest version that has both its `lpoco-v` and `lpoco-bringup-v` tags on GitHub (a CI checkout has no tags).
fn newest_released_version() -> String {
    let tags = run(Command::new("git").args(["ls-remote", "--tags", "--refs", URL]));
    let tags = String::from_utf8_lossy(&tags.stdout).into_owned();
    let has = |t: &str| tags.lines().any(|l| l.ends_with(&format!("refs/tags/{t}")));
    let mut vs: Vec<(Vec<u32>, String)> = tags
        .lines()
        .filter_map(|l| {
            l.rsplit("refs/tags/lpoco-v")
                .next()
                .filter(|_| l.contains("refs/tags/lpoco-v"))
        })
        .map(|v| {
            (
                v.split('.').filter_map(|n| n.parse().ok()).collect(),
                v.to_string(),
            )
        })
        .collect();
    vs.sort();
    vs.into_iter()
        .rev()
        .map(|(_, v)| v)
        .find(|v| has(&format!("lpoco-bringup-v{v}")))
        .expect("no version has both an lpoco-v and an lpoco-bringup-v tag")
}

#[test]
fn bringup_installs_from_tag() {
    let v = newest_released_version();
    let root = std::env::temp_dir().join(format!("lpoco-install-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    for (pkg, tag) in [
        ("lpoco", format!("lpoco-v{v}")),
        ("lpoco-bringup", format!("lpoco-bringup-v{v}")),
    ] {
        run(Command::new("cargo")
            .args(["install", "--locked", "--git", URL, "--tag", &tag, "--root"])
            .arg(&root)
            .arg(pkg));
    }
    let bin = root.join("bin");
    let lpoco = bin.join("lpoco");
    let out = run(Command::new(&lpoco).arg("--version"));
    assert!(
        String::from_utf8_lossy(&out.stdout).contains(&v),
        "lpoco --version is not {v}"
    );
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let list = run(Command::new(&lpoco).arg("list").env("PATH", &path));
    let list = String::from_utf8_lossy(&list.stdout).into_owned();
    assert!(list.contains("bringup") && list.contains(&v), "{list}");

    // the pin check, through the installed lpoco: a manifest asking for another family version
    let fx = repo().join("bringup/tests/fixtures");
    let dev = root.join("device.toml");
    let mut text = fs::read_to_string(fx.join("device.toml")).unwrap();
    text.push_str("\n[tools]\nlpoco = \"99.0\"\n");
    fs::write(&dev, text).unwrap();
    let o = Command::new(&lpoco)
        .args(["bringup", "image", "check"])
        .arg(fx.join("kernel"))
        .arg("--device")
        .arg(&dev)
        .env("PATH", &path)
        .output()
        .unwrap();
    assert_eq!(
        o.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let _ = fs::remove_dir_all(&root);
}
