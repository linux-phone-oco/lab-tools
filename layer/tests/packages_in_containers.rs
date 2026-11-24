// SPDX-License-Identifier: MIT OR Apache-2.0
//! The packages `lpoco-layer pack` writes, installed by each base's own package manager in a container
//! (proof: lpoco-layer-pack, bootimg-trigger-per-base): the real nfpm builds an apk, a deb, an rpm and an
//! archlinux package of one fixture layer; apk, apt, dnf and pacman install them; the files stand where the layer
//! put them, with their modes, and - for the three that write a preset - the boot package's postinstall ran
//! `systemctl preset` for the layer's unit (a stub systemctl in the container logs its arguments).
//! Needs docker, the network, nfpm on PATH (the lab host has it), so it is ignored until asked:
//!
//!   cargo test -p lpoco-layer --test packages_in_containers -- --ignored
//!
//! Ignored by CI: run by hand on the lab. Not covered: a real kernel package's scriptlets, the boot.img trigger
//! running after these installs (bootimg_containers.rs runs it on the same files copied in).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEVICE: &str = r#"
[device]
codename = "test-phone"

[boot]
base = 0
kernel_offset = 0x00008000
ramdisk_offset = 0x01000000
tags_offset = 0x00000100
pagesize = 4096
os_version = "11.0.0"
os_patch_level = "2021-01"
cmdline = "console=ttyMSM0,115200n8"
msm_id = [0x124, 0x20001]

[layer]
name = "test-phone"

[layer.kernel]
tree = "https://example.org/linux"
tag = "v7.1"
dtbs = ["qcom/test-phone.dtb"]

[layer.boot]
method = "android-bootimg"

[[layer.file]]
from = "ucm/HiFi.conf"
to = "/usr/share/alsa/ucm2/Test/phone/HiFi.conf"
kind = "data"

[[layer.file]]
from = "units/test-phone-glue.service"
to = "/usr/lib/systemd/system/test-phone-glue.service"
kind = "unit"
"#;

fn run(cmd: &mut Command) -> String {
    let o = cmd.output().unwrap();
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(o.status.success(), "{cmd:?}:\n{all}");
    all
}

/// The fixture layer, packed in `format` by the real nfpm into `dir/out`.
fn packed(format: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lpoco-pkg-ct-{format}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("ucm")).unwrap();
    fs::create_dir_all(dir.join("units")).unwrap();
    fs::write(dir.join("ucm/HiFi.conf"), "Syntax 4\n").unwrap();
    fs::write(
        dir.join("units/test-phone-glue.service"),
        "[Unit]\nDescription=glue\n[Service]\nExecStart=/bin/true\n[Install]\nWantedBy=multi-user.target\n",
    )
    .unwrap();
    fs::write(dir.join("device.toml"), DEVICE).unwrap();
    run(Command::new(env!("CARGO_BIN_EXE_lpoco-layer"))
        .args(["pack", "--format", format, "--manifest"])
        .arg(dir.join("device.toml"))
        .arg("--out")
        .arg(dir.join("out"))
        .env("SOURCE_DATE_EPOCH", "1700000000"));
    dir
}

fn in_container(image: &str, dir: &Path, script: &str) -> String {
    run(Command::new("docker")
        .args(["run", "--rm", "-v"])
        .arg(format!("{}:/w", dir.display()))
        .args([image, "sh", "-ec", script]))
}

/// What every base must show after its manager installed the packages (the stub systemctl logs to /tmp/systemctl).
const CHECK_FILES: &str = "test -f /usr/share/alsa/ucm2/Test/phone/HiFi.conf\n\
     test -f /usr/lib/systemd/system/test-phone-glue.service\n\
     test -x /usr/libexec/test-phone-bootimg\n";
const STUB_SYSTEMCTL: &str =
    "printf '#!/bin/sh\\necho \"$@\" >> /tmp/systemctl\\n' > /usr/bin/systemctl && chmod +x /usr/bin/systemctl\n";
const CHECK_PRESET: &str = "grep -qx 'preset test-phone-glue.service' /tmp/systemctl\n";

