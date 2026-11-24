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
        // tracefs, dynamic_debug and dmesg of the made-up phone (what a trace hook turns on and reads)
        put(&root, "sys/kernel/tracing/tracing_on", "0\n");
        put(&root, "sys/kernel/tracing/trace", "");
        put(
            &root,
            "sys/kernel/tracing/events/irq/irq_handler_entry/enable",
            "0\n",
        );
        put(
            &root,
            "sys/kernel/tracing/events/irq/irq_handler_entry/filter",
            "none\n",
        );
        put(&root, "sys/kernel/debug/dynamic_debug/control", "");
        put(
            &root,
            "bin/dmesg",
            "#!/bin/sh\necho \"[ 1.0] hci_qca: probe ok\"\necho \"[ 2.0] qcom_geni_serial: rx\"\n",
        );
        assert!(Command::new("chmod")
            .arg("+x")
            .arg(root.join("bin/dmesg"))
            .status()
            .unwrap()
            .success());
        put(&root, "armed", "");
        put(&root, "bin/systemctl", FAKE_SYSTEMCTL);
        assert!(Command::new("chmod")
            .arg("+x")
            .arg(root.join("bin/systemctl"))
            .status()
            .unwrap()
            .success());
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
cat "$1/systemctl.log" > "$1/systemctl_during" 2>/dev/null
for f in tracing_on events/irq/irq_handler_entry/enable events/irq/irq_handler_entry/filter; do cat "$1/sys/kernel/tracing/$f" > "$1/during_$(echo $f | tr / _)" 2>/dev/null; done
cat "$1/sys/kernel/debug/dynamic_debug/control" > "$1/dyndbg_during" 2>/dev/null
printf 'irq_handler_entry: irq=157 name=uart\nirq_handler_entry: irq=157 name=uart\n' >> "$1/sys/kernel/tracing/trace"
cat "$1/hook.log" > "$1/hook_during" 2>/dev/null
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
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    r.join("bin").display(),
                    std::env::var("PATH").unwrap()
                ),
            )
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

/// A systemctl that writes down "verb unit" (not is-active), says every unit is active unless a file
/// `inactive` beside the root names it, and fails for the verb in `fail` when there is one.
const FAKE_SYSTEMCTL: &str = "#!/bin/sh\nfor a; do u=$a; done\ngrep -qx \"$u\" \"$LPOCO_HEALTH_ROOT/inactive\" 2>/dev/null && [ \"$1\" = is-active ] && exit 1\n[ \"$1\" = is-active ] && exit 0\necho \"$1 $u\" >> \"$LPOCO_HEALTH_ROOT/systemctl.log\"\n[ \"$1\" != \"$(cat \"$LPOCO_HEALTH_ROOT/fail\" 2>/dev/null)\" ]\n";

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

#[test]
fn the_services_the_manifest_names_are_stopped_before_the_suspend_and_started_after() {
    let rig = Rig::new(
        "stop",
        &format!("{ARMED}SUSPEND_STOP='sensorspd hexagonrpcd'\n"),
    );
    let plan = rig.run(&["suspend", "plan"], false);
    let p: serde_json::Value = serde_json::from_slice(&plan.stdout).unwrap();
    assert_eq!(p["stop"][0], "sensorspd", "plan names them: {p}");
    assert_eq!(rig.read("systemctl.log"), "", "plan stops nothing");
    let o = rig.run(
        &["suspend", "cycle", "--seconds", "5", "--settle", "1"],
        true,
    );
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stdout));
    assert_eq!(
        rig.read("systemctl_during"),
        "stop sensorspd\nstop hexagonrpcd\n",
        "stopped, in order, before the phone went down"
    );
    assert_eq!(
        rig.read("systemctl.log"),
        "stop sensorspd\nstop hexagonrpcd\nstart hexagonrpcd\nstart sensorspd\n",
        "started again after, in the reverse order"
    );
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["stopped"][1], "hexagonrpcd", "{v}");
    assert_eq!(v["not_started"].as_array().unwrap().len(), 0, "{v}");
    assert!(!rig.p("var/lib/lpoco-health/suspend/stopped.json").exists());
}

#[test]
fn a_unit_that_was_not_running_is_left_alone() {
    let rig = Rig::new("idle", &format!("{ARMED}SUSPEND_STOP='sensorspd gone'\n"));
    put(&rig.root, "inactive", "gone\n");
    let o = rig.run(
        &["suspend", "cycle", "--seconds", "5", "--settle", "1"],
        true,
    );
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stdout));
    assert_eq!(
        rig.read("systemctl.log"),
        "stop sensorspd\nstart sensorspd\n",
        "the inactive one is neither stopped nor started"
    );
}

