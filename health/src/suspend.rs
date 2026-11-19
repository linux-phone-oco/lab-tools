// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco-health suspend plan|cycle`: one controlled suspend and wake, counted.
//!
//! What surya's keeper did by hand (surya-lab sleep/way-back.sh cycle) as a verb of the suite. The device's
//! manifest ([health]) declares the way back - `suspend_guard`, a command that succeeds while the phone's
//! watchdog is armed, so a suspend that does not return resets the phone - and the wake sources to quiet for
//! the cycle (`suspend_quiet`, wakeup files). Without a way back the cycle is refused and nothing suspends.
//! The cycle reads, before and after: the SoC's own sleep counters (qcom_stats), the suspend counter, the cpu
//! idle states (cluster states among them) and the wakeup sources; it puts every quieted source back; it keeps
//! the report as this boot's cycle, which the check soc-sleeps reads.
//!
//! Every path lies under LPOCO_HEALTH_ROOT (empty on the phone), so the tests run it on a made-up phone.
//! `plan` changes nothing: it says what a cycle would do, or why it would refuse.

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
}

/// What the manifest declares for the cycle.
#[derive(Debug, Default, PartialEq)]
struct Declared {
    guard: String,
    quiet: Vec<String>,
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
fn refusals(root: &Path, d: &Declared, seconds: u64) -> Vec<String> {
    let mut r = Vec::new();
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

/// What a cycle would do, or why it would refuse; changes nothing.
pub fn plan(ask: &Ask) -> (i32, Value) {
    let d = load_declared(ask);
    let why = refusals(&ask.root, &d, ask.seconds);
    let c = read_counters(&ask.root);
    (
        0,
        json!({"verb": "plan", "would_cycle": why.is_empty(), "refusals": why,
               "seconds": ask.seconds, "quiet": d.quiet, "way_back": d.guard, "before": snapshot(&c)}),
    )
}

/// The cycle: exit 0 when the SoC slept, 1 when it did not (said, not hidden), 2 when refused.
pub fn cycle(ask: &Ask) -> (i32, Value) {
    let d = load_declared(ask);
    let why = refusals(&ask.root, &d, ask.seconds);
    if !why.is_empty() {
        return (2, json!({"verb": "cycle", "refused": why}));
    }
    let root = &ask.root;
    let before = read_counters(root);
    let alarm = root.join("sys/class/rtc/rtc0/wakealarm");
    let target = before.rtc.unwrap_or(0) + ask.seconds;
    let quieted = Quieted::quiet(root, &d.quiet);
    let not_quieted = quieted.not_quieted.clone();
    let _ = std::fs::write(&alarm, "0");
    let armed = std::fs::write(&alarm, target.to_string()).is_ok();
    if !armed || text(&alarm) != target.to_string() {
        let _ = std::fs::write(&alarm, "0");
        return (
            2,
            json!({"verb": "cycle", "refused": ["the RTC wake alarm did not take"]}),
        );
    }
    let wrote = std::fs::write(root.join("sys/power/state"), "freeze");
    std::thread::sleep(Duration::from_secs(ask.settle));
    let after = read_counters(root);
    let _ = std::fs::write(&alarm, "0");
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
        "asleep_s": asleep,
        "soc": soc,
        "idle": idle,
        "wakeups": wakeups.iter().map(|(n, dl)| json!({"name": n, "delta": dl})).collect::<Vec<_>>(),
        "quiet": d.quiet,
        "way_back": d.guard,
    });
    if let Err(e) = wrote {
        report["state_write"] = json!(format!("{e}"));
    }
    let out = root.join(OUT);
    let kept = std::fs::create_dir_all(&out)
        .and_then(|_| std::fs::write(out.join("cycle.json"), format!("{report}\n")));
    if let Err(e) = kept {
        report["kept"] = json!(format!("not kept: {e}"));
    }
    (if report["soc_slept"] == true { 0 } else { 1 }, report)
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
