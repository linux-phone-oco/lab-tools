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
//!     check_from("the driver is bound", "research:2026-11-23-lsmod", || phone().sh("lsmod").map(|m| m.contains("pmicwdt")));
//! }
//! ```
//!
//! A [`Step`] is the one step type: a name, what kind it is (do, check, ask), the bring-up step id it is keyed
//! to where one exists, and what it runs. The attended runner here runs a chain of them with a person at hand;
//! the unattended step runner (bringup-step-runner) takes the same value, so an attended procedure and a nightly
//! health run are the same chain. A library step (like [`steps::ask`]) is a function that returns a `Step`.

pub mod bench;
#[doc(hidden)]
pub mod call;
mod cli;
mod device;
pub(crate) mod prompt;
mod source;
pub mod steps;
mod values;
pub use values::Val;

use argenv_resolve::{InputResolveExt, OnProblems};
use std::fmt::Display;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Everything an entry file needs, in one line: `use lpoco_attended::prelude::*;`.
pub mod prelude {
    pub use crate::bench::{bench, Act, ActKind, Bench};
    pub use crate::{
        ask_author, check, check_from, confirm, dev, group, hold_button, listen_for, look_for,
        part, phone, press_power, proc, procedure, replug_usb, retry, run_group, run_step, step,
        swipe, touch, unknown_part, unplug_usb, Group, Item, Kind, Outcome, Step, Val,
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
    /// where the value this check expects came from (see [`check_from`]); `None` for a step that is not an entry file's check
    pub source: Option<String>,
    /// an entry file's check: the dry run refuses it unless it names a source
    needs_source: bool,
    run: Box<dyn FnOnce() -> Result<(), String>>,
}

impl Step {
    pub fn new<R: Outcome>(kind: Kind, name: &str, run: impl FnOnce() -> R + 'static) -> Step {
        Step {
            name: name.to_string(),
            kind,
            id: None,
            doer: None,
            source: None,
            needs_source: false,
            run: Box::new(move || run().outcome()),
        }
    }

