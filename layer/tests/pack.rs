// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco layer pack` (proof: lpoco-layer-pack): a device's [layer], from a fixture, becomes one nfpm config
//! per package and format - the declared files and kernel-install's devicetree the same in every format, the
//! format's own glue (systemd preset, initramfs module list) in a boot package that apk does not get.

use lpoco_layer::{
    add_deviceinfo, check, deviceinfo, kernel_package, materialize, nfpm_yaml, packages,
    post_install, stale, Format,
};
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
fn apk_gets_no_preset_and_deb_gets_a_preset_and_an_initramfs_hook() {
    let apk = dests(Format::Apk, "a2");
    let boot = &apk.iter().find(|p| p.0 == "xiaomi-surya-boot").unwrap().1;
    assert!(!boot.iter().any(|d| d.contains("preset")), "{boot:?}");
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
    assert!(stale(layer, Format::Deb, "0.1.0", &dir, false).is_empty());
    // a layer that no longer has boot glue leaves it stale
    let mut l = layer.clone();
    l.initramfs = None;
    l.file.retain(|f| f.kind != lpoco_manifest::FileKind::Unit);
    assert_eq!(stale(&l, Format::Deb, "0.1.0", &dir, false), vec![old]);
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
    assert!(packages(layer, Format::Apk).iter().all(|p| p
        .post_install
        .as_deref()
        .is_none_or(|s| !s.contains("systemctl"))));
}

#[test]
fn a_dependency_token_is_named_per_format_by_the_layers_map_and_kept_when_unmapped() {
    let device = DEVICE.replace(
        "[layer.kernel]",
        "[layer.names.deb]\nqrtr = \"qrtr-tools\"\ntqftpserv = \"tqftpserv-bin\"\n\n[layer.names.rpm]\nqrtr = \"qrtr\"\n\n[layer.kernel]",
    );
    let m = Manifest::parse(&device).unwrap();
    let layer = m.layer.as_ref().unwrap();
    let (dir, stage) = fixture("n");
    let want = [
        (Format::Deb, ["- qrtr-tools", "- tqftpserv-bin (>= 1.2)"]),
        (Format::Rpm, ["- qrtr\n", "- tqftpserv >= 1.2"]),
        (Format::Apk, ["- qrtr\n", "- tqftpserv>=1.2"]),
        (Format::Archlinux, ["- qrtr\n", "- tqftpserv>=1.2"]),
    ];
    for (format, wants) in want {
        let p = packages(layer, format).remove(0);
        let e = materialize(&p, &dir, &stage).unwrap();
        let y = nfpm_yaml(&p, format, "0.1.0", &e, None).unwrap();
        for w in wants {
            assert!(y.contains(w), "{format:?}: {w:?} not in {y}");
        }
    }
}

#[test]
fn a_name_map_for_a_token_the_layer_does_not_need_is_refused() {
    let device = DEVICE.replace(
        "[layer.kernel]",
        "[layer.names.deb]\nqrtrr = \"qrtr-tools\"\n\n[layer.kernel]",
    );
    let e = Manifest::parse(&device).unwrap_err();
    assert!(e.contains("qrtrr"), "{e}");
    let device = DEVICE.replace(
        "[layer.kernel]",
        "[layer.names.deb]\nqrtr = \"two words\"\n\n[layer.kernel]",
    );
    assert!(Manifest::parse(&device).is_err());
    let device = DEVICE.replace(
        "[layer.kernel]",
        "[layer.names.deb]\nqrtr = \"qrtr>=1\"\n\n[layer.kernel]",
    );
    assert!(Manifest::parse(&device).is_err());
}

