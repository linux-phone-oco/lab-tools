// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco-host --help` and `-h` print the usage and exit 0 (proof: lpoco-host-help-prints-usage).

use std::process::Command;

fn run(flag: &str) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_lpoco-host"))
        .arg(flag)
        .output()
        .unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

#[test]
fn help_prints_the_usage_and_exits_0() {
    for flag in ["--help", "-h"] {
        let (code, out, err) = run(flag);
        assert_eq!(code, 0, "{flag}: stderr {err}");
        assert!(out.starts_with("Usage: lpoco-host"), "{flag}: {out}");
        assert!(out.contains("--version"), "{flag}: {out}");
        assert!(out.contains("plan"), "{flag}: {out}");
        assert!(out.contains("prepare now"), "{flag}: {out}");
        assert!(out.contains("check"), "{flag}: {out}");
        assert!(out.contains("--lab"), "{flag}: {out}");
        assert!(out.contains("LPOCO_LAB"), "{flag}: {out}");
    }
}

#[test]
fn family_names_the_programs_one_a_line_without_reading_a_lab() {
    // the answer a follow asks the host it just installed (host/src/main.rs run); no lab.toml is given or needed
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_lpoco-host"))
        .arg("family")
        .env_clear()
        .env("LPOCO_LAB", "/nonexistent/lab.toml")
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        o.status.success(),
        "{out}{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let names: Vec<&str> = out.lines().collect();
    assert!(
        names.contains(&"lpoco") && names.contains(&"lpoco-layer"),
        "{names:?}"
    );
    assert!(names.iter().all(|n| !n.contains(' ')), "{names:?}");
}
