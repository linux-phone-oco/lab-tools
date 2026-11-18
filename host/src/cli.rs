//! lpoco-host's invocation surface, declared once with argenv: its commands, their arguments and the
//! environment variables it reads. Run as `lpoco host ...`.
//!
//!   lpoco-host [plan]            what the lab host needs, as a diff per file and the exact root commands; changes nothing on the host (each device clone is fetched)
//!   lpoco-host prepare now       do it: files written, udev reloaded, modules loaded, tools installed - only where they differ
//!   lpoco-host check             the lab user can open each declared node that is present; each tool at its pin
//!   lpoco-host --version         this program's version (the lpoco family's one version)

use argenv_resolve::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    Plan,
    Prepare,
    Check,
}

impl Command {
    pub const TOKENS: &'static [&'static str] = &["plan", "prepare", "check"];
}

impl FromRaw for Command {
    fn from_raw(s: &str) -> Option<Command> {
        Some(match s.trim() {
            "plan" => Command::Plan,
            "prepare" => Command::Prepare,
            "check" => Command::Check,
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
        summary: "What to do (plan when none is given).",
        ..Input::EMPTY
    };

    NOW: String, Command = Input {
        key: "now",
        ty: Type::Enum,
        allowed: &["now"],
        subcommands: &[Command::Prepare],
        arg: Some(Arg::positional(0)),
        summary: "prepare changes the host only when it is told `now`.",
        ..Input::EMPTY
    };

    LAB: String = Input {
        key: "lab",
        ty: Type::String,
        arg: Some(Arg { value_name: "FILE", ..Arg::long("lab") }),
        env: Some(Env::new("LPOCO_LAB")),
        summary: "The lab's own declaration, lab.toml (~/Projects/lab-machine/lab.toml when none is given).",
        ..Input::EMPTY
    };

    ROOT: String = Input {
        key: "root",
        ty: Type::String,
        env: Some(Env::new("LPOCO_ROOT")),
        summary: "The program that runs a root command: it is given the command as its arguments.",
        ..Input::EMPTY
    };

    DEVLAB_ROOT: String = Input {
        key: "devlab_root",
        ty: Type::String,
        env: Some(Env::new("DEVLAB_ROOT")),
        summary: "The root prefix devlab already uses; read when LPOCO_ROOT is not set.",
        ..Input::EMPTY
    };

    FS_ROOT: String = Input {
        key: "fs_root",
        ty: Type::String,
        env: Some(Env::new("LPOCO_FS_ROOT")),
        summary: "A folder the host's own files (/etc/udev, /etc/modules-load.d) are read under, for tests; the root commands still name the real paths.",
        ..Input::EMPTY
    };

    HOME: String = Input {
        key: "home",
        ty: Type::String,
        env: Some(Env::new("HOME")),
        summary: "The lab user's home: ~/.local/bin takes the host's tools, ~/.cache/lpoco-host the staged files.",
        ..Input::EMPTY
    };

    USER: String = Input {
        key: "user",
        ty: Type::String,
        env: Some(Env::new("USER")),
        summary: "The lab user, whom check looks for in the gadget group.",
        ..Input::EMPTY
    };

    PATH: String = Input {
        key: "path",
        ty: Type::String,
        env: Some(Env::new("PATH")),
        summary: "Where the host's tools are looked for, to read their versions.",
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
    fn prepare_now_and_the_lab_from_the_environment() {
        let r = resolve(&["prepare", "now"], &[("LPOCO_LAB", "l.toml")]);
        assert_eq!(Contract::COMMAND.get_from(&r), Some(Command::Prepare));
        assert_eq!(Contract::NOW.get_from(&r).as_deref(), Some("now"));
        assert_eq!(Contract::LAB.get_from(&r).as_deref(), Some("l.toml"));
    }
}
