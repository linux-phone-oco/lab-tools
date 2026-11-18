//! bringup-generic-init: `lpoco-bringup initramfs init` writes the bring-up init from the device's
//! `[boot.gadget]` table - the configfs gadget (ACM, ECM, RNDIS), the address, DHCP, telnet and the ACM
//! shell - with none of the device's own init files. The phone's name, serial and product are its data.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lpoco-init-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

const BOOT: &str = "[device]\ncodename = \"p\"\n[connection]\nuser = \"u\"\naddresses = [\"a\"]\n\
[boot]\nkernel_offset = 0x8000\nramdisk_offset = 0x1000000\ntags_offset = 0x100\npagesize = 4096\ncmdline = \"x\"\n";

fn manifest(d: &std::path::Path, gadget: &str) -> String {
    let p = d.join("device.toml");
    fs::write(&p, format!("{BOOT}{gadget}")).unwrap();
    p.to_str().unwrap().to_string()
}

fn init(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lpoco-bringup"))
        .args(["initramfs", "init"])
        .args(args)
        .output()
        .unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

const GADGET: &str = "[boot.gadget]\nserial = \"pixel-first-boot\"\nproduct = \"Pixel 2 (mainline)\"\nlabel = \"pixel-init\"\n";

#[test]
fn the_init_names_the_devices_serial_product_and_label_and_nothing_of_another_phone() {
    let d = tmp("names");
    let o = init(&["--device", &manifest(&d, GADGET)]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let s = text(&o.stdout);
    assert!(s.starts_with("#!/bin/busybox sh\n"), "{s}");
    assert!(
        s.contains("echo \"pixel-first-boot\" > $G/strings/0x409/serialnumber"),
        "{s}"
    );
    assert!(
        s.contains("echo \"Pixel 2 (mainline)\" > $G/strings/0x409/product"),
        "{s}"
    );
    assert!(s.contains("say() { echo \"pixel-init: $*\""), "{s}");
    for part in [
        "functions/acm.usb0",
        "functions/ecm.usb0",
        "functions/rndis.usb0",
        "ifconfig usb0 172.16.42.1 netmask 255.255.255.0 up",
        "udhcpd -S /etc/udhcpd.conf",
        "telnetd -l /bin/sh -b 172.16.42.1",
        "/dev/ttyGS0",
        "/modules.list",
        "/selftest.sh",
    ] {
        assert!(s.contains(part), "missing {part}: {s}");
    }
}

#[test]
fn the_address_is_a_variable_and_the_old_one_is_not_left_behind() {
    let d = tmp("net");
    let o = init(&["--device", &manifest(&d, GADGET), "--net", "43"]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let s = text(&o.stdout);
    assert!(s.contains("172.16.43.1"), "{s}");
    assert!(s.contains("02:42:ac:10:2b:01"), "{s}");
    assert!(!s.contains("172.16.42") && !s.contains("ac:10:2a"), "{s}");
}

#[test]
fn rndis_can_be_left_out_and_the_script_is_valid_shell() {
    let d = tmp("norndis");
    let g = format!("{GADGET}rndis = false\n");
    let out = d.join("init");
    let o = init(&[
        "--device",
        &manifest(&d, &g),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let s = fs::read_to_string(&out).unwrap();
    assert!(
        !s.contains("functions/rndis.usb0") && !s.contains("configs/c.2"),
        "{s}"
    );
    let sh = Command::new("sh").arg("-n").arg(&out).output().unwrap();
    assert!(sh.status.success(), "{}", text(&sh.stderr));
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        fs::metadata(&out).unwrap().permissions().mode() & 0o777,
        0o755
    );
}

#[test]
fn a_device_without_a_gadget_table_is_refused_in_words() {
    let d = tmp("none");
    let o = init(&["--device", &manifest(&d, "")]);
    assert!(!o.status.success());
    assert!(
        text(&o.stderr).contains("[boot.gadget]"),
        "{}",
        text(&o.stderr)
    );
}

#[test]
fn a_serial_that_looks_like_an_address_comes_out_as_written_at_any_net() {
    let d = tmp("serial-addr");
    let g = "[boot.gadget]\nserial = \"172.16.42.9\"\nproduct = \"P ac:10:2a:\"\n";
    let o = init(&["--device", &manifest(&d, g), "--net", "43"]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let s = text(&o.stdout);
    assert!(
        s.contains("echo \"172.16.42.9\" > $G/strings/0x409/serialnumber"),
        "{s}"
    );
    assert!(
        s.contains("echo \"P ac:10:2a:\" > $G/strings/0x409/product"),
        "{s}"
    );
    assert!(s.contains("telnetd -l /bin/sh -b 172.16.43.1"), "{s}");
}

#[test]
fn a_word_that_would_break_out_of_its_quotes_is_refused() {
    for bad in ["a\\\"b", "a$b", "a`b`", "a\\\\b", "a@b", ""] {
        let d = tmp("bad");
        let g = format!("[boot.gadget]\nserial = \"{bad}\"\nproduct = \"p\"\n");
        let o = init(&["--device", &manifest(&d, &g)]);
        assert!(!o.status.success(), "{bad:?} was accepted");
        assert!(text(&o.stderr).contains("serial"), "{}", text(&o.stderr));
    }
}

#[test]
fn a_failed_bind_or_interface_is_said_not_skipped() {
    let d = tmp("fail");
    let s = text(&init(&["--device", &manifest(&d, GADGET)]).stdout);
    assert!(
        s.contains("binding $UDC failed") && s.contains("usb0 did not come up"),
        "{s}"
    );
}
