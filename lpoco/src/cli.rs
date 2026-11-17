//! lpoco's own invocation surface, declared once with argenv. Only the dispatcher's built-ins are
//! declared here; anything else is `lpoco NAME args...`, handed whole to `lpoco-NAME` (whose own
//! declaration is its own), so the arguments after NAME are never read by lpoco.
//!
//!   lpoco NAME args...            run lpoco-NAME with args, exit with its exit code
//!   lpoco list                    the lpoco-* programs found, with their versions
//!   lpoco completions SHELL       shell completions (bash, zsh, fish, nushell)
//!   lpoco man                     the manual page, in roff
//!   lpoco --version               this program's version (the family's)

use argenv_resolve::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    List,
    Completions,
    Man,
}

impl Command {
    pub const TOKENS: &'static [&'static str] = &["list", "completions", "man"];
}

impl FromRaw for Command {
    fn from_raw(s: &str) -> Option<Command> {
        Some(match s.trim() {
            "list" => Command::List,
            "completions" => Command::Completions,
            "man" => Command::Man,
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

    VERSION: bool = Input {
        key: "version",
        ty: Type::Bool,
        default: Some(false),
        arg: Some(Arg::long("version")),
        summary: "Print the version (one for the whole family) and stop.",
        ..Input::EMPTY
    };
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
