// SPDX-License-Identifier: MIT OR Apache-2.0
//! lpoco-attended: a lane's attended procedure as a plain list of named steps. The entry file reads like a
//! shell script; the plumbing lives here: the arguments (`--dry-run`, `--log`, `--help`), running each step,
//! stopping the run at the first that fails and naming it, the prompt-and-wait point, the log a lane files.
//! The phone is reached through devlab (`devlab phone sh|root`, the script on its standard input).
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

mod cli;
pub mod steps;

use argenv_resolve::{InputResolveExt, OnProblems};
use std::fmt::Display;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Everything an entry file needs, in one line: `use lpoco_attended::prelude::*;`.
pub mod prelude {
    pub use crate::{ask_author, check, phone, procedure, retry, run_step, step, Outcome, Step};
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
    run: Box<dyn FnOnce() -> Result<(), String>>,
}

impl Step {
    pub fn new<R: Outcome>(kind: Kind, name: &str, run: impl FnOnce() -> R + 'static) -> Step {
        Step {
            name: name.to_string(),
            kind,
            id: None,
            run: Box::new(move || run().outcome()),
        }
    }

    /// Key the step to the bring-up step it carries out.
    pub fn keyed(mut self, id: &str) -> Step {
        self.id = Some(id.to_string());
        self
    }

    /// Run it: the part both runners share.
    pub fn execute(self) -> Result<(), String> {
        (self.run)()
    }
}

struct Config {
    dry: bool,
    devlab: String,
    sysfs_usb: String,
    name: String,
}

struct State {
    log: Option<File>,
    n: u32,
}

static CONFIG: OnceLock<Config> = OnceLock::new();
static STATE: Mutex<State> = Mutex::new(State { log: None, n: 0 });

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

/// Begin the procedure: read the arguments (`--dry-run`, `--log FILE`, `--help`), open the log. Call it once,
/// first, and keep what it returns until the end of main. Exits 0 for `--help`; 1 for a wrong argument (as
/// argenv does in every lpoco program); 2 when the log cannot be written.
pub fn procedure(name: &str, device: &str) -> Run {
    let r = cli::Contract::parse_and_lint(OnProblems::FailOnError);
    if cli::Contract::HELP.get_from(&r).unwrap_or(false) {
        print!("{}", cli::usage(name));
        std::process::exit(0);
    }
    let dry = cli::Contract::DRY_RUN.get_from(&r).unwrap_or(false);
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
    let devlab = cli::Contract::DEVLAB
        .get_from(&r)
        .unwrap_or_else(|| "devlab".into());
    let sysfs_usb = cli::Contract::SYSFS_USB
        .get_from(&r)
        .unwrap_or_else(|| "/sys/bus/usb/devices".into());
    let c = Config {
        dry,
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
    let n = {
        let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
        st.n += 1;
        st.n
    };
    let (kind, name) = (s.kind.word(), s.name.clone());
    if c.dry {
        println!("{kind} {n}: {name} - dry run, not run");
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
    println!("{kind} {n}: {name}");
    note("begin", &name, kind);
    match s.execute() {
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
    run_step(steps::ask(what));
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
