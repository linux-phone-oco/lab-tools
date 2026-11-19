// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco-layer --help` and `-h` print the usage and exit 0.

use std::process::Command;

#[test]
fn help_prints_the_usage_and_exits_0() {
    for flag in ["--help", "-h"] {
        let o = Command::new(env!("CARGO_BIN_EXE_lpoco-layer"))
            .arg(flag)
            .output()
            .unwrap();
        let out = String::from_utf8_lossy(&o.stdout);
        assert_eq!(o.status.code(), Some(0), "{flag}");
        assert!(out.starts_with("Usage: lpoco-layer"), "{flag}: {out}");
        for want in ["pack", "--manifest", "--format", "--out", "LPOCO_NFPM"] {
            assert!(out.contains(want), "{flag}: {want} missing in {out}");
        }
    }
}
