// SPDX-License-Identifier: MIT OR Apache-2.0
//! lpoco-attended: a lane's attended procedure as a plain list of named steps. The entry file reads like a
//! shell script; the plumbing lives here: the arguments (`--dry-run`, `--log`, `--device`, `--until FIRST_PROMPT`, `--part`, `--help`), running each step,
//! stopping the run at the first that fails and naming it, the prompt-and-wait point, the log a lane files.
//! The phone is reached through devlab (`devlab --device FILE phone sh|root`, the script on its standard input).
//! `procedure(name, device)` finds that device's `device.toml` (`--device FILE`, else from the folder upwards), refuses a
//! manifest that is another phone's, and hands devlab that very file, so two phones on the bench are never mixed up.
//! A file to copy: `examples/attended.rs`.
//!
//! ```no_run
//! use lpoco_attended::prelude::*;
//! fn main() {
//!     let _run = procedure("boot the kernel", "xiaomi-surya");
//!     step("the phone answers", || phone().sh("true"));
//!     ask_author("plug the cable in / the logo shows");
//!     check("the driver is bound", || phone().sh("lsmod").map(|m| m.contains("pmicwdt")));
//! }
//! ```
//!
//! A [`Step`] is the one step type: a name, what kind it is (do, check, ask), the bring-up step id it is keyed
//! to where one exists, and what it runs. The attended runner here runs a chain of them with a person at hand;
//! the unattended step runner (bringup-step-runner) takes the same value, so an attended procedure and a nightly
//! health run are the same chain. A library step (like [`steps::ask`]) is a function that returns a `Step`.

pub mod bench;
mod cli;
mod device;
pub mod steps;
mod values;
pub use values::Val;

use argenv_resolve::{InputResolveExt, OnProblems};
use std::fmt::Display;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Everything an entry file needs, in one line: `use lpoco_attended::prelude::*;`.
pub mod prelude {
    pub use crate::bench::{bench, Act, ActKind, Bench};
    pub use crate::{
        ask_author, check, confirm, dev, hold_button, listen_for, look_for, part, phone,
        press_power, proc, procedure, replug_usb, retry, run_step, step, swipe, touch,
        unknown_part, unplug_usb, Outcome, Step, Val,
    };
}

/// What a step does with the world.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// changes something
    Do,
    /// only looks
    Check,
    /// waits for the author
    Ask,
}

impl Kind {
    fn word(self) -> &'static str {
        match self {
            Kind::Do => "step",
            Kind::Check => "check",
            Kind::Ask => "ask",
        }
    }
}

/// How a step's closure says it went: nothing means it went well, `false` that it did not hold, an `Err` why not.
/// A `Result` of any of these, and a `String` (a command's output), count by what is inside.
pub trait Outcome {
    fn outcome(self) -> Result<(), String>;
}

impl Outcome for () {
    fn outcome(self) -> Result<(), String> {
        Ok(())
    }
}

impl Outcome for String {
    fn outcome(self) -> Result<(), String> {
        Ok(())
    }
}

impl Outcome for bool {
    fn outcome(self) -> Result<(), String> {
        if self {
            Ok(())
        } else {
            Err("it did not hold".into())
        }
    }
}

impl<T: Outcome, E: Display> Outcome for Result<T, E> {
    fn outcome(self) -> Result<(), String> {
        self.map_err(|e| e.to_string())?.outcome()
    }
}

/// The one step type: serves this runner and the unattended one.
pub struct Step {
    pub name: String,
    pub kind: Kind,
    /// the bring-up step id (a step of the docs base) this step carries out, where one exists
    pub id: Option<String>,
    /// who does it where it is physical: a bench's name, `person` for the person at the bench
    pub doer: Option<String>,
    run: Box<dyn FnOnce() -> Result<(), String>>,
}

impl Step {
    pub fn new<R: Outcome>(kind: Kind, name: &str, run: impl FnOnce() -> R + 'static) -> Step {
        Step {
            name: name.to_string(),
            kind,
            id: None,
            doer: None,
            run: Box::new(move || run().outcome()),
        }
    }

