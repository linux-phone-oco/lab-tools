// SPDX-License-Identifier: MIT OR Apache-2.0
//! osfact's invocation surface, declared once with argenv.
//!
//!   osfact install now --plan FILE --dry-run     list the commands of an install.toml (fastboot boot, the way back); changes nothing

use argenv_resolve::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    Install,
}

impl FromRaw for Command {
    fn from_raw(s: &str) -> Option<Command> {
        match s.trim() {
            "install" => Some(Command::Install),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verb {
    Now,
}

impl FromRaw for Verb {
    fn from_raw(s: &str) -> Option<Verb> {
        match s.trim() {
            "now" => Some(Verb::Now),
            _ => None,
        }
    }
}

contract! {
    COMMAND: Command = Input {
        key: "command",
        ty: Type::Enum,
        allowed: &["install"],
        arg: Some(Arg::positional(0)),
        summary: "What to do (install: put an image on the device).",
        ..Input::EMPTY
    };

    VERB: Verb, Command = Input {
        key: "verb",
        ty: Type::Enum,
        allowed: &["now"],
        subcommands: &[Command::Install],
        arg: Some(Arg::positional(0)),
        summary: "now: say it means to act (a command that changes state needs an explicit verb).",
        ..Input::EMPTY
    };

    PLAN: String, Command = Input {
        key: "plan",
        ty: Type::String,
        subcommands: &[Command::Install],
        arg: Some(Arg { value_name: "FILE", ..Arg::long("plan") }),
        summary: "install: the install.toml of the image.",
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
        summary: "Print the version and stop.",
        ..Input::EMPTY
    };

    DRY_RUN: bool, Command = Input {
        key: "dry_run",
        ty: Type::Bool,
        subcommands: &[Command::Install],
        arg: Some(Arg::long("dry-run")),
        summary: "install: only list the commands; the skeleton runs nothing else yet.",
        ..Input::EMPTY
    };
}

/// The usage `--help` prints: every declared argument with its summary.
pub fn usage(name: &str, about: &str) -> String {
    let mut out = format!("Usage: {name} [OPTIONS] [ARGS]\n\n{about}\n\nArguments:\n");
    for r in Contract::records() {
        let sig = r.usage();
        if !sig.is_empty() {
            out.push_str(&format!(
                "  {sig:<22} {}\n",
                r.summary.clone().unwrap_or_default()
            ));
        }
    }
    out
}
