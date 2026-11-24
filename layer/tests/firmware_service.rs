// SPDX-License-Identifier: MIT OR Apache-2.0
//! layer-firmware-rule-plain-alpine (decision 2026-11-24-plain-alpine-not-nura): the apk carries its own firmware rule,
//! an OpenRC service that mounts the declared vendor partitions read-only and links the declared files under the
//! kernel's firmware path - no msm-firmware-loader, and no vendor file in any package. The service's script is run
//! here against a fake partition (a folder) with stand-ins for `mount` and `mountpoint`; the real mount of a loop
//! device is in `firmware_containers.rs`. The stand-ins' behaviour is mount(8)'s and mountpoint(1)'s documented one.

use lpoco_layer::{materialize, packages, Format};
use lpoco_manifest::Manifest;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SURYA: &str = r#"
[layer]
name = "xiaomi-surya"

[layer.firmware]
method = "extract-from-partitions"
compatible = "xiaomi,surya"

[[layer.firmware.source]]
partition = "vendor"
origin = "firmware"
destination = "qcom/sm7150/xiaomi/surya"
files = [{ name = "a615_zap.mdt" }, { name = "wlanmdsp.mbn", rename = "wlanmdsp.otaupdate" }]

[[layer.firmware.source]]
partition = "modem"
origin = "image"
destination = "qcom/sm7150/xiaomi/surya"
files = [{ name = "modem.mdt" }]
"#;