#[test]
fn a_service_that_would_not_start_again_is_named_and_fails_the_run() {
    let rig = Rig::new("nostart", &format!("{ARMED}SUSPEND_STOP='sensorspd'\n"));
    put(&rig.root, "fail", "start");
    let o = rig.run(
        &["suspend", "cycle", "--seconds", "5", "--settle", "1"],
        true,
    );
    assert_eq!(o.status.code(), Some(1), "a dead daemon is not a pass");
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["not_started"][0], "sensorspd", "{v}");
}

#[test]
fn a_service_that_would_not_stop_is_named() {
    let rig = Rig::new("nostop", &format!("{ARMED}SUSPEND_STOP='sensorspd'\n"));
    put(&rig.root, "fail", "stop");
    let o = rig.run(
        &["suspend", "cycle", "--seconds", "5", "--settle", "1"],
        true,
    );
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["not_stopped"][0], "sensorspd", "{v}");
}

#[test]
fn a_refused_cycle_stops_nothing() {
    let un = Rig::new(
        "unarmed-stop",
        &format!("{ARMED}SUSPEND_STOP='sensorspd'\n"),
    );
    fs::remove_file(un.p("armed")).unwrap();
    let o = un.run(&["suspend", "cycle"], false);
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(un.read("systemctl.log"), "", "refused: nothing stopped");
}

#[test]
fn services_left_stopped_by_a_cut_off_cycle_are_started_by_the_next() {
    let rig = Rig::new("stale-stop", &format!("{ARMED}SUSPEND_STOP='sensorspd'\n"));
    put(
        &rig.root,
        "var/lib/lpoco-health/suspend/stopped.json",
        "[\"hexagonrpcd\"]",
    );
    let _ = rig.run(
        &["suspend", "cycle", "--seconds", "5", "--settle", "1"],
        true,
    );
    assert_eq!(
        rig.read("systemctl.log"),
        "start hexagonrpcd\nstop sensorspd\nstart sensorspd\n"
    );
}

#[test]
fn the_before_hook_runs_ahead_of_the_suspend_and_the_after_hook_behind_it() {
    let rig = Rig::new("hooks", ARMED);
    let o = rig.run(
        &[
            "suspend",
            "cycle",
            "--seconds",
            "5",
            "--settle",
            "1",
            "--before",
            "echo before >> $LPOCO_HEALTH_ROOT/hook.log",
            "--after",
            "echo after >> $LPOCO_HEALTH_ROOT/hook.log",
        ],
        true,
    );
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
    assert_eq!(
        rig.read("hook_during"),
        "before\n",
        "only before, while suspended"
    );
    assert_eq!(rig.read("hook.log"), "before\nafter\n");
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["hooks"]["before"]["ok"], true, "{v}");
    assert_eq!(v["hooks"]["after"]["ok"], true, "{v}");
}

#[test]
fn a_before_hook_that_fails_stops_the_cycle_and_the_after_hook_still_runs() {
    let rig = Rig::new("hook-fails", ARMED);
    let o = rig.run(
        &[
            "suspend",
            "cycle",
            "--before",
            "echo before >> $LPOCO_HEALTH_ROOT/hook.log; exit 3",
            "--after",
            "echo after >> $LPOCO_HEALTH_ROOT/hook.log",
        ],
        false,
    );
    assert_eq!(o.status.code(), Some(2), "{}", text(&o.stdout));
    assert!(
        text(&o.stderr).contains("before hook"),
        "{}",
        text(&o.stderr)
    );
    assert!(rig.read("seen_state").is_empty(), "nothing suspended");
    assert_eq!(rig.read("hook.log"), "before\nafter\n");
}

#[test]
fn a_hook_is_not_run_by_a_refused_cycle_or_a_plan() {
    let rig = Rig::new("hook-refused", ARMED);
    fs::remove_file(rig.p("armed")).unwrap();
    let hook = "echo ran >> $LPOCO_HEALTH_ROOT/hook.log";
    let _ = rig.run(
        &["suspend", "cycle", "--before", hook, "--after", hook],
        false,
    );
    let p = rig.run(&["suspend", "plan", "--before", hook], false);
    assert_eq!(p.status.code(), Some(0));
    assert_eq!(rig.read("hook.log"), "");
}

