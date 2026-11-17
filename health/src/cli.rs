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
}

impl Command {
    pub const TOKENS: &'static [&'static str] =
        &["run", "latest", "list", "conditions", "off", "on"];
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
