//! lpoco-health: the phone's health, measured on the phone - after every boot (lpoco-health.timer)
//! and whenever the lab asks. Written onto the phone by devlab from the device's manifest; nothing
//! here is about one phone: what a phone is expected to have is in /etc/lpoco-health/health.env, and a
//! phone's own checks sit beside the generic ones.
//!
//! The checks are small shell files in postmarketOS's test shape: each reads what the kernel and the
//! system expose and prints a line per test - `pass|fail|skip TEST COMPONENT detail...`. Everything
//! that is strings and structure - the report, its JSON, the evidence, the verdict - is here.
//!
//! One report per run: /var/lib/lpoco-health/RUN.jsonl, a line per test carrying the run's facts
//! (device, boot, time, kernel, shell) - the shape of linux-phone-oco/docs' test_results. A run with a
//! failure keeps its evidence beside it (RUN.evidence/) and says plainly what is unhealthy.
//!
//!   lpoco-health           run every check now; exit 1 when unhealthy
//!   lpoco-health latest    print the latest report
//!   lpoco-health tone make|find ...   a test tone made, and found in a recording (src/tone.rs) -
//!                           what a check computes, it asks of this one binary, not of awk
//!   lpoco-health data plan|spent ...   the month's mobile data: what a check may spend, what it spent (src/data.rs)

mod awake;
mod cli;
mod data;
mod suspend;
mod switches;
mod tone;

use argenv_resolve::{InputResolveExt, OnProblems};
use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const LIB: &str = "/usr/lib/lpoco-health";
const OUT: &str = "/var/lib/lpoco-health";
const ENV: &str = "/etc/lpoco-health/health.env";
/// base: the hardware and the system; shell: through the running shell; telephony: what costs something
/// to run (a call, an SMS, mobile data) - each such check runs at most once a day (lib.sh once_a_day)
const LAYERS: [&str; 3] = ["base", "shell", "telephony"];
const CHECK_SECS: u64 = 120;
const KEEP_RUNS: usize = 60;

/// One test's finding, as a check said it.
#[derive(Debug, Clone, PartialEq)]
struct Finding {
    layer: String,
    component: String,
    test: String,
    outcome: String,
    detail: String,
}

/// A check's line: `OUTCOME TEST COMPONENT detail...`; any other line is the check talking.
fn parse_line(layer: &str, line: &str) -> Option<Finding> {
    let mut w = line.trim_end().splitn(4, ' ');
    let outcome = w.next()?;
    if !matches!(outcome, "pass" | "fail" | "skip") {
        return None;
    }
    let test = w.next().filter(|s| !s.is_empty())?;
    let component = w.next().filter(|s| !s.is_empty())?;
    Some(Finding {
        layer: layer.into(),
        component: component.into(),
        test: test.into(),
        outcome: outcome.into(),
        detail: w.next().unwrap_or("").trim().into(),
    })
}

/// What a check that could not be read says: never a silent gap.
fn findings_of(
    layer: &str,
    name: &str,
    out: &str,
    timed_out: bool,
    code: Option<i32>,
) -> Vec<Finding> {
    let mut f: Vec<Finding> = out.lines().filter_map(|l| parse_line(layer, l)).collect();
    let said = |detail: String| Finding {
        layer: layer.into(),
        component: "kernel".into(),
        test: name.into(),
        outcome: "fail".into(),
        detail,
    };
    if timed_out {
        f.push(said(format!("the check took over {CHECK_SECS} s")));
    } else if f.is_empty() {
        f.push(said(format!(
            "the check said nothing (exit {})",
            code.map_or("by signal".into(), |c| c.to_string())
        )));
    }
    f
}

/// KEY=VALUE lines, as a shell would source them (quotes taken off).
fn read_env(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.starts_with('#') {
                return None;
            }
            let (k, v) = l.split_once('=')?;
            Some((
                k.trim().into(),
                v.trim().trim_matches(|c| c == '"' || c == '\'').into(),
            ))
        })
        .collect()
}

/// Days since 1970-01-01 to (year, month, day) - Howard Hinnant's civil_from_days.
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// The run's name and moment: (YYYYMMDD-HHMMSS, YYYY-MM-DDTHH:MM:SSZ), in UTC.
fn stamps(secs: u64) -> (String, String) {
    let (y, mo, d) = civil((secs / 86_400) as i64);
    let s = secs % 86_400;
    let (h, mi, se) = (s / 3600, s % 3600 / 60, s % 60);
    (
        format!("{y:04}{mo:02}{d:02}-{h:02}{mi:02}{se:02}"),
        format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{se:02}Z"),
    )
}

