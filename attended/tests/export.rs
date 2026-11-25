// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco-attended export NAME`: a procedure's checks as a LAVA test definition others' labs can run (decision
//! 2026-11-19-procedures-reflect-and-lift, part 4; backlog item attended-export). The procedure is the real
//! example (examples/attended.rs) run by rust-script; the phone is never reached, so a stand-in devlab that
//! records its calls proves it.
//!
//! The shape expected is LAVA's "Lava-Test Test Definition 1.0", from its documentation, "Test Actions" of the
//! job definition reference <https://docs.lavasoftware.org/lava/technical-references/job-definition/actions/test.html>:
//! `metadata: {format, name}` and `run: steps:` as a list of shell lines, `lava-test-case ID --shell COMMAND`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lpoco-export-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(d.join("procedures")).unwrap();
    fs::write(
        d.join("device.toml"),
        "[device]\ncodename = \"xiaomi-surya\"\n",
    )
    .unwrap();
    d
}

/// examples/attended.rs as the repo's procedures/attended.rs, its git dependency pointed at this checkout.
fn procedure(dir: &Path) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = fs::read_to_string(root.join("examples/attended.rs")).unwrap();
    let git = src
        .lines()
        .find(|l| l.starts_with("//! lpoco-attended = { git"))
        .expect("the example depends on lpoco-attended by git tag");
    let local = format!("//! lpoco-attended = {{ path = \"{}\" }}", root.display());
    fs::write(dir.join("procedures/attended.rs"), src.replace(git, &local)).unwrap();
}

/// A devlab that only records that it was called: an export that reaches it fails the test.
fn devlab_that_notes(dir: &Path) -> PathBuf {
    let devlab = dir.join("devlab");
    fs::write(
        &devlab,
        format!(
            "#!/bin/sh\necho \"$*\" >> {}\n",
            dir.join("calls").display()
        ),
    )
    .unwrap();
    Command::new("chmod")
        .arg("+x")
        .arg(&devlab)
        .status()
        .unwrap();
    devlab
}

fn export(dir: &Path, extra: &[&str]) -> Output {
    procedure(dir);
    let devlab = devlab_that_notes(dir);
    Command::new(env!("CARGO_BIN_EXE_lpoco-attended"))
        .args(["export", "attended", "--device"])
        .arg(dir.join("device.toml"))
        .args(extra)
        .env("LPOCO_ATTENDED_DEVLAB", devlab)
        .current_dir(dir)
        .output()
        .unwrap()
}

/// The rules of a Lava-Test Test Definition 1.0 this export must meet, each from the documentation above.
fn lava_problems(yaml: &str) -> Vec<String> {
    let mut bad = Vec::new();
    let lines: Vec<&str> = yaml
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect();
    let has = |l: &str| lines.contains(&l);
    if !has("metadata:") {
        bad.push("no metadata:".into());
    }
    if !has("  format: Lava-Test Test Definition 1.0") {
        bad.push("metadata.format is not Lava-Test Test Definition 1.0".into());
    }
    if !lines.iter().any(|l| l.starts_with("  name: ")) {
        bad.push("metadata.name is missing".into());
    }
    if !has("run:") || !has("  steps:") {
        bad.push("no run: steps:".into());
    }
    let after = lines
        .iter()
        .position(|l| *l == "  steps:")
        .map_or(0, |i| i + 1);
    let steps: Vec<&&str> = lines[after..]
        .iter()
        .filter(|l| l.starts_with("    - "))
        .collect();
    if steps.is_empty() {
        bad.push("run.steps has no step".into());
    }
    for s in steps {
        let body = s.trim_start_matches("    - ").trim_matches('"');
        if !body.starts_with("lava-test-case ") || !body.contains(" --shell ") {
            bad.push(format!("not a lava-test-case --shell step: {s}"));
        }
        let id = body.split(' ').nth(1).unwrap_or("");
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            bad.push(format!("a test case id with a space or odd character: {s}"));
        }
    }
    bad
}

#[test]
fn the_checks_of_a_procedure_are_a_lava_test_definition_with_the_device_filled_in() {
    let dir = tmp("lava");
    let o = export(&dir, &[]);
    let out = String::from_utf8_lossy(&o.stdout).into_owned();
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "{err}\n{out}");
    assert_eq!(lava_problems(&out), Vec::<String>::new(), "{out}");
    // the two checks of the example are the steps, by their ids; the step that changes the phone is not one
    assert!(
        out.contains("lava-test-case the-kernel-is-the-one-we-flashed --shell"),
        "{out}"
    );
    assert!(
        out.contains("lava-test-case pmicwdt-is-bound --shell"),
        "{out}"
    );
    assert!(!out.contains("put-the-kernel-on-it"), "{out}");
    // each carries the script the check runs on the phone
    assert!(out.contains("uname -r") && out.contains("lsmod"), "{out}");
    // the device's values, from device.toml
    assert!(out.contains("xiaomi-surya"), "{out}");
    // and the check's source is said, so another lab knows where its expectation came from
    assert!(out.contains("research:2026-11-23-lsmod"), "{out}");
    assert!(
        !dir.join("calls").exists(),
        "the export reached devlab: {out}"
    );
}

#[test]
fn a_procedure_that_does_not_exist_is_refused_naming_it() {
    let dir = tmp("none");
    let o = Command::new(env!("CARGO_BIN_EXE_lpoco-attended"))
        .args(["export", "nothing", "--device"])
        .arg(dir.join("device.toml"))
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("nothing"));
}
