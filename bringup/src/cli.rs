// SPDX-License-Identifier: MIT OR Apache-2.0
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
//!   lpoco-bringup image ensure NAME [--device device.toml] [--cache DIR] [--seed DIR] [--source DIR]
//!                                                the image [images.NAME] by its sha256: its Nix store path, from the store, else the file:// cache, else the seed, else built from its [images.NAME.recipe] in Nix's sandbox (the build is held to the declared sha256 by Nix)
//!   lpoco-bringup image bisect SERIES --out DIR
//!                                                the images a series file declares (control = "...", [[image]] name, kernel, dtb, ramdisk,
//!                                                cmdline, lz4, footer), each the control with exactly those parts swapped, counted
//!   lpoco-bringup initramfs build --spec FILE --out FILE [--root DIR] [--net N] [--codec gzip|xz]
//!                                                a reproducible newc cpio from a gen_init_cpio spec, compressed (replaces build-initramfs.sh)
//!   lpoco-bringup initramfs init [--device device.toml] [--out FILE] [--net N]
//!                                                the bring-up /init made from the device's [boot.gadget] (configfs gadget, DHCP, telnet, ACM shell)
//!   lpoco-bringup initramfs files --out DIR [--net N]
//!                                                the files the init reads beside itself: udhcpd.conf, udhcpc.script, selftest.sh
//!   lpoco-bringup reference capture --via COMMAND --out DIR
//!                                                a snapshot of what the stock OS does (device tree, gpio, pinctrl, regulators, clocks, interrupts, drivers) taken over adb or ssh
//!   lpoco-bringup reference diff PART --live DIR --ours DIR [--match WORDS]
//!                                                every difference between two snapshots for one part, named; exit 1 on a difference, 2 when the part is not known yet
//!   lpoco-bringup kernel assemble LIST --lists FILE --repo DIR --records DIR [--reproduces COMMIT] [--rhizoid PROGRAM] [--tad PROGRAM]
//!                                                the record checks of one list (every topic has an upstream-item row, every carried subject its item's tag; exit 1 on a miss), then rhizoid assemble makes int/DEVICE/LIST/BASE/N
//!   lpoco-bringup kernel publish TAG --repo DIR [--device device.toml] [--rhizoid PROGRAM]
//!                                                once an [images.NAME] kernel_tag names TAG: push the tag, end rhizoid's worktree for it (--force)
//!   lpoco-bringup --version                      this program's version (the lpoco family's one version)

use argenv_resolve::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    Image,
    Initramfs,
    Reference,
    #[serde(rename = "dt-check")]
    DtCheck,
    Kernel,
}

impl Command {
    pub const TOKENS: &'static [&'static str] =
        &["image", "initramfs", "reference", "dt-check", "kernel"];
}