fn sh(cmd: &str) -> String {
    Command::new("sh")
        .args(["-c", cmd])
        .stderr(Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn read(path: &str) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// The graphical shell, read off what runs - the layer every shell check is about.
fn shell_running() -> String {
    let session = sh("basename \"$(readlink /var/lib/tinydm/default-session.desktop)\" .desktop");
    if !session.is_empty() && session != "." {
        return session;
    }
    for (unit, name) in [
        ("plasma-mobile", "plasma-mobile"),
        ("gdm", "gnome-mobile"),
        ("phosh", "phosh"),
    ] {
        if Command::new("systemctl")
            .args(["is-active", "-q", unit])
            .status()
            .is_ok_and(|s| s.success())
        {
            return name.into();
        }
    }
    "none".into()
}

/// Runs one check with a time limit; (output, timed out, exit code).
fn run_check(path: &Path, layer: &str, conditions: &str) -> (String, bool, Option<i32>) {
    let mut child = match Command::new("sh")
        .arg(path)
        .env("LAYER", layer)
        .env("LPOCO_CONDITIONS", conditions)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return (String::new(), false, None), // said as: the check said nothing
    };
    let mut out = child.stdout.take();
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(o) = out.as_mut() {
            let _ = o.read_to_string(&mut s);
        }
        s
    });
    let start = Instant::now();
    let (timed_out, code) = loop {
        match child.try_wait() {
            Ok(Some(st)) => break (false, st.code()),
            Ok(None) if start.elapsed() > Duration::from_secs(CHECK_SECS) => {
                let _ = child.kill();
                let _ = child.wait();
                break (true, None);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(_) => break (false, None),
        }
    };
    (reader.join().unwrap_or_default(), timed_out, code)
}

fn checks_in(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|d| {
            d.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "sh"))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// The verdict in plain words, and whether all is well. A failure in a known gap - a part this phone
/// has never had working, named in its manifest - stays in the report as measured, but is not ill
/// health: only a gap that starts to pass is news, and it is said.
fn verdict(findings: &[Finding], gaps: &[String], evidence: &str) -> (String, bool) {
    let n = |o: &str| findings.iter().filter(|f| f.outcome == o).count();
    let in_gap = |f: &&Finding| gaps.contains(&f.component);
    let failing: Vec<String> = findings
        .iter()
        .filter(|f| f.outcome == "fail" && !in_gap(f))
        .map(|f| format!("{}/{} ({})", f.component, f.test, f.detail))
        .collect();
    let gap_fails = findings
        .iter()
        .filter(|f| f.outcome == "fail")
        .filter(in_gap)
        .count();
    let closing: Vec<String> = findings
        .iter()
        .filter(|f| f.outcome == "pass")
        .filter(in_gap)
        .map(|f| format!("{}/{}", f.component, f.test))
        .collect();
    let news = if closing.is_empty() {
        String::new()
    } else {
        format!("; a known gap now passes: {}", closing.join(", "))
    };
    if failing.is_empty() {
        (
            format!(
                "healthy: {} pass, {} in known gaps, {} not measured, 0 fail{news}",
                n("pass"),
                gap_fails,
                n("skip")
            ),
            true,
        )
    } else {
        (
            format!(
                "UNHEALTHY: {} failing of {} - {}; evidence in {evidence}{news}",
                failing.len(),
                findings.len(),
                failing.join("; ")
            ),
            false,
        )
    }
}

fn row(f: &Finding, facts: &Value) -> Value {
    let mut r = facts.clone();
    let o = r.as_object_mut().expect("facts are an object");
    o.insert("layer".into(), json!(f.layer));
    o.insert("component".into(), json!(f.component));
    o.insert("test".into(), json!(f.test));
    o.insert("outcome".into(), json!(f.outcome));
    o.insert("detail".into(), json!(f.detail));
    r
}

fn keep_evidence(dir: &Path) {
    let _ = std::fs::create_dir_all(dir);
    let d = dir.display();
    sh(&format!(
        "journalctl -k -b 0 --no-pager -o short-monotonic > '{d}/kernel.log' 2>&1 || dmesg > '{d}/kernel.log'; \
         journalctl -b 0 -p err --no-pager > '{d}/errors.log' 2>&1; \
         systemctl --failed --no-pager > '{d}/failed-units.txt' 2>&1"
    ));
}

