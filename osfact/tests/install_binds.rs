// SPDX-License-Identifier: MIT OR Apache-2.0
//! Item osfact-enter-binds: the kernel's filesystems a declared root needs (`binds`) are mounted into it before the
//! chroot, in the declared order, logged, and a mount that fails stops the step naming it. The device is a stand-in
//! shell that records what it is sent (it answers sha256sum as install_run.rs's does); the phone's run is its keeper's.
//! Sources: the plan is walleye-root-in-ram.toml (its header names its sources); the list proc, sys, dev is the
//! item's proof (backlog osfact-enter-binds); the mounts are mount(8)'s: type proc, type sysfs, a bind of /dev.

use osfact::install::{self, Device, Shell};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const FIXTURE: &str = "tests/fixtures/plans/walleye-root-in-ram.toml";
const ENTER_COMMAND: &str = "command = \"/sbin/init\"";
const LIST: &str = "binds = [\"proc\", \"sys\", \"dev\"]";

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The fixture's enter step with `extra` lines added, its image files in a folder of this test with their real sha256.
fn plan(tag: &str, extra: &str) -> Result<(install::Plan, PathBuf), String> {
    let dir = std::env::temp_dir().join(format!("osfact-binds-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let root = vec![7u8; 5000];
    std::fs::write(dir.join("boot.img"), b"a boot image").unwrap();
    std::fs::write(dir.join("root.tar.gz"), &root).unwrap();
    let text =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE)).unwrap();
    assert!(
        text.contains(ENTER_COMMAND),
        "the fixture's enter step moved"
    );
    let text = text
        .replace(ENTER_COMMAND, &format!("{ENTER_COMMAND}\n{extra}"))
        .replace(
            "file = \"boot.img\"",
            &format!("file = \"{}/boot.img\"", dir.display()),
        )
        .replace(
            "file = \"root.tar.gz\"",
            &format!("file = \"{}/root.tar.gz\"", dir.display()),
        )
        .replace(&"0".repeat(64), &hex(b"a boot image"))
        .replace(&"1".repeat(64), &hex(&root));
    install::read(&text).map(|p| (p, dir))
}

/// Keeps every shell command in order; a command that contains `refuse` ends with status 1.
struct Stand {
    ops: Vec<String>,
    held: Vec<u8>,
    refuse: Option<&'static str>,
}

impl Device for Stand {
    fn boot(&mut self, _file: &str) -> Result<(), String> {
        Ok(())
    }
    fn connect(&mut self, _wait_s: u32) -> Result<(), String> {
        Ok(())
    }
    fn shell(&mut self, command: &str, _wait_s: u32) -> Result<Shell, String> {
        self.ops.push(command.to_string());
        if self.refuse.is_some_and(|r| command.contains(r)) {
            return Ok(Shell {
                status: 1,
                text: "mount: permission denied\n".into(),
            });
        }
        let text = match command.strip_prefix("sha256sum ") {
            Some(file) => format!("{}  {file}\n", hex(&self.held)),
            None if command.starts_with("cat ") && command.contains(".status") => "0\n".into(),
            None => String::new(),
        };
        Ok(Shell {
            status: i32::from(command.starts_with("mountpoint ")),
            text,
        })
    }
    fn send(&mut self, _port: u16, file: &str, _wait_s: u32) -> Result<u64, String> {
        self.held = std::fs::read(file).unwrap();
        Ok(self.held.len() as u64)
    }
    fn way_back(&mut self, _back: &install::WayBack) -> Result<(), String> {
        Ok(())
    }
    fn back_on(&mut self, _argv: &[String], _wait_s: u32) -> Result<(), String> {
        Ok(())
    }
}

fn stand(refuse: Option<&'static str>) -> Stand {
    Stand {
        ops: vec![],
        held: vec![],
        refuse,
    }
}

fn in_order(said: &str, lines: &[&str]) {
    let at: Vec<usize> = lines
        .iter()
        .map(|n| {
            said.find(n)
                .unwrap_or_else(|| panic!("no `{n}` in:\n{said}"))
        })
        .collect();
    assert!(
        at.windows(2).all(|p| p[0] < p[1]),
        "out of order {lines:?}:\n{said}"
    );
}

const MOUNTS: [&str; 6] = [
    "mount -t tmpfs -o size=400m tmpfs /mm",
    "mount -t proc proc /mm/proc",
    "mount -t sysfs sysfs /mm/sys",
    "mount --bind /dev /mm/dev",
    "setsid chroot /mm /sbin/init",
    "test -e /mm/sbin/init",
];

#[test]
fn the_dry_run_lists_the_declared_mounts_in_order_before_the_chroot() {
    let (plan, _) = plan("dry", LIST).expect("reads");
    in_order(&install::commands(&plan).unwrap().join("\n"), &MOUNTS);
}

#[test]
fn the_run_sends_each_mount_before_the_chroot_and_the_log_has_them() {
    let (plan, dir) = plan("run", LIST).expect("reads");
    let log = dir.join("install.log");
    let mut dev = stand(None);
    install::run_logged(&plan, &mut dev, &log, &mut |_| {})
        .unwrap_or_else(|e| panic!("{e}\n{:?}", dev.ops));
    in_order(&dev.ops.join("\n"), &MOUNTS);
    in_order(&std::fs::read_to_string(&log).unwrap(), &MOUNTS[1..5]);
}

#[test]
fn a_mount_that_fails_stops_the_step_naming_it_and_the_root_is_not_entered() {
    let (plan, _) = plan("fail", LIST).expect("reads");
    let mut dev = stand(Some("-t sysfs"));
    let e = install::run(&plan, &mut dev, &mut |_| {}).unwrap_err();
    assert!(
        e.contains("step 3") && e.contains("bind sys") && e.contains("permission denied"),
        "{e}"
    );
    let sent = dev.ops.join("\n");
    assert!(
        sent.contains("/mm/proc"),
        "the first mount is not tried:\n{sent}"
    );
    assert!(
        !sent.contains("--bind /dev") && !sent.contains("chroot"),
        "went on past the failure:\n{sent}"
    );
}

#[test]
fn a_name_osfact_does_not_know_or_the_same_twice_is_refused_at_read() {
    for (extra, named) in [
        ("binds = [\"proc\", \"cgroup\"]", "cgroup"),
        ("binds = [\"dev-pts\", \"dev\"]", "before"),
        ("binds = [\"proc\", \"proc\"]", "proc"),
    ] {
        let e = plan("bad", extra).err().expect("refused");
        assert!(e.contains("refuses") && e.contains(named), "{named}: {e}");
    }
}

#[test]
fn without_binds_the_enter_is_what_it_was() {
    let (plan, _) = plan("none", "").expect("reads");
    let said = install::commands(&plan).unwrap().join("\n");
    assert!(
        !said.contains("mount -t proc") && !said.contains("--bind"),
        "{said}"
    );
}
