// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco-health suspend plan|cycle`: one controlled suspend and wake, counted.
//!
//! What surya's keeper did by hand (surya-lab sleep/way-back.sh cycle) as a verb of the suite. The device's
//! manifest ([health]) declares the way back - `suspend_guard`, a command that succeeds while the phone's
//! watchdog is armed, so a suspend that does not return resets the phone - and the wake sources to quiet for
//! the cycle (`suspend_quiet`, wakeup files) and the services to stop for it (`suspend_stop`, started again after). Without a way back the cycle is refused and nothing suspends.
//! The cycle reads, before and after: the SoC's own sleep counters (qcom_stats), the suspend counter, the cpu
//! idle states (cluster states among them) and the wakeup sources; it puts every quieted source back; it keeps
//! the report as this boot's cycle, which the check soc-sleeps reads.
//!
//! Every path lies under LPOCO_HEALTH_ROOT (empty on the phone), so the tests run it on a made-up phone.
//! `plan` changes nothing: it says what a cycle would do, or why it would refuse.

use crate::trace;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const ENV: &str = "etc/lpoco-health/health.env";
const OUT: &str = "var/lib/lpoco-health/suspend";
const STATS: &str = "sys/kernel/debug/qcom_stats";
/// The RTC counts whole seconds: a shorter sleep may be over before the phone is down, and then nothing wakes it.
const MIN_SECONDS: u64 = 5;
const SOC_STATES: [&str; 6] = ["cxsd", "aosd", "ddr", "adsp", "cdsp", "modem"];

/// What one cycle is asked: its length, its settling time, and the phone it is on.
pub struct Ask {
    pub root: PathBuf,
    pub seconds: u64,
    pub settle: u64,
    /// one-off commands (sh -c): `before` after the services are stopped and ahead of the suspend, `after` once
    /// the counters are read - a probe's tracing on and off, with no wrapper around the verb
    pub before: String,
    pub after: String,
    /// switch on the manifest's trace sites for this cycle (`--trace`)
    pub trace: bool,
}

/// What the manifest declares for the cycle.
#[derive(Debug, Default, PartialEq)]
struct Declared {
    guard: String,
    quiet: Vec<String>,
    /// services stopped before the suspend and started after it (systemd units, in the order named)
    stop: Vec<String>,
    /// the dynamic_debug queries and irq numbers a `--trace` cycle switches on
    trace: trace::Sites,
}

/// The value of a line `KEY='...'` as devlab writes it (`'` inside is `'\''`): one pair of quotes off, the
/// escapes undone - exactly what a shell sourcing the file would read.
fn unquote(v: &str) -> String {
    let v = v.trim();
    match v.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')) {
        Some(inner) => inner.replace("'\\''", "'"),
        None => v.to_string(),
    }
}

fn declared(env_text: &str) -> Declared {
    let mut d = Declared::default();
    for l in env_text.lines() {
        let Some((k, v)) = l.trim().split_once('=') else {
            continue;
        };
        match k.trim() {
            "SUSPEND_GUARD" => d.guard = unquote(v),
            "SUSPEND_QUIET" => d.quiet = unquote(v).split_whitespace().map(String::from).collect(),
            "SUSPEND_STOP" => d.stop = unquote(v).split_whitespace().map(String::from).collect(),
            "SUSPEND_TRACE_DYNDBG" => {
                d.trace.dyndbg = unquote(v)
                    .split(';')
                    .map(|q| q.trim().to_string())
                    .filter(|q| !q.is_empty())
                    .collect()
            }
            "SUSPEND_TRACE_IRQ" => {
                d.trace.irq = unquote(v)
                    .split_whitespace()
                    .filter_map(|n| n.parse().ok())
                    .collect()
            }
            _ => {}
        }
    }
    d
}

/// `Count:` and `Accumulated Duration:` of one qcom_stats file.
fn parse_stat(text: &str) -> Option<(u64, u64)> {
    let field = |name: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(name))
            .and_then(|v| v.trim().parse::<u64>().ok())
    };
    Some((
        field("Count:")?,
        field("Accumulated Duration:").unwrap_or(0),
    ))
}

