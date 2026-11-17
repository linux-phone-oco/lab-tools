//! lpoco's own invocation surface, declared once with argenv. Only the dispatcher's built-ins are
//! declared here; anything else is `lpoco NAME args...`, handed whole to `lpoco-NAME` (whose own
//! declaration is its own), so the arguments after NAME are never read by lpoco.
//!
//!   lpoco NAME args...            run lpoco-NAME with args, exit with its exit code
//!   lpoco list                    the lpoco-* programs found, with their versions
//!   lpoco completions SHELL       shell completions (bash, zsh, fish, nushell)
//!   lpoco mcp                     the one MCP server for the family: the tools of lab, health and bringup
//!   lpoco man                     the manual page, in roff
//!   lpoco --version               this program's version (the family's)

use argenv_resolve::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    List,
    Completions,
    Man,
    Mcp,
}

impl Command {
    pub const TOKENS: &'static [&'static str] = &["list", "completions", "man", "mcp"];
}

impl FromRaw for Command {
    fn from_raw(s: &str) -> Option<Command> {
        Some(match s.trim() {
            "list" => Command::List,
            "completions" => Command::Completions,
            "man" => Command::Man,
            "mcp" => Command::Mcp,
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
        summary: "A built-in (any other word is the name of an lpoco-NAME program to run).",
        ..Input::EMPTY
    };

    SHELL: String, Command = Input {
        key: "shell",
        ty: Type::Enum,
        allowed: &["bash", "zsh", "fish", "nushell"],
        subcommands: &[Command::Completions],
        arg: Some(Arg::positional(0)),
        summary: "The shell to write completions for.",
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
        summary: "Print the version (one for the whole family) and stop.",
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

    #[test]
    fn the_declaration_holds() {
        assert!(
            Contract::problems().is_empty(),
            "{:#?}",
            Contract::problems()
        );
    }
}
