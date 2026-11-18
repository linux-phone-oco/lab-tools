//! lpoco-bringup's invocation surface, declared once with argenv: its commands, their arguments and the
//! environment variables it reads. Run as `lpoco bringup ...`.
//!
//!   lpoco-bringup image info IMG                 the header's facts, as a [boot] table
//!   lpoco-bringup image unpack IMG --out DIR     kernel, ramdisk, second, appended.dtb and boot.toml written to DIR
//!   lpoco-bringup image pack --kernel K --ramdisk R --out IMG [--dtb DTB] [--device device.toml]
//!                                                a header-v0 image, byte for byte as mkbootimg writes it
//!   lpoco-bringup image check IMG [--device device.toml]
//!                                                count the image against the manifest's [boot]; exit 1 on a miss
//!   lpoco-bringup image swap CONTROL --out IMG [--ramdisk R] [--kernel K] [--dtb DTB] [--cmdline TEXT]
//!                                                the control with those parts replaced, its AVB hash footer rebuilt (avbtool),
//!                                                and what differs said
//!   lpoco-bringup image bisect SERIES --out DIR
//!                                                the images a series file declares (control = "...", [[image]] name, kernel, dtb, ramdisk,
//!                                                cmdline, lz4, footer), each the control with exactly those parts swapped, counted
//!   lpoco-bringup initramfs build --spec FILE --out FILE [--root DIR] [--net N] [--codec gzip|xz]
//!                                                a reproducible newc cpio from a gen_init_cpio spec, compressed (replaces build-initramfs.sh)
//!   lpoco-bringup initramfs init [--device device.toml] [--out FILE] [--net N]
//!                                                the bring-up /init made from the device's [boot.gadget] (configfs gadget, DHCP, telnet, ACM shell)
//!   lpoco-bringup initramfs files --out DIR [--net N]
//!                                                the files the init reads beside itself: udhcpd.conf, udhcpc.script, selftest.sh
//!   lpoco-bringup --version                      this program's version (the lpoco family's one version)

use argenv_resolve::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    Image,
    Initramfs,
}

impl Command {
    pub const TOKENS: &'static [&'static str] = &["image", "initramfs"];
}

impl FromRaw for Command {
    fn from_raw(s: &str) -> Option<Command> {
        Some(match s.trim() {
            "image" => Command::Image,
            "initramfs" => Command::Initramfs,
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
    Swap,
    Bisect,
    Build,
    Init,
    Files,
}

impl Verb {
    pub const TOKENS: &'static [&'static str] = &[
        "info", "unpack", "pack", "check", "swap", "bisect", "build", "init", "files",
    ];
}

impl FromRaw for Verb {
    fn from_raw(s: &str) -> Option<Verb> {
        Some(match s.trim() {
            "info" => Verb::Info,
            "unpack" => Verb::Unpack,
            "pack" => Verb::Pack,
            "check" => Verb::Check,
            "swap" => Verb::Swap,
            "bisect" => Verb::Bisect,
            "build" => Verb::Build,
            "init" => Verb::Init,
            "files" => Verb::Files,
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
        summary: "What to work on (image: boot images; initramfs: the cpio the phone boots).",
        ..Input::EMPTY
    };

    VERB: Verb, Command = Input {
        key: "verb",
        ty: Type::Enum,
        allowed: Verb::TOKENS,
        subcommands: &[Command::Image, Command::Initramfs],
        arg: Some(Arg::positional(0)),
        summary: "What to do: image info, unpack, pack, check, swap or bisect; initramfs build, init or files.",
        ..Input::EMPTY
    };

    IMAGE: String, Command = Input {
        key: "image",
        ty: Type::String,
        subcommands: &[Command::Image],
        arg: Some(Arg { value_name: "IMG", ..Arg::positional(1) }),
        summary: "The boot image to read (info, unpack, check, swap: the control; bisect: the series file).",
        ..Input::EMPTY
    };

    DEVICE: String, Command = Input {
        key: "device",
        ty: Type::String,
        subcommands: &[Command::Image, Command::Initramfs],
        arg: Some(Arg { value_name: "FILE", ..Arg::long("device") }),
        env: Some(Env::new("LPOCO_DEVICE")),
        summary: "The device manifest whose [boot] table says the header facts (initramfs init: its [boot.gadget]) (device.toml in the current folder when none is given).",
        ..Input::EMPTY
    };

    KERNEL: String, Command = Input {
        key: "kernel",
        ty: Type::String,
        subcommands: &[Command::Image],
        arg: Some(Arg { value_name: "FILE", ..Arg::long("kernel") }),
        summary: "pack, swap: the kernel.",
        ..Input::EMPTY
    };

    DTB: String, Command = Input {
        key: "dtb",
        ty: Type::String,
        subcommands: &[Command::Image],
        arg: Some(Arg { value_name: "FILE", ..Arg::long("dtb") }),
        summary: "pack, swap: a device tree appended to the kernel.",
        ..Input::EMPTY
    };

    RAMDISK: String, Command = Input {
        key: "ramdisk",
        ty: Type::String,
        subcommands: &[Command::Image],
        arg: Some(Arg { value_name: "FILE", ..Arg::long("ramdisk") }),
        summary: "pack, swap: the ramdisk.",
        ..Input::EMPTY
    };

    CMDLINE: String, Command = Input {
        key: "cmdline",
        ty: Type::String,
        subcommands: &[Command::Image],
        arg: Some(Arg { value_name: "TEXT", ..Arg::long("cmdline") }),
        summary: "swap: the kernel command line to put in the header.",
        ..Input::EMPTY
    };

    OUT: String, Command = Input {
        key: "out",
        ty: Type::String,
        subcommands: &[Command::Image, Command::Initramfs],
        arg: Some(Arg { value_name: "PATH", ..Arg::long("out") }),
        summary: "pack, swap: the image to write; bisect: the folder the images are written to; unpack: the folder to write the parts to; initramfs build: the archive to write; initramfs init: the init script to write (standard output when none is given); initramfs files: the folder to write the files to.",
        ..Input::EMPTY
    };

    SPEC: String, Command = Input {
        key: "spec",
        ty: Type::String,
        subcommands: &[Command::Initramfs],
        arg: Some(Arg { value_name: "FILE", ..Arg::long("spec") }),
        summary: "initramfs build: the gen_init_cpio spec (dir, nod, file, slink lines; files named beside it).",
        ..Input::EMPTY
    };

    ROOT: String, Command = Input {
        key: "root",
        ty: Type::String,
        subcommands: &[Command::Initramfs],
        arg: Some(Arg { value_name: "DIR", ..Arg::long("root") }),
        summary: "initramfs build: the folder the spec's files are named in (the spec's own folder when none is given).",
        ..Input::EMPTY
    };

    NET: String, Command = Input {
        key: "net",
        ty: Type::String,
        subcommands: &[Command::Initramfs],
        arg: Some(Arg { value_name: "N", ..Arg::long("net") }),
        env: Some(Env::new("LPOCO_NET")),
        summary: "initramfs build, init: the gadget address 172.16.N.1 and the ECM MAC byte (42 when none is given; the files are written at 42).",
        ..Input::EMPTY
    };

    CODEC: String, Command = Input {
        key: "codec",
        ty: Type::String,
        subcommands: &[Command::Initramfs],
        arg: Some(Arg { value_name: "gzip|xz", ..Arg::long("codec") }),
        summary: "initramfs build: the compression the device's kernel reads (xz for the 4.4 kernel; gzip when none is given).",
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