fn num(path: &Path) -> Option<u64> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn text(path: &Path) -> String {
    std::fs::read_to_string(path)
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Everything counted at one moment.
#[derive(Debug, Default, Clone, PartialEq)]
struct Counters {
    suspends: Option<u64>,
    /// SoC sleep state -> (count, accumulated duration)
    soc: Vec<(String, u64, u64)>,
    /// cpu idle state name -> entries, summed over the cpus
    idle: Vec<(String, u64)>,
    /// wakeup source name -> events
    wakeups: Vec<(String, u64)>,
    rtc: Option<u64>,
}

fn read_counters(root: &Path) -> Counters {
    let mut c = Counters {
        suspends: num(&root.join("sys/power/suspend_stats/success")),
        rtc: num(&root.join("sys/class/rtc/rtc0/since_epoch")),
        ..Counters::default()
    };
    for s in SOC_STATES {
        if let Some((n, d)) = std::fs::read_to_string(root.join(STATS).join(s))
            .ok()
            .and_then(|t| parse_stat(&t))
        {
            c.soc.push((s.into(), n, d));
        }
    }
    let mut idle: std::collections::BTreeMap<String, u64> = Default::default();
    for cpu in dirs(&root.join("sys/devices/system/cpu"), "cpu") {
        for st in dirs(&cpu.join("cpuidle"), "state") {
            let name = text(&st.join("name"));
            if let (false, Some(u)) = (name.is_empty(), num(&st.join("usage"))) {
                *idle.entry(name).or_default() += u;
            }
        }
    }
    c.idle = idle.into_iter().collect();
    for w in dirs(&root.join("sys/class/wakeup"), "wakeup") {
        if let Some(n) = num(&w.join("event_count")) {
            c.wakeups.push((text(&w.join("name")), n));
        }
    }
    c
}

/// Subdirectories of `dir` whose name is `prefix` and a number, in order.
fn dirs(dir: &Path, prefix: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_prefix(prefix))
                .is_some_and(|r| !r.is_empty() && r.bytes().all(|b| b.is_ascii_digit()))
        })
        .collect();
    v.sort_by_key(|p| {
        let n = p.file_name().unwrap().to_string_lossy().into_owned();
        (n.len(), n)
    });
    v
}

fn snapshot(c: &Counters) -> Value {
    let mut soc = Map::new();
    for (n, count, ms) in &c.soc {
        soc.insert(n.clone(), json!({"count": count, "duration": ms}));
    }
    let idle: Map<String, Value> = c.idle.iter().map(|(n, u)| (n.clone(), json!(u))).collect();
    json!({"suspends": c.suspends, "soc": soc, "idle": idle, "rtc": c.rtc})
}

/// Why a cycle would be refused, in plain words; empty when none.
fn refusals(root: &Path, d: &Declared, ask: &Ask) -> Vec<String> {
    let seconds = ask.seconds;
    let mut r = Vec::new();
    if ask.trace {
        if d.trace.is_empty() {
            r.push("--trace asked, but the manifest's [health] declares no suspend_trace_dyndbg or suspend_trace_irq to trace".into());
        }
        r.extend(d.trace.problems());
    }
    if seconds < MIN_SECONDS {
        r.push(format!("--seconds {seconds} is too short: at least {MIN_SECONDS} (the RTC counts whole seconds; a sleep over before the phone is down is never woken)"));
    }
    if d.guard.is_empty() {
        r.push("no way back declared: the manifest's [health] suspend_guard names none, so the phone is never suspended".into());
    } else {
        let armed = Command::new("sh")
            .args(["-c", &d.guard])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if !armed {
            r.push(format!(
                "the way back is not armed: suspend_guard \"{}\" fails",
                d.guard
            ));
        }
    }
    if !root.join("sys/class/rtc/rtc0/since_epoch").exists()
        || !root.join("sys/class/rtc/rtc0/wakealarm").exists()
    {
        r.push("no RTC wake alarm to end the sleep (sys/class/rtc/rtc0)".into());
    }
    if num(&root.join("sys/power/suspend_stats/success")).is_none() {
        r.push(
            "the suspend counter cannot be read, so a suspend could not be told from none".into(),
        );
    }
    r
}

