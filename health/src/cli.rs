// SPDX-License-Identifier: MIT OR Apache-2.0
//! lpoco-health's invocation surface, declared once with argenv: its commands, their arguments and the
//! environment variables it reads. (`tone` and `data` are the checks' own helpers, with parsers of their own
//! in tone.rs and data.rs; they are dispatched before this surface is read.)
//!
//!   lpoco-health [run]                       run every test now; exit 1 when unhealthy
//!   lpoco-health latest                      print the latest report
//!   lpoco-health list                        the tests, switches and conditions as they stand
//!   lpoco-health conditions                  what this phone finds out about itself right now
//!   lpoco-health off TEST --reason WORDS     switch a test off, with a reason (--by WHO, or LPOCO_HEALTH_BY)
//!   lpoco-health on TEST                     switch it on again
//!   lpoco-health awake [--once]             hold the kernel wake lock lpoco-awake (the lab phone's keep-awake; --once: one step)
//!   lpoco-health suspend plan|cycle         one controlled suspend and wake, counted (--seconds N, --settle N, --before CMD, --after CMD; the way back is the manifest's)

use argenv_resolve::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    Run,
    Latest,
    List,
    Conditions,
    Off,
    On,
    Suspend,
    Awake,
}

impl Command {
    pub const TOKENS: &'static [&'static str] = &[
        "run",
        "latest",
        "list",
        "conditions",
        "off",
        "on",
        "suspend",
        "awake",
    ];
}

impl FromRaw for Command {
    fn from_raw(s: &str) -> Option<Command> {
        Some(match s.trim() {
            "run" => Command::Run,
            "latest" => Command::Latest,
            "list" => Command::List,
            "conditions" => Command::Conditions,
            "off" => Command::Off,
            "on" => Command::On,
            "suspend" => Command::Suspend,
            "awake" => Command::Awake,
            _ => return None,
        })
    }
}

contract! {
    COMMAND: Command = Input {
        key: "command",
        ty: Type::Enum,
        allowed: Command::TOKENS,
        arg: Some(Arg::positional(0)),
        summary: "What to do (run when none is said).",
        ..Input::EMPTY
    };

    TEST: String, Command = Input {
        key: "test",
        ty: Type::String,
        subcommands: &[Command::Off, Command::On],
        arg: Some(Arg::positional(0)),
        summary: "The test to switch (lpoco-health list says them).",
        ..Input::EMPTY
    };

    REASON: String, Command = Input {
        key: "reason",
        ty: Type::String,
        subcommands: &[Command::Off],
        arg: Some(Arg { value_name: "WORDS", ..Arg::long("reason") }),
        summary: "Why the test is off: a test is never switched off without one.",
        ..Input::EMPTY
    };

    BY: String, Command = Input {
        key: "by",
        ty: Type::String,
        subcommands: &[Command::Off],
        arg: Some(Arg { value_name: "WHO", ..Arg::long("by") }),
        env: Some(Env::new("LPOCO_HEALTH_BY")),
        summary: "Who switches it (the login by default).",
        ..Input::EMPTY
    };

    VERB: String, Command = Input {
        key: "verb",
        ty: Type::String,
        subcommands: &[Command::Suspend],
        arg: Some(Arg::positional(0)),
        summary: "suspend: plan (what a cycle would do, changing nothing) or cycle (one suspend and wake, counted).",
        ..Input::EMPTY
    };

    SECONDS: u64, Command = Input {
        key: "seconds",
        ty: Type::Uint,
        subcommands: &[Command::Suspend],
        default: Some(15),
        arg: Some(Arg { value_name: "N", ..Arg::long("seconds") }),
        summary: "suspend: how long the phone sleeps before the RTC wakes it (default 15; the way back must outlast it).",
        ..Input::EMPTY
    };

    SETTLE: u64, Command = Input {
        key: "settle",
        ty: Type::Uint,
        subcommands: &[Command::Suspend],
        default: Some(2),
        arg: Some(Arg { value_name: "N", ..Arg::long("settle") }),
        summary: "suspend: seconds to let the phone settle after waking before it is counted again (default 2).",
        ..Input::EMPTY
    };

    BEFORE: String, Command = Input {
        key: "before",
        ty: Type::String,
        subcommands: &[Command::Suspend],
        arg: Some(Arg { value_name: "CMD", ..Arg::long("before") }),
        summary: "suspend cycle: a command (sh -c) run after the services are stopped, ahead of the suspend; if it fails the cycle is refused.",
        ..Input::EMPTY
    };

    AFTER: String, Command = Input {
        key: "after",
        ty: Type::String,
        subcommands: &[Command::Suspend],
        arg: Some(Arg { value_name: "CMD", ..Arg::long("after") }),
        summary: "suspend cycle: a command (sh -c) run once the counters are read, before the services start again; always run when --before was.",
        ..Input::EMPTY
    };

    TRACE: bool, Command = Input {
        key: "trace",
        ty: Type::Bool,
        default: Some(false),
        subcommands: &[Command::Suspend],
        arg: Some(Arg::long("trace")),
        summary: "suspend cycle: switch on the manifest's trace for this cycle ([health] suspend_trace_dyndbg: dynamic_debug sites; suspend_trace_irq: irq handler entries) before the suspend, collect what they said after the wake into the report and var/lib/lpoco-health/suspend/cycle.trace and cycle.dmesg, and switch them off again. Without it nothing is changed.",
        ..Input::EMPTY
    };

    ONCE: bool, Command = Input {
        key: "once",
        ty: Type::Bool,
        default: Some(false),
        subcommands: &[Command::Awake],
        arg: Some(Arg::long("once")),
        summary: "awake: take one step (hold or release the wake lock as the phone stands) and stop, not every 5 s.",
        ..Input::EMPTY
    };

    ROOT: String = Input {
        key: "root",
        ty: Type::String,
        env: Some(Env::new("LPOCO_HEALTH_ROOT")),
        summary: "The phone's root, empty on the phone itself; a test points it at a made-up one.",
        ..Input::EMPTY
    };

    HELP: bool = Input {
        key: "help",
        ty: Type::Bool,
        default: Some(false),
        arg: Some(Arg { short: Some('h'), ..Arg::long("help") }),
        summary: "Print this usage and stop.",
        ..Input::EMPTY
    };

    VERSION: bool = Input {
        key: "version",
        ty: Type::Bool,
        default: Some(false),
        arg: Some(Arg::long("version")),
        summary: "Print the version (the lpoco family's one version) and stop.",
        ..Input::EMPTY
    };

    FORCE: bool, Command = Input {
        key: "force",
        ty: Type::Bool,
        default: Some(false),
        subcommands: &[Command::Off],
        arg: Some(Arg::long("force")),
        summary: "Switch off a test no run has named yet.",
        ..Input::EMPTY
    };
}

