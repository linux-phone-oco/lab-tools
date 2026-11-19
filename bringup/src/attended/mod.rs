// SPDX-License-Identifier: MIT OR Apache-2.0
//! lpoco-attended: a lane's attended procedure as a plain list of named steps. The entry file reads like a
//! shell script; the plumbing lives here: the arguments (`--dry-run`, `--log`, `--help`), running each step,
//! stopping the run at the first that fails and naming it, the prompt-and-wait point, the log a lane files.
//! The phone is reached through devlab (`devlab phone sh|root`, the script on its standard input).
//! An example to copy: `examples/attended.rs`.
//!
//! ```no_run
//! use lpoco_bringup::attended::*;
//! fn main() -> std::process::ExitCode {
//!     procedure("boot the kernel").run(|p| {
//!         p.step("the phone answers", phone("true").retry(3, 1))?;
//!         p.ask_author("plug the cable in", "the logo shows")?;
//!         p.check("the driver is bound", phone("lsmod").expect("pmicwdt"))?;
//!         Ok(())
//!     })
//! }
//! ```

mod cli;

use argenv_resolve::{InputResolveExt, OnProblems};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, Write};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Where an action runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Where {
    Host,
    Phone,
    PhoneRoot,
}

/// What a step does: a script, where it runs, what its output must hold, how often it is tried.
#[derive(Clone, Debug)]
pub struct Action {
    at: Where,
    script: String,
    expects: Vec<String>,
    tries: u32,
    every: u64,
}

fn action(at: Where, script: &str) -> Action {
    Action {
        at,
        script: script.to_string(),
        expects: vec![],
        tries: 1,
        every: 1,
    }
}

/// A script run on this machine (`sh -c`).
pub fn sh(script: &str) -> Action {
    action(Where::Host, script)
}

/// A script run on the phone as the user (devlab phone sh).
pub fn phone(script: &str) -> Action {
    action(Where::Phone, script)
}

/// A script run on the phone as root (devlab phone root).
pub fn phone_root(script: &str) -> Action {
    action(Where::PhoneRoot, script)
}

impl Action {
    /// The output (standard output and error) must contain `text`, or the step fails.
    pub fn expect(mut self, text: &str) -> Action {
        self.expects.push(text.to_string());
        self
    }

    /// Try up to `tries` times, `every` seconds apart, before the step fails.
    pub fn retry(mut self, tries: u32, every: u64) -> Action {
        self.tries = tries.max(1);
        self.every = every;
        self
    }

    fn says(&self) -> String {
        let at = match self.at {
            Where::Host => "host",
            Where::Phone => "phone",
            Where::PhoneRoot => "phone as root",
        };
        let mut s = format!("{at}: {}", self.script.trim());
        for e in &self.expects {
            s.push_str(&format!(" (expects {e:?})"));
        }
        if self.tries > 1 {
            s.push_str(&format!(
                " (up to {} tries, {} s apart)",
                self.tries, self.every
            ));
        }
        s
    }
}

/// Why the run stopped; the entry file passes it up with `?`.
#[derive(Debug)]
pub struct Stop(String);

impl std::fmt::Display for Stop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A procedure to run: give it a name, then `run` it with the list of steps.
pub struct Procedure {
    name: String,
}

pub fn procedure(name: &str) -> Procedure {
    Procedure {
        name: name.to_string(),
    }
}

/// The run in progress, which the entry file's steps are called on.
pub struct Run {
    dry: bool,
    log: Option<File>,
    devlab: String,
    n: u32,
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

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl Procedure {
    /// Read the arguments, run the steps, and exit 0 when all passed, 1 when a step stopped the run (named on
    /// standard error and in the log), 2 when the log cannot be written (wrong arguments exit 1 with the usage, as argenv does in every lpoco program). The log is appended to, so a run after a failure keeps its record.
    pub fn run(self, steps: impl FnOnce(&mut Run) -> Result<(), Stop>) -> ExitCode {
        let r = cli::Contract::parse_and_lint(OnProblems::FailOnError);
        if cli::Contract::HELP.get_from(&r).unwrap_or(false) {
            print!("{}", cli::usage(&self.name));
            return ExitCode::SUCCESS;
        }
        let dry = cli::Contract::DRY_RUN.get_from(&r).unwrap_or(false);
        let path = cli::Contract::LOG
            .get_from(&r)
            .unwrap_or_else(|| format!("attended-{}.log", slug(&self.name)));
        let log = match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(f) => Some(f),
            Err(e) => {
                eprintln!("{}: cannot write the log {path}: {e}", self.name);
                return ExitCode::from(2);
            }
        };
        let devlab = cli::Contract::DEVLAB
            .get_from(&r)
            .unwrap_or_else(|| "devlab".into());
        let mut run = Run {
            dry,
            log,
            devlab,
            n: 0,
        };
        run.note(
            "start",
            "",
            &format!("{}{}", self.name, if dry { " (dry run)" } else { "" }),
        );
        match steps(&mut run) {
            Ok(()) => {
                run.note("end", "", "all steps passed");
                println!(
                    "{}: all {} steps passed{}",
                    self.name,
                    run.n,
                    if dry {
                        " (dry run: nothing was done)"
                    } else {
                        ""
                    }
                );
                ExitCode::SUCCESS
            }
            Err(Stop(why)) => {
                run.note("end", "", &format!("stopped: {why}"));
                eprintln!("{}: stopped at {why}", self.name);
                ExitCode::from(1)
            }
        }
    }
}

