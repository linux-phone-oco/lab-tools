// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco-layer pack` run end to end against a stand-in nfpm (proof: lpoco-layer-pack): each package is
//! packed with the packager and target the format names, and a failing nfpm stops the run with the config kept.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEVICE: &str = r#"
[layer]
name = "xiaomi-surya"

[[layer.file]]
from = "ucm/HiFi.conf"
to = "/usr/share/alsa/ucm2/Xiaomi/surya/HiFi.conf"
kind = "data"
"#;

fn setup(tag: &str, nfpm_body: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("lpoco-layer-run-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("ucm")).unwrap();
    fs::write(dir.join("ucm/HiFi.conf"), "Syntax 4\n").unwrap();
    fs::write(dir.join("device.toml"), DEVICE).unwrap();
    let nfpm = dir.join("fake-nfpm");
    fs::write(&nfpm, nfpm_body).unwrap();
    fs::set_permissions(&nfpm, fs::Permissions::from_mode(0o755)).unwrap();
    (dir, nfpm)
}

fn pack(dir: &Path, nfpm: &Path, format: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_lpoco-layer"))
        .args(["pack", "--format", format, "--manifest"])
        .arg(dir.join("device.toml"))
        .arg("--out")
        .arg(dir.join("out"))
        .env("LPOCO_NFPM", nfpm)
        .output()
        .unwrap()
}

#[test]
fn each_package_is_packed_with_the_packager_and_target_of_its_format() {
    // $1 package, $2 --packager, $3 FORMAT, $4 --config, $5 CONF, $6 --target, $7 TARGET
    let (dir, nfpm) = setup("ok", "#!/bin/sh\n[ \"$2\" = --packager ] && [ \"$4\" = --config ] && [ \"$6\" = --target ] || exit 9\necho \"$3\" > \"$7\"\n");
    let o = pack(&dir, &nfpm, "deb");
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let target = dir.join("out/xiaomi-surya-config_0.1.0_all.deb");
    assert_eq!(fs::read_to_string(&target).unwrap(), "deb\n");
    assert!(String::from_utf8_lossy(&o.stdout).contains(&*target.to_string_lossy()));
    assert!(dir.join("out/xiaomi-surya-config.nfpm.yaml").exists());
}

#[test]
fn a_failing_nfpm_stops_the_run_and_the_config_is_kept() {
    let (dir, nfpm) = setup("fail", "#!/bin/sh\nexit 1\n");
    let o = pack(&dir, &nfpm, "rpm");
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("failed on"));
    assert!(dir.join("out/xiaomi-surya-config.nfpm.yaml").exists());
    assert!(!dir
        .join("out/xiaomi-surya-config-0.1.0-1.noarch.rpm")
        .exists());
}

#[test]
fn a_missing_nfpm_is_named_with_the_config_it_left() {
    let (dir, _) = setup("none", "");
    let o = pack(&dir, &dir.join("no-such-nfpm"), "apk");
    assert_eq!(o.status.code(), Some(2));
    let e = String::from_utf8_lossy(&o.stderr);
    assert!(
        e.contains("no-such-nfpm") && e.contains("the config is written"),
        "{e}"
    );
}

#[test]
fn the_boot_package_is_packed_with_the_postinstall_the_run_wrote() {
    // the stand-in copies its --config to its --target, so the test reads what nfpm was told
    let (dir, nfpm) = setup("post", "#!/bin/sh\ncp \"$5\" \"$7\"\n");
    fs::create_dir_all(dir.join("units")).unwrap();
    fs::write(dir.join("units/a.service"), "[Unit]\n").unwrap();
    let toml = format!(
        "{DEVICE}\n[[layer.file]]\nfrom = \"units/a.service\"\nto = \"/usr/lib/systemd/system/a.service\"\nkind = \"unit\"\n"
    );
    fs::write(dir.join("device.toml"), toml).unwrap();
    let o = pack(&dir, &nfpm, "deb");
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let script = dir.join("out/stage/xiaomi-surya-boot/scripts/postinstall");
    let body = fs::read_to_string(&script).unwrap();
    assert!(body.contains("systemctl preset a.service"), "{body}");
    let packed = fs::read_to_string(dir.join("out/xiaomi-surya-boot_0.1.0_all.deb")).unwrap();
    assert!(
        packed.contains(&format!("postinstall: \"{}\"", script.display())),
        "{packed}"
    );
}

