// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco-health --help` and `-h` print the usage and exit 0 (proof: lpoco-host-help-prints-usage).

use std::process::Command;

fn run(flag: &str) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_lpoco-health"))
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
        assert!(out.starts_with("Usage: lpoco-health"), "{flag}: {out}");
        assert!(out.contains("--version"), "{flag}: {out}");
        assert!(out.contains("latest"), "{flag}: {out}");
    }
}
