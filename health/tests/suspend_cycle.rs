// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco-health suspend plan|cycle` (backlog health-suspend-cycle): one controlled suspend and wake.
//! Run here against a made-up phone under LPOCO_HEALTH_ROOT: sysfs, the RTC, the power state (a fifo: whoever
//! reads it "suspends" the phone and the counters change), the manifest's env file. The phone proof is surya's.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Rig {
    root: PathBuf,
}

fn put(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

fn stat(count: u64, ms: u64) -> String {
    format!("Count: {count}\nLast Entered At: 1\nLast Exited At: 2\nAccumulated Duration: {ms}\n")
}

impl Rig {
    fn new(name: &str, env: &str) -> Rig {
        let root =
            std::env::temp_dir().join(format!("suspend-cycle-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        put(&root, "etc/lpoco-health/health.env", env);
        put(&root, "proc/sys/kernel/random/boot_id", "boot-aaa\n");
        put(&root, "sys/class/rtc/rtc0/since_epoch", "1000\n");
        put(&root, "sys/class/rtc/rtc0/wakealarm", "");
        put(&root, "sys/power/suspend_stats/success", "3\n");
        put(&root, "sys/kernel/debug/qcom_stats/cxsd", &stat(2, 100));
        put(&root, "sys/kernel/debug/qcom_stats/aosd", &stat(2, 100));
        put(
            &root,
            "sys/devices/system/cpu/cpu0/cpuidle/state1/name",
            "cluster-sleep\n",
        );
        put(
            &root,
            "sys/devices/system/cpu/cpu0/cpuidle/state1/usage",
            "10\n",
        );
        put(
            &root,
            "sys/devices/system/cpu/cpu1/cpuidle/state1/name",
            "cluster-sleep\n",
        );
        put(
            &root,
            "sys/devices/system/cpu/cpu1/cpuidle/state1/usage",
            "5\n",
        );
        put(&root, "sys/class/wakeup/wakeup0/name", "alarmtimer\n");
        put(&root, "sys/class/wakeup/wakeup0/event_count", "7\n");
        put(&root, "sys/class/wakeup/wakeup1/name", "serial\n");
        put(&root, "sys/class/wakeup/wakeup1/event_count", "1\n");
        put(
            &root,
            "sys/bus/platform/devices/88c000.serial/power/wakeup",
            "enabled\n",
        );
        put(&root, "armed", "");
        // after the cycle (what the fake phone puts in place when it "suspends")
        put(&root, "after/cxsd", &stat(3, 1600));
        put(&root, "after/aosd", &stat(3, 1600));
        put(&root, "after/state1usage", "14\n");
        put(&root, "after/state1usage1", "8\n");
        put(&root, "after/serial", "5\n");
        let fifo = root.join("sys/power/state");
        assert!(Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success());
        Rig { root }
    }

    fn p(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.p(rel)).unwrap_or_default()
    }

    fn run(&self, args: &[&str], suspends: bool) -> Output {
        let r = &self.root;
        let reader = suspends.then(|| {
            let script = r#"
cat "$1/sys/power/state" > "$1/seen_state"
echo "$(cat "$1/sys/class/wakeup/wakeup1/event_count" | tr -d '\n')" > "$1/serial_before"
cat "$1/sys/bus/platform/devices/88c000.serial/power/wakeup" > "$1/wakeup_during"
cp "$1/after/cxsd" "$1/sys/kernel/debug/qcom_stats/cxsd"
cp "$1/after/aosd" "$1/sys/kernel/debug/qcom_stats/aosd"
cp "$1/after/state1usage" "$1/sys/devices/system/cpu/cpu0/cpuidle/state1/usage"
cp "$1/after/state1usage1" "$1/sys/devices/system/cpu/cpu1/cpuidle/state1/usage"
cp "$1/after/serial" "$1/sys/class/wakeup/wakeup1/event_count"
echo 4 > "$1/sys/power/suspend_stats/success"
echo 1005 > "$1/sys/class/rtc/rtc0/since_epoch"
"#;
            Command::new("timeout")
                .args(["20", "sh", "-c", script, "sh"])
                .arg(r)
                .spawn()
                .unwrap()
        });
        let out = Command::new("timeout")
            .arg("20")
            .arg(env!("CARGO_BIN_EXE_lpoco-health"))
            .args(args)
            .env("LPOCO_HEALTH_ROOT", r)
            .output()
            .unwrap();
        if let Some(mut c) = reader {
            let _ = c.wait();
        }
        out
    }
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

const ARMED: &str = "SUSPEND_GUARD='test -e $LPOCO_HEALTH_ROOT/armed'\nSUSPEND_QUIET='/sys/bus/platform/devices/88c000.serial/power/wakeup'\n";

#[test]
fn plan_with_no_phone_says_what_it_would_do_and_touches_nothing() {
    let rig = Rig::new("plan", ARMED);
    let o = rig.run(&["suspend", "plan", "--seconds", "5"], false);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["verb"], "plan");
    assert_eq!(v["would_cycle"], true, "{v}");
    assert_eq!(v["seconds"], 5);
    assert_eq!(
        v["quiet"][0],
        "/sys/bus/platform/devices/88c000.serial/power/wakeup"
    );
    assert_eq!(v["before"]["suspends"], 3);
    assert_eq!(v["before"]["soc"]["cxsd"]["count"], 2);
    assert_eq!(
        rig.read("sys/class/rtc/rtc0/wakealarm"),
        "",
        "plan sets no alarm"
    );
    assert_eq!(
        rig.read("sys/bus/platform/devices/88c000.serial/power/wakeup"),
        "enabled\n"
    );
    assert!(!rig.p("var/lib/lpoco-health/suspend/cycle.json").exists());
}

#[test]
fn it_refuses_without_a_declared_way_back() {
    for env in ["SUSPEND_GUARD=''\n", ""] {
        let rig = Rig::new("noway", env);
        let o = rig.run(&["suspend", "cycle"], false);
        assert_eq!(o.status.code(), Some(2), "{}", text(&o.stdout));
        assert!(text(&o.stderr).contains("way back"), "{}", text(&o.stderr));
        assert!(rig.read("seen_state").is_empty(), "nothing suspended");
        let plan = rig.run(&["suspend", "plan"], false);
        let v: serde_json::Value = serde_json::from_slice(&plan.stdout).unwrap();
        assert_eq!(v["would_cycle"], false);
        assert!(
            v["refusals"][0].as_str().unwrap().contains("way back"),
            "{v}"
        );
    }
}

#[test]
fn it_refuses_when_the_way_back_is_not_armed() {
    let rig = Rig::new("unarmed", ARMED);
    fs::remove_file(rig.p("armed")).unwrap();
    let o = rig.run(&["suspend", "cycle"], false);
    assert_eq!(o.status.code(), Some(2));
    assert!(text(&o.stderr).contains("not armed"), "{}", text(&o.stderr));
}

#[test]
fn a_cycle_counts_before_and_after_quiets_the_wake_sources_and_puts_them_back() {
    let rig = Rig::new("cycle", ARMED);
    let o = rig.run(
        &["suspend", "cycle", "--seconds", "5", "--settle", "1"],
        true,
    );
    assert_eq!(
        o.status.code(),
        Some(0),
        "{} {}",
        text(&o.stdout),
        text(&o.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["verb"], "cycle");
    assert_eq!(v["boot"], "boot-aaa");
    assert_eq!(v["suspended"], true, "{v}");
    assert_eq!(v["soc_slept"], true, "{v}");
    assert_eq!(v["asleep_s"], 5);
    assert_eq!(v["soc"]["cxsd"]["delta"], 1);
    assert_eq!(v["soc"]["aosd"]["delta"], 1);
    assert_eq!(
        v["idle"]["cluster-sleep"], 7,
        "summed over the cpus: (14-10)+(8-5)"
    );
    assert_eq!(
        v["wakeups"][0]["name"], "serial",
        "the source that counted most, first: {v}"
    );
    assert_eq!(v["wakeups"][0]["delta"], 4);
    assert_eq!(rig.read("seen_state"), "freeze");
    assert_eq!(
        rig.read("wakeup_during").trim(),
        "disabled",
        "quiet during the cycle"
    );
    assert_eq!(
        rig.read("sys/bus/platform/devices/88c000.serial/power/wakeup")
            .trim(),
        "enabled",
        "put back"
    );
    assert_eq!(
        rig.read("sys/class/rtc/rtc0/wakealarm").trim(),
        "0",
        "alarm disarmed"
    );
    let kept: serde_json::Value =
        serde_json::from_str(&rig.read("var/lib/lpoco-health/suspend/cycle.json")).unwrap();
    assert_eq!(kept["boot"], "boot-aaa");
    assert_eq!(kept["soc_slept"], true);
}

#[test]
fn a_phone_that_did_not_suspend_is_said_so_not_called_sleeping() {
    let rig = Rig::new("nosleep", ARMED);
    // the fake phone does not suspend: nothing reads the power state, so the state write must not hang
    fs::remove_file(rig.p("sys/power/state")).unwrap();
    put(&rig.root, "sys/power/state", "");
    let o = rig.run(
        &["suspend", "cycle", "--seconds", "5", "--settle", "0"],
        false,
    );
    assert_eq!(o.status.code(), Some(1), "{}", text(&o.stderr));
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["suspended"], false);
    assert_eq!(v["soc_slept"], false);
    assert_eq!(
        rig.read("sys/bus/platform/devices/88c000.serial/power/wakeup")
            .trim(),
        "enabled"
    );
}

#[test]
fn a_cycle_shorter_than_the_rtc_can_time_is_refused() {
    let rig = Rig::new("short", ARMED);
    let o = rig.run(&["suspend", "cycle", "--seconds", "1"], false);
    assert_eq!(o.status.code(), Some(2));
    assert!(text(&o.stderr).contains("too short"), "{}", text(&o.stderr));
}

#[test]
fn a_source_that_cannot_be_quieted_is_named_not_claimed() {
    let rig = Rig::new("typo", &format!("{ARMED}SUSPEND_QUIET='/sys/bus/platform/devices/88c000.serial/power/wakeup /sys/no/such/wakeup'\n"));
    let o = rig.run(
        &["suspend", "cycle", "--seconds", "5", "--settle", "1"],
        true,
    );
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["not_quieted"][0], "/sys/no/such/wakeup", "{v}");
}

#[test]
fn a_wake_long_before_the_alarm_is_not_called_sleeping() {
    let rig = Rig::new("early", ARMED);
    // the fake phone says it slept 5 s of the 10 asked for
    let o = rig.run(
        &["suspend", "cycle", "--seconds", "10", "--settle", "1"],
        true,
    );
    assert_eq!(o.status.code(), Some(1), "{}", text(&o.stdout));
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["woke_early"], true, "{v}");
    assert_eq!(v["soc_slept"], false);
}

#[test]
fn quotes_in_the_guard_survive_the_env_file() {
    let rig = Rig::new(
        "quote",
        "SUSPEND_GUARD='test -e '\\''/nonexistent dir'\\'' || test -e '\\''/etc'\\'''\n",
    );
    let o = rig.run(&["suspend", "plan"], false);
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(
        v["way_back"], "test -e '/nonexistent dir' || test -e '/etc'",
        "{v}"
    );
}

#[test]
fn sources_left_quiet_by_a_cut_off_cycle_are_put_back_by_the_next() {
    let rig = Rig::new("stale", ARMED);
    let w = "sys/bus/platform/devices/88c000.serial/power/wakeup";
    put(&rig.root, w, "disabled\n");
    put(
        &rig.root,
        "var/lib/lpoco-health/suspend/quiet.json",
        &format!("{{\"{}\":\"enabled\"}}", rig.p(w).display()),
    );
    let _ = rig.run(
        &["suspend", "cycle", "--seconds", "5", "--settle", "1"],
        true,
    );
    assert_eq!(rig.read("wakeup_during").trim(), "disabled");
    assert_eq!(
        rig.read(w).trim(),
        "enabled",
        "the original, not 'disabled', is what comes back"
    );
}
