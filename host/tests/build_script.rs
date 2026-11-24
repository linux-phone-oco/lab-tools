// SPDX-License-Identifier: MIT OR Apache-2.0
//! health/build.sh builds lpoco-health for the targets the declarations name (`lpoco host targets`) when it is given
//! none, and for the one it is given when it is (proof: lpoco-host-rest, cross targets). cargo, rustc and lpoco are
//! stand-ins on PATH that write down what they were asked.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

fn exe(path: PathBuf, body: &str) {
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn run(tag: &str, args: &[&str], lpoco_says: &str) -> (std::process::Output, String) {
    let dir = std::env::temp_dir().join(format!("lpoco-build-sh-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("stubs")).unwrap();
    fs::create_dir_all(dir.join("libdir")).unwrap();
    let log = dir.join("log");
    exe(
        dir.join("stubs/lpoco"),
        &format!(
            "echo \"lpoco $@\" >> {}\nprintf '{lpoco_says}'",
            log.display()
        ),
    );
    exe(
        dir.join("stubs/cargo"),
        &format!("echo \"cargo $@\" >> {}", log.display()),
    );
    // a standard library for every target: no devlab-cross fallback is taken
    exe(
        dir.join("stubs/rustc"),
        &format!("echo {}", dir.join("libdir").display()),
    );
    for p in ["sh", "dirname", "cut", "echo"] {
        if let Ok(o) = Command::new("which").arg(p).output() {
            let from = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !from.is_empty() {
                let _ = std::os::unix::fs::symlink(from, dir.join("stubs").join(p));
            }
        }
    }
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("health/build.sh");
    let o = Command::new("sh")
        .arg(script)
        .args(args)
        .env_clear()
        .env("HOME", dir.join("home"))
        .env("PATH", dir.join("stubs"))
        .output()
        .unwrap();
    (o, fs::read_to_string(&log).unwrap_or_default())
}

#[test]
fn without_a_target_it_builds_for_each_the_declarations_name() {
    let (o, log) = run("declared", &[], "a-musl\\nb-musl\\n");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(log.contains("lpoco host targets"), "{log}");
    for t in ["a-musl", "b-musl"] {
        assert!(
            log.contains(&format!(
                "cargo build --release --locked -p lpoco-health --target {t}"
            )),
            "{log}"
        );
    }
}

#[test]
fn a_target_given_is_the_only_one_built_and_lpoco_is_not_asked() {
    let (o, log) = run("given", &["c-musl"], "a-musl\\n");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(!log.lines().any(|l| l.starts_with("lpoco ")), "{log}");
    assert!(log.contains("--target c-musl"), "{log}");
    assert!(!log.contains("a-musl"), "{log}");
}

#[test]
fn nothing_declared_and_nothing_given_is_a_refusal_not_a_silent_success() {
    let (o, log) = run("none", &[], "");
    assert!(!o.status.success(), "{log}");
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("no target"),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(!log.contains("cargo"), "{log}");
}
