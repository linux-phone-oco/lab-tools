//! lpoco-bringup's invocation surface, declared once with argenv: its commands, their arguments and the
//! environment variables it reads. Run as `lpoco bringup ...`.
//!
//!   lpoco-bringup image info IMG                 the header's facts, as a [boot] table
//!   lpoco-bringup image unpack IMG --out DIR     kernel, ramdisk, second, appended.dtb and boot.toml written to DIR
//!   lpoco-bringup image pack --kernel K --ramdisk R --out IMG [--dtb DTB] [--device device.toml]
//!                                                a header-v0 image, byte for byte as mkbootimg writes it
//!   lpoco-bringup image check IMG [--device device.toml]
//!                                                count the image against the manifest's [boot]; exit 1 on a miss
//!   lpoco-bringup --version                      this program's version (the lpoco family's one version)

use argenv_resolve::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    Image,
}

impl Command {
    pub const TOKENS: &'static [&'static str] = &["image"];
}

impl FromRaw for Command {
    fn from_raw(s: &str) -> Option<Command> {
        Some(match s.trim() {
            "image" => Command::Image,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verb {
    Info,
    Unpack,
    Pack,
    Check,
}

impl Verb {
    pub const TOKENS: &'static [&'static str] = &["info", "unpack", "pack", "check"];
}

impl FromRaw for Verb {
    fn from_raw(s: &str) -> Option<Verb> {
        Some(match s.trim() {
            "info" => Verb::Info,
            "unpack" => Verb::Unpack,
            "pack" => Verb::Pack,
            "check" => Verb::Check,
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
        summary: "What to work on (image: boot images).",
        ..Input::EMPTY
    };

    VERB: Verb, Command = Input {
        key: "verb",
        ty: Type::Enum,
        allowed: Verb::TOKENS,
        subcommands: &[Command::Image],
        arg: Some(Arg::positional(0)),
        summary: "What to do with the image: info, unpack, pack or check.",
        ..Input::EMPTY
    };

    IMAGE: String, Command = Input {
        key: "image",
        ty: Type::String,
        subcommands: &[Command::Image],
        arg: Some(Arg { value_name: "IMG", ..Arg::positional(1) }),
        summary: "The boot image to read (info, unpack, check).",
        ..Input::EMPTY
    };

    DEVICE: String, Command = Input {
        key: "device",
        ty: Type::String,
        subcommands: &[Command::Image],
        arg: Some(Arg { value_name: "FILE", ..Arg::long("device") }),
        env: Some(Env::new("LPOCO_DEVICE")),
        summary: "The device manifest whose [boot] table says the header facts (device.toml in the current folder when none is given).",
        ..Input::EMPTY
    };

    KERNEL: String, Command = Input {
        key: "kernel",
        ty: Type::String,
        subcommands: &[Command::Image],
        arg: Some(Arg { value_name: "FILE", ..Arg::long("kernel") }),
        summary: "pack: the kernel.",
        ..Input::EMPTY
    };

    DTB: String, Command = Input {
        key: "dtb",
        ty: Type::String,
        subcommands: &[Command::Image],
        arg: Some(Arg { value_name: "FILE", ..Arg::long("dtb") }),
        summary: "pack: a device tree appended to the kernel.",
        ..Input::EMPTY
    };

    RAMDISK: String, Command = Input {
        key: "ramdisk",
        ty: Type::String,
        subcommands: &[Command::Image],
        arg: Some(Arg { value_name: "FILE", ..Arg::long("ramdisk") }),
        summary: "pack: the ramdisk.",
        ..Input::EMPTY
    };

    OUT: String, Command = Input {
        key: "out",
        ty: Type::String,
        subcommands: &[Command::Image],
        arg: Some(Arg { value_name: "PATH", ..Arg::long("out") }),
        summary: "pack: the image to write; unpack: the folder to write the parts to.",
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
    fn check_takes_an_image_and_the_manifest_from_the_environment() {
        let r = resolve(
            &["image", "check", "boot.img"],
            &[("LPOCO_DEVICE", "w.toml")],
        );
        assert_eq!(Contract::COMMAND.get_from(&r), Some(Command::Image));
        assert_eq!(Contract::VERB.get_from(&r), Some(Verb::Check));
        assert_eq!(Contract::IMAGE.get_from(&r).as_deref(), Some("boot.img"));
        assert_eq!(Contract::DEVICE.get_from(&r).as_deref(), Some("w.toml"));
    }
}