#[test]
#[ignore = "needs docker, the network and nfpm"]
fn debian_installs_the_deb_packages_and_presets_the_unit() {
    let dir = packed("deb");
    in_container(
        "debian:trixie",
        &dir,
        &format!(
            "{STUB_SYSTEMCTL}apt-get update -qq >/dev/null && apt-get install -y -qq /w/out/*.deb >/dev/null\n{CHECK_FILES}{CHECK_PRESET}"
        ),
    );
}

#[test]
#[ignore = "needs docker, the network and nfpm"]
fn fedora_installs_the_rpm_packages_and_presets_the_unit() {
    let dir = packed("rpm");
    in_container(
        "fedora:latest",
        &dir,
        &format!(
            "{STUB_SYSTEMCTL}dnf install -y -q /w/out/*.rpm >/dev/null\n{CHECK_FILES}{CHECK_PRESET}"
        ),
    );
}

#[test]
#[ignore = "needs docker, the network and nfpm"]
fn arch_installs_the_archlinux_packages_and_presets_the_unit() {
    // the stub goes in after the upgrade: it brings systemd, whose own systemctl would replace it
    let dir = packed("archlinux");
    in_container(
        "archlinux:latest",
        &dir,
        &format!(
            "pacman -Syu --noconfirm >/dev/null 2>&1\n{STUB_SYSTEMCTL}pacman -U --noconfirm /w/out/*.pkg.tar.zst >/dev/null\n{CHECK_FILES}{CHECK_PRESET}"
        ),
    );
}

#[test]
#[ignore = "needs docker, the network and nfpm"]
fn alpine_installs_the_apk_packages() {
    // apk writes no systemd preset (Alpine runs OpenRC): the files only
    let dir = packed("apk");
    in_container(
        "alpine:3.24",
        &dir,
        "apk add --allow-untrusted /w/out/*.apk >/dev/null\n\
         test -f /usr/share/alsa/ucm2/Test/phone/HiFi.conf\n\
         test -f /etc/deviceinfo\n",
    );
}

#[test]
#[ignore = "needs nfpm"]
fn the_kernel_package_carries_arm64_in_each_formats_own_word() {
    // source: nfpm names the architecture per format (arm64 for deb, aarch64 for rpm and archlinux); the
    // apk's .PKGINFO says aarch64 too. Seen with nfpm 2.49.0 on the lab, 2026-11-24.
    let dir = packed("deb");
    let kb = dir.join("kb");
    fs::create_dir_all(kb.join("dtbs/qcom")).unwrap();
    fs::write(kb.join("Image"), vec![7u8; 2048]).unwrap();
    fs::write(kb.join("kernel.release"), "6.1.0-test\n").unwrap();
    fs::write(kb.join("dtbs/qcom/test-phone.dtb"), vec![1u8; 512]).unwrap();
    for (format, file) in [
        ("deb", "linux-test-phone_0.1.0_arm64.deb"),
        ("rpm", "linux-test-phone-0.1.0-1.aarch64.rpm"),
        ("archlinux", "linux-test-phone-0.1.0-1-aarch64.pkg.tar.zst"),
        ("apk", "linux-test-phone-0.1.0-r0.apk"),
    ] {
        let out = dir.join(format!("k-{format}"));
        run(Command::new(env!("CARGO_BIN_EXE_lpoco-layer"))
            .args(["pack", "--format", format, "--manifest"])
            .arg(dir.join("device.toml"))
            .arg("--out")
            .arg(&out)
            .arg("--kernel-build")
            .arg(&kb)
            .env("SOURCE_DATE_EPOCH", "1700000000"));
        assert!(out.join(file).exists(), "{format}: no {file} in {out:?}");
    }
    let pkginfo = run(Command::new("sh").arg("-c").arg(format!(
        "gzip -dc {} | tar -xO --ignore-zeros .PKGINFO",
        dir.join("k-apk/linux-test-phone-0.1.0-r0.apk").display()
    )));
    assert!(pkginfo.contains("arch = aarch64"), "{pkginfo}");
}