impl FromRaw for Command {
    fn from_raw(s: &str) -> Option<Command> {
        Some(match s.trim() {
            "image" => Command::Image,
            "initramfs" => Command::Initramfs,
            "reference" => Command::Reference,
            "dt-check" => Command::DtCheck,
            "kernel" => Command::Kernel,
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
    Ensure,
    Build,
    Init,
    Files,
    Capture,
    Diff,
    Binding,
    Dtbs,
    Assemble,
    Publish,
}

impl Verb {
    pub const TOKENS: &'static [&'static str] = &[
        "info", "unpack", "pack", "check", "swap", "bisect", "ensure", "build", "init", "files",
        "capture", "diff", "binding", "dtbs", "assemble", "publish",
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
            "ensure" => Verb::Ensure,
            "build" => Verb::Build,
            "init" => Verb::Init,
            "files" => Verb::Files,
            "capture" => Verb::Capture,
            "diff" => Verb::Diff,
            "binding" => Verb::Binding,
            "dtbs" => Verb::Dtbs,
            "assemble" => Verb::Assemble,
            "publish" => Verb::Publish,
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
        subcommands: &[Command::Image, Command::Initramfs, Command::Reference, Command::DtCheck, Command::Kernel],
        arg: Some(Arg::positional(0)),
        summary: "What to do: image info, unpack, pack, check, swap, bisect or ensure; initramfs build, init or files; reference capture or diff; dt-check binding or dtbs; kernel assemble or publish.",
        ..Input::EMPTY
    };

    IMAGE: String, Command = Input {
        key: "image",
        ty: Type::String,
        subcommands: &[Command::Image],
        arg: Some(Arg { value_name: "IMG", ..Arg::positional(1) }),
        summary: "The boot image to read (info, unpack, check, swap: the control; bisect: the series file; ensure: the name of an [images.NAME]).",
        ..Input::EMPTY
    };

    DEVICE: String, Command = Input {
        key: "device",
        ty: Type::String,
        subcommands: &[Command::Image, Command::Initramfs, Command::Kernel],
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
        subcommands: &[Command::Image, Command::Initramfs, Command::Reference, Command::DtCheck],
        arg: Some(Arg { value_name: "PATH", ..Arg::long("out") }),
        summary: "pack, swap: the image to write; bisect: the folder the images are written to; unpack: the folder to write the parts to; initramfs build: the archive to write; initramfs init: the init script to write (standard output when none is given); initramfs files: the folder to write the files to; reference capture: the snapshot folder to write; dt-check: the scratch folder the container writes to (O=).",
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

    PART: String, Command = Input {
        key: "part",
        ty: Type::String,
        subcommands: &[Command::Reference],
        arg: Some(Arg { value_name: "PART", ..Arg::positional(1) }),
        summary: "reference diff: the part to compare (ufs, touch ...); its lines are those whose node, or text line, hold the part name or a --match word.",
        ..Input::EMPTY
    };

    MATCH: String, Command = Input {
        key: "match",
        ty: Type::String,
        subcommands: &[Command::Reference],
        arg: Some(Arg { value_name: "WORDS", ..Arg::long("match") }),
        summary: "reference diff: comma-separated words (case-insensitive) that say which lines belong to the part, in place of the part name.",
        ..Input::EMPTY
    };

    LIVE: String, Command = Input {
        key: "live",
        ty: Type::String,
        subcommands: &[Command::Reference],
        arg: Some(Arg { value_name: "DIR", ..Arg::long("live") }),
        summary: "reference diff: the snapshot of the stock OS (a folder made by reference capture).",
        ..Input::EMPTY
    };

    OURS: String, Command = Input {
        key: "ours",
        ty: Type::String,
        subcommands: &[Command::Reference],
        arg: Some(Arg { value_name: "DIR", ..Arg::long("ours") }),
        summary: "reference diff: the snapshot of our mainline boot.",
        ..Input::EMPTY
    };

    VIA: String, Command = Input {
        key: "via",
        ty: Type::String,
        subcommands: &[Command::Reference],
        arg: Some(Arg { value_name: "COMMAND", ..Arg::long("via") }),
        env: Some(Env::new("LPOCO_REFERENCE_VIA")),
        summary: "reference capture: the transport to a phone with a root shell, run with the capture script on its standard input (adb shell su 0 sh; ssh phone sudo sh).",
        ..Input::EMPTY
    };

    SRC: String, Command = Input {
        key: "src",
        ty: Type::String,
        subcommands: &[Command::DtCheck],
        arg: Some(Arg { value_name: "SRC", ..Arg::positional(1) }),
        summary: "dt-check: the kernel source folder, mounted read-only in the container.",
        ..Input::EMPTY
    };

    DT_IMAGE: String, Command = Input {
        key: "dt_image",
        ty: Type::String,
        subcommands: &[Command::DtCheck],
        arg: Some(Arg { value_name: "NAME@sha256:DIGEST", ..Arg::long("image") }),
        env: Some(Env::new("LPOCO_DT_IMAGE")),
        summary: "dt-check: the container image, by digest and never a tag (alpine:edge pinned, with dtschema from py3-dt-schema).",
        ..Input::EMPTY
    };

    SCHEMA_FILES: String, Command = Input {
        key: "schema_files",
        ty: Type::String,
        subcommands: &[Command::DtCheck],
        arg: Some(Arg { value_name: "FILES", ..Arg::long("schema-files") }),
        summary: "dt-check binding: DT_SCHEMA_FILES, the binding schemas to check (all of them when none is given).",
        ..Input::EMPTY
    };

    DTBS: String, Command = Input {
        key: "dtbs",
        ty: Type::String,
        subcommands: &[Command::DtCheck],
        arg: Some(Arg { value_name: "A.dtb,B.dtb", ..Arg::long("dtbs") }),
        summary: "dt-check dtbs: the comma-separated dtbs to build and check, as make names them (qcom/sm7125-xiaomi-joyeuse.dtb).",
        ..Input::EMPTY
    };

    ARCH: String, Command = Input {
        key: "arch",
        ty: Type::String,
        subcommands: &[Command::DtCheck],
        arg: Some(Arg { value_name: "ARCH", ..Arg::long("arch") }),
        summary: "dt-check: the kernel ARCH (arm64 when none is given).",
        ..Input::EMPTY
    };

    DOCKER: String, Command = Input {
        key: "docker",
        ty: Type::String,
        subcommands: &[Command::DtCheck],
        arg: Some(Arg { value_name: "PROGRAM", ..Arg::long("docker") }),
        env: Some(Env::new("LPOCO_DOCKER")),
        summary: "dt-check: the container program (docker when none is given).",
        ..Input::EMPTY
    };

    LIST: String, Command = Input {
        key: "list",
        ty: Type::String,
        subcommands: &[Command::Kernel],
        arg: Some(Arg { value_name: "LIST", ..Arg::positional(1) }),
        summary: "kernel assemble: the named list of the lists file; kernel publish: the int/ tag an image names.",
        ..Input::EMPTY
    };

    LISTS: String, Command = Input {
        key: "lists",
        ty: Type::String,
        subcommands: &[Command::Kernel],
        arg: Some(Arg { value_name: "FILE", ..Arg::long("lists") }),
        env: Some(Env::new("LPOCO_KERNEL_LISTS")),
        summary: "kernel assemble: the lab's kernel/topics.toml, in rhizoid's list format.",
        ..Input::EMPTY
    };

    REPO: String, Command = Input {
        key: "repo",
        ty: Type::String,
        subcommands: &[Command::Kernel],
        arg: Some(Arg { value_name: "DIR", ..Arg::long("repo") }),
        env: Some(Env::new("LPOCO_KERNEL_REPO")),
        summary: "kernel: the lab clone of the kernel repository.",
        ..Input::EMPTY
    };

    RECORDS: String, Command = Input {
        key: "records",
        ty: Type::String,
        subcommands: &[Command::Kernel],
        arg: Some(Arg { value_name: "DIR", ..Arg::long("records") }),
        env: Some(Env::new("LPOCO_RECORDS")),
        summary: "kernel assemble: the base whose upstream-item rows the topics are checked against (tad q carried runs there).",
        ..Input::EMPTY
    };

    RHIZOID: String, Command = Input {
        key: "rhizoid",
        ty: Type::String,
        subcommands: &[Command::Kernel],
        arg: Some(Arg { value_name: "PROGRAM", ..Arg::long("rhizoid") }),
        env: Some(Env::new("LPOCO_RHIZOID")),
        summary: "kernel: git-rhizoid's program (rhizoid on PATH when none is given).",
        ..Input::EMPTY
    };

    REPRODUCES: String, Command = Input {
        key: "reproduces",
        ty: Type::String,
        subcommands: &[Command::Kernel],
        arg: Some(Arg { value_name: "COMMIT", ..Arg::long("reproduces") }),
        env: Some(Env::new("LPOCO_KERNEL_REPRODUCES")),
        summary: "kernel assemble: a commit whose tree the tag must equal (rhizoid assemble --reproduces); its equal or differs verdict is printed and a difference ends 1.",
        ..Input::EMPTY
    };

    TAD: String, Command = Input {
        key: "tad",
        ty: Type::String,
        subcommands: &[Command::Kernel],
        arg: Some(Arg { value_name: "PROGRAM", ..Arg::long("tad") }),
        env: Some(Env::new("LPOCO_TAD")),
        summary: "kernel assemble: the tad program (tad on PATH when none is given).",
        ..Input::EMPTY
    };

    CACHE: String, Command = Input {
        key: "cache",
        ty: Type::String,
        subcommands: &[Command::Image],
        arg: Some(Arg { value_name: "DIR", ..Arg::long("cache") }),
        env: Some(Env::new("LPOCO_IMAGE_CACHE")),
        summary: "ensure: the folder that is the lab's file:// binary cache of images (what is built or seeded is copied to it; what the store lacks is taken from it).",
        ..Input::EMPTY
    };

    SEED: String, Command = Input {
        key: "seed",
        ty: Type::String,
        subcommands: &[Command::Image],
        arg: Some(Arg { value_name: "DIR", ..Arg::long("seed") }),
        env: Some(Env::new("LPOCO_IMAGE_SEED")),
        summary: "ensure: a folder of files; one whose sha256 is a wanted one (an image, or a part of a recipe) is taken into the store - how a hand-built image enters once.",
        ..Input::EMPTY
    };

    SOURCE: String, Command = Input {
        key: "source",
        ty: Type::String,
        subcommands: &[Command::Image],
        arg: Some(Arg { value_name: "DIR", ..Arg::long("source") }),
        env: Some(Env::new("LPOCO_SOURCE")),
        summary: "ensure: the lab-tools source the sandbox builds lpoco-bringup from (this version's release tag from GitHub when none is given).",
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