    /// Key the step to the bring-up step it carries out.
    pub fn keyed(mut self, id: &str) -> Step {
        self.id = Some(id.to_string());
        self
    }

    /// Name who does the step.
    pub fn by(mut self, doer: &str) -> Step {
        self.doer = Some(doer.to_string());
        self
    }

    /// Whether the run waits for the author at this step: it asks, and a person is the one asked.
    fn prompts(&self) -> bool {
        self.kind == Kind::Ask && self.doer.as_deref().is_none_or(|d| d == bench::PERSON)
    }

    /// Run it: the part both runners share.
    pub fn execute(self) -> Result<(), String> {
        (self.run)()
    }
}

struct Config {
    dry: bool,
    until_first_prompt: bool,
    part: Option<String>,
    /// the device.toml of the procedure's device, the one devlab is handed
    device: std::path::PathBuf,
    /// what that file declares: steps read their keys from it
    manifest: lpoco_manifest::Manifest,
    /// the same file as the tables it says, for the keys a procedure reads by name
    raw: toml::Value,
    devlab: String,
    sysfs_usb: String,
    name: String,
}

struct State {
    log: Option<File>,
    n: u32,
    /// the step being run: what the phone says is filed under it
    step: String,
}

static CONFIG: OnceLock<Config> = OnceLock::new();
/// whether the entry file asked for its part: a `--part` it never looks at must not run the whole procedure
static PART_ASKED: AtomicBool = AtomicBool::new(false);
static STATE: Mutex<State> = Mutex::new(State {
    log: None,
    n: 0,
    step: String::new(),
});

fn config() -> &'static Config {
    CONFIG
        .get()
        .expect("lpoco-attended: call procedure(name, device) before the first step")
}

fn slug(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

/// The longest output filed for one call, as it stands on the log line (newlines are written as ` | `); a longer
/// one keeps its last bytes (the end of a dmesg is the recent part) and says how much was cut.
const OUT_MAX: usize = 4000;

/// File what the phone said to the step being run, under that step's name; nothing outside a step. The log is a
/// plain file: a step that makes the phone print a secret files it, so a procedure never prints one.
fn note_out(out: &str) {
    let step = STATE.lock().unwrap_or_else(|e| e.into_inner()).step.clone();
    let flat = out.trim().replace('\r', "").replace('\n', " | ");
    if flat.is_empty() || step.is_empty() {
        return;
    }
    let mut start = flat.len().saturating_sub(OUT_MAX);
    while !flat.is_char_boundary(start) {
        start += 1;
    }
    let cut = if start > 0 {
        format!(
            "[cut: the last {} of {} bytes] ",
            flat.len() - start,
            flat.len()
        )
    } else {
        String::new()
    };
    note("out", &step, &format!("{cut}{}", &flat[start..]));
}

pub(crate) fn note(what: &str, step: &str, detail: &str) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Some(f) = STATE.lock().unwrap_or_else(|e| e.into_inner()).log.as_mut() {
        let _ = writeln!(f, "{secs} {what} {step}: {}", detail.replace('\n', " | "));
    }
}

/// Ends the run when it goes out of scope at the end of main: the summary, the log's last line.
#[must_use = "bind it: let _run = procedure(..); dropped at once it ends the run"]
pub struct Run(());

impl Drop for Run {
    fn drop(&mut self) {
        let n = STATE.lock().unwrap_or_else(|e| e.into_inner()).n;
        let c = config();
        if std::thread::panicking() {
            note(
                "PANIC",
                "",
                "the procedure panicked; the run did not finish",
            );
            return;
        }
        note(
            "end",
            "",
            if c.dry {
                "all steps walked (dry run: nothing was done)"
            } else {
                "all steps passed"
            },
        );
        println!(
            "{}: all {n} steps passed{}",
            c.name,
            if c.dry {
                " (dry run: nothing was done)"
            } else {
                ""
            }
        );
    }
}

