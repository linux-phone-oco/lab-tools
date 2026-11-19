// SPDX-License-Identifier: MIT OR Apache-2.0
//! The suite check suspend-irq-quiet (backlog suspend-irq-quiet-check, proof suspend-irq-quiet): one
//! 15 s s2idle cycle with an RTC wake alarm and the guard armed; the interrupts across it stay under the
//! manifest's threshold, or the check fails naming the top three sources. Run here against a made-up
//! /proc/interrupts, RTC and power state: the check's paths come from the environment, the phone's by default.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

const CHECK: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/suite/checks/base/92-suspend-irq.sh"
);
const LIB: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/suite/lib.sh");

const BEFORE: &str = "           CPU0       CPU1\n  10:        100        100   GICv3  arch_timer\n 157:         10         10   GICv3  serial\n 200:          0          0   GICv3  mmc\n";
// after: arch_timer +30, serial +300, mmc +5 = 335
const AFTER: &str = "           CPU0       CPU1\n  10:        115        115   GICv3  arch_timer\n 157:        160        160   GICv3  serial\n 200:          3          2   GICv3  mmc\n";

struct Rig {
    dir: PathBuf,
}

impl Rig {
    fn new(name: &str) -> Rig {
        let dir = std::env::temp_dir().join(format!("suspend-irq-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("rtc")).unwrap();
        fs::write(dir.join("interrupts"), BEFORE).unwrap();
        fs::write(dir.join("after"), AFTER).unwrap();
        fs::write(dir.join("rtc/since_epoch"), "1000\n").unwrap();
        fs::write(dir.join("stats"), "3\n").unwrap();
        fs::write(dir.join("rtc/wakealarm"), "").unwrap();
        // the power state is a fifo: whoever writes it "suspends" the phone, and the interrupts change
        assert!(Command::new("mkfifo")
            .arg(dir.join("state"))
            .status()
            .unwrap()
            .success());
        Rig { dir }
    }

    /// run the check with `extra` environment; a fake phone reads the power state and "wakes" with
    /// the after-interrupts unless `no_suspend` (then nothing may ever write the state)
    fn run(&self, extra: &[(&str, &str)], no_suspend: bool) -> String {
        let d = &self.dir;
        let mut reader = if no_suspend {
            None
        } else {
            Some(
                Command::new("timeout")
                    .args(["20", "sh", "-c"])
                    .arg("cat \"$1\" >/dev/null; cp \"$3\" \"$2\"; echo 4 > \"$4\"")
                    .arg("sh")
                    .arg(d.join("state"))
                    .arg(d.join("interrupts"))
                    .arg(d.join("after"))
                    .arg(d.join("stats"))
                    .spawn()
                    .unwrap(),
            )
        };
        let mut c = Command::new("timeout");
        c.args(["20", "sh", CHECK])
            .env("LPOCO_HEALTH_LIB", LIB)
            .env("PROC_INTERRUPTS", d.join("interrupts"))
            .env("RTC_DIR", d.join("rtc"))
            .env("POWER_STATE", d.join("state"))
            .env("SUSPEND_STATS", d.join("stats"))
            .env("SUSPEND_SECONDS", "1")
            .env("SUSPEND_SETTLE", "2");
        for (k, v) in extra {
            c.env(k, v);
        }
        let out = c.output().unwrap();
        if let Some(r) = reader.as_mut() {
            let _ = r.wait();
        }
        let s = String::from_utf8_lossy(&out.stdout).into_owned();
        if no_suspend {
            // a suspend nobody asked for would have blocked on the fifo; reaching here means it did not
            assert!(out.status.success(), "{s}");
        }
        s
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn over_the_threshold_fails_naming_the_top_three_sources() {
    let r = Rig::new("over");
    let o = r.run(
        &[("SUSPEND_IRQ_MAX", "100"), ("SUSPEND_GUARD", "true")],
        false,
    );
    assert!(o.starts_with("fail suspend-irq-quiet deep-sleep"), "{o}");
    assert!(o.contains("serial"), "{o}");
    assert!(o.contains("arch_timer"), "{o}");
    assert!(o.contains("mmc"), "{o}");
    assert!(o.contains("335"), "the total is said: {o}");
    assert_eq!(
        fs::read_to_string(r.dir.join("rtc/wakealarm"))
            .unwrap()
            .trim(),
        "0",
        "alarm cleared"
    );
    assert!(
        o.find("serial").unwrap() < o.find("arch_timer").unwrap(),
        "most active first: {o}"
    );
}

#[test]
fn under_the_threshold_passes() {
    let r = Rig::new("under");
    let o = r.run(
        &[("SUSPEND_IRQ_MAX", "1000"), ("SUSPEND_GUARD", "true")],
        false,
    );
    assert!(o.starts_with("pass suspend-irq-quiet deep-sleep"), "{o}");
}

#[test]
fn no_threshold_is_not_measured_and_never_suspends() {
    let r = Rig::new("nothr");
    let o = r.run(&[("SUSPEND_GUARD", "true")], true);
    assert!(o.starts_with("skip suspend-irq-quiet deep-sleep"), "{o}");
    assert!(o.contains("not measured"), "{o}");
}

#[test]
fn no_guard_or_a_failing_guard_skips_without_suspending() {
    let r = Rig::new("noguard");
    for guard in [None, Some("false")] {
        let mut env = vec![("SUSPEND_IRQ_MAX", "100")];
        if let Some(g) = guard {
            env.push(("SUSPEND_GUARD", g));
        }
        let o = r.run(&env, true);
        assert!(o.starts_with("skip suspend-irq-quiet"), "{o}");
        assert!(o.contains("watchdog"), "{o}");
    }
}

#[test]
fn no_rtc_alarm_skips_without_suspending() {
    let r = Rig::new("nortc");
    fs::remove_file(r.dir.join("rtc/wakealarm")).unwrap();
    let o = r.run(
        &[("SUSPEND_IRQ_MAX", "100"), ("SUSPEND_GUARD", "true")],
        true,
    );
    assert!(o.starts_with("skip suspend-irq-quiet"), "{o}");
    assert!(o.contains("RTC"), "{o}");
}

#[test]
fn a_phone_that_did_not_really_suspend_is_not_measured() {
    let r = Rig::new("stuck");
    fs::write(r.dir.join("stuck"), "3\n").unwrap();
    let stuck = r.dir.join("stuck");
    let o = r.run(
        &[
            ("SUSPEND_IRQ_MAX", "1000"),
            ("SUSPEND_GUARD", "true"),
            ("SUSPEND_STATS", stuck.to_str().unwrap()),
        ],
        false,
    );
    assert!(o.starts_with("skip suspend-irq-quiet"), "{o}");
    assert!(o.contains("did not suspend"), "{o}");
}

#[test]
fn an_alarm_that_does_not_take_skips_without_suspending() {
    let r = Rig::new("alarmfails");
    fs::remove_file(r.dir.join("rtc/wakealarm")).unwrap();
    fs::create_dir(r.dir.join("rtc/wakealarm")).unwrap();
    let o = r.run(
        &[("SUSPEND_IRQ_MAX", "100"), ("SUSPEND_GUARD", "true")],
        true,
    );
    assert!(o.starts_with("skip suspend-irq-quiet"), "{o}");
    assert!(o.contains("RTC"), "{o}");
}

#[test]
fn an_unreadable_suspend_counter_skips_without_suspending() {
    let r = Rig::new("nostats");
    let o = r.run(
        &[
            ("SUSPEND_IRQ_MAX", "100"),
            ("SUSPEND_GUARD", "true"),
            ("SUSPEND_STATS", "/nonexistent/stats"),
        ],
        true,
    );
    assert!(o.starts_with("skip suspend-irq-quiet"), "{o}");
    assert!(o.contains("counter"), "{o}");
}