fn unique(what: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let d = std::env::temp_dir().join(format!("lpoco-fw-{what}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&d).unwrap();
    d
}

/// The destinations and files of the apk's packages for `toml`.
fn apk_items(toml: &str, dir: &Path) -> Vec<(String, String, String)> {
    let m = Manifest::parse(toml).unwrap();
    let layer = m.layer.as_ref().unwrap();
    packages(layer, Format::Apk)
        .iter()
        .flat_map(|p| {
            materialize(p, dir, &dir.join("stage"))
                .unwrap()
                .into_iter()
                .map(|e| {
                    (
                        p.name.clone(),
                        e.dst,
                        fs::read_to_string(e.src).unwrap_or_default(),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn body(toml: &str, dst: &str) -> String {
    let dir = unique("body");
    apk_items(toml, &dir)
        .into_iter()
        .find(|(_, d, _)| d == dst)
        .unwrap_or_else(|| panic!("{dst} is in no package"))
        .2
}

#[test]
fn the_apk_gets_a_firmware_package_with_the_script_the_table_and_an_openrc_service() {
    let dir = unique("items");
    let items = apk_items(SURYA, &dir);
    for want in [
        "/usr/libexec/xiaomi-surya-firmware",
        "/etc/xiaomi-surya-firmware/sources",
        "/etc/init.d/xiaomi-surya-firmware",
    ] {
        let at = items.iter().find(|(_, d, _)| d == want);
        assert_eq!(
            at.map(|i| i.0.as_str()),
            Some("xiaomi-surya-firmware"),
            "{want} in {:?}",
            items.iter().map(|i| &i.1).collect::<Vec<_>>()
        );
    }
    let table = body(SURYA, "/etc/xiaomi-surya-firmware/sources");
    for line in [
        "vendor firmware qcom/sm7150/xiaomi/surya a615_zap.mdt a615_zap.mdt",
        "vendor firmware qcom/sm7150/xiaomi/surya wlanmdsp.mbn wlanmdsp.otaupdate",
        "modem image qcom/sm7150/xiaomi/surya modem.mdt modem.mdt",
    ] {
        assert!(table.lines().any(|l| l == line), "{line:?} in:\n{table}");
    }
    let init = body(SURYA, "/etc/init.d/xiaomi-surya-firmware");
    assert!(init.starts_with("#!/sbin/openrc-run"), "{init}");
    // enabled on first install, in the sysinit runlevel, where the drivers load
    let m = Manifest::parse(SURYA).unwrap();
    let pkg = packages(m.layer.as_ref().unwrap(), Format::Apk)
        .into_iter()
        .find(|p| p.name == "xiaomi-surya-firmware")
        .unwrap();
    assert!(
        pkg.post_install
            .as_deref()
            .is_some_and(|s| s.contains("rc-update add xiaomi-surya-firmware sysinit")),
        "{:?}",
        pkg.post_install
    );
}

/// The links of the apk's packages: where each is, and what it points at.
fn links(toml: &str) -> Vec<(String, String)> {
    let dir = unique("links");
    let m = Manifest::parse(toml).unwrap();
    packages(m.layer.as_ref().unwrap(), Format::Apk)
        .iter()
        .flat_map(|p| materialize(p, &dir, &dir.join("stage")).unwrap())
        .filter(|e| e.link)
        .map(|e| (e.dst, e.src.to_string_lossy().into_owned()))
        .collect()
}

#[test]
fn the_firmware_is_linked_by_the_package_into_the_mounts_under_run_because_the_root_is_read_only_then(
) {
    let got = links(SURYA);
    for want in [
        (
            "/lib/firmware/qcom/sm7150/xiaomi/surya/a615_zap.mdt",
            "/run/xiaomi-surya-firmware/vendor/firmware/a615_zap.mdt",
        ),
        (
            "/lib/firmware/qcom/sm7150/xiaomi/surya/wlanmdsp.otaupdate",
            "/run/xiaomi-surya-firmware/vendor/firmware/wlanmdsp.mbn",
        ),
        (
            "/lib/firmware/qcom/sm7150/xiaomi/surya/modem.mdt",
            "/run/xiaomi-surya-firmware/modem/image/modem.mdt",
        ),
    ] {
        assert!(
            got.contains(&(want.0.to_string(), want.1.to_string())),
            "{want:?} in {got:?}"
        );
    }
    assert_eq!(got.len(), 3, "a link beyond the declared files: {got:?}");
    let init = body(SURYA, "/etc/init.d/xiaomi-surya-firmware");
    // before the drivers probe (hwdrivers, in sysinit, loads them)
    assert!(init.contains("before modules hwdrivers"), "{init}");
    let m = Manifest::parse(SURYA).unwrap();
    let pkg = packages(m.layer.as_ref().unwrap(), Format::Apk)
        .into_iter()
        .find(|p| p.name == "xiaomi-surya-firmware")
        .unwrap();
    assert!(
        pkg.depends.contains(&"openrc".to_string()),
        "{:?}",
        pkg.depends
    );
}

#[test]
fn no_other_format_gets_the_service_and_no_package_holds_a_vendor_file() {
    let m = Manifest::parse(SURYA).unwrap();
    let layer = m.layer.as_ref().unwrap();
    for f in [Format::Deb, Format::Rpm, Format::Archlinux] {
        assert!(
            packages(layer, f)
                .iter()
                .all(|p| !p.name.ends_with("-firmware")),
            "{f:?}"
        );
    }
    // under /lib/firmware the apk holds links only: each file of a package there would be a vendor file
    let dir = unique("novendor");
    for p in packages(layer, Format::Apk) {
        for e in materialize(&p, &dir, &dir.join("stage")).unwrap() {
            assert!(
                e.link || !e.dst.starts_with("/lib/firmware"),
                "{} packs {}",
                p.name,
                e.dst
            );
        }
    }
}

fn say(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// A root with a fake partition per name (a folder under dev/disk/by-partlabel, holding `files`), and `mount` and
/// `mountpoint` stand-ins: `mount -o ro SRC DST` fails when SRC is no folder, else makes DST a link to SRC; DST is a
/// mount point when it is a link.
fn run(root: &Path, partitions: &[(&str, &[&str], &str)]) -> Output {
    let by = root.join("dev/disk/by-partlabel");
    for (part, files, folder) in partitions {
        let at = by.join(part).join(folder);
        fs::create_dir_all(&at).unwrap();
        for f in *files {
            fs::write(at.join(f), format!("{part}/{folder}/{f}")).unwrap();
        }
    }
    let bin = root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    say(
        &bin.join("mount"),
        "[ \"$1\" = -o ] && [ \"$2\" = ro ] || exit 64\n[ -d \"$3\" ] || exit 32\nrmdir \"$4\"\nln -s \"$3\" \"$4\"",
    );
    say(&bin.join("mountpoint"), "[ \"$1\" = -q ] && [ -L \"$2\" ]");
    let script = root.join("script");
    fs::write(&script, body(SURYA, "/usr/libexec/xiaomi-surya-firmware")).unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let table = root.join("etc/xiaomi-surya-firmware");
    fs::create_dir_all(&table).unwrap();
    fs::write(
        table.join("sources"),
        body(SURYA, "/etc/xiaomi-surya-firmware/sources"),
    )
    .unwrap();
    Command::new("sh")
        .arg(&script)
        .env("LPOCO_FW_ROOT", root)
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .output()
        .unwrap()
}

/// What the file at `target` (a path of the phone's root) holds in the fake root.
fn read_in(root: &Path, target: &str) -> std::io::Result<String> {
    fs::read_to_string(root.join(target.trim_start_matches('/')))
}

#[test]
fn the_service_mounts_the_partitions_so_every_link_of_the_package_reads_its_file_and_no_other_file_has_one(
) {
    let root = unique("run");
    let o = run(
        &root,
        &[
            (
                "vendor",
                &["a615_zap.mdt", "wlanmdsp.mbn", "secret.bin"],
                "firmware",
            ),
            ("modem", &["modem.mdt"], "image"),
        ],
    );
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    for (dst, target) in links(SURYA) {
        let file = target.rsplit('/').next().unwrap();
        let part = target.split('/').nth(3).unwrap();
        assert!(
            read_in(&root, &target).unwrap().ends_with(&format!(
                "{part}/{}/{file}",
                target.split('/').nth(4).unwrap()
            )),
            "{dst} -> {target}"
        );
    }
    assert!(links(SURYA).iter().all(|(_, t)| !t.ends_with("secret.bin")));
    // a second run changes nothing and does not fail (a restart of the service)
    let again = Command::new("sh")
        .arg(root.join("script"))
        .env("LPOCO_FW_ROOT", &root)
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", root.join("bin").display()),
        )
        .output()
        .unwrap();
    assert_eq!(
        again.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&again.stderr)
    );
}

#[test]
fn a_declared_file_the_partition_lacks_is_named_and_ends_the_run_after_the_others_are_checked() {
    let root = unique("missing");
    let o = run(
        &root,
        &[
            ("vendor", &["a615_zap.mdt"], "firmware"),
            ("modem", &["modem.mdt"], "image"),
        ],
    );
    assert_ne!(o.status.code(), Some(0));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("vendor/firmware/wlanmdsp.mbn"), "{err}");
    assert!(
        !err.contains("modem/image"),
        "a present file is named: {err}"
    );
}

#[test]
fn a_partition_that_does_not_mount_is_named_and_the_others_are_still_mounted() {
    let root = unique("nopart");
    let o = run(
        &root,
        &[("vendor", &["a615_zap.mdt", "wlanmdsp.mbn"], "firmware")],
    );
    assert_ne!(o.status.code(), Some(0));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("cannot mount partition modem"), "{err}");
    assert!(
        read_in(
            &root,
            "/run/xiaomi-surya-firmware/vendor/firmware/a615_zap.mdt"
        )
        .is_ok(),
        "the other partition is not mounted"
    );
}