/// A finished kernel build as `lpoco layer pack --kernel-build` reads it: Image, kernel.release, the declared
/// dtbs under dtbs/, and the module tree (what belongs in /usr/lib/modules/RELEASE) under modules/.
fn kernel_build(tag: &str, release: &str) -> PathBuf {
    let b = fixture(tag).0.join("kbuild");
    fs::create_dir_all(b.join("dtbs/qcom")).unwrap();
    fs::create_dir_all(b.join("modules/kernel")).unwrap();
    fs::write(b.join("Image"), "image").unwrap();
    fs::write(b.join("kernel.release"), format!("{release}\n")).unwrap();
    for d in ["sm7150-xiaomi-surya-huaxing", "sm7150-xiaomi-surya-tianma"] {
        fs::write(b.join(format!("dtbs/qcom/{d}.dtb")), "dtb").unwrap();
    }
    fs::write(b.join("modules/modules.order"), "").unwrap();
    b
}

#[test]
fn the_kernel_package_lays_out_what_kernel_install_reads_and_is_arm64() {
    let b = kernel_build("k1", "7.1.0-surya");
    let (dir, stage) = fixture("k1b");
    let m = Manifest::parse(DEVICE).unwrap();
    let layer = m.layer.as_ref().unwrap();
    let p = kernel_package(layer, &b).unwrap();
    assert_eq!(p.name, "linux-xiaomi-surya");
    assert_eq!(p.arch, Some("arm64"));
    let mut dst: Vec<String> = materialize(&p, &dir, &stage)
        .unwrap()
        .into_iter()
        .map(|e| e.dst)
        .collect();
    dst.sort();
    assert_eq!(
        dst,
        [
            "/usr/lib/modules/7.1.0-surya",
            "/usr/lib/modules/7.1.0-surya/dtb/qcom/sm7150-xiaomi-surya-huaxing.dtb",
            "/usr/lib/modules/7.1.0-surya/dtb/qcom/sm7150-xiaomi-surya-tianma.dtb",
            "/usr/lib/modules/7.1.0-surya/vmlinuz",
        ]
    );
    let es = materialize(&p, &dir, &stage).unwrap();
    let y = nfpm_yaml(&p, Format::Deb, "0.1.0", &es, None).unwrap();
    assert!(y.contains("arch: arm64\n"), "{y}");
    assert!(y.contains("type: tree"), "{y}");
    // each format writes the architecture its own way, in the file name too
    assert_eq!(
        Format::Deb.file_name_for("linux-x", "1", Some("arm64")),
        "linux-x_1_arm64.deb"
    );
    assert_eq!(
        Format::Rpm.file_name_for("linux-x", "1", Some("arm64")),
        "linux-x-1-1.aarch64.rpm"
    );
    assert_eq!(
        Format::Archlinux.file_name_for("linux-x", "1", Some("arm64")),
        "linux-x-1-1-aarch64.pkg.tar.zst"
    );
    assert_eq!(
        Format::Apk.file_name_for("linux-x", "1", Some("arm64")),
        "linux-x-1-r0.apk"
    );
}

#[test]
fn a_kernel_build_that_cannot_be_packed_right_is_refused_and_says_why() {
    let m = Manifest::parse(DEVICE).unwrap();
    let layer = m.layer.as_ref().unwrap();
    // a declared dtb the build lacks
    let b = kernel_build("k2", "7.1.0");
    fs::remove_file(b.join("dtbs/qcom/sm7150-xiaomi-surya-tianma.dtb")).unwrap();
    let e = kernel_package(layer, &b).unwrap_err();
    assert!(e.contains("sm7150-xiaomi-surya-tianma.dtb"), "{e}");
    // no Image
    let b = kernel_build("k3", "7.1.0");
    fs::remove_file(b.join("Image")).unwrap();
    assert!(kernel_package(layer, &b).unwrap_err().contains("Image"));
    // a release that would escape /usr/lib/modules or break a path
    for bad in ["../x", "a b", "", "a/b"] {
        let b = kernel_build("k4", bad);
        let e = kernel_package(layer, &b).unwrap_err();
        assert!(e.contains("release"), "{bad:?}: {e}");
    }
    // a layer with no [layer.kernel] has nothing to put the build under
    let mut l = layer.clone();
    l.kernel = None;
    let b = kernel_build("k5", "7.1.0");
    assert!(kernel_package(&l, &b)
        .unwrap_err()
        .contains("[layer.kernel]"));
}