impl Run {
    fn note(&mut self, what: &str, step: &str, detail: &str) {
        if let Some(f) = self.log.as_mut() {
            let detail = detail.replace('\n', " | ");
            let _ = writeln!(f, "{} {what} {step}: {detail}", now());
        }
    }

    /// Do something; the run stops here when it fails.
    pub fn step(&mut self, name: &str, what: Action) -> Result<(), Stop> {
        self.act("step", name, what)
    }

    /// Look at something; the same as a step, kept apart so the entry file says which steps only look.
    pub fn check(&mut self, name: &str, what: Action) -> Result<(), Stop> {
        self.act("check", name, what)
    }

    fn act(&mut self, kind: &str, name: &str, what: Action) -> Result<(), Stop> {
        self.n += 1;
        if self.dry {
            println!("{kind} {}: {name} - would run {}", self.n, what.says());
            self.note("dry", name, &what.says());
            return Ok(());
        }
        println!("{kind} {}: {name}", self.n);
        let mut last = String::new();
        for t in 1..=what.tries {
            if t > 1 {
                std::thread::sleep(Duration::from_secs(what.every));
            }
            match self.once(&what) {
                Ok(out) => {
                    self.note("ok", name, out.trim());
                    return Ok(());
                }
                Err(why) => last = why,
            }
        }
        self.note("FAIL", name, &last);
        Err(Stop(format!("{kind} {} {name:?}: {last}", self.n)))
    }

    fn once(&self, what: &Action) -> Result<String, String> {
        let mut cmd = match what.at {
            Where::Host => {
                let mut c = Command::new("sh");
                c.args(["-c", &what.script]);
                c
            }
            Where::Phone | Where::PhoneRoot => {
                let mut c = Command::new(&self.devlab);
                c.args([
                    "phone",
                    if what.at == Where::Phone {
                        "sh"
                    } else {
                        "root"
                    },
                ]);
                c
            }
        };
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let on_phone = what.at != Where::Host;
        cmd.stdin(if on_phone {
            Stdio::piped()
        } else {
            Stdio::null()
        });
        let mut child = cmd.spawn().map_err(|e| format!("cannot start it: {e}"))?;
        if on_phone {
            let mut stdin = child.stdin.take().unwrap();
            let _ = stdin.write_all(what.script.as_bytes());
        }
        let o = child
            .wait_with_output()
            .map_err(|e| format!("cannot wait for it: {e}"))?;
        let out = format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        );
        if !o.status.success() {
            return Err(format!(
                "it exited {}: {}",
                o.status
                    .code()
                    .map_or("by signal".into(), |c| c.to_string()),
                out.trim()
            ));
        }
        for e in &what.expects {
            if !out.contains(e.as_str()) {
                return Err(format!("its output lacks {e:?}: {}", out.trim()));
            }
        }
        Ok(out)
    }

    /// A point where the author does something and looks at something, and says how it went: "ok" goes on,
    /// anything else stops the run and is recorded as what was seen; no answer stops it too.
    pub fn ask_author(&mut self, to_do: &str, to_check: &str) -> Result<(), Stop> {
        self.n += 1;
        let name = format!("ask the author: {to_do}");
        if self.dry {
            println!(
                "ask {}: would ask the author to {to_do}, and to check that {to_check}",
                self.n
            );
            self.note("dry", &name, &format!("would check: {to_check}"));
            return Ok(());
        }
        println!("ask {}: ATTENDED - please {to_do}.", self.n);
        println!("  Check that {to_check}. Answer ok, or say what you saw instead:");
        self.note("begin", &name, to_check);
        self.note("begin", &name, to_check);
        let mut line = String::new();
        let mut got = 1;
        while got > 0 && line.trim().is_empty() {
            line.clear();
            got = std::io::stdin().lock().read_line(&mut line).unwrap_or(0);
        }
        let answer = line.trim();
        if got == 0 {
            self.note("FAIL", &name, "nobody answered");
            return Err(Stop(format!("ask {} {name:?}: nobody answered", self.n)));
        }
        self.note(
            "asked",
            &name,
            &format!("check: {to_check}; answer: {answer}"),
        );
        if answer.eq_ignore_ascii_case("ok") {
            Ok(())
        } else {
            Err(Stop(format!(
                "ask {} {name:?}: expected that {to_check}, the author saw: {answer}",
                self.n
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quiet_run() -> Run {
        Run {
            dry: false,
            log: None,
            devlab: "devlab".into(),
            n: 0,
        }
    }

    #[test]
    fn a_step_is_tried_again_until_it_passes_and_fails_when_the_tries_run_out() {
        let dir = std::env::temp_dir().join(format!("lpoco-attended-retry-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let count = dir.join("count");
        // passes on the third try: the script appends a line and succeeds once there are three
        let script = format!(
            "echo x >> {c}; test $(wc -l < {c}) -ge 3",
            c = count.display()
        );
        let mut r = quiet_run();
        assert!(r.step("third time lucky", sh(&script).retry(3, 0)).is_ok());
        assert_eq!(std::fs::read_to_string(&count).unwrap().lines().count(), 3);
        // never passes: two tries, then the stop names the step
        let e = r.step("never", sh("false").retry(2, 0)).unwrap_err();
        assert!(e.to_string().contains("\"never\""), "{e}");
    }
}
