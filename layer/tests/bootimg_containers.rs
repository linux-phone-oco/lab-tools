// SPDX-License-Identifier: MIT OR Apache-2.0
//! The proof of bootimg-trigger-per-base, in a container per base: the files the packager is told to install are
//! put in place, the base's own trigger runs on a new kernel (kernel-install on Fedora and Debian, pacman's libalpm
//! on Arch) and the boot.img it leaves is counted by `lpoco-bringup image check` against the manifest, the dtb
//! appended. Needs docker, the network and LPOCO_BRINGUP (the lpoco-bringup binary), so it is ignored until asked:
//!
//!   LPOCO_BRINGUP=target/debug/lpoco-bringup cargo test -p lpoco-layer --test bootimg_containers -- --ignored
//!
//! Fedora runs the real `kernel-install add`, Arch a real `pacman -U` and Debian a real `dpkg -i` of a one-file linux-image
//! whose postinst runs /etc/kernel/postinst.d, bridged to kernel-install by systemd-boot (bootctl stubbed: no ESP).
//!
//! Ignored by CI: run by hand. The entry lands under /boot/MACHINE-ID here (no ESP; a phone layout may differ). On Debian the plugin runs twice per install
//! (initramfs-tools' post-update hook passes the initrd, zz-systemd-boot does not): the second finds it in the entry.
//!
//! The packages are the real ones: nfpm builds them and apt, dnf and pacman install them (nfpm on PATH is needed too).
//!
//! Not covered: a real kernel package's scriptlets.

use lpoco_layer::{add_boot_trigger, materialize, packages, Format};
use lpoco_manifest::Manifest;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEVICE: &str = r#"
[device]
codename = "google-walleye"

[boot]
base = 0
kernel_offset = 0x00008000
ramdisk_offset = 0x01000000
tags_offset = 0x00000100
pagesize = 4096
os_version = "11.0.0"
os_patch_level = "2021-01"
cmdline = "console=ttyMSM0,115200n8 androidboot.hardware=walleye"
msm_id = [0x124, 0x20001]

[layer]
name = "google-walleye"

[layer.kernel]
tree = "https://example.org/linux"
tag = "v7.1"
dtbs = ["qcom/msm8998-google-walleye.dtb"]

[layer.boot]
method = "android-bootimg"
"#;

const DTS: &str =
    "/dts-v1/;\n/ { compatible = \"google,walleye\"; qcom,msm-id = <0x124 0x20001>; };\n";

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

/// The work folder: the package files under `stage/`, the kernel, ramdisk and device tree, the manifest.
fn work(format: Format, tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lpoco-bootimg-ct-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let m = Manifest::parse(DEVICE).unwrap();
    let mut pkgs = packages(m.layer.as_ref().unwrap(), format);
    add_boot_trigger(
        &mut pkgs,
        m.layer.as_ref().unwrap(),
        m.boot.as_ref().unwrap(),
        format,
    )
    .unwrap();
    for p in &pkgs {
        for e in materialize(p, &dir, &dir.join("tmpstage")).unwrap() {
            let at = dir.join("files").join(e.dst.trim_start_matches('/'));
            fs::create_dir_all(at.parent().unwrap()).unwrap();
            fs::copy(&e.src, &at).unwrap();
            fs::set_permissions(&at, std::os::unix::fs::PermissionsExt::from_mode(e.mode)).unwrap();
        }
    }
    fs::write(dir.join("device.toml"), DEVICE).unwrap();
    // the packages are the real ones: nfpm builds them, and the container's own package manager installs them
    run(Command::new(env!("CARGO_BIN_EXE_lpoco-layer"))
        .args(["pack", "--format", format.token(), "--manifest"])
        .arg(dir.join("device.toml"))
        .arg("--out")
        .arg(dir.join("out"))
        .env("SOURCE_DATE_EPOCH", "1700000000"));
    fs::write(dir.join("t.dts"), DTS).unwrap();
    run(Command::new("dtc")
        .args(["-I", "dts", "-O", "dtb", "-o"])
        .arg(dir.join("w.dtb"))
        .arg(dir.join("t.dts")));
    run(Command::new("sh").arg("-c").arg(format!(
        "head -c 4096 /dev/urandom | gzip -c > {d}/Image.gz; head -c 2048 /dev/urandom > {d}/initrd",
        d = dir.display()
    )));
    dir
}

fn in_container(image: &str, dir: &Path, script: &str) {
    run(Command::new("docker")
        .args(["run", "--rm", "-v"])
        .arg(format!("{}:/w", dir.display()))
        .args([image, "sh", "-ec", script]));
}

fn counted(dir: &Path, img: &str) {
    let bringup = std::env::var("LPOCO_BRINGUP").expect("LPOCO_BRINGUP: the lpoco-bringup binary");
    let out = run(Command::new(bringup)
        .args(["image", "check"])
        .arg(dir.join(img))
        .arg("--device")
        .arg(dir.join("device.toml")));
    assert!(out.contains("appended dtb"), "{out}");
}

