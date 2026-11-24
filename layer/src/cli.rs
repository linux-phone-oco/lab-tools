// SPDX-License-Identifier: MIT OR Apache-2.0
//! lpoco-layer's invocation surface, declared once with argenv: its command, its arguments and the environment
//! variables it reads. Run as `lpoco layer ...`.
//!
//!   lpoco-layer pack --manifest FILE --format apk|deb|rpm|archlinux --out DIR [--kernel-build DIR]   one package per piece of the layer
//!   lpoco-layer --version                                                       this program's version (the family's)

use argenv_resolve::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    Pack,
}

impl Command {
    pub const TOKENS: &'static [&'static str] = &["pack"];
}

impl FromRaw for Command {
    fn from_raw(s: &str) -> Option<Command> {
        Some(match s.trim() {
            "pack" => Command::Pack,
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
        summary: "What to do: pack the layer.",
        ..Input::EMPTY
    };

    MANIFEST: String = Input {
        key: "manifest",
        ty: Type::String,
        arg: Some(Arg { value_name: "FILE", ..Arg::long("manifest") }),
        env: Some(Env::new("LPOCO_MANIFEST")),
        summary: "The device's device.toml, whose [layer] is packed (device.toml in the current folder when none is given).",
        ..Input::EMPTY
    };

    FORMAT: String = Input {
        key: "format",
        ty: Type::Enum,
        allowed: &["apk", "deb", "rpm", "archlinux"],
        arg: Some(Arg { value_name: "FORMAT", ..Arg::long("format") }),
        summary: "The package format.",
        ..Input::EMPTY
    };

    OUT: String = Input {
        key: "out",
        ty: Type::String,
        arg: Some(Arg { value_name: "DIR", ..Arg::long("out") }),
        summary: "Where the packages and their nfpm configs are written (the current folder when none is given).",
        ..Input::EMPTY
    };

    PKG_VERSION: String = Input {
        key: "pkg_version",
        ty: Type::String,
        arg: Some(Arg { value_name: "VERSION", ..Arg::long("pkg-version") }),
        env: Some(Env::new("LPOCO_LAYER_VERSION")),
        summary: "The packages' version (0.1.0 when none is given: the layer declares none yet).",
        ..Input::EMPTY
    };

    KERNEL_BUILD: String = Input {
        key: "kernel_build",
        ty: Type::String,
        arg: Some(Arg { value_name: "DIR", ..Arg::long("kernel-build") }),
        env: Some(Env::new("LPOCO_KERNEL_BUILD")),
        summary: "A finished kernel build (Image, kernel.release, dtbs/, optionally modules/): also packs it as linux-NAME for arm64 (none given: no kernel package).",
        ..Input::EMPTY
    };

    NFPM: String = Input {
        key: "nfpm",
        ty: Type::String,
        env: Some(Env::new("LPOCO_NFPM")),
        summary: "The nfpm program that packs each config (nfpm, from PATH, when none is given).",
        ..Input::EMPTY
    };

    SOURCE_DATE_EPOCH: String = Input {
        key: "source_date_epoch",
        ty: Type::String,
        env: Some(Env::new("SOURCE_DATE_EPOCH")),
        summary: "The time nfpm stamps into the packages, in seconds since the epoch (the time of the last commit that touched the layer's folder when none is given, so the same committed layer gives the same bytes; uncommitted changes are packed under that same time).",
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

/// The usage `--help` prints, read from the declaration above.
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
