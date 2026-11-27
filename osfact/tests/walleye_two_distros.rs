// SPDX-License-Identifier: MIT OR Apache-2.0
//! The failing test of osfact (item image-factory-any-distro-any-phone, decision 2026-11-27-osfact-own-rust-in-lab-tools):
//! the same google-walleye declaration packs an installable walleye image for Alpine and for Debian, each with its
//! install command. It fails until osfact packs (item osfact-walleye-two-distros), and it needs what a build needs:
//! the network (each distro's mirror), apk.static and mmdebstrap, fakeroot, e2fsprogs' debugfs and sha256sum, and
//! the proven parts of walleye in OSFACT_WALLEYE_PARTS (its kernel and its device layer packed for apk and deb).
//!     OSFACT_WALLEYE_PARTS=DIR cargo test -p osfact --test walleye_two_distros -- --ignored

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// What `debugfs` reads of one file inside an ext4 image, without mounting it.
fn file_in_image(img: &Path, file: &str) -> String {
    let out = Command::new("debugfs")
        .args(["-R", &format!("cat {file}")])
        .arg(img)
        .output()
        .expect("debugfs runs");
    assert!(out.status.success(), "debugfs on {}: failed", img.display());
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn sha256(file: &Path) -> String {
    let out = Command::new("sha256sum")
        .arg(file)
        .output()
        .expect("sha256sum runs");
    assert!(out.status.success(), "sha256sum {}", file.display());
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .expect("a sum")
        .to_string()
}

/// osfact packs walleye for one distro profile, and the image it leaves is walleye's and that distro's.
fn packs_walleye_for(distro: &str, package_db: &str, layer_entry: &str) {
    let parts = std::env::var("OSFACT_WALLEYE_PARTS")
        .expect("OSFACT_WALLEYE_PARTS names the folder of walleye's proven parts");
    let out = std::env::temp_dir().join(format!("osfact-walleye-{distro}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&out);
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let run = Command::new(cargo)
        .args([
            "run",
            "--quiet",
            "--package",
            "osfact",
            "--bin",
            "osfact",
            "--",
            "pack",
            "now",
        ])
        .arg("--device")
        .arg(fixtures().join("google-walleye/device.toml"))
        .arg("--distro")
        .arg(fixtures().join(format!("distros/{distro}.toml")))
        .arg("--parts")
        .arg(&parts)
        .arg("--out")
        .arg(&out)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo runs");
    assert!(
        run.status.success(),
        "osfact pack now for {distro} failed: {}",
        String::from_utf8_lossy(&run.stderr)
    );

    // the root file system: an ext4 image of this distro, with walleye's device layer installed by its own format
    let rootfs = out.join("rootfs.img");
    let bytes = fs::read(&rootfs).expect("OUT/rootfs.img");
    assert_eq!(
        bytes.get(1080..1082),
        Some(&[0x53, 0xef][..]),
        "rootfs.img is not ext4"
    );
    let os = file_in_image(&rootfs, "/etc/os-release");
    assert!(
        os.lines().any(|l| l == format!("ID={distro}")),
        "{distro}: os-release says {os}"
    );
    let db = file_in_image(&rootfs, package_db);
    assert!(
        db.lines().any(|l| l == layer_entry),
        "{distro}: {layer_entry} not in {package_db}"
    );

    // the boot artifact: walleye's boot format, android-bootimg with its AVB footer, the size of its boot partition
    let boot = out.join("boot.img");
    let b = fs::read(&boot).expect("OUT/boot.img");
    assert_eq!(
        b.get(..8),
        Some(&b"ANDROID!"[..]),
        "boot.img is not an Android boot image"
    );
    assert_eq!(
        b.len(),
        33554432,
        "boot.img is not [boot] partition_size long"
    );
    assert_eq!(
        b.get(b.len() - 64..b.len() - 60),
        Some(&b"AVBf"[..]),
        "boot.img has no AVB footer"
    );

    // the install: the declaration's [install] steps, each with the sha256 of what it writes, and one command
    let plan = fs::read_to_string(out.join("install.toml")).expect("OUT/install.toml");
    for want in ["system_{other}", "boot_{other}", "android-dd", "fastboot"] {
        assert!(plan.contains(want), "{distro}: install.toml lacks {want}");
    }
    for written in [&rootfs, &boot] {
        let sum = sha256(written);
        assert!(
            plan.contains(&sum),
            "{distro}: install.toml lacks {}'s sha256",
            written.display()
        );
    }
    let said = String::from_utf8_lossy(&run.stdout);
    let command = said
        .lines()
        .rev()
        .find_map(|l| l.strip_prefix("install: "))
        .unwrap_or_else(|| panic!("{distro}: no 'install: COMMAND' line:\n{said}"));
    assert!(
        command.starts_with("osfact install now ") && command.contains(&*out.to_string_lossy()),
        "{distro}: the install command is not osfact's for this image: {command}"
    );
}

#[test]
#[ignore = "fails until osfact packs (item osfact-walleye-two-distros); needs the network, apk.static, mmdebstrap, debugfs and OSFACT_WALLEYE_PARTS"]
fn the_walleye_declaration_packs_an_installable_alpine_image() {
    packs_walleye_for("alpine", "/lib/apk/db/installed", "P:google-walleye-config");
}

#[test]
#[ignore = "fails until osfact packs (item osfact-walleye-two-distros); needs the network, apk.static, mmdebstrap, debugfs and OSFACT_WALLEYE_PARTS"]
fn the_walleye_declaration_packs_an_installable_debian_image() {
    packs_walleye_for(
        "debian",
        "/var/lib/dpkg/status",
        "Package: google-walleye-config",
    );
}