/// Begin the procedure: read the arguments (`--dry-run`, `--log FILE`, `--device FILE`, `--until FIRST_PROMPT`, `--part NAME`, `--help`), load the device's manifest, open the log. Call it once,
/// first, and keep what it returns until the end of main. Exits 0 for `--help`; 1 for a wrong argument (as
/// argenv does in every lpoco program); 2 when the device's device.toml is missing, invalid or another phone's (even in a dry run; only its schema and codename are checked, devlab judges the rest), when the log cannot be written, or when `--part` names a part the procedure has not got (a run stopped by `--until` exits 0 and leaves the log without an `end` line).
pub fn procedure(name: &str, device: &str) -> Run {
    let r = cli::Contract::parse_and_lint(OnProblems::FailOnError);
    if cli::Contract::HELP.get_from(&r).unwrap_or(false) {
        print!("{}", cli::usage(name));
        std::process::exit(0);
    }
    let dry = cli::Contract::DRY_RUN.get_from(&r).unwrap_or(false);
    let (device_file, manifest, raw) =
        device::load(cli::Contract::DEVICE.get_from(&r).as_deref(), device).unwrap_or_else(|e| {
            eprintln!("{name}: {e}");
            std::process::exit(2)
        });
    let path = cli::Contract::LOG
        .get_from(&r)
        .unwrap_or_else(|| format!("attended-{}.log", slug(name)));
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .unwrap_or_else(|e| {
            eprintln!("{name}: cannot write the log {path}: {e}");
            std::process::exit(2)
        });
    let until_first_prompt = cli::Contract::UNTIL.get_from(&r).as_deref() == Some("FIRST_PROMPT");
    let part = cli::Contract::PART.get_from(&r);
    let devlab = cli::Contract::DEVLAB
        .get_from(&r)
        .unwrap_or_else(|| "devlab".into());
    let sysfs_usb = cli::Contract::SYSFS_USB
        .get_from(&r)
        .unwrap_or_else(|| "/sys/bus/usb/devices".into());
    let c = Config {
        dry,
        until_first_prompt,
        part,
        device: device_file,
        manifest,
        raw,
        devlab,
        sysfs_usb,
        name: name.to_string(),
    };
    CONFIG.set(c).ok().expect("procedure() is called once");
    STATE.lock().unwrap_or_else(|e| e.into_inner()).log = Some(log);
    note(
        "start",
        "",
        &format!("{name} on {device}{}", if dry { " (dry run)" } else { "" }),
    );
    Run(())
}

/// Run one step as the attended runner does: say it, run it (a dry run only says what it would do), and on a
/// failure name it, log it and end the process with exit 1.
pub fn run_step(s: Step) {
    let c = config();
    if c.part.is_some() && !PART_ASKED.load(Ordering::Relaxed) {
        eprintln!(
            "{}: this procedure has no parts (--part {:?} was given); nothing was run",
            c.name,
            c.part.as_deref().unwrap_or_default()
        );
        std::process::exit(2);
    }
    let n = {
        let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
        st.n += 1;
        st.step = s.name.clone();
        st.n
    };
    let (kind, name) = (s.kind.word(), s.name.clone());
    if s.prompts() && c.until_first_prompt {
        note(
            "stop",
            &name,
            "--until FIRST_PROMPT: the author is left alone",
        );
        println!(
            "{}: stopped before the first prompt ({:?}) after {} step(s){}",
            c.name,
            name,
            n - 1,
            if c.dry {
                " (dry run: nothing was done)"
            } else {
                ""
            }
        );
        std::process::exit(0);
    }
    if c.dry {
        let by = s
            .doer
            .as_deref()
            .map_or(String::new(), |d| format!(" (by {d})"));
        println!("{kind} {n}: {name} - dry run, not run{by}");
        note(
            "dry",
            &name,
            &format!(
                "{kind}{}",
                s.id.as_deref()
                    .map(|i| format!(" (bring-up step {i})"))
                    .unwrap_or_default()
            ),
        );
        return;
    }
    // a person is the usual doer and not said; a bench is
    let by = s
        .doer
        .as_deref()
        .filter(|d| *d != bench::PERSON)
        .map_or(String::new(), |d| format!(" by {d}"));
    println!("{kind} {n}: {name}{by}");
    note("begin", &name, &format!("{kind}{by}"));
    let result = s.execute();
    STATE.lock().unwrap_or_else(|e| e.into_inner()).step.clear();
    match result {
        Ok(()) => note("ok", &name, ""),
        Err(why) => {
            note("FAIL", &name, &why);
            eprintln!("{}: stopped at {kind} {n} {name:?}: {why}", c.name);
            std::process::exit(1);
        }
    }
}

