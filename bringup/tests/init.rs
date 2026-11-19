// SPDX-License-Identifier: MIT OR Apache-2.0
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

fn files(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lpoco-bringup"))
        .args(["initramfs", "files"])
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn the_init_ships_its_dhcp_and_selftest_files_at_the_chosen_address() {
    let d = tmp("files");
    let out = d.join("out");
    let o = files(&["--out", out.to_str().unwrap(), "--net", "43"]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let conf = fs::read_to_string(out.join("udhcpd.conf")).unwrap();
    assert!(
        conf.contains("start 172.16.43.2") && !conf.contains("172.16.42"),
        "{conf}"
    );
    let script = fs::read_to_string(out.join("udhcpc.script")).unwrap();
    assert!(script.contains("$ip/24"), "{script}");
    assert!(
        conf.contains("end 172.16.43.2") && conf.contains("interface usb0"),
        "{conf}"
    );
    let st = fs::read_to_string(out.join("selftest.sh")).unwrap();
    assert!(st.contains("inet 172.16.43.2/"), "{st}");
    assert!(
        st.contains("172.16.43.1") && !st.contains("172.16.42"),
        "{st}"
    );
    use std::os::unix::fs::PermissionsExt;
    for (f, want) in [
        ("udhcpd.conf", 0o644),
        ("udhcpc.script", 0o755),
        ("selftest.sh", 0o755),
    ] {
        let m = fs::metadata(out.join(f)).unwrap().permissions().mode() & 0o777;
        assert_eq!(m, want, "{f}");
        if f == "udhcpd.conf" {
            continue;
        }
        let sh = Command::new("sh")
            .arg("-n")
            .arg(out.join(f))
            .output()
            .unwrap();
        assert!(sh.status.success(), "{f}: {}", text(&sh.stderr));
    }
}

#[test]
fn files_without_an_out_folder_is_refused_in_words() {
    let o = files(&[]);
    assert!(!o.status.success());
    assert!(text(&o.stderr).contains("--out"), "{}", text(&o.stderr));
}

#[test]
fn at_the_default_net_the_files_are_the_shipped_text() {
    let d = tmp("files-default");
    let out = d.join("out");
    let o = files(&["--out", out.to_str().unwrap()]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let conf = fs::read_to_string(out.join("udhcpd.conf")).unwrap();
    assert!(
        conf.contains("start 172.16.42.2") && conf.contains("end 172.16.42.2"),
        "{conf}"
    );
    let st = fs::read_to_string(out.join("selftest.sh")).unwrap();
    assert!(st.contains("inet 172.16.42.2/"), "{st}");
}

#[test]
fn a_window_adds_a_read_only_mass_storage_lun_the_init_keeps_written() {
    let d = tmp("window");
    let g = format!("{GADGET}window = true\n");
    let o = init(&["--device", &manifest(&d, &g)]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let s = text(&o.stdout);
    for part in [
        "functions/mass_storage.0",
        "echo $WIN > $L/file",
        "ln -s $G/functions/mass_storage.0 $G/configs/c.1/",
        "/proc/uptime",
        "dmesg | tail",
        "window: lun file failed",
    ] {
        assert!(s.contains(part), "missing {part}: {s}");
    }
    assert!(s.contains("echo 1 > $L/ro"), "{s}");
}

#[test]
fn without_a_window_the_init_has_no_mass_storage() {
    let d = tmp("nowindow");
    let o = init(&["--device", &manifest(&d, GADGET)]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert!(!text(&o.stdout).contains("mass_storage"));
}

#[test]
fn the_window_is_ready_before_the_bind_and_the_script_is_valid_shell() {
    let d = tmp("windoworder");
    let g = format!("{GADGET}window = true\n");
    let o = init(&["--device", &manifest(&d, &g)]);
    let s = text(&o.stdout);
    let at = |p: &str| s.find(p).unwrap_or_else(|| panic!("missing {p}: {s}"));
    assert!(at("echo 1 > $L/ro") < at("echo $WIN > $L/file"));
    assert!(at("ln -s $G/functions/mass_storage.0") < at("> $G/UDC"));
    assert!(
        at("dmesg | tail") < at("> $G/UDC"),
        "first image written before the bind"
    );
    assert!(
        s.contains("conv=notrunc,sync"),
        "a shorter snapshot must not leave an older tail"
    );
    let f = d.join("init");
    fs::write(&f, &s).unwrap();
    assert!(Command::new("sh")
        .arg("-n")
        .arg(&f)
        .status()
        .unwrap()
        .success());
}

#[test]
fn an_ffs_console_swaps_the_kernel_acm_for_the_helper_that_binds_the_udc() {
    let d = tmp("ffs");
    let g = format!("{GADGET}console = \"ffs\"\nrndis = true\n");
    let o = init(&["--device", &manifest(&d, &g)]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let s = text(&o.stdout);
    assert!(
        !s.contains("acm.usb0") && !s.contains("ttyGS0"),
        "no kernel acm function: {s}"
    );
    for part in [
        "mkdir -p $G/functions/ffs.acm",
        "ln -s $G/functions/ffs.acm $G/configs/c.1/",
        "ln -s $G/functions/ffs.acm $G/configs/c.2/",
        "mount -t functionfs acm /dev/ffs",
        "/bin/ffs-acm &",
        "ffs-acm missing",
    ] {
        assert!(s.contains(part), "missing {part}: {s}");
    }
    assert!(
        !s.contains("> $G/UDC"),
        "the helper binds, after it wrote ep0; the init does not: {s}"
    );
    let at = |p: &str| s.find(p).unwrap_or_else(|| panic!("missing {p}: {s}"));
    assert!(at("mount -t functionfs") < at("/bin/ffs-acm &"));
    assert!(at("ln -s $G/functions/ffs.acm $G/configs/c.1/") < at("/bin/ffs-acm &"));
    let f = d.join("init");
    fs::write(&f, &s).unwrap();
    assert!(Command::new("sh")
        .arg("-n")
        .arg(&f)
        .status()
        .unwrap()
        .success());
}

#[test]
fn the_default_console_is_the_kernel_acm_and_an_unknown_one_is_refused() {
    let d = tmp("console");
    let s = text(&init(&["--device", &manifest(&d, GADGET)]).stdout);
    assert!(
        s.contains("functions/acm.usb0") && !s.contains("ffs"),
        "{s}"
    );
    let g = format!("{GADGET}console = \"telepathy\"\n");
    let o = init(&["--device", &manifest(&d, &g)]);
    assert!(!o.status.success());
    assert!(text(&o.stderr).contains("console"), "{}", text(&o.stderr));
}