#[test]
fn the_modules_tree_keeps_its_own_file_modes_and_a_clash_with_the_kernels_files_is_refused() {
    let b = kernel_build("k6", "7.1.0");
    let (dir, stage) = fixture("k6b");
    let m = Manifest::parse(DEVICE).unwrap();
    let layer = m.layer.as_ref().unwrap();
    let p = kernel_package(layer, &b).unwrap();
    let es = materialize(&p, &dir, &stage).unwrap();
    let y = nfpm_yaml(&p, Format::Deb, "0.1.0", &es, None).unwrap();
    // per entry: the tree has no mode line (nfpm would stamp it on every module), the files do
    let entries: Vec<&str> = y.split("\n- src:").skip(1).collect();
    assert_eq!(entries.len(), 4, "{y}");
    for e in &entries {
        let tree = e.contains("type: tree");
        assert_eq!(tree, e.contains("dst: \"/usr/lib/modules/7.1.0\"\n"), "{e}");
        assert_eq!(tree, !e.contains("mode:"), "{e}");
    }
    // no modules folder: no tree entry
    fs::remove_dir_all(b.join("modules")).unwrap();
    let p = kernel_package(layer, &b).unwrap();
    assert!(p
        .items
        .iter()
        .all(|i| !matches!(i.source, lpoco_layer::Source::Dir(_))));
    // a module tree that holds the kernel's own file names would overwrite them
    fs::create_dir_all(b.join("modules")).unwrap();
    fs::write(b.join("modules/vmlinuz"), "x").unwrap();
    assert!(kernel_package(layer, &b).unwrap_err().contains("overwrite"));
}

#[test]
fn a_kernel_package_an_earlier_run_left_is_stale_when_no_build_is_given_now() {
    let (dir, _) = fixture("k7");
    let m = Manifest::parse(DEVICE).unwrap();
    let layer = m.layer.as_ref().unwrap();
    let old = dir.join(Format::Deb.file_name_for("linux-xiaomi-surya", "0.1.0", Some("arm64")));
    fs::write(&old, "x").unwrap();
    assert_eq!(stale(layer, Format::Deb, "0.1.0", &dir, false), vec![old]);
    assert!(stale(layer, Format::Deb, "0.1.0", &dir, true).is_empty());
}

/// Runs a shell snippet in `dir` and returns what it printed; the test's stand-in for mkinitfs and the package manager.
fn sh(dir: &std::path::Path, script: &str) -> String {
    let o = std::process::Command::new("sh")
        .arg("-c")
        .arg(script)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(o.status.success(), "{script}: {o:?}");
    String::from_utf8(o.stdout).unwrap() + &String::from_utf8(o.stderr).unwrap()
}

