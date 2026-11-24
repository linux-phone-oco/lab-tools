// SPDX-License-Identifier: MIT OR Apache-2.0
//! The helpers that run on a phone are built for the phone's architecture in CI (backlog bringup-init-finish): a
//! change that breaks lpoco-ffs-acm for aarch64 musl (the bring-up init runs it in the ramdisk, static) fails the
//! pull request, not the next boot. Reads .github/workflows/ci.yml, as release_family.rs reads the release files.

use std::fs;
use std::path::PathBuf;

fn ci() -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(".github/workflows/ci.yml");
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

#[test]
fn ci_builds_lpoco_ffs_acm_for_aarch64_musl() {
    let ci = ci();
    assert!(
        ci.contains("rustup target add aarch64-unknown-linux-musl"),
        "the target's standard library is added"
    );
    // the same linker health/build.sh uses on the lab: the aarch64 GCC links the static musl binary
    assert!(
        ci.contains("CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-gnu-gcc"),
        "{ci}"
    );
    assert!(
        ci.contains(
            "cargo build --release --locked -p lpoco-ffs-acm --target aarch64-unknown-linux-musl"
        ),
        "{ci}"
    );
}
