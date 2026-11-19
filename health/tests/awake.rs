// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco-health awake` (backlog lpoco-awake): the lab phone's keep-awake - the kernel wake lock `lpoco-awake`.
//! Run here against a made-up phone under LPOCO_HEALTH_ROOT, one step at a time (`--once`): the lock is held
//! always; with /etc/lpoco/allow-autosleep it is held only while the UDC is configured or a gate the manifest
//! named (`AWAKE_GATES` in health.env) says unsafe. The phone proof (surya-awake-from-package) is surya's.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Rig {
    root: PathBuf,
    bin: PathBuf,
}

fn put(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

impl Rig {
    /// A phone with a UDC in `udc` state, the gates the env file names, a controller that is `hci`
    /// ("UP RUNNING" or "DOWN") and a process table with `procs` as (comm, state).
    fn new(name: &str, env: &str, udc: &str, hci: &str, procs: &[(&str, &str)]) -> Rig {
        let root = std::env::temp_dir().join(format!("awake-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        put(&root, "etc/lpoco-health/health.env", env);
        put(
            &root,
            "sys/class/udc/a600000.usb/state",
            &format!("{udc}\n"),
        );
        put(&root, "sys/power/wake_lock", "");
        put(&root, "sys/power/wake_unlock", "");
        for (i, (comm, state)) in procs.iter().enumerate() {
            put(
                &root,
                &format!("proc/{}/stat", 100 + i),
                &format!("{} ({comm}) {state} 1 1 1 0 -1\n", 100 + i),
            );
        }
        // a controller reader that says what hciconfig says
        let bin = root.join("bin");
        put(
            &root,
            "bin/hciconfig",
            &format!("#!/bin/sh\necho 'hci0:\t{hci} BD Address: 00:00'\n"),
        );
        let _ = Command::new("chmod")
            .arg("755")
            .arg(bin.join("hciconfig"))
            .status();
        Rig { root, bin }
    }

    fn p(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.p(rel)).unwrap_or_default()
    }

    fn step(&self) -> Output {
        Command::new("timeout")
            .arg("20")
            .arg(env!("CARGO_BIN_EXE_lpoco-health"))
            .args(["awake", "--once"])
            .env("LPOCO_HEALTH_ROOT", &self.root)
            .env("PATH", format!("{}:/usr/bin:/bin", self.bin.display()))
            .output()
            .unwrap()
    }

    fn held(&self) -> bool {
        self.read("sys/power/wake_lock").contains("lpoco-awake")
    }

    fn released(&self) -> bool {
        self.read("sys/power/wake_unlock").contains("lpoco-awake")
    }
}

const UP: &str = "UP RUNNING";
const GATES: &str = "AWAKE_GATES='bluetooth-not-up'\n";

fn allow(rig: &Rig) {
    put(&rig.root, "etc/lpoco/allow-autosleep", "");
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

#[test]
fn the_lock_is_held_always_unless_autosleep_is_allowed() {
    for udc in ["configured", "not attached", "suspended"] {
        let rig = Rig::new("always", GATES, udc, UP, &[]);
        let o = rig.step();
        assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
        assert!(rig.held(), "held with the UDC {udc}");
        assert!(!rig.released());
    }
}

#[test]
fn with_autosleep_allowed_the_lock_follows_the_udc() {
    let rig = Rig::new("udc", GATES, "configured", UP, &[]);
    allow(&rig);
    assert_eq!(rig.step().status.code(), Some(0));
    assert!(rig.held(), "configured: held");

    let rig = Rig::new("udc-gone", GATES, "not attached", UP, &[]);
    allow(&rig);
    assert_eq!(rig.step().status.code(), Some(0));
    assert!(!rig.held(), "not configured: not held");
    assert!(rig.released(), "not configured: released");
}

#[test]
fn a_named_gate_that_says_unsafe_holds_the_lock_again() {
    // bluetooth down
    let rig = Rig::new("bt-down", GATES, "not attached", "DOWN", &[]);
    allow(&rig);
    rig.step();
    assert!(rig.held(), "hci0 not UP: held");
    // an rfkill stuck in D state (the r4 wedge)
    let rig = Rig::new("rfkill-d", GATES, "not attached", UP, &[("rfkill", "D")]);
    allow(&rig);
    rig.step();
    assert!(rig.held(), "rfkill in D state: held");
    // an rfkill that is fine, and a gate nobody named
    let rig = Rig::new("rfkill-s", GATES, "not attached", UP, &[("rfkill", "S")]);
    allow(&rig);
    rig.step();
    assert!(rig.released() && !rig.held());
    let rig = Rig::new("ungated", "", "not attached", "DOWN", &[]);
    allow(&rig);
    rig.step();
    assert!(
        rig.released() && !rig.held(),
        "no gate named: the bluetooth state is not asked"
    );
}

#[test]
fn a_gate_the_package_does_not_know_is_refused_where_gates_decide_and_never_unlocks_the_phone() {
    let env = "AWAKE_GATES='bluetooth-not-up sunspots'\n";
    // autosleep allowed: the gates decide, a manifest the package cannot read holds nothing
    let rig = Rig::new("unknown-allowed", env, "configured", UP, &[]);
    allow(&rig);
    let o = rig.step();
    assert_eq!(o.status.code(), Some(2), "{}", text(&o.stdout));
    assert!(text(&o.stderr).contains("sunspots"), "{}", text(&o.stderr));
    assert!(!rig.held());
    // held always: the phone stays awake all the same, and the gate is named
    let rig = Rig::new("unknown-always", env, "not attached", UP, &[]);
    let o = rig.step();
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
    assert!(text(&o.stderr).contains("sunspots"), "{}", text(&o.stderr));
    assert!(rig.held(), "the safe default holds");
}