fn prune(out: &Path) {
    let mut runs: Vec<PathBuf> = std::fs::read_dir(out)
        .map(|d| {
            d.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.extension().is_some_and(|x| x == "jsonl")
                        && p.file_name().is_some_and(|n| n != "latest.jsonl")
                })
                .collect()
        })
        .unwrap_or_default();
    runs.sort();
    runs.reverse();
    for old in runs.iter().skip(KEEP_RUNS) {
        let _ = std::fs::remove_file(old);
        let _ = std::fs::remove_dir_all(old.with_extension("evidence"));
    }
}

/// The conditions this phone finds out about itself right now: every conditions/*.sh prints
/// `NAME detail` lines for what holds (no-sim, no-modem, no-network). Never kept: each run finds out again.
fn conditions() -> String {
    let mut found = String::new();
    for script in checks_in(&Path::new(LIB).join("conditions")) {
        let (text, _, _) = run_check(&script, "conditions", "");
        for line in text.lines() {
            let mut w = line.splitn(2, ' ');
            if let (Some(name), Some(detail)) = (w.next(), w.next()) {
                if name.starts_with("no-") && !detail.trim().is_empty() {
                    found.push_str(&format!("{name} {}\n", detail.trim()));
                }
            }
        }
    }
    found
}

/// A check file's test name: its file name without the order number - 10-mobile-data.sh is mobile-data.
fn check_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    match stem.split_once('-') {
        Some((n, rest)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => rest.into(),
        _ => stem,
    }
}

/// Every test the phone knows: the ones its latest report names, and the check files themselves.
fn known_tests(out: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_to_string(out.join("latest.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|r| r.get("test").and_then(Value::as_str).map(str::to_string))
        .collect();
    for layer in LAYERS {
        for dir in ["checks", "device"] {
            v.extend(
                checks_in(&Path::new(LIB).join(dir).join(layer))
                    .iter()
                    .map(|p| check_name(p)),
            );
        }
    }
    v.sort();
    v.dedup();
    v
}

/// A finding of a test somebody switched off becomes a skip that says so - not measured, never broken.
fn apply_switches(findings: Vec<Finding>, switches: &[switches::Switch]) -> Vec<Finding> {
    findings
        .into_iter()
        .map(|mut f| {
            if let Some(w) = switches::find(switches, &f.test) {
                f.outcome = "skip".into();
                f.detail = switches::said(w);
            }
            f
        })
        .collect()
}

fn who() -> String {
    std::env::var("SUDO_USER")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "root".into())
}

fn now_utc() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    stamps(secs).1
}

/// `lpoco-health list`: the tests with their state, the switches, the conditions.
fn list(out: &Path) {
    let sw = switches::load(Path::new(switches::FILE));
    println!("tests:");
    for t in known_tests(out) {
        match switches::find(&sw, &t) {
            Some(w) => println!("  {t}  OFF - {}", switches::said(w)),
            None => println!("  {t}  on"),
        }
    }
    for w in sw.iter().filter(|w| !known_tests(out).contains(&w.test)) {
        println!(
            "  {}  OFF - {} (no run has named this test)",
            w.test,
            switches::said(w)
        );
    }
    println!("conditions found now:");
    let c = conditions();
    if c.is_empty() {
        println!("  none");
    }
    for l in c.lines() {
        println!("  {l}");
    }
}

