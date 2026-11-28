// SPDX-License-Identifier: MIT OR Apache-2.0
//! osfact's invocation surface, declared once with argenv.
//!
//!   osfact install now --plan FILE --dry-run     list the commands of an install.toml (fastboot boot, the way back); changes nothing
//!   osfact rootfs now --distro FILE --arch ARCH --out DIR [--size-mib N] [--dry-run]   a rootless root tree and its ext4 image, built twice and compared
//!   osfact pack now --device FILE --distro FILE --parts DIR --out DIR [--size-mib N] [--dry-run]   the root image, the boot image and install.toml of one device and one distro

use argenv_resolve::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    Install,
    Rootfs,
    Pack,
}

impl FromRaw for Command {
    fn from_raw(s: &str) -> Option<Command> {
        match s.trim() {
            "install" => Some(Command::Install),
            "rootfs" => Some(Command::Rootfs),
            "pack" => Some(Command::Pack),
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
        allowed: &["install", "rootfs", "pack"],
        arg: Some(Arg::positional(0)),
        summary: "What to do (install: put an image on the device; rootfs: make the root file system image of a distro profile).",
        ..Input::EMPTY
    };

    VERB: Verb, Command = Input {
        key: "verb",
        ty: Type::Enum,
        allowed: &["now"],
        subcommands: &[Command::Install, Command::Rootfs, Command::Pack],
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

    DISTRO: String, Command = Input {
        key: "distro",
        ty: Type::String,
        required: true,
        subcommands: &[Command::Rootfs, Command::Pack],
        arg: Some(Arg { value_name: "FILE", ..Arg::long("distro") }),
        summary: "rootfs, pack: the distro profile (its package format, repositories and packages).",
        ..Input::EMPTY
    };

    ARCH: String, Command = Input {
        key: "arch",
        ty: Type::String,
        required: true,
        subcommands: &[Command::Rootfs],
        arg: Some(Arg { value_name: "ARCH", ..Arg::long("arch") }),
        summary: "rootfs: the machine kind of the image's packages (the device's, e.g. aarch64).",
        ..Input::EMPTY
    };

    OUT: String, Command = Input {
        key: "out",
        ty: Type::String,
        required: true,
        subcommands: &[Command::Rootfs, Command::Pack],
        arg: Some(Arg { value_name: "DIR", ..Arg::long("out") }),
        summary: "rootfs, pack: the folder that gets the images (and the work of the build).",
        ..Input::EMPTY
    };

    SIZE_MIB: u64, Command = Input {
        key: "size_mib",
        ty: Type::Uint,
        subcommands: &[Command::Rootfs, Command::Pack],
        default: Some(512),
        arg: Some(Arg { value_name: "N", ..Arg::long("size-mib") }),
        summary: "rootfs, pack: the root image's size in MiB (default 512, sparse).",
        ..Input::EMPTY
    };

    DEVICE: String, Command = Input {
        key: "device",
        ty: Type::String,
        required: true,
        subcommands: &[Command::Pack],
        arg: Some(Arg { value_name: "FILE", ..Arg::long("device") }),
        summary: "pack: the device's declaration (its [device], [boot] and the way its images are installed).",
        ..Input::EMPTY
    };

    PARTS: String, Command = Input {
        key: "parts",
        ty: Type::String,
        required: true,
        subcommands: &[Command::Pack],
        arg: Some(Arg { value_name: "DIR", ..Arg::long("parts") }),
        summary: "pack: the folder of the proven parts (kernel, ramdisk and the declaration's dtb path).",
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
        subcommands: &[Command::Install, Command::Rootfs, Command::Pack],
        arg: Some(Arg::long("dry-run")),
        summary: "install, rootfs, pack: only list the commands; nothing is run or written.",
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
