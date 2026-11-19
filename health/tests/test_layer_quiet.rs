// SPDX-License-Identifier: MIT OR Apache-2.0
//! The suite check test-layer-quiet (backlog health-test-layer-quiet, decision 2026-11-19-hardware-test-layer):
//! a phone that booted the hardware-test layer (no phone shell) runs no shell process. Run here against a
//! made-up phone under LPOCO_HEALTH_ROOT; the runner tells the check which layer the phone booted
//! (LPOCO_BOOTED, from /proc/cmdline) and the check names what it finds in the process table.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

const CHECK: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/suite/checks/base/95-test-layer.sh"
);
const LIB: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/suite/lib.sh");

fn phone(name: &str, env: &str, comms: &[&str]) -> PathBuf {
    let root = std::env::temp_dir().join(format!("layer-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("etc/lpoco-health")).unwrap();
    fs::write(root.join("etc/lpoco-health/health.env"), env).unwrap();
    for (i, c) in comms.iter().enumerate() {
        let d = root.join(format!("proc/{}", 100 + i));
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("comm"), format!("{c}\n")).unwrap();
    }
    root
}

fn run(root: &PathBuf, booted: &str) -> String {
    let o = Command::new("timeout")
        .args(["20", "sh", CHECK])
        .env("LPOCO_HEALTH_ROOT", root)
        .env("LPOCO_HEALTH_LIB", LIB)
        .env("LPOCO_BOOTED", booted)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn a_test_layer_boot_with_a_shell_process_running_fails_and_names_it() {
    let root = phone("sxmo", "", &["systemd", "sxmo_winit.sh", "sshd"]);
    let out = run(&root, "test");
    assert!(out.starts_with("fail test-layer-quiet "), "{out}");
    assert!(out.contains("sxmo_winit.sh"), "{out}");
    assert!(!out.contains("sshd"), "{out}");
}

#[test]
fn a_test_layer_boot_with_no_shell_passes() {
    let root = phone("quiet", "", &["systemd", "sshd"]);
    let out = run(&root, "test");
    assert!(out.starts_with("pass test-layer-quiet "), "{out}");
}

#[test]
fn a_shell_boot_is_not_measured_and_says_which_shell_runs() {
    let root = phone("shell", "", &["systemd", "sxmo_winit.sh"]);
    let out = run(&root, "shell");
    assert!(out.starts_with("skip test-layer-quiet "), "{out}");
    assert!(
        out.contains("shell layer") && out.contains("sxmo_winit.sh"),
        "{out}"
    );
}

#[test]
fn an_unknown_boot_is_not_known_yet_not_failing() {
    let root = phone("unknown", "", &["systemd"]);
    let out = run(&root, "unknown");
    assert!(out.starts_with("skip test-layer-quiet "), "{out}");
    assert!(out.contains("not known"), "{out}");
}
