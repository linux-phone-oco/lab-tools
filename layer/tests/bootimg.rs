// SPDX-License-Identifier: MIT OR Apache-2.0
//! The boot trigger of an `android-bootimg` layer (proof: bootimg-trigger-per-base): one script rewrites the phone's
//! boot.img when a kernel is installed, and each base gets the trigger it has - a kernel-install plugin (deb, rpm),
//! a libalpm hook (archlinux), a kernel-hooks.d script on Alpine (plain Alpine: no boot-deploy).

use lpoco_layer::{add_boot_trigger, bootimg_script, materialize, packages, Format};
use lpoco_manifest::Manifest;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const WALLEYE: &str = r#"
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

[layer]
name = "google-walleye"

[layer.kernel]
tree = "https://example.org/linux"
tag = "v7.1"
dtbs = ["qcom/msm8998-google-walleye.dtb"]

[layer.boot]
method = "android-bootimg"
"#;

fn dests(format: Format) -> Vec<(String, Vec<String>)> {
    let m = Manifest::parse(WALLEYE).unwrap();
    let mut pkgs = packages(m.layer.as_ref().unwrap(), format);
    add_boot_trigger(
        &mut pkgs,
        m.layer.as_ref().unwrap(),
        m.boot.as_ref().unwrap(),
        format,
    )
    .unwrap();
    let dir = std::env::temp_dir().join(format!("lpoco-bootimg-{}", std::process::id()));
    pkgs.iter()
        .map(|p| {
            let mut d: Vec<String> = materialize(p, &dir, &dir.join("stage"))
                .unwrap()
                .into_iter()
                .map(|e| e.dst)
                .collect();
            d.sort();
            (p.name.clone(), d)
        })
        .collect()
}

fn boot_files(format: Format) -> Vec<String> {
    dests(format)
        .into_iter()
        .find(|p| p.0 == "google-walleye-boot")
        .map(|p| p.1)
        .unwrap_or_default()
}

#[test]
fn deb_and_rpm_get_the_script_and_a_kernel_install_plugin() {
    for f in [Format::Deb, Format::Rpm] {
        let got = boot_files(f);
        for want in [
            "/usr/lib/kernel/install.d/95-google-walleye-bootimg.install",
            "/usr/libexec/google-walleye-bootimg",
        ] {
            assert!(got.contains(&want.to_string()), "{f:?}: {want} in {got:?}");
        }
    }
}

#[test]
fn archlinux_gets_the_script_and_a_libalpm_hook() {
    let got = boot_files(Format::Archlinux);
    for want in [
        "/usr/share/libalpm/hooks/95-google-walleye-bootimg.hook",
        "/usr/libexec/google-walleye-bootimg",
    ] {
        assert!(got.contains(&want.to_string()), "{want} in {got:?}");
    }
}

#[test]
fn alpine_gets_the_script_and_a_kernel_hooks_hook_not_an_apk_trigger() {
    // decision 2026-11-24-plain-alpine-not-nura: plain Alpine, no boot-deploy and no /etc/deviceinfo; its own
    // kernel-hooks package runs /etc/kernel-hooks.d after every kernel install (nfpm cannot write an apk trigger)
    let got = boot_files(Format::Apk);
    for want in [
        "/usr/libexec/google-walleye-bootimg",
        "/etc/kernel-hooks.d/95-google-walleye-bootimg",
    ] {
        assert!(got.contains(&want.to_string()), "{want} in {got:?}");
    }
    let hook = body_of(Format::Apk, "/etc/kernel-hooks.d/95-google-walleye-bootimg");
    assert!(
        !hook.contains("deviceinfo"),
        "the hook reads no deviceinfo:\n{hook}"
    );
}

#[test]
fn the_kernel_hooks_hook_indexes_the_modules_and_makes_its_own_initramfs_before_the_triggers_do() {
    let h = body_of(Format::Apk, "/etc/kernel-hooks.d/95-google-walleye-bootimg");
    for want in [
        "/boot/vmlinuz-$flavor",
        "depmod \"$kver\"",
        "mkinitfs -o \"$tmp/initramfs\" \"$kver\"",
        "--dtbs boot/dtbs-$flavor",
        "--out /boot/google-walleye-$flavor.img",
        "[ -n \"${2:-}\" ] || exit 0",
        "--initrd \"$tmp/initramfs\"",
    ] {
        assert!(h.contains(want), "{want:?} missing from:\n{h}");
    }
}