/// Do something; the run stops here when the closure fails.
pub fn step<R: Outcome>(name: &str, run: impl FnOnce() -> R + 'static) {
    run_step(Step::new(Kind::Do, name, run));
}

/// Look at something; the run stops here when it does not hold. Same as a step, kept apart so the file says
/// which steps only look.
pub fn check<R: Outcome>(name: &str, run: impl FnOnce() -> R + 'static) {
    run_step(Step::new(Kind::Check, name, run));
}

/// A point where the author does something and looks at something and says how it went ("what to do / what
/// to check"): `ok` goes on, anything else stops the run and is logged as what was seen.
pub fn ask_author(what: &str) {
    confirm(what);
}

/// The author's own words ("what to do / what to check"), done by the person at the bench; the same as [`ask_author`].
pub fn confirm(what: &str) {
    run_step(steps::confirm(what));
}

/// Press the power button, then check `check`.
pub fn press_power(check: &str) {
    run_step(steps::press_power(check));
}

/// Hold a button (`"volume down"`).
pub fn hold_button(button: &str, check: &str) {
    run_step(steps::hold_button(button, check));
}

/// Unplug the USB cable.
pub fn unplug_usb(check: &str) {
    run_step(steps::unplug_usb(check));
}

/// Plug the USB cable back in.
pub fn replug_usb(check: &str) {
    run_step(steps::replug_usb(check));
}

/// Touch something on the screen.
pub fn touch(target: &str, check: &str) {
    run_step(steps::touch(target, check));
}

/// Swipe (`"up from the bottom edge"`).
pub fn swipe(way: &str, check: &str) {
    run_step(steps::swipe(way, check));
}

/// Look at the phone for something.
pub fn look_for(what: &str) {
    run_step(steps::look_for(what));
}

/// Listen to the phone for something.
pub fn listen_for(what: &str) {
    run_step(steps::listen_for(what));
}

/// The part of the procedure asked for with `--part NAME`, none for the whole: the entry file matches on it, one
/// checklist per part, and sends a name it does not know to [`unknown_part`].
pub fn part() -> Option<String> {
    PART_ASKED.store(true, Ordering::Relaxed);
    config().part.clone()
}

/// End the run with exit 2 and the reason: a procedure that cannot know a value it needs must not start.
pub(crate) fn stop(why: &str) -> ! {
    eprintln!("{}: {why}", config().name);
    std::process::exit(2)
}

/// The value at a dotted key of the device's `device.toml` (`dev("health.battery_floor_mv")`): the table that owns
/// the fact. A key the file does not say ends the run with exit 2 naming it. Read it where the entry file builds its
/// steps, not inside a step's closure: a dry run does not run closures, so only a read made outside one is found by it.
pub fn dev(key: &str) -> Val {
    values::find(&config().raw, key).unwrap_or_else(|| stop(&format!("device.toml has no {key}")))
}

/// A value of this procedure's own `[procedures.NAME]` table (`proc("window")`); NAME is the procedure's name in
/// lowercase words joined by `-`. Only what no other table owns goes there. A missing key ends the run as `dev` does.
pub fn proc(key: &str) -> Val {
    let name = words(&config().name);
    if name.is_empty() {
        stop("the procedure's name has no letters or digits, so it has no [procedures.NAME] table")
    }
    let table = format!("procedures.{name}.{key}");
    values::find(&config().raw, &table)
        .unwrap_or_else(|| stop(&format!("device.toml has no {table}")))
}

/// Lowercase words joined by `-`: how a procedure's name is written as a table name.
fn words(name: &str) -> String {
    name.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>()
        .join("-")
}

/// A `--part` the procedure has not got: name it and the known ones, end the run with exit 2.
pub fn unknown_part(asked: &str, known: &[&str]) -> ! {
    eprintln!(
        "{}: no part {asked:?}; the parts are: {}",
        config().name,
        known.join(", ")
    );
    std::process::exit(2)
}