#[test]
fn a_hook_that_prints_leaves_the_report_alone_and_a_failing_after_hook_is_reported() {
    let rig = Rig::new("hook-prints", ARMED);
    let o = rig.run(
        &[
            "suspend",
            "cycle",
            "--seconds",
            "5",
            "--settle",
            "1",
            "--before",
            "echo noise",
            "--after",
            "cat $LPOCO_HEALTH_ROOT/systemctl.log >> $LPOCO_HEALTH_ROOT/hook.log; exit 4",
        ],
        true,
    );
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
    let v: serde_json::Value =
        serde_json::from_slice(&o.stdout).expect("stdout is one JSON report");
    assert_eq!(v["hooks"]["after"]["ok"], false, "{v}");
    assert!(text(&o.stderr).contains("noise"));
}

const TRACED: &str = "SUSPEND_GUARD='test -e $LPOCO_HEALTH_ROOT/armed'\nSUSPEND_TRACE_DYNDBG='file drivers/bluetooth/hci_qca.c;file drivers/tty/serial/qcom_geni_serial.c'\nSUSPEND_TRACE_IRQ='157'\n";

// source of the interfaces: the kernel's tracing documentation (Documentation/trace/events.rst: events/SUBSYS/EVENT/
// enable and filter, tracing_on, trace) and dynamic-debug-howto.rst (`echo 'file x.c +p' > .../control`); the sites
// and irq 157 (qcom_geni_serial_uart0) are the ones surya's sleep/bt-trace.sh used.

#[test]
fn a_traced_cycle_turns_the_sites_on_before_the_suspend_collects_them_after_and_puts_everything_back(
) {
    let rig = Rig::new("trace", TRACED);
    let o = rig.run(
        &[
            "suspend",
            "cycle",
            "--seconds",
            "5",
            "--settle",
            "1",
            "--trace",
        ],
        true,
    );
    assert_eq!(
        o.status.code(),
        Some(0),
        "{} {}",
        text(&o.stdout),
        text(&o.stderr)
    );
    // during the suspend: tracing on, the event on, filtered to the irq, the dynamic-debug sites on
    assert_eq!(rig.read("during_tracing_on").trim(), "1");
    assert_eq!(
        rig.read("during_events_irq_irq_handler_entry_enable")
            .trim(),
        "1"
    );
    assert_eq!(
        rig.read("during_events_irq_irq_handler_entry_filter")
            .trim(),
        "irq == 157"
    );
    let during = rig.read("dyndbg_during");
    assert!(
        during.contains("file drivers/bluetooth/hci_qca.c +p")
            && during.contains("file drivers/tty/serial/qcom_geni_serial.c +p"),
        "{during}"
    );
    // after: everything off, the filter cleared, the sites switched off again
    assert_eq!(rig.read("sys/kernel/tracing/tracing_on").trim(), "0");
    assert_eq!(
        rig.read("sys/kernel/tracing/events/irq/irq_handler_entry/enable")
            .trim(),
        "0"
    );
    assert_eq!(
        rig.read("sys/kernel/tracing/events/irq/irq_handler_entry/filter")
            .trim(),
        "0",
        "the filter is cleared the way the kernel documents (echo 0)"
    );
    let after = rig.read("sys/kernel/debug/dynamic_debug/control");
    assert!(
        after.contains("file drivers/bluetooth/hci_qca.c -p")
            && after.contains("file drivers/tty/serial/qcom_geni_serial.c -p"),
        "{after}"
    );
    // collected into the run's report and kept beside it
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["trace"]["irq_events"], 2, "{v}");
    assert_eq!(v["trace"]["irq"][0], 157);
    assert_eq!(v["trace"]["dyndbg"][0], "file drivers/bluetooth/hci_qca.c");
    let kept = rig.read("var/lib/lpoco-health/suspend/cycle.trace");
    assert!(kept.contains("irq=157"), "{kept}");
    let dmesg = rig.read("var/lib/lpoco-health/suspend/cycle.dmesg");
    assert!(dmesg.contains("hci_qca: probe ok"), "{dmesg}");
}

#[test]
fn without_trace_nothing_of_the_trace_is_touched() {
    let rig = Rig::new("notrace", TRACED);
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
    assert_eq!(rig.read("during_tracing_on").trim(), "0");
    assert_eq!(rig.read("sys/kernel/debug/dynamic_debug/control"), "");
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert!(v["trace"].is_null(), "{v}");
    assert!(!rig.p("var/lib/lpoco-health/suspend/cycle.trace").exists());
}

#[test]
fn trace_asked_with_nothing_declared_is_refused_and_nothing_suspends() {
    let rig = Rig::new("notdeclared", ARMED);
    let o = rig.run(&["suspend", "cycle", "--trace"], false);
    assert_eq!(o.status.code(), Some(2), "{}", text(&o.stdout));
    assert!(text(&o.stderr).contains("trace"), "{}", text(&o.stderr));
    assert!(rig.read("seen_state").is_empty(), "nothing suspended");
}

