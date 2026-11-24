// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco layer export` (backlog layer-export-makefile): a source tarball with the layer's tree and an Asahi-style
//! Makefile of install targets, for a distribution's own packager that builds from source (asahi-alarm's
//! `make install-arch`, Fedora's `%make_install install-fedora`: research base-distro 2026-11-19-agnostic-layer).
//! `make install DESTDIR=x` puts the files every format shares; `install-debian|fedora|arch|alpine` add that
//! format's own files (the boot glue). Each lists exactly what the format's package holds.
//!
//! Nothing aimed at Nura: no /etc/deviceinfo (the apk's Nura-only file) is exported; that waits on the author's word.

use lpoco_layer::{packages, Format};
use lpoco_manifest::Manifest;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEVICE: &str = r#"
[layer]
name = "xiaomi-surya"

[layer.kernel]
tree = "https://example.org/linux"
tag = "v7.1"
dtbs = ["qcom/sm7150-xiaomi-surya-huaxing.dtb"]

[layer.boot]
method = "efi-kernel-install"

[layer.initramfs]
modules = ["ufs_qcom"]

[[layer.file]]
from = "ucm/HiFi.conf"
to = "/usr/share/alsa/ucm2/Xiaomi/surya/HiFi.conf"
kind = "data"

[[layer.file]]
from = "lab/surya-awake.service"
to = "/usr/lib/systemd/system/surya-awake.service"
kind = "unit"

[[layer.file]]
from = "bin/surya-glue"
to = "/usr/libexec/surya-glue"
kind = "script"
"#;

fn fixture(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lpoco-export-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    for d in ["ucm", "lab", "bin"] {
        fs::create_dir_all(dir.join(d)).unwrap();
    }
    fs::write(dir.join("ucm/HiFi.conf"), "Syntax 4\n").unwrap();
    fs::write(
        dir.join("lab/surya-awake.service"),
        "[Unit]\nDescription=awake\n",
    )
    .unwrap();
    fs::write(dir.join("bin/surya-glue"), "#!/bin/sh\necho glue\n").unwrap();
    fs::write(dir.join("device.toml"), DEVICE).unwrap();
    dir
}

fn run(c: &mut Command) -> String {
    let o = c.output().unwrap();
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(o.status.success(), "{c:?}:\n{all}");
    all
}

/// export, unpack, and the folder the tarball holds.
fn exported(dir: &Path) -> PathBuf {
    run(Command::new(env!("CARGO_BIN_EXE_lpoco-layer"))
        .args(["export", "--manifest"])
        .arg(dir.join("device.toml"))
        .arg("--out")
        .arg(dir.join("out"))
        .arg("--pkg-version")
        .arg("0.3.0")
        .env("SOURCE_DATE_EPOCH", "1700000000"));
    let tarball = dir.join("out/xiaomi-surya-0.3.0.tar.gz");
    assert!(tarball.exists(), "no tarball in {:?}", dir.join("out"));
    let src = dir.join("src");
    fs::create_dir_all(&src).unwrap();
    run(Command::new("tar")
        .arg("-xzf")
        .arg(&tarball)
        .arg("-C")
        .arg(&src));
    src.join("xiaomi-surya-0.3.0")
}

/// What `make TARGET DESTDIR=root` leaves: every file as `path mode`.
fn installed(tree: &Path, target: &str, tag: &str) -> BTreeSet<String> {
    let root = tree.parent().unwrap().join(format!("root-{tag}"));
    let _ = fs::remove_dir_all(&root);
    run(Command::new("make")
        .arg("-C")
        .arg(tree)
        .arg(target)
        .arg(format!("DESTDIR={}", root.display())));
    let mut out = BTreeSet::new();
    let mut stack = vec![root.clone()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                use std::os::unix::fs::PermissionsExt;
                let mode = fs::metadata(&p).unwrap().permissions().mode() & 0o777;
                out.insert(format!(
                    "/{} {:04o}",
                    p.strip_prefix(&root).unwrap().display(),
                    mode
                ));
            }
        }
    }
    out
}

/// What the format's packages hold, as `path mode`.
fn package_files(format: Format) -> BTreeSet<String> {
    let m = Manifest::parse(DEVICE).unwrap();
    packages(m.layer.as_ref().unwrap(), format)
        .into_iter()
        .flat_map(|p| p.items)
        .map(|i| format!("{} {:04o}", i.dst, i.mode))
        .collect()
}

#[test]
fn make_install_yields_the_files_every_format_shares() {
    let dir = fixture("common");
    let tree = exported(&dir);
    let got = installed(&tree, "install", "common");
    let m = Manifest::parse(DEVICE).unwrap();
    let config: BTreeSet<String> = packages(m.layer.as_ref().unwrap(), Format::Deb)
        .into_iter()
        .filter(|p| p.name == "xiaomi-surya-config")
        .flat_map(|p| p.items)
        .map(|i| format!("{} {:04o}", i.dst, i.mode))
        .collect();
    assert!(!config.is_empty());
    assert_eq!(got, config);
}

#[test]
fn each_distribution_target_yields_its_packages_file_list() {
    // source: research base-distro 2026-11-19-agnostic-layer: Asahi's Makefile of install-fedora, install-arch ...
    let dir = fixture("formats");
    let tree = exported(&dir);
    for (target, format) in [
        ("install-debian", Format::Deb),
        ("install-fedora", Format::Rpm),
        ("install-arch", Format::Archlinux),
        ("install-alpine", Format::Apk),
    ] {
        let got = installed(&tree, target, target);
        assert_eq!(got, package_files(format), "{target}");
    }
}

#[test]
fn nothing_aimed_at_nura_is_exported() {
    let dir = fixture("nura");
    let tree = exported(&dir);
    let all = fs::read_to_string(tree.join("Makefile")).unwrap();
    assert!(!all.contains("deviceinfo"), "{all}");
    assert!(!tree.join("files").join("alpine/etc/deviceinfo").exists());
}

#[test]
fn the_tarball_is_the_same_bytes_every_time() {
    let dir = fixture("repro");
    exported(&dir);
    let sum = |d: &Path| {
        let o = Command::new("sha256sum")
            .arg(d.join("out/xiaomi-surya-0.3.0.tar.gz"))
            .output()
            .unwrap();
        String::from_utf8(o.stdout)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .to_string()
    };
    let first = sum(&dir);
    std::thread::sleep(std::time::Duration::from_secs(2));
    fs::remove_dir_all(dir.join("out")).unwrap();
    exported(&dir);
    assert_eq!(first, sum(&dir));
}

#[test]
fn a_destination_a_makefile_would_misread_is_refused() {
    let dir = fixture("bad");
    fs::write(
        dir.join("device.toml"),
        DEVICE.replace("/usr/libexec/surya-glue", "/usr/libexec/surya glue"),
    )
    .unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_lpoco-layer"))
        .args(["export", "--manifest"])
        .arg(dir.join("device.toml"))
        .arg("--out")
        .arg(dir.join("out"))
        .output()
        .unwrap();
    assert!(!o.status.success());
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("Makefile"),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
}