    /// Name where the value the step expects came from.
    pub fn sourced(mut self, source: &str) -> Step {
        self.source = Some(source.to_string());
        self
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
    /// start at the step of this name; the steps before it are skipped
    from: Option<String>,
    /// the device.toml of the procedure's device, the one devlab is handed
    device: std::path::PathBuf,
    /// what that file declares: steps read their keys from it
    manifest: lpoco_manifest::Manifest,
    /// the same file as the tables it says, for the keys a procedure reads by name
    raw: toml::Value,
    devlab: String,
    /// the lpoco-bringup program that ensures an image a recipe makes
    bringup: String,
    sysfs_usb: String,
    /// the longest one call to the phone may take; none unless asked for
    call_limit: Option<std::time::Duration>,
    /// where a pending prompt is filed and its answer file read: DIR/DEVICE
    prompt_files: prompt::Files,
    /// the longest a prompt waits for the author when the terminal has nothing to give; none unless asked for
    answer_limit: Option<std::time::Duration>,
    name: String,
}

struct State {
    log: Option<File>,
    n: u32,
    /// the step being run: what the phone says is filed under it
    step: String,
    /// every step name this run has met, for the names `--from` is checked against
    seen: Vec<String>,
    /// whether the step `--from` names has come
    from_reached: bool,
    /// how many steps `--from` skipped
    skipped: u32,
    /// the names of the groups being run, outermost first: a step is logged under them
    groups: Vec<String>,
}

static CONFIG: OnceLock<Config> = OnceLock::new();
/// whether the entry file asked for its part: a `--part` it never looks at must not run the whole procedure
static PART_ASKED: AtomicBool = AtomicBool::new(false);
static NEXT_GROUP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

static STATE: Mutex<State> = Mutex::new(State {
    log: None,
    n: 0,
    step: String::new(),
    seen: Vec::new(),
    from_reached: false,
    skipped: 0,
    groups: Vec::new(),
});

thread_local! {
    /// the way backs of the groups now open that no step has started yet (a group skipped whole owes none)
    static PENDING: std::cell::RefCell<Vec<(u64, Option<Step>)>> = const { std::cell::RefCell::new(Vec::new()) };
    /// the way backs owed, each with the id of its group: a step of the group has begun; run in reverse order when the run fails
    static OWED: std::cell::RefCell<Vec<(u64, Step)>> = const { std::cell::RefCell::new(Vec::new()) };
}

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
        let (seen, reached) = {
            let st = STATE.lock().unwrap_or_else(|e| e.into_inner());
            (st.seen.clone(), st.from_reached)
        };
        if let (Some(from), false) = (c.from.as_deref(), reached) {
            eprintln!(
                "{}: no step is called {from:?} (--from); the steps are: {}",
                c.name,
                seen.iter()
                    .map(|s| format!("{s:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            std::process::exit(2);
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

/// Begin the procedure: read the arguments (`--dry-run`, `--log FILE`, `--device FILE`, `--until FIRST_PROMPT`, `--part NAME`, `--from STEP`, `--help`), load the device's manifest, open the log. Call it once,
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
    let from = cli::Contract::FROM.get_from(&r);
    let devlab = cli::Contract::DEVLAB
        .get_from(&r)
        .unwrap_or_else(|| "devlab".into());
    let bringup = cli::Contract::BRINGUP
        .get_from(&r)
        .unwrap_or_else(|| "lpoco-bringup".into());
    let sysfs_usb = cli::Contract::SYSFS_USB
        .get_from(&r)
        .unwrap_or_else(|| "/sys/bus/usb/devices".into());
    let call_limit = seconds("--call-limit", cli::Contract::CALL_LIMIT.get_from(&r))
        .unwrap_or_else(|e| {
            eprintln!("{name}: {e}");
            std::process::exit(2)
        });
    let answer_limit = seconds("--answer-limit", cli::Contract::ANSWER_LIMIT.get_from(&r))
        .unwrap_or_else(|e| {
            eprintln!("{name}: {e}");
            std::process::exit(2)
        });
    let prompt_base = cli::Contract::PROMPT_DIR
        .get_from(&r)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
                .join(".cache/lpoco-attended")
        });
    let prompt_files = prompt::Files::new(prompt::dir_for(&prompt_base, device));
    let c = Config {
        dry,
        until_first_prompt,
        part,
        from,
        device: device_file,
        manifest,
        raw,
        devlab,
        bringup,
        sysfs_usb,
        call_limit,
        prompt_files,
        answer_limit,
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
        st.step = label(&st.groups, &s.name);
        st.n
    };
    let (kind, name) = (s.kind.word(), s.name.clone());
    let (indent, path) = {
        let st = STATE.lock().unwrap_or_else(|e| e.into_inner());
        ("  ".repeat(st.groups.len()), label(&st.groups, &s.name))
    };
    // whether an earlier step has this very name: a --from would then start at the earlier one
    let repeated = {
        let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let repeated = st.seen.contains(&s.name);
        st.seen.push(s.name.clone());
        repeated
    };
    if let Some(from) = c.from.as_deref() {
        let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
        if !st.from_reached && s.name != from {
            st.step.clear();
            st.skipped += 1;
            drop(st);
            println!("{indent}{kind} {n}: {name} - skipped (before --from)");
            note("skip", &path, "before --from");
            return;
        }
        st.from_reached = true;
    }
    if s.prompts() && c.until_first_prompt {
        let skipped = STATE.lock().unwrap_or_else(|e| e.into_inner()).skipped;
        let resume = if repeated {
            format!("no --from resumes here: an earlier step is also called {name:?}")
        } else {
            let part = c
                .part
                .as_deref()
                .map_or(String::new(), |p| format!(" --part {p}"));
            format!("resume with{part} --from {name:?}")
        };
        note(
            "stop",
            &name,
            &format!("--until FIRST_PROMPT: the author is left alone; {resume}"),
        );
        println!(
            "{}: stopped before the first prompt ({:?}) after {} step(s){}; {resume}",
            c.name,
            name,
            n - 1 - skipped,
            if c.dry {
                " (dry run: nothing was done)"
            } else {
                ""
            }
        );
        std::process::exit(0);
    }
    if c.dry {
        if s.needs_source {
            let from = c.device.parent().unwrap_or(std::path::Path::new("."));
            if let Err(why) = source::judge(from, s.source.as_deref()) {
                note("FAIL", &path, &why);
                eprintln!("{}: stopped at {kind} {n} {name:?}: {why}", c.name);
                leave_failed();
            }
        }
        let by = s
            .doer
            .as_deref()
            .map_or(String::new(), |d| format!(" (by {d})"));
        println!("{indent}{kind} {n}: {name} - dry run, not run{by}");
        note(
            "dry",
            &path,
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
    arm_way_backs();
    println!("{indent}{kind} {n}: {name}{by}");
    note("begin", &path, &format!("{kind}{by}"));
    // a step that panics fails like one that returns an error: the way backs owed must still run
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.execute()))
        .unwrap_or_else(|_| {
            note("PANIC", &path, "the step panicked; the way backs owed run");
            Err("the step panicked".to_string())
        });
    STATE.lock().unwrap_or_else(|e| e.into_inner()).step.clear();
    match result {
        Ok(()) => note("ok", &path, ""),
        Err(why) => {
            note("FAIL", &path, &why);
            let within = if path == name {
                String::new()
            } else {
                format!(
                    " in {}",
                    path.strip_suffix(&format!(" > {name}")).unwrap_or(&path)
                )
            };
            eprintln!("{}: stopped at {kind} {n} {name:?}{within}: {why}", c.name);
            leave_failed();
        }
    }
}

/// What a group holds: a step, or a group inside it.
pub enum Item {
    Step(Step),
    Group(Group),
}

impl From<Step> for Item {
    fn from(s: Step) -> Item {
        Item::Step(s)
    }
}

impl From<Group> for Item {
    fn from(g: Group) -> Item {
        Item::Group(g)
    }
}

/// Steps that go together under one name: one line in the entry file, logged under that name, listed nested by
/// `--dry-run`, named by `--from`. It may carry a way back ([`Group::way_back`]).
pub struct Group {
    name: String,
    items: Vec<Item>,
    way_back: Option<Step>,
    settles: bool,
}

/// A named group of steps (or of groups): `run_group(group("reach the phone", [a, b]))`. A mix of steps and groups
/// takes `Item::from` on each (`[Item::from(step), Item::from(inner_group)]`).
pub fn group<I: Into<Item>>(name: &str, items: impl IntoIterator<Item = I>) -> Group {
    Group {
        name: name.to_string(),
        items: items.into_iter().map(Into::into).collect(),
        way_back: None,
        settles: false,
    }
}

impl Group {
    /// What to do when the run fails after this group's first step began: the step runs once, after the failing
    /// step, with the way backs of the other groups open or finished, the last to begin first. A run that passes
    /// runs none; a group skipped whole by `--from` owes none; a way back that fails is named and the others
    /// still run. A group that has finished still owes its way back until a later [`Group::settling`] group
    /// (like `leave_known_good`) has passed; `--until FIRST_PROMPT` runs none, and `--from` arms the group again at
    /// its first step run. A step that panics counts as failed; so does a key `dev()` or a step needs that
    /// device.toml lacks (exit 2).
    pub fn way_back(mut self, back: Step) -> Group {
        self.way_back = Some(back);
        self
    }

    /// This group puts the phone back (like `leave_known_good`): once all its steps have passed, the way backs of the
    /// groups that finished before it (at any depth) are settled, and a later failure runs none of them, nor this
    /// group's own. The way backs of groups
    /// still open around it stay owed. A failing step in this group settles nothing: every way back owed runs.
    pub fn settling(mut self) -> Group {
        self.settles = true;
        self
    }
}

/// The steps of a group, in order, each logged under the group's name (`GROUP > STEP`); the run stops at the first
/// that fails, after the way backs of its groups.
pub fn run_group(g: Group) {
    let c = config();
    let (indent, from_here) = {
        let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
        st.seen.push(g.name.clone());
        let named = c.from.as_deref() == Some(g.name.as_str());
        if named {
            st.from_reached = true;
        }
        ("  ".repeat(st.groups.len()), named)
    };
    let back = g
        .way_back
        .as_ref()
        .map_or(String::new(), |b| format!(" (way back: {})", b.name));
    let back = if g.settles {
        format!("{back} (settles the way backs of finished groups)")
    } else {
        back
    };
    println!("{indent}group: {}{back}", g.name);
    let path = label(
        &STATE.lock().unwrap_or_else(|e| e.into_inner()).groups,
        &g.name,
    );
    note(
        "group",
        &path,
        &format!(
            "{}{back}",
            if from_here { "--from starts here" } else { "" }
        ),
    );
    STATE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .groups
        .push(g.name);
    let id = NEXT_GROUP.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    PENDING.with(|p| p.borrow_mut().push((id, g.way_back)));
    for item in g.items {
        match item {
            Item::Step(s) => run_step(s),
            Item::Group(g) => run_group(g),
        }
    }
    PENDING.with(|p| p.borrow_mut().pop());
    if g.settles {
        settle_finished();
    }
    STATE.lock().unwrap_or_else(|e| e.into_inner()).groups.pop();
}

/// A step's name as it is logged: under the groups it is in, outermost first.
fn label(groups: &[String], name: &str) -> String {
    groups
        .iter()
        .map(String::as_str)
        .chain([name])
        .collect::<Vec<_>>()
        .join(" > ")
}

/// A step is about to begin: the way backs of the groups it is in are owed from now on.
fn arm_way_backs() {
    let armed: Vec<(u64, Step)> = PENDING.with(|p| {
        p.borrow_mut()
            .iter_mut()
            .filter_map(|(id, back)| Some((*id, back.take()?)))
            .collect()
    });
    OWED.with(|o| o.borrow_mut().extend(armed));
}

/// A settling group has passed: the way backs owed by groups no longer open (the ones that finished before it,
/// whatever their depth, and its own) are settled; those of the groups still open around it stay owed.
fn settle_finished() {
    let open: Vec<u64> = PENDING.with(|p| p.borrow().iter().map(|(id, _)| *id).collect());
    OWED.with(|o| o.borrow_mut().retain(|(owner, _)| open.contains(owner)));
}

/// End the run that failed: run the way backs owed, the last owed first, then exit 1.
fn leave_failed() -> ! {
    leave_with(1)
}

/// End the run with `code` after the way backs owed (a run that cannot go on, exit 2, leaves the phone as a failed one does).
pub(crate) fn leave_with(code: i32) -> ! {
    let owed = OWED.with(|o| std::mem::take(&mut *o.borrow_mut()));
    for (_, back) in owed.into_iter().rev() {
        let name = back.name.clone();
        println!("way back: {name}");
        note("way-back", &name, "begin");
        STATE.lock().unwrap_or_else(|e| e.into_inner()).step = name.clone();
        let result = back.execute();
        STATE.lock().unwrap_or_else(|e| e.into_inner()).step.clear();
        match result {
            Ok(()) => note("way-back", &name, "ok"),
            Err(why) => {
                note("FAIL", &format!("way back {name}"), &why);
                eprintln!("{}: the way back {name:?} failed: {why}", config().name);
            }
        }
    }
    std::process::exit(code)
}

/// Do something; the run stops here when the closure fails.
pub fn step<R: Outcome>(name: &str, run: impl FnOnce() -> R + 'static) {
    run_step(Step::new(Kind::Do, name, run));
}

/// Look at something; the run stops here when it does not hold. Same as a step, kept apart so the file says
/// which steps only look.
///
/// The dry run (so `lpoco attended check`) refuses a check made this way: it names no source. Use [`check_from`].
pub fn check<R: Outcome>(name: &str, run: impl FnOnce() -> R + 'static) {
    let mut s = Step::new(Kind::Check, name, run);
    s.needs_source = true;
    run_step(s);
}

/// A check that names its source: where the value it expects came from, so a check typed from memory cannot pass
/// for one read from the phone. `source` is one of
/// - `research:ID`, a research row of the project's base (not looked up here: the base is not this program's);
/// - a path ending in `.md`, a doc of the repository;
/// - a path under a `research` folder, a file of text a phone printed, captured.
///
/// A path is read from the folder of the device's `device.toml`. The dry run exits 1 naming the check when the
/// source is empty, the file is not there, holds nothing, is not text, or is not under a `research` folder.
pub fn check_from<R: Outcome>(name: &str, source: &str, run: impl FnOnce() -> R + 'static) {
    let mut s = Step::new(Kind::Check, name, run).sourced(source);
    s.needs_source = true;
    run_step(s);
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
    leave_with(2)
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
    leave_with(2)
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
        leave_with(2)
    })
}

/// Ask the person one prompt: the terminal and the answer file, whichever answers first.
pub(crate) fn ask_person(p: &prompt::Prompt) -> Result<prompt::Answer, String> {
    let c = config();
    prompt::ask(&c.prompt_files, p, c.answer_limit)
}

pub(crate) fn sysfs_usb() -> &'static str {
    &config().sysfs_usb
}