#[test]
fn a_layer_that_boots_by_efi_gets_no_boot_image_trigger() {
    let m = Manifest::parse(&WALLEYE.replace("android-bootimg", "efi-kernel-install")).unwrap();
    let mut pkgs = packages(m.layer.as_ref().unwrap(), Format::Deb);
    let before = pkgs.len();
    add_boot_trigger(
        &mut pkgs,
        m.layer.as_ref().unwrap(),
        m.boot.as_ref().unwrap(),
        Format::Deb,
    )
    .unwrap();
    assert_eq!(pkgs.len(), before);
    assert!(pkgs
        .iter()
        .all(|p| p.items.iter().all(|i| !i.dst.contains("bootimg"))));
}

#[test]
fn the_trigger_package_needs_the_tool_that_writes_the_image() {
    let m = Manifest::parse(WALLEYE).unwrap();
    for (f, dep) in [
        (Format::Deb, "mkbootimg"),
        (Format::Rpm, "android-tools"),
        (Format::Archlinux, "python"),
        (Format::Apk, "android-tools-mkbootimg"),
        (Format::Apk, "kernel-hooks"),
        (Format::Apk, "mkinitfs"),
        (Format::Apk, "kmod"),
    ] {
        let mut pkgs = packages(m.layer.as_ref().unwrap(), f);
        add_boot_trigger(
            &mut pkgs,
            m.layer.as_ref().unwrap(),
            m.boot.as_ref().unwrap(),
            f,
        )
        .unwrap();
        let p = pkgs
            .iter()
            .find(|p| p.name == "google-walleye-boot")
            .unwrap();
        assert!(
            p.depends.contains(&dep.to_string()),
            "{f:?}: {:?}",
            p.depends
        );
    }
}

#[test]
fn a_cmdline_a_shell_would_run_is_refused_in_the_script() {
    for bad in ["a'b", "a$(id)", "a`id`", "a\\\\b", "a\nb"] {
        let toml = WALLEYE.replace("androidboot.hardware=walleye", &bad.replace('\n', "\\n"));
        let m = Manifest::parse(&toml).unwrap();
        let e = bootimg_script(m.layer.as_ref().unwrap(), m.boot.as_ref().unwrap()).unwrap_err();
        assert!(e.contains("cmdline"), "{bad:?}: {e}");
    }
}

/// A stand-in mkbootimg that writes its arguments, one per line, into the output it is told with --output.
fn stand_in(dir: &Path) -> PathBuf {
    let bin = dir.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let mk = bin.join("mkbootimg");
    fs::write(
        &mk,
        "#!/bin/sh\nout=\nprev=\nfor a in \"$@\"; do [ \"$prev\" = --output ] && out=$a; prev=$a; done\n[ -n \"$out\" ] || exit 9\nprintf '%s\\n' \"$@\" > \"$out\"\nfor a in \"$@\"; do case $a in --kernel) ;; esac; done\nfor k; do :; done\nk=\nprev=\nfor a in \"$@\"; do [ \"$prev\" = --kernel ] && k=$a; prev=$a; done\nprintf 'KERNEL=' >> \"$out\"; cat \"$k\" >> \"$out\"\n",
    )
    .unwrap();
    fs::set_permissions(&mk, fs::Permissions::from_mode(0o755)).unwrap();
    bin
}

