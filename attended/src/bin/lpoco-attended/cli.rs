// SPDX-License-Identifier: MIT OR Apache-2.0
//! lpoco-attended's invocation surface, declared once with argenv. Run as `lpoco attended ...`.
//!
//!   lpoco-attended new NAME [--device FILE]          write procedures/NAME.rs for the repo's device, ready to run
//!   lpoco-attended run NAME [--dry-run] [--log FILE] [--device FILE] [--until FIRST_PROMPT] [--part NAME]
//!                                                    run procedures/NAME.rs; the options go to the procedure as given
//!   lpoco-attended check                             dry-run every procedure; exit 1 naming each that fails
//!   lpoco-attended --version                         this program's version (the lpoco family's one version)

use argenv_resolve::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verb {
    New,
    Run,
    Check,
}

impl Verb {
    pub const TOKENS: &'static [&'static str] = &["new", "run", "check"];
}

impl FromRaw for Verb {
    fn from_raw(s: &str) -> Option<Verb> {
        Some(match s.trim() {
            "new" => Verb::New,
            "run" => Verb::Run,
            "check" => Verb::Check,
            _ => return None,
        })
    }
}

contract! {
    VERB: Verb = Input {
        key: "verb",
        ty: Type::Enum,
        allowed: Verb::TOKENS,
        arg: Some(Arg::positional(0)),
        summary: "What to do: new (scaffold a procedure), run (one by name) or check (dry-run them all).",
        ..Input::EMPTY
    };

    NAME: String = Input {
        key: "name",
        ty: Type::String,
        arg: Some(Arg { value_name: "NAME", ..Arg::positional(1) }),
        summary: "new, run: the procedure, procedures/NAME.rs (lowercase words, digits and hyphens).",
        ..Input::EMPTY
    };

    EXTRA: String = Input {
        key: "extra",
        ty: Type::String,
        arg: Some(Arg { value_name: "EXTRA", ..Arg::positional(2) }),
        summary: "None: a third word is refused, not dropped.",
        ..Input::EMPTY
    };

    DRY_RUN: bool = Input {
        key: "dry_run",
        ty: Type::Bool,
        default: Some(false),
        arg: Some(Arg::long("dry-run")),
        summary: "run: walk every step and say what it would do; reach no phone, ask no one.",
        ..Input::EMPTY
    };

    LOG: String = Input {
        key: "log",
        ty: Type::String,
        arg: Some(Arg { value_name: "FILE", ..Arg::long("log") }),
        summary: "run: the log the procedure writes.",
        ..Input::EMPTY
    };

    DEVICE: String = Input {
        key: "device",
        ty: Type::String,
        arg: Some(Arg { value_name: "FILE", ..Arg::long("device") }),
        summary: "The device manifest (device.toml here or in a folder above when none is given): new reads the codename from it, run hands it on.",
        ..Input::EMPTY
    };

    UNTIL: String = Input {
        key: "until",
        ty: Type::Enum,
        allowed: &["FIRST_PROMPT"],
        arg: Some(Arg { value_name: "POINT", ..Arg::long("until") }),
        summary: "run: stop cleanly at the first ask_author, never answering it.",
        ..Input::EMPTY
    };


    CALL_LIMIT: String = Input {
        key: "call_limit",
        ty: Type::String,
        arg: Some(Arg { value_name: "SECONDS", ..Arg::long("call-limit") }),
        summary: "run: the longest one call to the phone may take, in seconds (none when not given).",
        ..Input::EMPTY
    };
    PART: String = Input {
        key: "part",
        ty: Type::String,
        arg: Some(Arg { value_name: "NAME", ..Arg::long("part") }),
        summary: "run: one named part of the procedure instead of the whole.",
        ..Input::EMPTY
    };

    RUST_SCRIPT: String = Input {
        key: "rust_script",
        ty: Type::String,
        env: Some(Env::new("LPOCO_ATTENDED_RUST_SCRIPT")),
        summary: "The rust-script program that runs a procedure (rust-script when none is given); a test points it at a stand-in.",
        ..Input::EMPTY
    };

    LIB: String = Input {
        key: "lib",
        ty: Type::String,
        env: Some(Env::new("LPOCO_ATTENDED_LIB")),
        summary: "new: depend on the lpoco-attended checkout at this path instead of this release's git tag (working on lab-tools itself).",
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
}

/// The usage `--help` prints, read from the declaration above.
pub fn usage(name: &str) -> String {
    let mut out = format!(
        "Usage: {name} new NAME | run NAME [OPTIONS] | check\n\nA device repo's attended procedures, procedures/NAME.rs: scaffold one, run one, dry-run them all.\n"
    );
    let mut args = String::new();
    let mut envs = String::new();
    for r in &Contract::records() {
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

    #[test]
    fn the_declaration_holds() {
        assert!(
            Contract::problems().is_empty(),
            "{:#?}",
            Contract::problems()
        );
    }
}