/// A limit in seconds, parsed at the edge: whole seconds, at least one; none when none is given (a call waits as
/// long as devlab does: its own boot wait is ten minutes and more).
fn seconds(flag: &str, given: Option<String>) -> Result<Option<std::time::Duration>, String> {
    let Some(s) = given else {
        return Ok(None);
    };
    match s.trim().parse::<u64>() {
        Ok(n) if n >= 1 => Ok(Some(std::time::Duration::from_secs(n))),
        _ => Err(format!("{flag} wants whole seconds, at least 1; got '{s}'")),
    }
}

/// One call to devlab for the device of this run, its input on standard input; what it said (standard output and
/// error) when it exits 0. `reached` names what a dry run does not reach, `failed` how a failure is worded. A
/// call past the call limit is cut and fails.
fn call_devlab(args: &[&str], stdin: &str, reached: &str, failed: &str) -> Result<String, String> {
    if config().dry {
        return Err(format!("dry run: {reached} is not reached"));
    }
    let mut cmd = Command::new(&config().devlab);
    cmd.arg("--device").arg(&config().device).args(args);
    let o =
        call::run_limited(cmd, stdin, config().call_limit).map_err(|e| format!("{failed}: {e}"))?;
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
            "{failed} exited {}: {}",
            o.status
                .code()
                .map_or("by signal".into(), |c| c.to_string()),
            out.trim()
        ))
    }
}