fn main() {
    let out = Path::new(OUT);
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("tone") {
        std::process::exit(tone::main(&args[1..]));
    }
    if args.first().map(String::as_str) == Some("data") {
        std::process::exit(data::main(&args[1..]));
    }
    let r = cli::Contract::parse_and_lint(OnProblems::FailOnError);
    if cli::Contract::HELP.get_from(&r).unwrap_or(false) {
        print!("{}", cli::usage("lpoco-health", "The phone's health, measured on the phone. Commands: (none) run every check, latest, tone make|find, data plan|spent, suspend plan|cycle, awake."));
        return;
    }
    if cli::Contract::VERSION.get_from(&r).unwrap_or(false) {
        println!("lpoco-health {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    match cli::Contract::COMMAND.get_from(&r) {
        Some(cli::Command::Latest) => {
            print!(
                "{}",
                std::fs::read_to_string(out.join("latest.jsonl")).unwrap_or_default()
            );
            return;
        }
        Some(cli::Command::List) => return list(out),
        Some(cli::Command::Conditions) => return print!("{}", conditions()),
        Some(cmd @ (cli::Command::Off | cli::Command::On)) => {
            let Some(test) = cli::Contract::TEST.get_from(&r) else {
                eprintln!("lpoco-health: which test? (lpoco-health list says them)");
                std::process::exit(2);
            };
            let path = Path::new(switches::FILE);
            let mut sw = switches::load(path);
            let done = if cmd == cli::Command::Off {
                if !cli::Contract::FORCE.get_from(&r).unwrap_or(false)
                    && !known_tests(out).contains(&test)
                {
                    eprintln!("lpoco-health: no test is named {test} (lpoco-health list says them; --force switches it off anyway)");
                    std::process::exit(2);
                }
                switches::off(
                    &mut sw,
                    &test,
                    &cli::Contract::REASON.get_from(&r).unwrap_or_default(),
                    &cli::Contract::BY.get_from(&r).unwrap_or_else(who),
                    &now_utc(),
                )
                .map(|_| format!("{test}: off"))
            } else if switches::on(&mut sw, &test) {
                Ok(format!("{test}: on"))
            } else {
                Err(format!("{test} was not switched off"))
            };
            match done.and_then(|m| {
                switches::save(path, &sw)
                    .map(|_| m)
                    .map_err(|e| format!("cannot keep the switches in {}: {e}", switches::FILE))
            }) {
                Ok(m) => println!("{m}"),
                Err(e) => {
                    eprintln!("lpoco-health: {e}");
                    std::process::exit(2);
                }
            }
            return;
        }
        Some(cli::Command::Awake) => {
            let root: std::path::PathBuf =
                cli::Contract::ROOT.get_from(&r).unwrap_or_default().into();
            std::process::exit(awake::run(
                &root,
                cli::Contract::ONCE.get_from(&r).unwrap_or(false),
            ));
        }
        Some(cli::Command::Suspend) => {
            let ask = suspend::Ask {
                root: cli::Contract::ROOT.get_from(&r).unwrap_or_default().into(),
                seconds: cli::Contract::SECONDS.get_from(&r).unwrap_or(15),
                settle: cli::Contract::SETTLE.get_from(&r).unwrap_or(2),
            };
            let (code, report) = match cli::Contract::VERB.get_from(&r).as_deref() {
                Some("plan") => suspend::plan(&ask),
                Some("cycle") => suspend::cycle(&ask),
                _ => {
                    eprintln!("lpoco-health: suspend what? plan or cycle");
                    std::process::exit(2);
                }
            };
            println!("{report}");
            if let Some(why) = report["refused"].as_array() {
                for w in why {
                    eprintln!("lpoco-health: refused: {}", w.as_str().unwrap_or_default());
                }
            }
            std::process::exit(code);
        }
        Some(cli::Command::Run) | None => {}
    }
    let env = read_env(&std::fs::read_to_string(ENV).unwrap_or_default());
    let device = env
        .iter()
        .find(|(k, _)| k == "DEVICE")
        .map_or("unknown".to_string(), |(_, v)| v.clone());
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (run, at) = stamps(secs);
    let facts = json!({
        "run": run,
        "device": device,
        "boot": read("/proc/sys/kernel/random/boot_id"),
        "at": at,
        "kernel": read("/proc/sys/kernel/osrelease"),
        "shell": shell_running(),
    });

    let found = conditions();
    let mut findings = Vec::new();
    for layer in LAYERS {
        for dir in ["checks", "device"] {
            for check in checks_in(&Path::new(LIB).join(dir).join(layer)) {
                let name = check
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let (text, timed_out, code) = run_check(&check, layer, &found);
                findings.extend(findings_of(layer, &name, &text, timed_out, code));
            }
        }
    }

    if let Err(e) = std::fs::create_dir_all(out) {
        eprintln!("lpoco-health: cannot keep reports in {OUT}: {e}");
        std::process::exit(2);
    }
    let findings = apply_switches(findings, &switches::load(Path::new(switches::FILE)));
    let report: String = findings
        .iter()
        .map(|f| format!("{}\n", row(f, &facts)))
        .collect();
    let file = out.join(format!("{run}.jsonl"));
    if let Err(e) = std::fs::write(&file, report) {
        eprintln!("lpoco-health: cannot write {}: {e}", file.display());
        std::process::exit(2);
    }
    let latest = out.join("latest.jsonl");
    let _ = std::fs::remove_file(&latest);
    let _ = std::os::unix::fs::symlink(format!("{run}.jsonl"), &latest);

    let evidence = out.join(format!("{run}.evidence"));
    let gaps: Vec<String> = env
        .iter()
        .find(|(k, _)| k == "GAPS")
        .map(|(_, v)| v.split_whitespace().map(str::to_string).collect())
        .unwrap_or_default();
    let (said, well) = verdict(&findings, &gaps, &evidence.display().to_string());
    if !well {
        keep_evidence(&evidence);
    }
    let said = if found.is_empty() {
        said
    } else {
        format!(
            "{said} [conditions: {}]",
            found
                .lines()
                .map(|l| l.split(' ').next().unwrap_or(""))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    eprintln!("lpoco-health {run}: {said}");
    let _ = Command::new("logger")
        .args(["-t", "lpoco-health", &format!("{run}: {said}")])
        .status();
    prune(out);
    std::process::exit(if well { 0 } else { 1 });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_check_line_is_outcome_test_component_and_the_rest_is_detail() {
        assert_eq!(
            parse_line("base", "pass sound-card audio-speaker card \"X3\" present"),
            Some(Finding {
                layer: "base".into(),
                component: "audio-speaker".into(),
                test: "sound-card".into(),
                outcome: "pass".into(),
                detail: "card \"X3\" present".into(),
            })
        );
        assert_eq!(parse_line("base", "skip a b").unwrap().detail, "");
        assert_eq!(
            parse_line("base", "checking the modem..."),
            None,
            "talk is not a finding"
        );
        assert_eq!(parse_line("base", "pass only-a-test"), None);
    }

    #[test]
    fn a_check_that_says_nothing_or_hangs_is_a_finding_not_a_gap() {
        let f = findings_of("base", "40-sound", "talk only\n", false, Some(2));
        assert_eq!(f.len(), 1);
        assert_eq!(
            (f[0].outcome.as_str(), f[0].test.as_str()),
            ("fail", "40-sound")
        );
        assert!(f[0].detail.contains("exit 2"));
        let h = findings_of("base", "90-suspend", "pass x kernel ok\n", true, None);
        assert_eq!(
            h.len(),
            2,
            "what it said before hanging is kept, and the hang is said"
        );
    }

    #[test]
    fn the_env_file_reads_as_a_shell_would_source_it() {
        let e = read_env("# a comment\nDEVICE=xiaomi-surya\nSOUND_CARDS=\"POCO X3\"\n\n");
        assert_eq!(
            e,
            vec![
                ("DEVICE".to_string(), "xiaomi-surya".to_string()),
                ("SOUND_CARDS".to_string(), "POCO X3".to_string())
            ]
        );
    }

    #[test]
    fn stamps_are_utc_and_sort_as_time_does() {
        assert_eq!(
            stamps(0),
            ("19700101-000000".into(), "1970-01-01T00:00:00Z".into())
        );
        // 2026-11-16T23:09:04Z
        assert_eq!(stamps(1_794_870_544).1, "2026-11-16T23:09:04Z");
        assert_eq!(stamps(951_782_400).1, "2000-02-29T00:00:00Z", "a leap day");
    }

    #[test]
    fn the_verdict_names_every_failure_plainly() {
        let f = |o: &str, t: &str| Finding {
            layer: "base".into(),
            component: "wifi".into(),
            test: t.into(),
            outcome: o.into(),
            detail: "d".into(),
        };
        let (s, well) = verdict(&[f("pass", "a"), f("skip", "b")], &[], "E");
        assert!(well);
        assert_eq!(
            s,
            "healthy: 1 pass, 0 in known gaps, 1 not measured, 0 fail"
        );
        let gap = ["wifi".to_string()];
        let (s, well) = verdict(&[f("fail", "c")], &gap, "E");
        assert!(well, "a known gap failing is not ill health");
        assert_eq!(
            s,
            "healthy: 0 pass, 1 in known gaps, 0 not measured, 0 fail"
        );
        let (s, _) = verdict(&[f("pass", "c")], &gap, "E");
        assert!(s.ends_with("; a known gap now passes: wifi/c"), "{s}");
        let (s, well) = verdict(&[f("pass", "a"), f("fail", "c")], &[], "E");
        assert!(!well);
        assert_eq!(s, "UNHEALTHY: 1 failing of 2 - wifi/c (d); evidence in E");
    }

    #[test]
    fn every_row_carries_its_run() {
        let facts = json!({"run": "r", "device": "d"});
        let f = parse_line(
            "shell",
            "fail default-output audio-speaker the default output is dummy",
        )
        .unwrap();
        let r = row(&f, &facts);
        assert_eq!(r["run"], "r");
        assert_eq!(r["layer"], "shell");
        assert_eq!(r["detail"], "the default output is dummy");
    }
}