#[test]
fn a_kernel_build_given_is_packed_as_linux_name_for_arm64_and_none_given_makes_none() {
    let (dir, nfpm) = setup("kern", "#!/bin/sh\ncp \"$5\" \"$7\"\n");
    let toml = format!(
        "{DEVICE}\n[layer.kernel]\ntree = \"https://example.org/linux\"\ntag = \"v7.1\"\ndtbs = [\"qcom/a.dtb\"]\n"
    );
    fs::write(dir.join("device.toml"), toml).unwrap();
    let b = dir.join("kbuild");
    fs::create_dir_all(b.join("dtbs/qcom")).unwrap();
    fs::write(b.join("dtbs/qcom/a.dtb"), "d").unwrap();
    fs::write(b.join("Image"), "i").unwrap();
    fs::write(b.join("kernel.release"), "7.1.0\n").unwrap();
    let o = pack(&dir, &nfpm, "deb");
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(!dir.join("out/linux-xiaomi-surya.nfpm.yaml").exists());
    let o = Command::new(env!("CARGO_BIN_EXE_lpoco-layer"))
        .args(["pack", "--format", "deb", "--manifest"])
        .arg(dir.join("device.toml"))
        .arg("--out")
        .arg(dir.join("out"))
        .arg("--kernel-build")
        .arg(&b)
        .env("LPOCO_NFPM", &nfpm)
        .output()
        .unwrap();
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let packed = fs::read_to_string(dir.join("out/linux-xiaomi-surya_0.1.0_arm64.deb")).unwrap();
    assert!(packed.contains("arch: arm64"), "{packed}");
    assert!(
        packed.contains("/usr/lib/modules/7.1.0/vmlinuz"),
        "{packed}"
    );
}

#[test]
fn apk_packs_the_deviceinfo_from_boot_and_deb_does_not() {
    let (dir, nfpm) = setup("di", "#!/bin/sh\nexit 0\n");
    let dev = format!(
        "[device]\ncodename = \"google-walleye\"\n\n[boot]\nkernel_offset = 0x8000\nramdisk_offset = 0x1000000\ntags_offset = 0x100\npagesize = 4096\ncmdline = \"console=ttyMSM0\"\n{DEVICE}"
    );
    fs::write(dir.join("device.toml"), dev).unwrap();
    let o = pack(&dir, &nfpm, "apk");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let conf = fs::read_to_string(dir.join("out/xiaomi-surya-config.nfpm.yaml")).unwrap();
    assert!(conf.contains("/etc/deviceinfo"), "{conf}");
    let o = pack(&dir, &nfpm, "deb");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let conf = fs::read_to_string(dir.join("out/xiaomi-surya-config.nfpm.yaml")).unwrap();
    assert!(!conf.contains("/etc/deviceinfo"), "{conf}");
    // a cmdline a shell would run stops the run before anything is packed
    fs::write(
        dir.join("device.toml"),
        fs::read_to_string(dir.join("device.toml"))
            .unwrap()
            .replace("console=ttyMSM0", "a$(id)"),
    )
    .unwrap();
    let o = pack(&dir, &nfpm, "apk");
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("cmdline"));
}

#[test]
fn apk_also_writes_the_cmdline_as_a_kernel_cmdline_d_file() {
    let (dir, nfpm) = setup("cd", "#!/bin/sh\nexit 0\n");
    let dev = format!(
        "[device]\ncodename = \"google-walleye\"\n\n[boot]\nkernel_offset = 0x8000\nramdisk_offset = 0x1000000\ntags_offset = 0x100\npagesize = 4096\ncmdline = \"console=ttyMSM0 quiet\"\n{DEVICE}"
    );
    fs::write(dir.join("device.toml"), dev).unwrap();
    let o = pack(&dir, &nfpm, "apk");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let conf = fs::read_to_string(dir.join("out/xiaomi-surya-config.nfpm.yaml")).unwrap();
    assert!(
        conf.contains("/usr/lib/kernel-cmdline.d/50-xiaomi-surya.conf"),
        "{conf}"
    );
    let body = fs::read_to_string(
        dir.join("out/stage/xiaomi-surya-config/usr/lib/kernel-cmdline.d/50-xiaomi-surya.conf"),
    )
    .unwrap();
    assert_eq!(body, "console=ttyMSM0 quiet\n");
    // the other formats have no such file from here
    let o = pack(&dir, &nfpm, "deb");
    assert!(o.status.success());
    let conf = fs::read_to_string(dir.join("out/xiaomi-surya-config.nfpm.yaml")).unwrap();
    assert!(!conf.contains("kernel-cmdline.d"), "{conf}");
}
