// SPDX-License-Identifier: MIT OR Apache-2.0
//! The install run (item osfact-root-to-ram): from an install.toml osfact boots the image from RAM, waits for the
//! declared connection, pushes the root image compressed into RAM, checks its sha256 on the device, and enters it -
//! in that order, nothing written to the device; a failed step runs the declared way back and names itself.
//! The device here is a stand-in that keeps what it was told and hashes what it was sent (it is not a phone; the
//! phone's run is walleye's keeper's, witnessed). Plans: tests/fixtures/plans, whose values name their sources.

use osfact::install::{self, Device, Shell};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

fn plan_text(name: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/plans")
        .join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A work folder with a boot image and a root image of known bytes; the plan's files and sha256 point at them.
struct Work {
    dir: PathBuf,
    root_bytes: Vec<u8>,
}

impl Work {
    fn new(tag: &str) -> Work {
        let dir = std::env::temp_dir().join(format!("osfact-run-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("boot.img"), b"a boot image").unwrap();
        let root_bytes: Vec<u8> = (0..300_000u32).map(|i| (i % 253) as u8).collect();
        std::fs::write(dir.join("root.tar.gz"), &root_bytes).unwrap();
        Work { dir, root_bytes }
    }

    /// The fixture plan with its images' files in this folder and their real sha256.
    fn plan(&self, fixture: &str) -> install::Plan {
        let boot = hex(b"a boot image");
        let text = plan_text(fixture)
            .replace(
                "file = \"boot.img\"",
                &format!("file = \"{}/boot.img\"", self.dir.display()),
            )
            .replace(
                "file = \"root.tar.gz\"",
                &format!("file = \"{}/root.tar.gz\"", self.dir.display()),
            )
            .replace(&"0".repeat(64), &boot)
            .replace(&"1".repeat(64), &hex(&self.root_bytes));
        install::read(&text).expect("the fixture reads")
    }
}

/// What the stand-in does wrong, if anything.
#[derive(Default)]
struct Faults {
    no_connection: bool,
    lose_the_link_while_sending: bool,
    flip_a_byte_in_transit: bool,
}

/// A stand-in device: it records every operation in order, keeps the bytes it was sent, and answers the
/// shell commands osfact runs the way a busybox would for these (sha256sum of what it holds, the status file).
struct Stand {
    ops: Vec<String>,
    faults: Faults,
    held: Vec<u8>,
}

impl Stand {
    fn new(faults: Faults) -> Stand {
        Stand {
            ops: Vec::new(),
            faults,
            held: Vec::new(),
        }
    }
    fn said(&self) -> String {
        self.ops.join("\n")
    }
    fn at(&self, needle: &str) -> usize {
        self.ops
            .iter()
            .position(|o| o.contains(needle))
            .unwrap_or_else(|| panic!("no op `{needle}` in:\n{}", self.said()))
    }
}

impl Device for Stand {
    fn boot(&mut self, file: &str) -> Result<(), String> {
        self.ops
            .push(format!("boot {}", file.rsplit('/').next().unwrap()));
        Ok(())
    }
    fn connect(&mut self, wait_s: u32) -> Result<(), String> {
        self.ops.push(format!("connect within {wait_s}"));
        if self.faults.no_connection {
            return Err("nothing answered".into());
        }
        Ok(())
    }
    fn shell(&mut self, command: &str, _wait_s: u32) -> Result<Shell, String> {
        self.ops.push(format!("shell {command}"));
        let text = if command.starts_with("sha256sum ") {
            format!(
                "{}  {}\n",
                hex(&self.held),
                command.trim_start_matches("sha256sum ")
            )
        } else if command.starts_with("cat ") && command.ends_with(".status") {
            "0\n".into()
        } else {
            String::new()
        };
        let status = i32::from(command.starts_with("mountpoint "));
        Ok(Shell { status, text })
    }
    fn send(&mut self, port: u16, file: &str, _wait_s: u32) -> Result<u64, String> {
        self.ops
            .push(format!("send {port} {}", file.rsplit('/').next().unwrap()));
        if self.faults.lose_the_link_while_sending {
            return Err("pushing: connection reset by peer".into());
        }
        self.held = std::fs::read(file).unwrap();
        if self.faults.flip_a_byte_in_transit {
            self.held[1000] ^= 0xff;
        }
        Ok(self.held.len() as u64)
    }
    fn way_back(&mut self, back: &install::WayBack) -> Result<(), String> {
        self.ops.push(format!("way back after {} s", back.wait));
        Ok(())
    }
}

fn run(plan: &install::Plan, stand: &mut Stand) -> Result<(), String> {
    install::run(plan, stand, &mut |_| {})
}

#[test]
fn install_boots_then_connects_then_pushes_then_checks_the_sum_then_enters_in_that_order() {
    let w = Work::new("order");
    let mut s = Stand::new(Faults::default());
    run(&w.plan("walleye-root-in-ram.toml"), &mut s)
        .unwrap_or_else(|e| panic!("{e}\n{}", s.said()));
    let order = [
        s.at("boot boot.img"),
        s.at("connect within 60"),
        s.at("shell ( nc -l -p 9001 > /tmp/root.tar.gz;"),
        s.at("send 9001 root.tar.gz"),
        s.at("shell sha256sum /tmp/root.tar.gz"),
        s.at("shell mount -t tmpfs -o size=400m tmpfs /mm"),
        s.at("gzip -dc /tmp/root.tar.gz | tar -x -C /mm"),
        s.at("chroot /mm /sbin/init"),
        s.at("shell test -e /mm/sbin/init"),
    ];
    assert!(
        order.windows(2).all(|p| p[0] < p[1]),
        "out of order: {order:?}\n{}",
        s.said()
    );
    assert!(
        !s.said().contains("way back"),
        "a good run goes the way back:\n{}",
        s.said()
    );
}

#[test]
fn a_sha256_mismatch_on_the_device_runs_the_way_back_and_fails_naming_the_push() {
    let w = Work::new("mismatch");
    let mut s = Stand::new(Faults {
        flip_a_byte_in_transit: true,
        ..Faults::default()
    });
    let e = run(&w.plan("walleye-root-in-ram.toml"), &mut s).unwrap_err();
    assert!(
        e.contains("step 2") && e.contains("push root") && e.contains("sha256"),
        "{e}"
    );
    assert!(
        s.ops.last().unwrap().starts_with("way back after 240"),
        "the way back is not last:\n{}",
        s.said()
    );
    assert!(
        !s.said().contains("tmpfs") && !s.said().contains("chroot"),
        "a bad image was entered:\n{}",
        s.said()
    );
}

#[test]
fn a_lost_connection_while_pushing_runs_the_way_back_and_fails_naming_the_push() {
    let w = Work::new("lost");
    let mut s = Stand::new(Faults {
        lose_the_link_while_sending: true,
        ..Faults::default()
    });
    let e = run(&w.plan("walleye-root-in-ram.toml"), &mut s).unwrap_err();
    assert!(e.contains("step 2") && e.contains("push root"), "{e}");
    assert!(e.contains("connection reset"), "the cause is not said: {e}");
    assert!(
        s.ops.last().unwrap().starts_with("way back"),
        "{}",
        s.said()
    );
}

#[test]
fn no_connection_after_the_boot_runs_the_way_back_and_names_the_boot() {
    let w = Work::new("noconn");
    let mut s = Stand::new(Faults {
        no_connection: true,
        ..Faults::default()
    });
    let e = run(&w.plan("walleye-root-in-ram.toml"), &mut s).unwrap_err();
    assert!(e.contains("step 1") && e.contains("connection"), "{e}");
    assert!(
        s.ops.last().unwrap().starts_with("way back"),
        "{}",
        s.said()
    );
    assert!(!s.said().contains("send "), "{}", s.said());
}

#[test]
fn an_image_that_is_not_the_packed_one_is_never_sent() {
    let w = Work::new("hostsum");
    std::fs::write(w.dir.join("root.tar.gz"), b"changed after the pack").unwrap();
    let mut s = Stand::new(Faults::default());
    let e = run(&w.plan("walleye-root-in-ram.toml"), &mut s).unwrap_err();
    assert!(e.contains("step 2") && e.contains("sha256"), "{e}");
    assert!(!s.said().contains("send "), "{}", s.said());
    assert!(
        s.ops.last().unwrap().starts_with("way back"),
        "{}",
        s.said()
    );
}

#[test]
fn a_plan_that_enters_by_switch_root_is_refused_at_read() {
    let e = install::read(&plan_text("walleye-switch-root.toml"))
        .err()
        .expect("refused");
    assert!(e.contains("refuses") && e.contains("switch-root"), "{e}");
}

#[test]
fn the_dry_run_lists_boot_push_enter_and_the_way_back_and_runs_nothing() {
    let plan =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plans/walleye-root-in-ram.toml");
    let out = Command::new(env!("CARGO_BIN_EXE_osfact"))
        .args(["install", "now", "--dry-run", "--plan"])
        .arg(&plan)
        .output()
        .expect("osfact runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let at = |n: &str| said.find(n).unwrap_or_else(|| panic!("no `{n}`:\n{said}"));
    let order = [
        at("fastboot -s $WALLEYE_SERIAL boot boot.img"),
        at("wait for the connection 172.16.43.1:23"),
        at("push root.tar.gz to 172.16.43.1:9001"),
        at("enter root: chroot /mm /sbin/init"),
        at("way back: in the shell `sync; echo b > /proc/sysrq-trigger`"),
    ];
    assert!(order.windows(2).all(|p| p[0] < p[1]), "{said}");
    assert!(!said.contains("flash"), "{said}");
}

#[test]
fn a_plan_with_a_connection_and_a_fastboot_way_back_is_refused_at_read() {
    // after the boot the device is not in fastboot: a fastboot reboot would wait for ever (review of this item)
    let text = plan_text("walleye-root-in-ram.toml").replace(
        "via = \"shell\"\ndo = \"reboot\"",
        "via = \"fastboot\"\ndo = \"reboot\"",
    );
    let e = install::read(&text).err().expect("refused");
    assert!(e.contains("refuses") && e.contains("via shell"), "{e}");
}