/// Run devlab itself on this computer (`devlab ARGS`, e.g. `kernel try NAME`): its output when it exits 0, its
/// output named when it does not. A dry run reaches nothing.
pub(crate) fn devlab(args: &[&str]) -> Result<String, String> {
    call_devlab(args, "", "devlab", &format!("devlab {}", args.join(" ")))
}

/// `lpoco-bringup image ensure NAME` for the device of this run (`--dry-run`: only where it would come from, which
/// reads and changes nothing, so a dry run asks it too): what it said on standard output, trimmed, when it exits 0.
/// A build takes minutes, so no limit is put on it.
pub(crate) fn ensure_image(name: &str, dry_run: bool) -> Result<String, String> {
    let mut cmd = Command::new(&config().bringup);
    cmd.args(["image", "ensure", name]);
    if dry_run {
        cmd.arg("--dry-run");
    }
    cmd.arg("--device").arg(&config().device);
    let failed = format!("lpoco-bringup image ensure {name}");
    let o = call::run_limited(cmd, "", None).map_err(|e| format!("{failed}: {e}"))?;
    let (out, err) = (
        String::from_utf8_lossy(&o.stdout).trim().to_string(),
        String::from_utf8_lossy(&o.stderr).trim().to_string(),
    );
    if o.status.success() {
        if !err.is_empty() {
            note_out(&format!("{err}\n"));
            eprintln!("{err}");
        }
        // the answer is the last line: one store path (absolute, and there), or one `NAME: ORIGIN (PATH)` line
        let line = out.lines().last().unwrap_or("").trim().to_string();
        let absolute = dry_run || std::path::Path::new(&line).is_absolute();
        if line.is_empty() || !absolute || (!dry_run && !std::path::Path::new(&line).exists()) {
            return Err(format!("{failed} gave no path to an image: {out:?}"));
        }
        Ok(line)
    } else {
        Err(format!("{failed} exited {}: {err}", o.status))
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
        call_devlab(&["phone", who], script, "the phone", "the phone's script")
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