fn run_script(
    tag: &str,
    dtb_where: Option<&str>,
    extra: &[&str],
) -> (PathBuf, std::process::Output) {
    let dir = std::env::temp_dir().join(format!("lpoco-bootimg-run-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let m = Manifest::parse(WALLEYE).unwrap();
    let body = bootimg_script(m.layer.as_ref().unwrap(), m.boot.as_ref().unwrap()).unwrap();
    let script = dir.join("script");
    fs::write(&script, body).unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(dir.join("Image.gz"), "KERNEL").unwrap();
    fs::write(dir.join("initrd"), "RAMDISK").unwrap();
    // the kernel package's device trees, where LPOCO_DTB_ROOT points (the script looks under it)
    let root = dir.join("root");
    if let Some(at) = dtb_where {
        let p = root.join(at).join("qcom");
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join("msm8998-google-walleye.dtb"), "DTB").unwrap();
    }
    let bin = stand_in(&dir);
    let o = Command::new(&script)
        .args(["6.1.0-walleye", "--kernel"])
        .arg(dir.join("Image.gz"))
        .arg("--initrd")
        .arg(dir.join("initrd"))
        .arg("--out")
        .arg(dir.join("boot.img"))
        .args(extra)
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("LPOCO_DTB_ROOT", &root)
        .output()
        .unwrap();
    (dir, o)
}

#[test]
fn the_script_appends_the_dtb_to_the_kernel_and_packs_with_the_manifests_header() {
    let (dir, o) = run_script("ok", Some("usr/lib/modules/6.1.0-walleye/dtb"), &[]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let img = fs::read_to_string(dir.join("boot.img")).unwrap();
    for want in [
        "--base\n0x00000000\n",
        "--kernel_offset\n0x00008000\n",
        "--ramdisk_offset\n0x01000000\n",
        "--tags_offset\n0x00000100\n",
        "--pagesize\n4096\n",
        "--header_version\n0\n",
        "--os_version\n11.0.0\n",
        "--os_patch_level\n2021-01\n",
        "--cmdline\nconsole=ttyMSM0,115200n8 androidboot.hardware=walleye\n",
        "KERNEL=KERNELDTB",
    ] {
        assert!(img.contains(want), "{want:?} missing from:\n{img}");
    }
}

#[test]
fn the_script_names_the_places_it_looked_when_the_dtb_is_not_there_and_leaves_no_image() {
    let (dir, o) = run_script("nodtb", None, &[]);
    assert_ne!(o.status.code(), Some(0));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("msm8998-google-walleye.dtb"), "{err}");
    assert!(err.contains("usr/lib/modules/6.1.0-walleye/dtb"), "{err}");
    assert!(!dir.join("boot.img").exists());
}

fn body_of(format: Format, dst: &str) -> String {
    let m = Manifest::parse(WALLEYE).unwrap();
    let mut pkgs = packages(m.layer.as_ref().unwrap(), format);
    add_boot_trigger(
        &mut pkgs,
        m.layer.as_ref().unwrap(),
        m.boot.as_ref().unwrap(),
        format,
    )
    .unwrap();
    // one folder per call: tests run in parallel and a shared one is written by two at once
    let dir = std::env::temp_dir().join(format!(
        "lpoco-bootimg-body-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    for p in &pkgs {
        for e in materialize(p, &dir, &dir.join("stage")).unwrap() {
            if e.dst == dst {
                return fs::read_to_string(e.src).unwrap();
            }
        }
    }
    panic!("{dst} is in no package");
}

#[test]
fn the_plugin_writes_into_the_entry_directory_and_fails_loudly_without_an_initrd_or_entry() {
    let p = body_of(
        Format::Rpm,
        "/usr/lib/kernel/install.d/95-google-walleye-bootimg.install",
    );
    for want in [
        "KERNEL_INSTALL_STAGING_AREA",
        "\"$entry/boot.img\"",
        "no initrd for",
        "no entry directory",
        "[ \"$1\" = add ] || exit 0",
    ] {
        assert!(p.contains(want), "{want:?} missing from:\n{p}");
    }
}

#[test]
fn the_hook_runs_after_the_transaction_on_the_kernels_it_is_told() {
    let h = body_of(
        Format::Archlinux,
        "/usr/share/libalpm/hooks/95-google-walleye-bootimg.hook",
    );
    for want in [
        "NeedsTargets",
        "Target = usr/lib/modules/*/vmlinuz",
        "When = PostTransaction",
        "--alpm",
    ] {
        assert!(h.contains(want), "{want:?} missing from:\n{h}");
    }
}

#[test]
fn an_empty_or_dashed_value_is_refused_before_it_can_swallow_a_flag() {
    for (from, to) in [("11.0.0", ""), ("11.0.0", "-x"), ("2021-01", "..")] {
        let m = Manifest::parse(&WALLEYE.replace(from, to)).unwrap();
        assert!(
            bootimg_script(m.layer.as_ref().unwrap(), m.boot.as_ref().unwrap()).is_err(),
            "{to:?}"
        );
    }
}

#[test]
fn a_dtbs_folder_is_looked_in_first_and_named_when_it_lacks_the_tree() {
    // Alpine's kernel packages put the trees in /boot/dtbs-FLAVOR; the hook says so with --dtbs
    let (dir, o) = run_script(
        "dtbs",
        Some("boot/dtbs-walleye"),
        &["--dtbs", "boot/dtbs-walleye"],
    );
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(fs::read_to_string(dir.join("boot.img"))
        .unwrap()
        .contains("KERNEL=KERNELDTB"));
    let (_, o) = run_script("dtbs-lacking", None, &["--dtbs", "boot/dtbs-walleye"]);
    assert_ne!(o.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&o.stderr)
            .contains("boot/dtbs-walleye/qcom/msm8998-google-walleye.dtb"),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
}

/// The apk hook run for real, with stand-ins for what it calls (depmod, mkinitfs, the layer's script), which write
/// their arguments to a log: kernel-hooks' README gives the arguments (`<flavor> <new-version> [<old-version>]`,
/// the new version empty when the kernel is removed), and the container proof gave the version's shape (6.18.65-0).
fn run_hook(tag: &str, args: &[&str]) -> (String, std::process::Output) {
    let dir = std::env::temp_dir().join(format!("lpoco-bootimg-hook-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let bin = dir.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let log = dir.join("log");
    let say = |p: &Path, name: &str| {
        fs::write(
            p,
            format!("#!/bin/sh\necho \"{name} $*\" >> {}\n", log.display()),
        )
        .unwrap();
        fs::set_permissions(p, fs::Permissions::from_mode(0o755)).unwrap();
    };
    say(&bin.join("depmod"), "depmod");
    say(&bin.join("mkinitfs"), "mkinitfs");
    say(&dir.join("bootimg"), "bootimg");
    let hook = body_of(Format::Apk, "/etc/kernel-hooks.d/95-google-walleye-bootimg").replace(
        "/usr/libexec/google-walleye-bootimg",
        dir.join("bootimg").to_str().unwrap(),
    );
    let path = dir.join("hook");
    fs::write(&path, hook).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    let o = Command::new("sh")
        .arg(&path)
        .args(args)
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .output()
        .unwrap();
    (fs::read_to_string(&log).unwrap_or_default(), o)
}

#[test]
fn the_apk_hook_indexes_makes_the_initramfs_and_calls_the_script_with_alpines_names() {
    let (log, o) = run_hook("add", &["lts", "6.18.65-0", ""]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let lines: Vec<&str> = log.lines().collect();
    assert_eq!(lines[0], "depmod 6.18.65-0-lts", "{log}");
    assert!(
        lines[1].starts_with("mkinitfs -o ") && lines[1].ends_with(" 6.18.65-0-lts"),
        "{log}"
    );
    let tmp = lines[1].split_whitespace().nth(2).unwrap();
    assert!(
        lines[2].ends_with(&format!(
            "bootimg 6.18.65-0-lts --kernel /boot/vmlinuz-lts --initrd {tmp} --dtbs boot/dtbs-lts --out /boot/google-walleye-lts.img"
        )),
        "{log}"
    );
    assert!(
        !Path::new(tmp).exists(),
        "the temporary initramfs is left: {tmp}"
    );
}

#[test]
fn the_apk_hook_does_nothing_when_a_kernel_is_removed() {
    let (log, o) = run_hook("remove", &["lts", "", "6.18.65-0"]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert_eq!(log, "", "something ran on a removal");
}
