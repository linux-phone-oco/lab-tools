// SPDX-License-Identifier: MIT OR Apache-2.0
//! restart2, the phone-side helper that takes a phone from its Linux to its bootloader (backlog item
//! linux-to-bootloader-by-restart2): busybox's `reboot bootloader` drops its argument, so the helper hands
//! `bootloader` to reboot(2) RESTART2 itself. Only its dry run is run here: a real run would restart this machine.

use std::process::{Command, Output};

fn restart2(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_restart2"))
        .args(args)
        .env_remove("RESTART2_COMMAND")
        .output()
        .unwrap()
}

fn said(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

#[test]
fn bootloader_is_the_string_a_dry_run_says_it_would_hand_to_reboot() {
    let o = restart2(&["--command", "bootloader", "--dry-run"]);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    assert!(
        said(&o).contains("RESTART2, \"bootloader\""),
        "{}",
        said(&o)
    );
}

#[test]
fn a_string_off_the_allow_list_is_refused_before_the_kernel_is_asked() {
    for bad in ["", "recovery", "Bootloader", "bootloader ", "poweroff"] {
        let o = restart2(&["--command", bad, "--dry-run"]);
        assert_eq!(o.status.code(), Some(1), "{bad:?}: {}", said(&o));
        assert!(said(&o).contains("not allowed"), "{bad:?}: {}", said(&o));
    }
}

#[test]
fn no_command_and_a_misspelled_flag_both_stop_before_anything_can_restart() {
    assert_eq!(restart2(&[]).status.code(), Some(2));
    // `--dryrun` is not `--dry-run`; `recovery` is off the allow-list, so even an ignored flag restarts nothing
    let o = restart2(&["--command", "recovery", "--dryrun"]);
    assert_eq!(o.status.code(), Some(1), "{}", said(&o));
    assert!(said(&o).contains("not a declared flag"), "{}", said(&o));
}

#[test]
fn the_command_may_come_from_the_environment_the_declared_variable() {
    let o = Command::new(env!("CARGO_BIN_EXE_restart2"))
        .arg("--dry-run")
        .env("RESTART2_COMMAND", "bootloader")
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
}