#[test]
fn apk_names_its_modules_as_mkinitfs_reads_them_and_turns_the_feature_on_once() {
    let (dir, stage) = fixture("mk");
    let m = Manifest::parse(DEVICE).unwrap();
    let layer = m.layer.as_ref().unwrap();
    assert!(check(layer, Format::Apk).is_ok());
    let pkgs = packages(layer, Format::Apk);
    let boot = pkgs.iter().find(|p| p.name == "xiaomi-surya-boot").unwrap();
    assert_eq!(
        boot.depends,
        vec!["xiaomi-surya-config".to_string(), "mkinitfs".to_string()]
    );
    let es = materialize(boot, &dir, &stage).unwrap();
    assert_eq!(es.len(), 1);
    assert_eq!(es[0].dst, "/etc/mkinitfs/features.d/xiaomi-surya.modules");
    // mkinitfs globs each line against files under /lib/modules/KERNEL: a bare name matches nothing
    let kd = dir.join("kmods");
    let deep = kd.join("kernel/drivers/ufs/host");
    fs::create_dir_all(&deep).unwrap();
    fs::write(deep.join("ufs-qcom.ko.zst"), "").unwrap();
    let other = kd.join("kernel/drivers/phy/qualcomm");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("phy-qcom-qmp-ufs.ko"), "").unwrap();
    fs::write(other.join("phy-qcom-qmp-pcie.ko"), "").unwrap();
    fs::write(other.join("phy-qcom-qmp-ufs-extra.ko"), "").unwrap();
    fs::write(deep.join("ufs-qcom-ice.ko"), "").unwrap();
    let seven = kd.join("kernel/a/b/c/d/e/f/g");
    fs::create_dir_all(&seven).unwrap();
    fs::write(seven.join("phy_qcom_qmp_ufs.ko.xz"), "").unwrap();
    let list = es[0].src.display().to_string();
    let found = sh(
        &kd,
        &format!(
            "sed -e '/^$/d' -e '/^#/d' {list} | while IFS= read -r glob; do for f in $glob; do [ -e \"$f\" ] && echo \"$f\"; done; done | sort -u"
        ),
    );
    assert_eq!(
        found,
        "kernel/a/b/c/d/e/f/g/phy_qcom_qmp_ufs.ko.xz\nkernel/drivers/phy/qualcomm/phy-qcom-qmp-ufs.ko\nkernel/drivers/ufs/host/ufs-qcom.ko.zst\n"
    );
    // the scriptlet switches the feature on in mkinitfs.conf once and keeps what the user set
    let s = boot.post_install.as_ref().unwrap();
    let conf = dir.join("mkinitfs.conf");
    let run = |content: Option<&str>| {
        match content {
            Some(c) => fs::write(&conf, c).unwrap(),
            None => drop(fs::remove_file(&conf)),
        }
        let script = s.replace("/etc/mkinitfs/mkinitfs.conf", &conf.display().to_string());
        let out = sh(&dir, &script);
        (fs::read_to_string(&conf).unwrap_or_default(), out)
    };
    let (c, _) = run(Some("features=\"ata base ext4\"\n# mine\n"));
    assert_eq!(c, "features=\"ata base ext4 xiaomi-surya\"\n# mine\n");
    let (c2, _) = run(Some(&c));
    assert_eq!(c2, c, "a second install changes nothing");
    // NAME already there - first, middle, alone - is left; a longer name that starts with it is not NAME
    for have in [
        "xiaomi-surya base",
        "base xiaomi-surya ext4",
        "xiaomi-surya",
    ] {
        let conf_text = format!("features=\"{have}\"\n");
        assert_eq!(run(Some(&conf_text)).0, conf_text, "{have}");
    }
    assert_eq!(
        run(Some("features=\"base xiaomi-surya-old\"\n")).0,
        "features=\"base xiaomi-surya-old xiaomi-surya\"\n"
    );
    // a dot in the layer name is a dot, not any character
    let mut dotted = layer.clone();
    dotted.name = "a.b".into();
    let pk = packages(&dotted, Format::Apk);
    let s2 = pk
        .iter()
        .find(|p| p.name == "a.b-boot")
        .unwrap()
        .post_install
        .clone()
        .unwrap();
    let script = s2.replace("/etc/mkinitfs/mkinitfs.conf", &conf.display().to_string());
    fs::write(&conf, "features=\"base aXb\"\n").unwrap();
    sh(&dir, &script);
    assert_eq!(
        fs::read_to_string(&conf).unwrap(),
        "features=\"base aXb a.b\"\n"
    );
    let (c3, w) = run(Some("# no features line\n"));
    assert_eq!(c3, "# no features line\n");
    assert!(w.contains("xiaomi-surya") && w.contains("by hand"), "{w}");
    let (_, w) = run(None);
    assert!(w.contains("by hand"), "a missing conf is said: {w}");
    // no modules, no boot package for apk
    let mut l = layer.clone();
    l.initramfs = None;
    assert!(packages(&l, Format::Apk)
        .iter()
        .all(|p| p.name != "xiaomi-surya-boot"));
}

