// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco layer pack` (proof: lpoco-layer-pack): a device's [layer], from a fixture, becomes one nfpm config
//! per package and format - the declared files and kernel-install's devicetree the same in every format, the
//! format's own glue (systemd preset, initramfs module list) in a boot package that apk does not get.

use lpoco_layer::{check, materialize, nfpm_yaml, packages, post_install, stale, Format};
use lpoco_manifest::Manifest;
use std::fs;
use std::path::PathBuf;

const DEVICE: &str = r#"
[layer]
name = "xiaomi-surya"
needs = ["qrtr", "tqftpserv>=1.2"]

[layer.kernel]
tree = "https://example.org/linux"
tag = "v7.1"
dtbs = ["qcom/sm7150-xiaomi-surya-huaxing.dtb", "qcom/sm7150-xiaomi-surya-tianma.dtb"]

[layer.boot]
method = "efi-kernel-install"

[layer.initramfs]
modules = ["ufs_qcom", "phy_qcom_qmp_ufs"]

[[layer.file]]
from = "ucm/HiFi.conf"
to = "/usr/share/alsa/ucm2/Xiaomi/surya/HiFi.conf"
kind = "data"

[[layer.file]]
from = "lab/surya-awake.service"
to = "/usr/lib/systemd/system/surya-awake.service"
kind = "unit"
"#;