/// The usage `--help` prints: the program's line, then every declared argument and environment
/// variable with its summary, all read from the declaration above.
pub fn usage(name: &str, about: &str) -> String {
    let mut out = format!("Usage: {name} [OPTIONS] [ARGS]\n\n{about}\n");
    let records = Contract::records();
    let mut args = String::new();
    let mut envs = String::new();
    for r in &records {
        let summary = r.summary.clone().unwrap_or_default();
        let sig = r.usage();
        if !sig.is_empty() {
            args.push_str(&format!("  {sig:<22} {summary}\n"));
        }
        for e in r.env_names() {
            envs.push_str(&format!("  {e:<22} {summary}\n"));
        }
    }
    if !args.is_empty() {
        out.push_str(&format!("\nArguments:\n{args}"));
    }
    if !envs.is_empty() {
        out.push_str(&format!("\nEnvironment:\n{envs}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn resolve(args: &[&str], env: &[(&str, &str)]) -> Resolution {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let env: BTreeMap<String, String> = env
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Invocation {
            args: &args,
            env: &env,
        }
        .resolve(&Contract::records())
    }

    #[test]
    fn the_declaration_holds() {
        assert!(
            Contract::problems().is_empty(),
            "{:#?}",
            Contract::problems()
        );
    }

    #[test]
    fn off_takes_a_test_a_reason_and_a_hand_from_the_environment() {
        let r = resolve(
            &["off", "sms-loopback", "--reason", "no plan"],
            &[("LPOCO_HEALTH_BY", "grenudi")],
        );
        assert_eq!(Contract::COMMAND.get_from(&r), Some(Command::Off));
        assert_eq!(Contract::TEST.get_from(&r).as_deref(), Some("sms-loopback"));
        assert_eq!(Contract::REASON.get_from(&r).as_deref(), Some("no plan"));
        assert_eq!(Contract::BY.get_from(&r).as_deref(), Some("grenudi"));
    }
}