#[test]
fn a_modules_symlink_that_leaves_the_tree_is_refused_and_one_inside_is_kept() {
    let b = kernel_build("k8", "7.1.0");
    let m = Manifest::parse(DEVICE).unwrap();
    let layer = m.layer.as_ref().unwrap();
    // a link inside the tree stays a link in the package
    fs::create_dir_all(b.join("modules/kernel/a")).unwrap();
    fs::write(b.join("modules/kernel/a/x.ko"), "x").unwrap();
    std::os::unix::fs::symlink("kernel/a/x.ko", b.join("modules/x.ko")).unwrap();
    let r = kernel_package(layer, &b);
    assert!(r.is_ok(), "{:?}", r.err());
    // `build` and `source` point at the build machine's tree: they would dangle on the phone
    std::os::unix::fs::symlink("/home/lab/linux", b.join("modules/build")).unwrap();
    let e = kernel_package(layer, &b).unwrap_err();
    assert!(e.contains("modules/build") && e.contains("outside"), "{e}");
    fs::remove_file(b.join("modules/build")).unwrap();
    std::os::unix::fs::symlink("../../../linux", b.join("modules/source")).unwrap();
    let e = kernel_package(layer, &b).unwrap_err();
    assert!(e.contains("modules/source"), "{e}");
}

#[test]
fn a_link_that_climbs_out_through_another_link_or_a_linked_modules_folder_is_refused() {
    let b = kernel_build("k9", "7.1.0");
    let layer_m = Manifest::parse(DEVICE).unwrap();
    let layer = layer_m.layer.as_ref().unwrap();
    fs::create_dir_all(b.join("modules/kernel/a/b")).unwrap();
    std::os::unix::fs::symlink("../..", b.join("modules/kernel/a/b/l1")).unwrap();
    std::os::unix::fs::symlink("kernel/a/b/l1/../..", b.join("modules/x")).unwrap();
    assert!(kernel_package(layer, &b).unwrap_err().contains("modules/x"));
    fs::remove_file(b.join("modules/x")).unwrap();
    fs::remove_file(b.join("modules/kernel/a/b/l1")).unwrap();
    assert!(kernel_package(layer, &b).is_ok());
    let real = b.join("elsewhere");
    fs::rename(b.join("modules"), &real).unwrap();
    std::os::unix::fs::symlink(&real, b.join("modules")).unwrap();
    assert!(kernel_package(layer, &b).unwrap_err().contains("is a link"));
}

const WALLEYE: &str = r#"
[device]
codename = "google-walleye"

[boot]
base = 0
kernel_offset = 0x00008000
ramdisk_offset = 0x01000000
tags_offset = 0x00000100
pagesize = 4096
cmdline = "console=ttyMSM0,115200n8 androidboot.hardware=walleye"

[layer]
name = "google-walleye"

[[layer.file]]
from = "ucm/HiFi.conf"
to = "/usr/share/alsa/ucm2/Google/walleye/HiFi.conf"
kind = "data"
"#;

