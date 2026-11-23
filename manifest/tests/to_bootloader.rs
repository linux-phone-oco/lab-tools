// SPDX-License-Identifier: MIT OR Apache-2.0
//! From a phone's Linux to its bootloader (backlog item linux-to-bootloader-by-restart2): busybox's
//! `reboot bootloader` drops its argument, so the device declares the helper that hands `bootloader` to
//! reboot(2) RESTART2 - an `[images.NAME]` pushed to the phone and the command that runs it.

use lpoco_manifest::Manifest;

const HEAD: &str = "[device]\ncodename = \"p\"\n[boot]\nkernel_offset = 0\nramdisk_offset = 0\ntags_offset = 0\npagesize = 4096\ncmdline = \"x\"\n";
const SHA: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn declared(to_bootloader: &str, image: &str) -> Result<Manifest, String> {
    Manifest::parse(&format!(
        "{HEAD}{to_bootloader}[images]\ndir = \"images\"\n{image}"
    ))
}

fn tool() -> String {
    format!("[images.restart2]\npath = \"restart2/restart2\"\nsha256 = \"{SHA}\"\n")
}

const TABLE: &str = "[boot.to_bootloader]\ntool = \"restart2\"\nto = \"/tmp/restart2\"\nrun = \"/tmp/restart2 --command bootloader\"\n";

#[test]
fn a_helper_pushed_to_the_phone_and_the_command_that_runs_it_are_declared() {
    let m = declared(TABLE, &tool()).unwrap();
    let t = m.boot.unwrap().to_bootloader.unwrap();
    assert_eq!(t.tool, "restart2");
    assert_eq!(t.to, "/tmp/restart2");
    assert_eq!(t.run, "/tmp/restart2 --command bootloader");
}

#[test]
fn a_helper_that_is_no_image_of_the_device_is_refused() {
    let e = declared(TABLE, "").unwrap_err();
    assert!(e.contains("[images.restart2]"), "{e}");
}

#[test]
fn a_helper_without_a_checksum_is_refused_it_is_run_as_root() {
    let e = declared(TABLE, "[images.restart2]\npath = \"restart2/restart2\"\n").unwrap_err();
    assert!(e.contains("sha256"), "{e}");
}

#[test]
fn a_command_that_does_not_run_the_pushed_file_is_refused() {
    let t = TABLE.replace(
        "run = \"/tmp/restart2 --command bootloader\"",
        "run = \"reboot bootloader\"",
    );
    let e = declared(&t, &tool()).unwrap_err();
    assert!(e.contains("run"), "{e}");
}

#[test]
fn a_place_the_helper_may_not_be_pushed_to_is_refused_naming_to() {
    for bad in [
        "relative/restart2",
        "/sbin/init",
        "/tmp/../bin/busybox",
        "/tmp//restart2",
        "/tmp/",
        "/tmp",
        "/tmp/a b",
        "/tmp/a;b",
        "/tmp/a|b",
        "/tmp/a&b",
        "/tmp/a`b",
        "/tmp/a'b",
        "/tmp/a\"b",
        "/tmp/a>b",
        "/tmp/a*b",
        "/tmp/a\tb",
        "",
    ] {
        let t = format!(
            "[boot.to_bootloader]\ntool = \"restart2\"\nto = {bad:?}\nrun = {:?}\n",
            format!("{bad} --command bootloader")
        );
        let e = declared(&t, &tool()).unwrap_err();
        assert!(e.contains("boot.to_bootloader: to"), "{bad:?}: {e}");
    }
    assert!(declared(&TABLE.replace("/tmp/restart2", "/run/x/restart2"), &tool()).is_ok());
}

#[test]
fn a_command_a_shell_would_read_more_into_is_refused_naming_run() {
    for bad in [
        "/tmp/restart2 --command bootloader; reboot",
        "/tmp/restart2 $(id)",
        "/tmp/restart2 bootloader\n/bin/sh",
        "/tmp/restart2 a|b",
        "/tmp/restart2 a&b",
        "/tmp/restart2 `id`",
        "/tmp/restart2 'a'",
        "/tmp/restart2 \"a\"",
        "/tmp/restart2 >x",
        "/tmp/restart2 *",
        "/tmp/restart2\ta",
        "/tmp/restart2  --command bootloader",
        "/tmp/restart2 --command bootloader ",
        "reboot bootloader",
    ] {
        let t = format!(
            "[boot.to_bootloader]\ntool = \"restart2\"\nto = \"/tmp/restart2\"\nrun = {bad:?}\n"
        );
        let e = declared(&t, &tool()).unwrap_err();
        assert!(e.contains("boot.to_bootloader: run"), "{bad:?}: {e}");
    }
}