#[test]
#[ignore = "needs docker, the network and LPOCO_BRINGUP"]
fn fedora_kernel_install_leaves_a_boot_img_with_the_dtb_appended() {
    let dir = work(Format::Rpm, "fedora");
    // kernel-install's staging area is $3; the plugin writes boot.img there, and `add` keeps it in the entry
    in_container(
        "fedora:latest",
        &dir,
        "dnf install -y -q systemd-udev /w/out/*.rpm >/dev/null\n\
         mkdir -p /usr/lib/modules/6.1.0-walleye/dtb/qcom /boot /etc/kernel\n\
         cp /w/w.dtb /usr/lib/modules/6.1.0-walleye/dtb/qcom/msm8998-google-walleye.dtb\n\
         systemd-id128 new > /etc/machine-id\n\
         echo layout=bls > /etc/kernel/install.conf\n\
         KERNEL_INSTALL_BYPASS=0 kernel-install add 6.1.0-walleye /w/Image.gz /w/initrd\n\
         cp /boot/*/6.1.0-walleye/boot.img /w/boot.img",
    );
    counted(&dir, "boot.img");
}

#[test]
#[ignore = "needs docker, the network and LPOCO_BRINGUP"]
fn debian_linux_image_install_leaves_a_boot_img_with_the_dtb_appended() {
    let dir = work(Format::Deb, "debian");
    // a one-file linux-image whose postinst runs /etc/kernel/postinst.d as Debian's does; systemd-boot's bridge there
    // calls kernel-install. bootctl is stubbed: a container has no ESP, and without one the bridge does nothing.
    in_container(
        "debian:trixie",
        &dir,
        "apt-get update -qq >/dev/null && apt-get install -y -qq systemd-boot initramfs-tools >/dev/null\n\
         printf '#!/bin/sh\\nexit 0\\n' > /usr/bin/bootctl && chmod +x /usr/bin/bootctl\n\
         apt-get install -y -qq /w/out/*.deb >/dev/null\n\
         systemd-id128 new > /etc/machine-id\n\
         echo layout=bls > /etc/kernel/install.conf\n\
         V=6.1.0-walleye\n\
         mkdir -p /p/DEBIAN /p/boot /p/usr/lib/linux-image-$V/qcom\n\
         cp /w/Image.gz /p/boot/vmlinuz-$V\n\
         cp /w/w.dtb /p/usr/lib/linux-image-$V/qcom/msm8998-google-walleye.dtb\n\
         printf 'Package: linux-image-%s\\nVersion: 1\\nArchitecture: all\\nMaintainer: t <t@t>\\nDescription: a kernel\\n' $V > /p/DEBIAN/control\n\
         printf '#!/bin/sh\\nset -e\\nrun-parts --report --exit-on-error --arg=%s --arg=/boot/vmlinuz-%s /etc/kernel/postinst.d\\n' $V $V > /p/DEBIAN/postinst\n\
         chmod 755 /p/DEBIAN/postinst\n\
         dpkg-deb --build /p /k.deb >/dev/null\n\
         dpkg -i /k.deb\n\
         cp /boot/*/$V/boot.img /w/boot.img",
    );
    counted(&dir, "boot.img");
}

#[test]
#[ignore = "needs docker, the network and LPOCO_BRINGUP"]
fn arch_pacman_hook_leaves_a_boot_img_with_the_dtb_appended() {
    let dir = work(Format::Archlinux, "arch");
    // a one-file package whose install runs pacman's hooks, as a kernel package's would: pacman -U on a tar
    in_container(
        "archlinux:latest",
        &dir,
        "pacman -Syu --noconfirm android-tools python >/dev/null\n\
         pacman -U --noconfirm /w/out/*.pkg.tar.zst >/dev/null\n\
         mkdir -p /pkg/usr/lib/modules/6.1.0-walleye/dtb/qcom /boot\n\
         cd /pkg\n\
         echo walleye > usr/lib/modules/6.1.0-walleye/pkgbase\n\
         cp /w/Image.gz usr/lib/modules/6.1.0-walleye/vmlinuz\n\
         cp /w/w.dtb usr/lib/modules/6.1.0-walleye/dtb/qcom/msm8998-google-walleye.dtb\n\
         cp /w/initrd /boot/initramfs-walleye.img\n\
         printf 'pkgname = fakekernel\\npkgver = 1-1\\npkgdesc = a kernel\\narch = any\\nsize = 1\\n' > .PKGINFO\n\
         bsdtar -cf /fakekernel.pkg.tar --no-recursion -- .PKGINFO usr/lib/modules/6.1.0-walleye/pkgbase usr/lib/modules/6.1.0-walleye/vmlinuz usr/lib/modules/6.1.0-walleye/dtb/qcom/msm8998-google-walleye.dtb\n\
         pacman -U --noconfirm /fakekernel.pkg.tar\n\
         cp /boot/google-walleye-walleye.img /w/boot.img",
    );
    counted(&dir, "boot.img");
}