fn fixture(tag: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("lpoco-layer-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("ucm")).unwrap();
    fs::create_dir_all(dir.join("lab")).unwrap();
    fs::write(dir.join("ucm/HiFi.conf"), "Syntax 4\n").unwrap();
    fs::write(dir.join("lab/surya-awake.service"), "[Unit]\n").unwrap();
    (dir.clone(), dir.join("stage"))
}

fn dests(format: Format, tag: &str) -> Vec<(String, Vec<String>)> {
    let (dir, stage) = fixture(tag);
    let m = Manifest::parse(DEVICE).unwrap();
    packages(m.layer.as_ref().unwrap(), format)
        .iter()
        .map(|p| {
            let mut d: Vec<String> = materialize(p, &dir, &stage)
                .unwrap()
                .into_iter()
                .map(|e| e.dst)
                .collect();
            d.sort();
            (p.name.clone(), d)
        })
        .collect()
}

#[test]
fn the_declared_files_are_the_same_in_every_format() {
    let apk = dests(Format::Apk, "a");
    let deb = dests(Format::Deb, "d");
    let config = |l: &Vec<(String, Vec<String>)>| {
        l.iter()
            .find(|p| p.0 == "xiaomi-surya-config")
            .unwrap()
            .1
            .clone()
    };
    assert_eq!(
        config(&apk),
        vec![
            "/usr/lib/kernel/devicetree",
            "/usr/lib/systemd/system/surya-awake.service",
            "/usr/share/alsa/ucm2/Xiaomi/surya/HiFi.conf"
        ]
    );
    assert_eq!(config(&apk), config(&deb));
}

#[test]
fn apk_gets_no_boot_package_and_deb_gets_a_preset_and_an_initramfs_hook() {
    assert_eq!(dests(Format::Apk, "a2").len(), 1);
    let deb = dests(Format::Deb, "d2");
    let boot = &deb.iter().find(|p| p.0 == "xiaomi-surya-boot").unwrap().1;
    for want in [
        "/usr/lib/systemd/system-preset/50-xiaomi-surya.preset",
        "/usr/share/initramfs-tools/hooks/xiaomi-surya",
    ] {
        assert!(boot.iter().any(|d| d == want), "{want} not in {boot:?}");
    }
}

#[test]
fn the_other_formats_name_their_own_initramfs_tool() {
    let want = [
        (
            Format::Rpm,
            "/usr/lib/dracut/dracut.conf.d/50-xiaomi-surya.conf",
        ),
        (
            Format::Archlinux,
            "/etc/mkinitcpio.conf.d/xiaomi-surya.conf",
        ),
    ];
    for (f, path) in want {
        let boot = dests(f, "o")
            .into_iter()
            .find(|p| p.0 == "xiaomi-surya-boot")
            .unwrap()
            .1;
        assert!(boot.iter().any(|d| d == path), "{path} not in {boot:?}");
    }
}

#[test]
fn the_generated_files_say_what_the_declaration_says() {
    let (dir, stage) = fixture("g");
    let m = Manifest::parse(DEVICE).unwrap();
    let pkgs = packages(m.layer.as_ref().unwrap(), Format::Deb);
    let entries: Vec<_> = pkgs
        .iter()
        .flat_map(|p| materialize(p, &dir, &stage).unwrap())
        .collect();
    let text = |dst: &str| {
        let e = entries.iter().find(|e| e.dst == dst).unwrap();
        fs::read_to_string(&e.src).unwrap()
    };
    assert_eq!(
        text("/usr/lib/kernel/devicetree"),
        "qcom/sm7150-xiaomi-surya-huaxing.dtb\n"
    );
    assert_eq!(
        text("/usr/lib/systemd/system-preset/50-xiaomi-surya.preset"),
        "enable surya-awake.service\n"
    );
    let hook = text("/usr/share/initramfs-tools/hooks/xiaomi-surya");
    assert!(hook.contains("manual_add_modules ufs_qcom\n"), "{hook}");
    assert!(
        hook.contains("manual_add_modules phy_qcom_qmp_ufs\n"),
        "{hook}"
    );
    let hook_entry = entries
        .iter()
        .find(|e| e.dst.ends_with("hooks/xiaomi-surya"))
        .unwrap();
    assert_eq!(hook_entry.mode, 0o755);
}

#[test]
fn dependencies_are_written_the_way_each_format_writes_them() {
    let (dir, stage) = fixture("y");
    let m = Manifest::parse(DEVICE).unwrap();
    let layer = m.layer.as_ref().unwrap();
    let yaml = |f: Format| {
        let p = packages(layer, f)
            .into_iter()
            .find(|p| p.name == "xiaomi-surya-config")
            .unwrap();
        let e = materialize(&p, &dir, &stage).unwrap();
        nfpm_yaml(&p, f, "0.1.0", &e, None).unwrap()
    };
    assert!(yaml(Format::Deb).contains("- tqftpserv (>= 1.2)"));
    assert!(yaml(Format::Rpm).contains("- tqftpserv >= 1.2"));
    assert!(yaml(Format::Apk).contains("- tqftpserv>=1.2"));
    assert!(yaml(Format::Apk).contains("- qrtr\n"));
    assert!(yaml(Format::Deb).contains("name: xiaomi-surya-config"));
    assert!(yaml(Format::Deb).contains("arch: all"));
}

#[test]
fn a_file_the_layer_names_but_the_tree_lacks_is_refused() {
    let (dir, stage) = fixture("m");
    fs::remove_file(dir.join("ucm/HiFi.conf")).unwrap();
    let m = Manifest::parse(DEVICE).unwrap();
    let p = packages(m.layer.as_ref().unwrap(), Format::Apk).remove(0);
    let e = materialize(&p, &dir, &stage).unwrap_err();
    assert!(e.contains("ucm/HiFi.conf"), "{e}");
}

#[test]
fn debians_strict_bounds_are_doubled_and_a_malformed_token_is_refused() {
    let (dir, stage) = fixture("s");
    let m = Manifest::parse(DEVICE).unwrap();
    let mut layer = m.layer.clone().unwrap();
    layer.needs = vec!["a>1".into(), "c<2".into(), "d<=3".into(), "e=4".into()];
    let p = packages(&layer, Format::Deb).remove(0);
    let e = materialize(&p, &dir, &stage).unwrap();
    let y = nfpm_yaml(&p, Format::Deb, "0.1.0", &e, None).unwrap();
    for want in ["- a (>> 1)", "- c (<< 2)", "- d (<= 3)", "- e (= 4)"] {
        assert!(y.contains(want), "{want} not in {y}");
    }
    for bad in ["b>=", ">=1", "b=>2"] {
        layer.needs = vec![bad.into()];
        let p = packages(&layer, Format::Deb).remove(0);
        assert!(
            nfpm_yaml(&p, Format::Deb, "0.1.0", &e, None).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn a_kept_config_names_its_files_by_absolute_path() {
    let (dir, stage) = fixture("p");
    let m = Manifest::parse(DEVICE).unwrap();
    let p = packages(m.layer.as_ref().unwrap(), Format::Apk).remove(0);
    let e = materialize(&p, &dir, &stage).unwrap();
    assert!(e.iter().all(|e| e.src.is_absolute()), "{e:?}");
}

#[test]
fn a_layer_that_cannot_be_packed_right_is_refused_before_anything_is_written() {
    let m = Manifest::parse(DEVICE).unwrap();
    assert!(check(m.layer.as_ref().unwrap(), Format::Deb).is_ok());
    // kernel-install needs the devicetree file: without a dtb the package would boot nothing
    let mut layer = m.layer.clone().unwrap();
    layer.kernel.as_mut().unwrap().dtbs.clear();
    let e = check(&layer, Format::Deb).unwrap_err();
    layer.kernel = None;
    assert!(check(&layer, Format::Deb).is_err());
    assert!(e.contains("dtbs"), "{e}");
    // a module name goes into a shell hook and a config line: one word of kernel-module characters only
    for bad in ["a b", "a;rm", "", "x\ny", "$(id)"] {
        let mut layer = m.layer.clone().unwrap();
        layer.initramfs.as_mut().unwrap().modules = vec![bad.into()];
        let e = check(&layer, Format::Deb).unwrap_err();
        layer.kernel = None;
        assert!(check(&layer, Format::Deb).is_err());
        assert!(e.contains("module"), "{bad:?}: {e}");
    }
}

#[test]
fn a_boot_package_an_earlier_run_left_is_named_when_this_format_makes_none() {
    let (dir, _) = fixture("t");
    let m = Manifest::parse(DEVICE).unwrap();
    let layer = m.layer.as_ref().unwrap();
    let old = dir.join(Format::Deb.file_name("xiaomi-surya-boot", "0.1.0"));
    fs::write(&old, "x").unwrap();
    // the same format still makes -boot: nothing is stale
    assert!(stale(layer, Format::Deb, "0.1.0", &dir).is_empty());
    // a layer that no longer has boot glue leaves it stale
    let mut l = layer.clone();
    l.initramfs = None;
    l.file.retain(|f| f.kind != lpoco_manifest::FileKind::Unit);
    assert_eq!(stale(&l, Format::Deb, "0.1.0", &dir), vec![old]);
}

#[test]
fn the_boot_package_applies_its_preset_after_install_and_follows_the_config_package() {
    let (dir, stage) = fixture("q");
    let m = Manifest::parse(DEVICE).unwrap();
    let layer = m.layer.as_ref().unwrap();
    let boot = packages(layer, Format::Deb)
        .into_iter()
        .find(|p| p.name == "xiaomi-surya-boot")
        .unwrap();
    // the units sit in -config: -boot must be unpacked after it for the preset to find them
    assert_eq!(boot.depends, vec!["xiaomi-surya-config".to_string()]);
    let script = post_install(&boot, &stage).unwrap().unwrap();
    let body = fs::read_to_string(&script).unwrap();
    assert!(body.starts_with("#!/bin/sh\n"), "{body}");
    assert!(
        body.contains("systemctl preset surya-awake.service"),
        "{body}"
    );
    let e = materialize(&boot, &dir, &stage).unwrap();
    let y = nfpm_yaml(&boot, Format::Deb, "0.1.0", &e, Some(&script)).unwrap();
    assert!(y.contains("scripts:\n  postinstall: \""), "{y}");
    assert!(y.contains("- xiaomi-surya-config\n"), "{y}");
    // no unit, no preset, no script
    let mut l = layer.clone();
    l.file.retain(|f| f.kind != lpoco_manifest::FileKind::Unit);
    let boot = packages(&l, Format::Deb)
        .into_iter()
        .find(|p| p.name == "xiaomi-surya-boot")
        .unwrap();
    assert!(post_install(&boot, &stage).unwrap().is_none());
}

#[test]
fn a_unit_name_that_would_run_as_root_in_the_scriptlet_is_refused() {
    let m = Manifest::parse(DEVICE).unwrap();
    for bad in ["a;rm -rf x.service", "x$(id).service", "a b.service"] {
        let mut layer = m.layer.clone().unwrap();
        layer
            .file
            .iter_mut()
            .find(|f| f.kind == lpoco_manifest::FileKind::Unit)
            .unwrap()
            .to = format!("/usr/lib/systemd/system/{bad}");
        let e = check(&layer, Format::Deb).unwrap_err();
        assert!(e.contains("unit"), "{bad:?}: {e}");
        // apk writes no preset and no scriptlet
        assert!(check(&layer, Format::Apk).is_ok(), "{bad:?}");
    }
}

#[test]
fn the_preset_runs_on_first_install_only_in_each_format_and_also_in_a_chroot() {
    let m = Manifest::parse(DEVICE).unwrap();
    let layer = m.layer.as_ref().unwrap();
    let script = |f: Format| {
        packages(layer, f)
            .into_iter()
            .find(|p| p.name == "xiaomi-surya-boot")
            .unwrap()
            .post_install
            .unwrap()
    };
    assert!(script(Format::Deb).contains("[ -z \"$2\" ]"));
    assert!(script(Format::Rpm).contains("[ \"$1\" = 1 ]"));
    assert!(!script(Format::Archlinux).contains("$1"));
    for f in [Format::Deb, Format::Rpm, Format::Archlinux] {
        let s = script(f);
        assert!(!s.contains("/run/systemd"), "{s}");
        assert!(s.contains("command -v systemctl"), "{s}");
    }
    assert!(packages(layer, Format::Apk)
        .iter()
        .all(|p| p.post_install.is_none()));
}