/// Services stopped for the cycle, and started again however the cycle ends (the reverse of the order stopped).
/// Only a unit that was active is stopped, and only that is owed a start; the owed ones are kept in
/// `stopped.json`, so a cycle cut off hard is made good by the next.
struct Stopped {
    owed: Vec<String>,
    /// units whose stop failed: said in the report
    not_stopped: Vec<String>,
    kept: PathBuf,
}

fn systemctl(args: &[&str], unit: &str) -> bool {
    Command::new("systemctl")
        .args(args)
        .args(["--", unit])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

impl Stopped {
    fn keep(&self) {
        let _ = std::fs::create_dir_all(self.kept.parent().unwrap());
        let _ = std::fs::write(&self.kept, json!(self.owed).to_string());
    }

    fn stop(root: &Path, units: &[String]) -> Stopped {
        let kept = root.join(OUT).join("stopped.json");
        let mut s = Stopped {
            owed: serde_json::from_str::<Vec<String>>(
                &std::fs::read_to_string(&kept).unwrap_or_default(),
            )
            .unwrap_or_default(),
            not_stopped: Vec::new(),
            kept,
        };
        s.start_back(); // what a cut-off cycle left stopped, first
        for u in units {
            if !systemctl(&["is-active", "--quiet"], u) {
                continue;
            }
            // owed a start once a stop was tried, whether or not the stop said it worked
            s.owed.push(u.clone());
            s.keep();
            if !systemctl(&["stop"], u) {
                s.not_stopped.push(u.clone());
            }
        }
        s
    }

    /// Start them again, last stopped first; the ones that would not start are returned.
    fn start_back(&mut self) -> Vec<String> {
        let mut failed = Vec::new();
        for u in self.owed.drain(..).rev() {
            if !systemctl(&["start"], &u) {
                failed.push(u);
            }
        }
        let _ = std::fs::remove_file(&self.kept);
        failed
    }
}

impl Drop for Stopped {
    fn drop(&mut self) {
        self.start_back();
    }
}

fn load_declared(ask: &Ask) -> Declared {
    declared(&std::fs::read_to_string(ask.root.join(ENV)).unwrap_or_default())
}

/// Wake sources put quiet for the cycle, and put back however the cycle ends.
struct Quieted {
    put_back: Vec<(PathBuf, String)>,
    /// declared sources that could not be quieted (missing, unwritable): said in the report
    not_quieted: Vec<String>,
    kept: PathBuf,
}

impl Quieted {
    /// A cycle cut off by a signal leaves its sources quiet and its originals here: put them back first, so
    /// that "disabled" is never taken for the original.
    fn restore_stale(root: &Path) {
        let kept = root.join(OUT).join("quiet.json");
        if let Ok(v) =
            serde_json::from_str::<Value>(&std::fs::read_to_string(&kept).unwrap_or_default())
        {
            for (p, old) in v.as_object().into_iter().flatten() {
                let _ = std::fs::write(p, old.as_str().unwrap_or_default());
            }
        }
        let _ = std::fs::remove_file(kept);
    }

    fn quiet(root: &Path, files: &[String]) -> Quieted {
        Quieted::restore_stale(root);
        let mut put_back = Vec::new();
        let mut not_quieted = Vec::new();
        for f in files {
            let p = root.join(f.trim_start_matches('/'));
            let old = text(&p);
            if !old.is_empty() && std::fs::write(&p, "disabled").is_ok() {
                put_back.push((p, old));
            } else {
                not_quieted.push(f.clone());
            }
        }
        let kept = root.join(OUT).join("quiet.json");
        let m: Map<String, Value> = put_back
            .iter()
            .map(|(p, o)| (p.to_string_lossy().into_owned(), json!(o)))
            .collect();
        let _ = std::fs::create_dir_all(root.join(OUT));
        let _ = std::fs::write(&kept, Value::Object(m).to_string());
        Quieted {
            put_back,
            not_quieted,
            kept,
        }
    }
}

impl Drop for Quieted {
    fn drop(&mut self) {
        for (p, old) in &self.put_back {
            let _ = std::fs::write(p, old);
        }
        let _ = std::fs::remove_file(&self.kept);
    }
}

/// One hook command through `sh -c`, its trouble in words; an empty command is no hook.
fn hook(cmd: &str) -> Hook {
    if cmd.trim().is_empty() {
        return None;
    }
    Some(
        match Command::new("sh")
            .args(["-c", cmd])
            .stdin(Stdio::null())
            .stdout(Stdio::from(std::io::stderr())) // the report owns stdout
            .status()
        {
            Ok(s) if s.success() => Ok(()),
            Ok(s) => Err(format!(
                "exit {}",
                s.code().map_or("by signal".into(), |c| c.to_string())
            )),
            Err(e) => Err(format!("not run: {e}")),
        },
    )
}

type Hook = Option<Result<(), String>>;

fn hooks_json(ask: &Ask, before: &Hook, after: &Hook) -> Value {
    json!({"before": hook_json(&ask.before, before), "after": hook_json(&ask.after, after)})
}

fn hook_json(cmd: &str, r: &Option<Result<(), String>>) -> Value {
    match r {
        None => Value::Null,
        Some(Ok(())) => json!({"cmd": cmd, "ok": true}),
        Some(Err(e)) => json!({"cmd": cmd, "ok": false, "why": e}),
    }
}

/// What a cycle would do, or why it would refuse; changes nothing.
pub fn plan(ask: &Ask) -> (i32, Value) {
    let d = load_declared(ask);
    let why = refusals(&ask.root, &d, ask);
    let c = read_counters(&ask.root);
    (
        0,
        json!({"verb": "plan", "would_cycle": why.is_empty(), "refusals": why,
               "seconds": ask.seconds, "quiet": d.quiet, "stop": d.stop, "way_back": d.guard, "before": snapshot(&c),
               "trace": {"dyndbg": d.trace.dyndbg, "irq": d.trace.irq}}),
    )
}

/// The cycle: exit 0 when the SoC slept, 1 when it did not (said, not hidden), 2 when refused.
pub fn cycle(ask: &Ask) -> (i32, Value) {
    let d = load_declared(ask);
    let why = refusals(&ask.root, &d, ask);
    if !why.is_empty() {
        return (2, json!({"verb": "cycle", "refused": why}));
    }
    let root = &ask.root;
    let alarm = root.join("sys/class/rtc/rtc0/wakealarm");
    let out = root.join(OUT);
    // a cycle cut off while tracing left its sites on: off first, whether this one traces or not
    trace::Tracing::put_right_what_a_cut_off_cycle_left(root, &out);
    let quieted = Quieted::quiet(root, &d.quiet);
    let not_quieted = quieted.not_quieted.clone();
    // stopped before the alarm is armed: a slow stop must not eat the sleep
    let mut stopped = Stopped::stop(root, &d.stop);
    let not_stopped = stopped.not_stopped.clone();
    let before_hook = hook(&ask.before);
    if let Some(Err(e)) = &before_hook {
        let after_hook = hook(&ask.after);
        return (
            2,
            json!({"verb": "cycle", "refused": [format!("the before hook failed: {e}")],
                   "hooks": hooks_json(ask, &before_hook, &after_hook)}),
        );
    }
    let tracing = if ask.trace {
        match trace::Tracing::start(root, &out, d.trace.clone()) {
            Ok(t) => Some(t),
            Err(e) => {
                let after_hook = hook(&ask.after);
                return (
                    2,
                    json!({"verb": "cycle", "refused": [format!("the trace could not be switched on: {e}")],
                           "hooks": hooks_json(ask, &before_hook, &after_hook)}),
                );
            }
        }
    } else {
        None
    };
    let before = read_counters(root);
    let target = before.rtc.unwrap_or(0) + ask.seconds;
    let _ = std::fs::write(&alarm, "0");
    let armed = std::fs::write(&alarm, target.to_string()).is_ok();
    if !armed || text(&alarm) != target.to_string() {
        drop(tracing);
        let _ = std::fs::write(&alarm, "0");
        let after_hook = hook(&ask.after);
        return (
            2,
            json!({"verb": "cycle", "refused": ["the RTC wake alarm did not take"],
                   "hooks": hooks_json(ask, &before_hook, &after_hook)}),
        );
    }
    let wrote = std::fs::write(root.join("sys/power/state"), "freeze");
    std::thread::sleep(Duration::from_secs(ask.settle));
    let after = read_counters(root);
    let _ = std::fs::write(&alarm, "0");
    let traced = tracing.map(|t| t.stop());
    let after_hook = hook(&ask.after);
    let not_started = stopped.start_back();
    drop(quieted);

    let suspended = after.suspends.unwrap_or(0) > before.suspends.unwrap_or(0);
    let mut soc = Map::new();
    let mut slept = false;
    for (n, c1, ms1) in &after.soc {
        let (c0, ms0) = before
            .soc
            .iter()
            .find(|(m, _, _)| m == n)
            .map_or((0, 0), |(_, c, m)| (*c, *m));
        let dc = c1.saturating_sub(c0);
        if matches!(n.as_str(), "cxsd" | "aosd") && dc > 0 {
            slept = true;
        }
        soc.insert(
            n.clone(),
            json!({"before": c0, "after": c1, "delta": dc, "duration_delta": ms1.saturating_sub(ms0)}),
        );
    }
    let mut idle = Map::new();
    for (n, u1) in &after.idle {
        let u0 = before
            .idle
            .iter()
            .find(|(m, _)| m == n)
            .map_or(0, |(_, u)| *u);
        idle.insert(n.clone(), json!(u1.saturating_sub(u0)));
    }
    let mut wakeups: Vec<(String, u64)> = after
        .wakeups
        .iter()
        .map(|(n, e1)| {
            let e0 = before
                .wakeups
                .iter()
                .find(|(m, _)| m == n)
                .map_or(0, |(_, e)| *e);
            (n.clone(), e1.saturating_sub(e0))
        })
        .filter(|(_, dl)| *dl > 0)
        .collect();
    wakeups.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let asleep = match (before.rtc, after.rtc) {
        (Some(a), Some(b)) if b >= a => json!(b - a),
        _ => Value::Null,
    };
    // the RTC alarm should end the sleep at `seconds`: a wake long before it is something else's doing
    let woke_early = matches!(asleep.as_u64(), Some(a) if a + 1 < ask.seconds);
    let mut report = json!({
        "verb": "cycle",
        "boot": text(&root.join("proc/sys/kernel/random/boot_id")),
        "seconds": ask.seconds,
        "suspended": suspended,
        "woke_early": woke_early,
        "soc_slept": suspended && slept && !woke_early,
        "not_quieted": not_quieted,
        "stopped": d.stop,
        "not_started": not_started,
        "not_stopped": not_stopped,
        "asleep_s": asleep,
        "soc": soc,
        "idle": idle,
        "wakeups": wakeups.iter().map(|(n, dl)| json!({"name": n, "delta": dl})).collect::<Vec<_>>(),
        "quiet": d.quiet,
        "way_back": d.guard,
        "hooks": hooks_json(ask, &before_hook, &after_hook),
    });
    if let Some(c) = &traced {
        report["trace"] = json!({"dyndbg": d.trace.dyndbg, "irq": d.trace.irq, "irq_events": c.irq_events,
            "trace_file": c.trace_file, "dmesg_file": c.dmesg_file,
            "problems": c.problems});
    }
    if let Err(e) = wrote {
        report["state_write"] = json!(format!("{e}"));
    }
    let kept = std::fs::create_dir_all(&out)
        .and_then(|_| std::fs::write(out.join("cycle.json"), format!("{report}\n")));
    if let Err(e) = kept {
        report["kept"] = json!(format!("not kept: {e}"));
    }
    (
        if report["soc_slept"] == true && not_started.is_empty() {
            0
        } else {
            1
        },
        report,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stats_file_is_read_by_its_count_and_duration() {
        let t = "Count: 12\nLast Entered At: 5\nLast Exited At: 6\nAccumulated Duration: 777\n";
        assert_eq!(parse_stat(t), Some((12, 777)));
        assert_eq!(parse_stat("nothing"), None);
    }

    #[test]
    fn the_manifest_declares_the_way_back_and_the_quiet_sources() {
        let d = declared("SUSPEND_GUARD='way-back.sh check'\nSUSPEND_QUIET='/a/b /c/d'\n");
        assert_eq!(d.guard, "way-back.sh check");
        assert_eq!(d.quiet, vec!["/a/b", "/c/d"]);
        assert_eq!(declared(""), Declared::default());
    }
}