/// Try `run` up to `tries` times, `every` seconds apart, until it passes; the last failure is returned.
pub fn retry<R: Outcome>(tries: u32, every: u64, mut run: impl FnMut() -> R) -> Result<(), String> {
    let mut last = Err("no tries".to_string());
    for t in 0..tries.max(1) {
        if t > 0 {
            std::thread::sleep(Duration::from_secs(every));
        }
        last = run().outcome();
        if last.is_ok() {
            break;
        }
    }
    last
}

/// A key a step needs from the device's device.toml, or the run ends with exit 2 naming the key, the file and the step
/// (in a dry run too). A step reads it when it is built, so in a real run the steps before it have already run: put the
/// steps that read a key first, or dry-run the procedure before the real run.
pub(crate) fn declared<T>(
    key: &str,
    step: &str,
    find: impl FnOnce(&lpoco_manifest::Manifest) -> Option<T>,
) -> T {
    let c = config();
    find(&c.manifest).unwrap_or_else(|| {
        eprintln!(
            "{}: {} has no {key}, which {step} reads",
            c.name,
            c.device.display()
        );
        std::process::exit(2)
    })
}

pub(crate) fn sysfs_usb() -> &'static str {
    &config().sysfs_usb
}

/// Run devlab itself on this computer (`devlab ARGS`, e.g. `kernel try NAME`): its output when it exits 0, its
/// output named when it does not. A dry run reaches nothing.
pub(crate) fn devlab(args: &[&str]) -> Result<String, String> {
    if config().dry {
        return Err("dry run: devlab is not reached".into());
    }
    let o = Command::new(&config().devlab)
        .arg("--device")
        .arg(&config().device)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot start {}: {e}", config().devlab))?;
    let out = format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    if o.status.success() {
        note_out(&out);
        Ok(out)
    } else {
        Err(format!(
            "devlab {} exited {}: {}",
            args.join(" "),
            o.status
                .code()
                .map_or("by signal".into(), |c| c.to_string()),
            out.trim()
        ))
    }
}

/// The phone, reached through devlab.
pub struct Phone(());

pub fn phone() -> Phone {
    Phone(())
}

impl Phone {
    /// Run a script on the phone as the user; its output (standard output and error) when it exits 0.
    pub fn sh(&self, script: &str) -> Result<String, String> {
        self.via("sh", script)
    }

    /// Run a script on the phone as root.
    pub fn root(&self, script: &str) -> Result<String, String> {
        self.via("root", script)
    }

    fn via(&self, who: &str, script: &str) -> Result<String, String> {
        if config().dry {
            return Err("dry run: the phone is not reached".into());
        }
        let mut child = Command::new(&config().devlab)
            .arg("--device")
            .arg(&config().device)
            .args(["phone", who])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot start {}: {e}", config().devlab))?;
        let mut stdin = child.stdin.take().unwrap();
        let _ = stdin.write_all(script.as_bytes());
        drop(stdin);
        let o = child
            .wait_with_output()
            .map_err(|e| format!("cannot wait for devlab: {e}"))?;
        let out = format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        );
        if o.status.success() {
            note_out(&out);
            Ok(out)
        } else {
            Err(format!(
                "the phone's script exited {}: {}",
                o.status
                    .code()
                    .map_or("by signal".into(), |c| c.to_string()),
                out.trim()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_closure_is_tried_again_until_it_passes_and_fails_when_the_tries_run_out() {
        let mut n = 0;
        let r = retry(3, 0, || {
            n += 1;
            n >= 3
        });
        assert_eq!((r, n), (Ok(()), 3));
        let mut m = 0;
        let r = retry(2, 0, || {
            m += 1;
            false
        });
        assert_eq!((r, m), (Err("it did not hold".to_string()), 2));
    }

    #[test]
    fn what_a_closure_returns_counts_by_what_is_inside() {
        assert!(().outcome().is_ok());
        assert!(Ok::<_, String>(true).outcome().is_ok());
        assert!(Ok::<_, String>(false).outcome().is_err());
        assert_eq!(Err::<(), _>("no").outcome(), Err("no".to_string()));
    }
}
