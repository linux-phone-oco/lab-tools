// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco-layer pack` makes the same bytes from the same layer (proof: layer-pack-reproducible): nfpm stamps the
//! time into its packages, so pack gives it SOURCE_DATE_EPOCH - the one given, else the layer's commit time.
//! Source for the variable: https://reproducible-builds.org/specs/source-date-epoch/ (nfpm reads it; two packs of
//! walleye's layer with it set and two seconds apart gave equal sha256 for apk and deb, 2026-11-24).

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const DEVICE: &str = r#"
[layer]
name = "xiaomi-surya"

[[layer.file]]
from = "ucm/HiFi.conf"
to = "/usr/share/alsa/ucm2/Xiaomi/surya/HiFi.conf"
kind = "data"
"#;

/// A layer folder; with `commit_at` it is a git repository whose one commit was made at that time (git's date format).
fn layer(tag: &str, commit_at: Option<&str>) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lpoco-layer-repro-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("ucm")).unwrap();
    fs::write(dir.join("ucm/HiFi.conf"), "Syntax 4\n").unwrap();
    fs::write(dir.join("device.toml"), DEVICE).unwrap();
    if let Some(at) = commit_at {
        let git = |args: &[&str]| {
            let o = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .env("GIT_AUTHOR_NAME", "t")
                .env("GIT_AUTHOR_EMAIL", "t@t")
                .env("GIT_COMMITTER_NAME", "t")
                .env("GIT_COMMITTER_EMAIL", "t@t")
                .env("GIT_AUTHOR_DATE", at)
                .env("GIT_COMMITTER_DATE", at)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_SYSTEM", "/dev/null")
                .output()
                .unwrap();
            assert!(o.status.success(), "git {args:?}");
        };
        git(&["init", "-q", "-b", "main"]);
        git(&["add", "."]);
        git(&["commit", "-q", "-m", "layer"]);
    }
    dir
}

/// A nfpm that writes what SOURCE_DATE_EPOCH it was given (or `unset`) into its target.
fn echoing_nfpm(dir: &Path) -> PathBuf {
    let nfpm = dir.join("fake-nfpm");
    fs::write(
        &nfpm,
        "#!/bin/sh\necho \"${SOURCE_DATE_EPOCH-unset}\" > \"$7\"\n",
    )
    .unwrap();
    fs::set_permissions(&nfpm, fs::Permissions::from_mode(0o755)).unwrap();
    nfpm
}

fn pack(dir: &Path, nfpm: &Path, epoch: Option<&str>) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_lpoco-layer"));
    c.args(["pack", "--format", "deb", "--manifest"])
        .arg(dir.join("device.toml"))
        .arg("--out")
        .arg(dir.join("out"))
        .env_remove("SOURCE_DATE_EPOCH")
        .env("LPOCO_NFPM", nfpm);
    if let Some(e) = epoch {
        c.env("SOURCE_DATE_EPOCH", e);
    }
    c.output().unwrap()
}

fn given_to_nfpm(dir: &Path) -> String {
    fs::read_to_string(dir.join("out/xiaomi-surya-config_0.1.0_all.deb"))
        .unwrap()
        .trim()
        .to_string()
}

#[test]
fn a_source_date_epoch_that_is_given_reaches_nfpm_as_it_is() {
    let dir = layer("given", Some("2020-01-02T03:04:05Z"));
    let o = pack(&dir, &echoing_nfpm(&dir), Some("1700000000"));
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(given_to_nfpm(&dir), "1700000000");
}

#[test]
fn without_one_the_layers_commit_time_is_given() {
    // 2020-01-02T03:04:05Z is 1577934245 seconds after the epoch
    let dir = layer("commit", Some("2020-01-02T03:04:05Z"));
    let o = pack(&dir, &echoing_nfpm(&dir), None);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(given_to_nfpm(&dir), "1577934245");
}

#[test]
fn a_layer_that_is_in_no_repository_and_no_time_given_is_said_not_reproducible() {
    let dir = layer("nogit", None);
    let o = pack(&dir, &echoing_nfpm(&dir), None);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(given_to_nfpm(&dir), "unset");
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("not reproducible"),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn a_source_date_epoch_that_is_not_seconds_is_refused() {
    let dir = layer("bad", None);
    let o = pack(&dir, &echoing_nfpm(&dir), Some("yesterday"));
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("SOURCE_DATE_EPOCH"));
    assert!(!dir.join("out/xiaomi-surya-config_0.1.0_all.deb").exists());
}

/// Run by hand where nfpm is installed (CI has none): `cargo test -p lpoco-layer --test reproducible -- --ignored`.
#[test]
#[ignore = "needs the real nfpm on PATH (the lab host has it)"]
fn two_packs_of_one_layer_give_equal_sha256_for_apk_and_deb() {
    let dir = layer("real", Some("2020-01-02T03:04:05Z"));
    let sums = |tag: &str, format: &str| {
        let out = dir.join(tag);
        let o = Command::new(env!("CARGO_BIN_EXE_lpoco-layer"))
            .args(["pack", "--format", format, "--manifest"])
            .arg(dir.join("device.toml"))
            .arg("--out")
            .arg(&out)
            .env_remove("SOURCE_DATE_EPOCH")
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let file = fs::read_dir(&out)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|x| x == "apk" || x == "deb"))
            .unwrap();
        let s = Command::new("sha256sum").arg(&file).output().unwrap();
        String::from_utf8(s.stdout)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .to_string()
    };
    for format in ["apk", "deb"] {
        let first = sums(&format!("one-{format}"), format);
        std::thread::sleep(std::time::Duration::from_secs(2));
        let second = sums(&format!("two-{format}"), format);
        assert_eq!(first, second, "{format}: two packs differ");
    }
}

#[test]
fn a_source_date_epoch_nfpm_cannot_hold_is_refused_not_silently_dropped() {
    // nfpm reads it as a signed 64-bit number and stamps the time now for a larger one (seen 2026-11-24)
    let dir = layer("big", None);
    let o = pack(&dir, &echoing_nfpm(&dir), Some("99999999999999999999999"));
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("SOURCE_DATE_EPOCH"));
}

#[test]
fn a_layer_in_a_bigger_repository_takes_its_own_commit_time_not_the_repositorys() {
    // the layer is the folder `layer/` of a repository: committed in 2020; a later commit elsewhere must not move the bytes
    let root = layer("sub", Some("2020-01-02T03:04:05Z"));
    let dir = root.join("layer");
    fs::create_dir_all(&dir).unwrap();
    for f in ["device.toml", "ucm"] {
        fs::rename(root.join(f), dir.join(f)).unwrap();
    }
    let git = |at: &str, args: &[&str]| {
        let o = Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(args)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .env("GIT_AUTHOR_DATE", at)
            .env("GIT_COMMITTER_DATE", at)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .output()
            .unwrap();
        assert!(o.status.success(), "git {args:?}");
    };
    git("2020-01-02T03:04:05Z", &["add", "-A"]);
    git(
        "2020-01-02T03:04:05Z",
        &["commit", "-q", "-m", "move into layer/"],
    );
    fs::write(root.join("other.txt"), "later\n").unwrap();
    git("2024-01-01T00:00:00Z", &["add", "other.txt"]);
    git("2024-01-01T00:00:00Z", &["commit", "-q", "-m", "elsewhere"]);
    let o = pack(&dir, &echoing_nfpm(&root), None);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    // layer/ was last committed at the 2020 move; the 2024 commit touched other.txt only
    assert_eq!(given_to_nfpm(&dir), "1577934245");
}
