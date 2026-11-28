// SPDX-License-Identifier: MIT OR Apache-2.0
//! The profile reader and the apk adapter, on the fixture profiles (names live here, never in src/: tests/no_names_in_source.rs).
//! Sources: tests/fixtures/distros/{alpine,debian}.toml; the arguments are those of google-walleye-lab
//! tools/walleye-rootfs/src/main.rs install_packages (origin/main 2026-11-28).

use osfact::apk::Apk;
use osfact::distro;
use osfact::pkgformat::PackageFormat;
use std::path::Path;

const APK_PROFILE: &str = include_str!("fixtures/distros/alpine.toml");
const DEB_PROFILE: &str = include_str!("fixtures/distros/debian.toml");

#[test]
fn both_fixture_profiles_read() {
    let a = distro::read(APK_PROFILE).expect("apk profile");
    assert_eq!(a.distro.format, "apk");
    assert_eq!(a.pin.len(), 2);
    assert!(distro::read(DEB_PROFILE)
        .expect("deb profile")
        .pin
        .is_empty());
}

#[test]
fn a_missing_pin_is_named() {
    let a = distro::read(APK_PROFILE).expect("profile");
    assert!(a.pinned("nothing").unwrap_err().contains("nothing"));
}

#[test]
fn the_install_is_rootless_scriptless_and_checked_against_the_keys() {
    let p = distro::read(APK_PROFILE).expect("profile");
    let tools = Apk.tools(Path::new("/w"));
    let cmd = Apk
        .root_tree(&p, &tools, Path::new("/w/tree-1"), "aarch64")
        .join(" ");
    assert!(cmd.starts_with("/w/tools/sbin/apk.static --usermode --root /w/tree-1 --keys-dir /w/keys-pkg/usr/share/apk/keys --initdb --no-scripts"));
    assert!(cmd.ends_with(" add alpine-base openssh e2fsprogs"));
}

#[test]
fn the_downloads_are_the_pinned_packages_of_the_host_machine() {
    let p = distro::read(APK_PROFILE).expect("profile");
    let f = Apk.fetches(&p, "x86_64").expect("fetches");
    assert_eq!(
        f[0].url,
        "https://dl-cdn.alpinelinux.org/alpine/v3.24/main/x86_64/apk-tools-static-3.0.9-r0.apk"
    );
    assert_eq!(f[1].package, "alpine-keys");
}

// Captured text: the apk database of a tree apk-tools-static 3.0.9-r0 made from the fixture profile, with
// --usermode, on 2026-11-28 (tests/fixtures/apk/installed, 2066 lines, unedited).
const APK_DATABASE: &str = include_str!("fixtures/apk/installed");

#[test]
fn the_database_gives_the_setuid_file_the_mode_usermode_could_not_apply() {
    let files = osfact::apk::database_files(APK_DATABASE).expect("the captured database parses");
    assert!(
        files.contains(&("bin/bbsuid".to_string(), 0, 0, 0o4111)),
        "bin/bbsuid is not 0:0 4111 in the database"
    );
}

#[test]
fn the_owner_and_mode_commands_group_files_and_name_the_tree() {
    let files = osfact::apk::database_files(APK_DATABASE).expect("the captured database parses");
    let cmds = osfact::apk::owner_commands(Path::new("/t"), &files);
    assert!(cmds
        .iter()
        .any(|c| c[0] == "chmod" && c[1] == "4111" && c[2] == "/t/bin/bbsuid"));
    assert!(cmds.iter().all(|c| c
        .iter()
        .skip(2)
        .all(|a| a.starts_with("/t/") || a.starts_with("0:"))));
    assert!(
        cmds.iter().all(|c| c.len() <= 203),
        "a command takes at most 200 files"
    );
}
