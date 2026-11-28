// SPDX-License-Identifier: MIT OR Apache-2.0
//! The proof of item osfact-root-tree-apk: a distro profile whose package format is apk becomes a rootless root
//! tree (apk --root --initdb --usermode) and an ext4 image of it (mke2fs -d under fakeroot), built twice and compared.
//!
//! Where each expected value comes from (a check names its source):
//! The apk arguments and the mke2fs/fakeroot steps: google-walleye-lab tools/walleye-rootfs/src/main.rs, install_packages and make_image (origin/main of 2026-11-28);
//! The two pinned downloads: google-walleye-lab rootfs/pins (apk-tools-static 3.0.9-r0, alpine-keys 2.6-r0), on the v3.24 mirror (its cli.rs default);
//! The profile: tests/fixtures/distros/alpine.toml.
//! The first test runs anywhere (--dry-run lists the commands, runs none). The second builds for real and needs the
//! network, fakeroot, mke2fs and debugfs: `cargo test -p osfact --test rootfs_apk -- --ignored`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn osfact(args: &[&str], out: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_osfact"))
        .args(["rootfs", "now", "--distro"])
        .arg(fixtures().join("distros/alpine.toml"))
        .args(["--arch", "aarch64", "--out"])
        .arg(out)
        .args(args)
        .output()
        .expect("osfact runs")
}

fn scratch(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("osfact-{name}-{}", std::process::id()))
}

#[test]
fn a_dry_run_lists_the_rootless_apk_commands_and_runs_none() {
    let out = scratch("rootfs-dry");
    let run = osfact(&["--dry-run"], &out);
    assert!(
        run.status.success(),
        "osfact rootfs now --dry-run failed: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let said = String::from_utf8_lossy(&run.stdout);
    for want in [
        // walleye-rootfs install_packages: the rootless install
        "--usermode",
        "--initdb",
        "--no-scripts",
        "--arch aarch64",
        "-X https://dl-cdn.alpinelinux.org/alpine/v3.24/main",
        "-X https://dl-cdn.alpinelinux.org/alpine/v3.24/community",
        " add alpine-base openssh e2fsprogs",
        // walleye-rootfs make_image: ext4 from a tar, owned by root through fakeroot
        "fakeroot",
        "mke2fs",
        "-d ",
        // the reproducibility knobs (walleye-rootfs make_image): fixed id, hash seed, time, tar order
        "-U 7e1c3a52-0b6d-4f0e-9a41-2d8a6c1e5b90",
        "hash_seed=7e1c3a52-0b6d-4f0e-9a41-2d8a6c1e5b90",
        "E2FSPROGS_FAKE_TIME=1767225600",
        "--sort=name",
        "--mtime=@1767225600",
        "chmod 0755",
        "check: the two builds are the same bytes",
        "-sSfL",
    ] {
        assert!(said.contains(want), "the dry run lacks `{want}`:\n{said}");
    }
    assert!(!out.exists(), "a dry run made {}", out.display());
}

#[test]
fn a_profile_whose_format_osfact_does_not_know_is_refused_naming_it() {
    let dir = scratch("rootfs-refused");
    std::fs::create_dir_all(&dir).expect("dir");
    let profile = dir.join("unknown.toml");
    let text = std::fs::read_to_string(fixtures().join("distros/alpine.toml"))
        .expect("fixture")
        .replace("format = \"apk\"", "format = \"zzz\"");
    std::fs::write(&profile, text).expect("write");
    let run = Command::new(env!("CARGO_BIN_EXE_osfact"))
        .args(["rootfs", "now", "--distro"])
        .arg(&profile)
        .args(["--arch", "aarch64", "--dry-run", "--out"])
        .arg(dir.join("out"))
        .output()
        .expect("osfact runs");
    assert!(!run.status.success(), "an unknown format was accepted");
    assert!(
        String::from_utf8_lossy(&run.stderr).contains("zzz"),
        "the refusal does not name the format: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// What `debugfs` reads of one file inside an ext4 image, without mounting it.
fn file_in_image(img: &Path, file: &str) -> String {
    let out = Command::new("debugfs")
        .args(["-R", &format!("cat {file}")])
        .arg(img)
        .output()
        .expect("debugfs runs");
    assert!(out.status.success(), "debugfs on {}", img.display());
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

#[test]
#[ignore = "builds for real: needs the network, fakeroot, mke2fs and debugfs"]
fn the_profile_builds_an_ext4_root_image_twice_with_the_same_bytes() {
    let out = scratch("rootfs-real");
    let _ = std::fs::remove_dir_all(&out);
    let run = osfact(&[], &out);
    let said = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success(),
        "osfact rootfs now failed: {}\n{said}",
        String::from_utf8_lossy(&run.stderr)
    );
    let img = out.join("rootfs.img");
    let bytes = std::fs::read(&img).expect("OUT/rootfs.img");
    // an ext4 superblock's magic sits at offset 1080 (the same test the walleye test uses)
    assert_eq!(bytes.get(1080..1082), Some(&[0x53, 0xef][..]), "not ext4");
    // /etc/os-release is a symlink to ../usr/lib/os-release (read with debugfs stat on a real image, 2026-11-28)
    let os = file_in_image(&img, "/usr/lib/os-release");
    assert!(os.lines().any(|l| l == "ID=alpine"), "os-release: {os}");
    let db = file_in_image(&img, "/lib/apk/db/installed");
    assert!(
        db.lines().any(|l| l == "P:alpine-base"),
        "apk database lacks alpine-base"
    );
    assert!(
        said.lines().any(|l| l == "built twice: equal"),
        "the two builds are not said equal:\n{said}"
    );
    let sum = sha256(&img);
    assert!(
        said.lines()
            .any(|l| l == format!("rootfs.img sha256 {sum}")),
        "the printed sha256 is not the image's ({sum}):\n{said}"
    );
    let _ = std::fs::remove_dir_all(&out);
}

/// Found 2026-11-28 by running the release's code with a relative --out (walleye's pack, item
/// osfact-pack-trusts-the-profile-keys): apk reads --keys-dir after it enters the root, so a relative keys path
/// finds no key and every index is "UNTRUSTED signature". Source: that run's apk output, reproduced in this lab.
#[test]
fn a_relative_out_gives_apk_the_keys_by_an_absolute_path() {
    let cwd = scratch("rootfs-relative");
    std::fs::create_dir_all(&cwd).expect("dir");
    let run = Command::new(env!("CARGO_BIN_EXE_osfact"))
        .current_dir(&cwd)
        .args(["rootfs", "now", "--distro"])
        .arg(fixtures().join("distros/alpine.toml"))
        .args(["--arch", "aarch64", "--dry-run", "--out", "rel"])
        .output()
        .expect("osfact runs");
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let said = String::from_utf8_lossy(&run.stdout);
    let keys = said
        .split("--keys-dir ")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .expect("the dry run names --keys-dir");
    assert!(
        keys.starts_with('/') && keys.ends_with("rel/work/keys-pkg/usr/share/apk/keys"),
        "--keys-dir is not absolute under the out dir: {keys}"
    );
    let _ = std::fs::remove_dir_all(&cwd);
}
