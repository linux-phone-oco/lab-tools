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
