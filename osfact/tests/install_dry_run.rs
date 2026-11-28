// SPDX-License-Identifier: MIT OR Apache-2.0
//! The install command's dry run (item osfact-walleye-alpine-skeleton): from an install.toml it lists the
//! commands - fastboot boot of the image from RAM, then the way back - and it refuses a plan that flashes.
//! Fixtures: tests/fixtures/plans, taken from google-walleye-lab's device.toml ([fastboot], [boot] back_wait).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn plan(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/plans")
        .join(name)
}

fn install(plan: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_osfact"))
        .args(["install", "now", "--dry-run", "--plan"])
        .arg(plan)
        .output()
        .expect("osfact runs")
}

#[test]
fn the_dry_run_lists_fastboot_boot_and_the_way_back() {
    let out = install(&plan("walleye-ram.toml"));
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "dry run failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        said.contains("fastboot -s $WALLEYE_SERIAL boot boot.img"),
        "no fastboot boot line:\n{said}"
    );
    assert!(
        said.contains("way back: fastboot -s $WALLEYE_SERIAL reboot"),
        "no way back line:\n{said}"
    );
    assert!(
        !said.contains("flash"),
        "a dry run of a RAM plan names a flash:\n{said}"
    );
}

#[test]
fn a_plan_that_flashes_is_refused() {
    let out = install(&plan("walleye-flash.toml"));
    assert!(!out.status.success(), "a flashing plan was accepted");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("refuses") && err.contains("flash"),
        "refusal not said: {err}"
    );
}

#[test]
fn a_plan_with_no_boot_step_is_refused() {
    let out = install(&plan("walleye-no-step.toml"));
    assert!(
        !out.status.success(),
        "a plan with no boot step was accepted"
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("no boot step"));
}

#[test]
fn help_and_version_are_declared() {
    for flag in ["--help", "--version"] {
        let out = Command::new(env!("CARGO_BIN_EXE_osfact"))
            .arg(flag)
            .output()
            .expect("osfact runs");
        assert!(
            out.status.success(),
            "{flag} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