#[test]
fn a_cut_off_traced_cycle_is_put_right_by_the_next_cycle() {
    // left on by a cycle that was killed: the next cycle (traced or not) switches them off first
    let rig = Rig::new("stale-trace", TRACED);
    fs::write(rig.p("sys/kernel/tracing/tracing_on"), "1\n").unwrap();
    fs::write(
        rig.p("sys/kernel/tracing/events/irq/irq_handler_entry/enable"),
        "1\n",
    )
    .unwrap();
    fs::create_dir_all(rig.p("var/lib/lpoco-health/suspend")).unwrap();
    fs::write(
        rig.p("var/lib/lpoco-health/suspend/trace.json"),
        r#"{"dyndbg":["file drivers/bluetooth/hci_qca.c"],"irq":[157]}"#, // as Tracing::start writes it
    )
    .unwrap();
    let o = rig.run(
        &["suspend", "cycle", "--seconds", "5", "--settle", "1"],
        true,
    );
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
    assert_eq!(rig.read("sys/kernel/tracing/tracing_on").trim(), "0");
    assert_eq!(
        rig.read("sys/kernel/tracing/events/irq/irq_handler_entry/enable")
            .trim(),
        "0"
    );
    assert!(rig
        .read("sys/kernel/debug/dynamic_debug/control")
        .contains("file drivers/bluetooth/hci_qca.c -p"));
}

#[test]
fn a_traced_cycle_puts_tracefs_back_as_it_found_it() {
    // the kernel's default is tracing on; an event may be on with a filter already: they are written back, not cleared
    let rig = Rig::new("trace-held", TRACED);
    fs::write(rig.p("sys/kernel/tracing/tracing_on"), "1\n").unwrap();
    fs::write(
        rig.p("sys/kernel/tracing/events/irq/irq_handler_entry/enable"),
        "1\n",
    )
    .unwrap();
    fs::write(
        rig.p("sys/kernel/tracing/events/irq/irq_handler_entry/filter"),
        "irq == 3\n",
    )
    .unwrap();
    let o = rig.run(
        &[
            "suspend",
            "cycle",
            "--seconds",
            "5",
            "--settle",
            "1",
            "--trace",
        ],
        true,
    );
    assert_eq!(
        o.status.code(),
        Some(0),
        "{} {}",
        text(&o.stdout),
        text(&o.stderr)
    );
    assert_eq!(rig.read("sys/kernel/tracing/tracing_on").trim(), "1");
    assert_eq!(
        rig.read("sys/kernel/tracing/events/irq/irq_handler_entry/enable")
            .trim(),
        "1"
    );
    assert_eq!(
        rig.read("sys/kernel/tracing/events/irq/irq_handler_entry/filter")
            .trim(),
        "irq == 3"
    );
    assert!(
        !rig.p("var/lib/lpoco-health/suspend/trace.json").exists(),
        "the record of what was on is gone once it is all back"
    );
}

#[test]
fn a_trace_that_cannot_be_switched_on_refuses_the_cycle_and_nothing_suspends() {
    let rig = Rig::new("trace-no-tracefs", TRACED);
    fs::remove_dir_all(rig.p("sys/kernel/tracing")).unwrap();
    let o = rig.run(&["suspend", "cycle", "--trace"], false);
    assert_eq!(o.status.code(), Some(2), "{}", text(&o.stdout));
    assert!(
        text(&o.stderr).contains("trace could not be switched on"),
        "{}",
        text(&o.stderr)
    );
    assert!(rig.read("seen_state").is_empty(), "nothing suspended");
}

#[test]
fn a_query_that_carries_its_own_flag_is_refused_before_anything_is_written() {
    let env =
        "SUSPEND_GUARD='test -e $LPOCO_HEALTH_ROOT/armed'\nSUSPEND_TRACE_DYNDBG='file a.c +p'\n";
    let rig = Rig::new("trace-flag", env);
    let o = rig.run(&["suspend", "cycle", "--trace"], false);
    assert_eq!(o.status.code(), Some(2), "{}", text(&o.stdout));
    assert!(
        text(&o.stderr).contains("without a flag"),
        "{}",
        text(&o.stderr)
    );
    assert_eq!(rig.read("sys/kernel/debug/dynamic_debug/control"), "");
}

#[test]
fn plan_names_the_trace_the_manifest_declares() {
    let rig = Rig::new("trace-plan", TRACED);
    let o = rig.run(&["suspend", "plan", "--seconds", "5"], false);
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["trace"]["irq"][0], 157, "{v}");
    assert_eq!(
        v["trace"]["dyndbg"][1],
        "file drivers/tty/serial/qcom_geni_serial.c"
    );
}