#[test]
fn nura_gets_the_phones_deviceinfo_from_boot_and_the_others_do_not() {
    let (dir, stage) = fixture("di1");
    let m = Manifest::parse(WALLEYE).unwrap();
    let body = deviceinfo(&m)
        .unwrap()
        .expect("a [boot] makes a deviceinfo");
    for want in [
        "deviceinfo_codename=\"google-walleye\"\n",
        "deviceinfo_arch=\"aarch64\"\n",
        "deviceinfo_flash_pagesize=\"4096\"\n",
        "deviceinfo_flash_offset_base=\"0x00000000\"\n",
        "deviceinfo_flash_offset_kernel=\"0x00008000\"\n",
        "deviceinfo_flash_offset_ramdisk=\"0x01000000\"\n",
        "deviceinfo_flash_offset_tags=\"0x00000100\"\n",
        "deviceinfo_kernel_cmdline=\"console=ttyMSM0,115200n8 androidboot.hardware=walleye\"\n",
    ] {
        assert!(body.contains(want), "{want:?} missing from:\n{body}");
    }
    let layer = m.layer.as_ref().unwrap();
    let mut pkgs = packages(layer, Format::Apk);
    add_deviceinfo(&mut pkgs, layer, &body);
    let p = pkgs
        .iter()
        .find(|p| p.name == "google-walleye-config")
        .unwrap();
    let got: Vec<String> = materialize(p, &dir, &stage)
        .unwrap()
        .into_iter()
        .map(|e| e.dst)
        .collect();
    assert!(got.contains(&"/etc/deviceinfo".to_string()), "{got:?}");
    // no [boot], no deviceinfo
    let none = Manifest::parse(DEVICE).unwrap();
    assert_eq!(deviceinfo(&none).unwrap(), None);
}

#[test]
fn a_cmdline_a_shell_would_run_is_refused_in_the_deviceinfo() {
    // deviceinfo is sourced by shell scripts (mkinitfs, boot-deploy): a quote, $, backtick or backslash would run
    for bad in ["a\\\"b", "a$(id)", "a`id`", "a\\\\b"] {
        let m = Manifest::parse(&WALLEYE.replace("androidboot.hardware=walleye", bad)).unwrap();
        let e = deviceinfo(&m).unwrap_err();
        assert!(e.contains("cmdline"), "{bad}: {e}");
    }
}

#[test]
fn a_codename_a_shell_would_run_is_refused_and_the_file_sources_to_what_is_declared() {
    for bad in ["x\\\"; id; \\\"", "$(id)", "a b", ""] {
        let m = Manifest::parse(
            &WALLEYE.replace("google-walleye\"\n\n[boot]", &format!("{bad}\"\n\n[boot]")),
        )
        .unwrap();
        let e = deviceinfo(&m).unwrap_err();
        assert!(e.contains("codename"), "{bad}: {e}");
    }
    let m = Manifest::parse(WALLEYE).unwrap();
    let body = deviceinfo(&m).unwrap().unwrap();
    assert!(
        body.contains("deviceinfo_flash_offset_second=\"0x00000000\"\n"),
        "{body}"
    );
    assert!(
        body.contains("deviceinfo_generate_bootimg=\"true\"\n")
            && body.contains("deviceinfo_header_version=\"0\"\n"),
        "{body}"
    );
    let (dir, _) = fixture("di2");
    fs::write(dir.join("deviceinfo"), &body).unwrap();
    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg(". \"$1\"; printf '%s|%s' \"$deviceinfo_codename\" \"$deviceinfo_kernel_cmdline\"")
        .arg("sh")
        .arg(dir.join("deviceinfo"))
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "google-walleye|console=ttyMSM0,115200n8 androidboot.hardware=walleye"
    );
}

#[test]
fn a_layer_without_files_still_gets_its_dependencies_on_the_config_package_that_holds_deviceinfo() {
    let m = Manifest::parse(
        "[device]\ncodename = \"google-walleye\"\n[boot]\nkernel_offset = 0x8000\nramdisk_offset = 0x1000000\ntags_offset = 0x100\npagesize = 4096\ncmdline = \"x\"\n[layer]\nname = \"google-walleye\"\nneeds = [\"qrtr\"]\n",
    )
    .unwrap();
    let layer = m.layer.as_ref().unwrap();
    let mut pkgs = packages(layer, Format::Apk);
    add_deviceinfo(&mut pkgs, layer, &deviceinfo(&m).unwrap().unwrap());
    assert_eq!(pkgs[0].name, "google-walleye-config");
    assert_eq!(pkgs[0].depends, vec!["qrtr".to_string()]);
}
