// SPDX-License-Identifier: MIT OR Apache-2.0
//! lpoco-tone's invocation surface, declared once with argenv (backlog lpoco-tone-argenv): its commands,
//! their arguments and the environment it reads (none). The words are those it has always had.
//!
//!   lpoco-tone make [--freq HZ] [--secs S] [--level DBFS] [--rate HZ] [--channels N] OUT
//!   lpoco-tone find [--freq HZ] [--min-snr DB] INPUT      JSON on stdout; exit 0 when found, 1 when not
//!
//! argenv reads the first word as the command; the flags a command takes are listed in `FLAGS` and
//! checked here, as lpoco-lab does, so a flag for the other command is a usage mistake.

use argenv_resolve::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    Make,
    Find,
}

impl Command {
    pub const TOKENS: &'static [&'static str] = &["make", "find"];
}

impl FromRaw for Command {
    fn from_raw(s: &str) -> Option<Command> {
        Some(match s.trim() {
            "make" => Command::Make,
            "find" => Command::Find,
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
        summary: "What to do: make a tone, or find one in a recording.",
        ..Input::EMPTY
    };

    OUT: String, Command = Input {
        key: "out",
        ty: Type::String,
        subcommands: &[Command::Make],
        arg: Some(Arg { value_name: "OUT", ..Arg::positional(0) }),
        summary: "make: the WAV file to write (16-bit).",
        ..Input::EMPTY
    };

    INPUT: String, Command = Input {
        key: "input",
        ty: Type::String,
        subcommands: &[Command::Find],
        arg: Some(Arg { value_name: "INPUT", ..Arg::positional(0) }),
        summary: "find: the WAV recording to measure the frequency in.",
        ..Input::EMPTY
    };

    FREQ: f64, Command = Input {
        key: "freq",
        ty: Type::Float,
        subcommands: &[Command::Make, Command::Find],
        default: Some(1000.0),
        arg: Some(Arg { value_name: "HZ", ..Arg::long("freq") }),
        summary: "The tone's frequency in Hz (default 1000): the one to make, or the one to find.",
        ..Input::EMPTY
    };

    SECS: f64, Command = Input {
        key: "secs",
        ty: Type::Float,
        subcommands: &[Command::Make],
        default: Some(3.0),
        arg: Some(Arg { value_name: "SECONDS", ..Arg::long("secs") }),
        summary: "make: how long the tone lasts (default 3).",
        ..Input::EMPTY
    };

    LEVEL: f64, Command = Input {
        key: "level",
        ty: Type::Float,
        subcommands: &[Command::Make],
        default: Some(-20.0),
        arg: Some(Arg { value_name: "DBFS", ..Arg::long("level") }),
        summary: "make: level in dBFS, 0 being full scale - gentle by default (-20).",
        ..Input::EMPTY
    };

    RATE: u64, Command = Input {
        key: "rate",
        ty: Type::Uint,
        subcommands: &[Command::Make],
        default: Some(48_000),
        arg: Some(Arg { value_name: "HZ", ..Arg::long("rate") }),
        summary: "make: the sample rate (default 48000).",
        ..Input::EMPTY
    };

    CHANNELS: u64, Command = Input {
        key: "channels",
        ty: Type::Uint,
        subcommands: &[Command::Make],
        default: Some(2),
        arg: Some(Arg { value_name: "N", ..Arg::long("channels") }),
        summary: "make: how many channels (default 2).",
        ..Input::EMPTY
    };

    MIN_SNR: f64, Command = Input {
        key: "min_snr",
        ty: Type::Float,
        subcommands: &[Command::Find],
        default: Some(20.0),
        arg: Some(Arg { value_name: "DB", ..Arg::long("min-snr") }),
        summary: "find: how many dB above the noise the tone must stand to count as found (default 20).",
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
        arg: Some(Arg { short: Some('V'), ..Arg::long("version") }),
        summary: "Print the version (the lpoco family's one version) and stop.",
        ..Input::EMPTY
    };
}

/// The positionals and flags (keys) each command takes, in order.
pub const TREE: &[(&str, &[&str], &[&str])] = &[
    (
        "make",
        &["out"],
        &["freq", "secs", "level", "rate", "channels"],
    ),
    ("find", &["input"], &["freq", "min_snr"]),
];

/// The usage `--help` prints: the program's line, then every declared argument with its summary, all read
/// from the declaration above. `word` narrows it to one command's arguments.
pub fn usage(word: Option<&str>) -> String {
    let mut out = String::from(
        "Usage: lpoco-tone make [OPTIONS] OUT\n       lpoco-tone find [OPTIONS] INPUT\n\n\
         Make a test tone, and find it in a recording - so a device's speaker can test its microphone.\n",
    );
    let mut args = String::new();
    for r in &Contract::records() {
        let applies = match (word, TREE.iter().find(|t| Some(t.0) == word)) {
            (Some(_), Some(t)) => {
                t.1.contains(&r.key.as_str()) || t.2.contains(&r.key.as_str()) || r.key == "help"
            }
            _ => true,
        };
        let sig = r.usage();
        if applies && !sig.is_empty() {
            let summary = r.summary.clone().unwrap_or_default();
            args.push_str(&format!("  {sig:<24} {summary}\n"));
        }
    }
    out.push_str(&format!("\nArguments:\n{args}"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_declaration_holds() {
        assert!(
            Contract::problems().is_empty(),
            "{:#?}",
            Contract::problems()
        );
    }

    #[test]
    fn the_tree_names_only_declared_inputs() {
        let keys: Vec<String> = Contract::records().iter().map(|r| r.key.clone()).collect();
        for (_, args, flags) in TREE {
            for k in args.iter().chain(flags.iter()) {
                assert!(keys.iter().any(|d| d == k), "{k} is not declared");
            }
        }
    }
}
