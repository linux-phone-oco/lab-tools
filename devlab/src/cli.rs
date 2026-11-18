//! lpoco-lab's invocation surface, declared once with argenv: its commands, their arguments and the
//! environment variables it reads (backlog lpoco-lab-argenv; devlab is the same program under its old
//! name). The words are the ones clap gave it; the dispatcher in main.rs builds its commands from this.
//!
//! argenv reads one level of subcommand: the first word is the command (`phone`, `kernel`, ...), and
//! for a command with actions the second word is its action, the rest positionals scoped to the
//! command. `TREE` says which action takes which of the declared inputs, and is what `--help` prints
//! and the dispatcher checks the count of arguments against; a test checks every name in it is declared.

use argenv_resolve::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    Phone,
    Flash,
    Provision,
    Test,
    Hear,
    Health,
    Gather,
    Suite,
    Mcp,
    Watch,
    Fastboot,
    Keeper,
    Guard,
    Kernel,
}

impl Command {
    pub const TOKENS: &'static [&'static str] = &[
        "phone",
        "flash",
        "provision",
        "test",
        "hear",
        "health",
        "gather",
        "suite",
        "mcp",
        "watch",
        "fastboot",
        "keeper",
        "guard",
        "kernel",
    ];
}

impl FromRaw for Command {
    fn from_raw(s: &str) -> Option<Command> {
        Some(match s.trim() {
            "phone" => Command::Phone,
            "flash" => Command::Flash,
            "provision" => Command::Provision,
            "test" => Command::Test,
            "hear" => Command::Hear,
            "health" => Command::Health,
            "gather" => Command::Gather,
            "suite" => Command::Suite,
            "mcp" => Command::Mcp,
            "watch" => Command::Watch,
            "fastboot" => Command::Fastboot,
            "keeper" => Command::Keeper,
            "guard" => Command::Guard,
            "kernel" => Command::Kernel,
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
        summary: "What to do.",
        ..Input::EMPTY
    };

    // --- the words after the command or its action ---

    NOW: String, Command = Input {
        key: "now",
        ty: Type::String,
        subcommands: &[Command::Flash, Command::Provision, Command::Gather],
        arg: Some(Arg { value_name: "NOW", ..Arg::positional(0) }),
        summary: "The word `now`: it changes the phone or this lab, so it is said.",
        ..Input::EMPTY
    };

    IMAGE: String, Command = Input {
        key: "image",
        ty: Type::String,
        subcommands: &[Command::Flash],
        arg: Some(Arg { value_name: "IMAGE", ..Arg::positional(1) }),
        summary: "The image to flash.",
        ..Input::EMPTY
    };

    BOOTLOADER_FILE: String, Command = Input {
        key: "bootloader_file",
        ty: Type::String,
        subcommands: &[Command::Flash],
        arg: Some(Arg { value_name: "BOOTLOADER", ..Arg::positional(2) }),
        summary: "The bootloader image.",
        ..Input::EMPTY
    };

    ACTION_NOW: String, Command = Input {
        key: "action_now",
        ty: Type::String,
        subcommands: &[Command::Suite, Command::Fastboot],
        arg: Some(Arg { value_name: "NOW", ..Arg::positional(1) }),
        summary: "The word `now`: it changes the phone, so it is said.",
        ..Input::EMPTY
    };

    KERNEL_NOW: String, Command = Input {
        key: "kernel_now",
        ty: Type::String,
        subcommands: &[Command::Kernel],
        arg: Some(Arg { value_name: "NOW", ..Arg::positional(2) }),
        summary: "The word `now`: it changes which kernel the phone boots.",
        ..Input::EMPTY
    };

    NAME: String, Command = Input {
        key: "name",
        ty: Type::String,
        subcommands: &[Command::Kernel, Command::Keeper, Command::Fastboot],
        arg: Some(Arg { value_name: "NAME", ..Arg::positional(1) }),
        summary: "The kernel entry's name, the keeper's, or the bootloader variable's.",
        ..Input::EMPTY
    };

    VMLINUZ: String, Command = Input {
        key: "vmlinuz",
        ty: Type::String,
        subcommands: &[Command::Kernel],
        arg: Some(Arg { value_name: "VMLINUZ", ..Arg::positional(2) }),
        summary: "The kernel image (EFI) on the lab machine.",
        ..Input::EMPTY
    };

    DTB_FILE: String, Command = Input {
        key: "dtb_file",
        ty: Type::String,
        subcommands: &[Command::Kernel],
        arg: Some(Arg { value_name: "DTB", ..Arg::positional(3) }),
        summary: "The device tree on the lab machine.",
        ..Input::EMPTY
    };

    MODULES: String, Command = Input {
        key: "modules",
        ty: Type::String,
        subcommands: &[Command::Kernel],
        arg: Some(Arg { value_name: "MODULES", ..Arg::positional(4) }),
        summary: "A tar.gz holding lib/modules/RELEASE.",
        ..Input::EMPTY
    };

    LOCAL: String, Command = Input {
        key: "local",
        ty: Type::String,
        subcommands: &[Command::Phone],
        arg: Some(Arg { value_name: "LOCAL", ..Arg::positional(1) }),
        summary: "A file on the lab machine.",
        ..Input::EMPTY
    };

    REMOTE: String, Command = Input {
        key: "remote",
        ty: Type::String,
        subcommands: &[Command::Phone],
        arg: Some(Arg { value_name: "REMOTE", ..Arg::positional(2) }),
        summary: "Where it goes on the phone.",
        ..Input::EMPTY
    };

    TEST: String, Command = Input {
        key: "test",
        ty: Type::String,
        subcommands: &[Command::Suite],
        arg: Some(Arg { value_name: "TEST", ..Arg::positional(1) }),
        summary: "The phone's test to switch (suite list says them).",
        ..Input::EMPTY
    };

    FB_IMAGE: String, Command = Input {
        key: "fb_image",
        ty: Type::String,
        subcommands: &[Command::Fastboot],
        arg: Some(Arg { value_name: "IMAGE", ..Arg::positional(2) }),
        summary: "The Android boot image to boot from memory.",
        ..Input::EMPTY
    };

    SLOT: String, Command = Input {
        key: "slot",
        ty: Type::String,
        subcommands: &[Command::Fastboot],
        arg: Some(Arg { value_name: "SLOT", ..Arg::positional(2) }),
        summary: "The slot to make active: a or b.",
        ..Input::EMPTY
    };

    // --- flags ---

    DEVICE: String = Input {
        key: "device",
        ty: Type::String,
        arg: Some(Arg { value_name: "DEVICE", ..Arg::long("device") }),
        summary: "The device manifest (default: device.toml here or in a folder above).",
        ..Input::EMPTY
    };

    DTB: String, Command = Input {
        key: "dtb",
        ty: Type::String,
        subcommands: &[Command::Flash, Command::Kernel],
        arg: Some(Arg { value_name: "DTB", ..Arg::long("dtb") }),
        summary: "A device tree to put in the image's place for it (flash), or to boot NAME's kernel and modules with, once, as the entry NAME-dt (kernel try).",
        ..Input::EMPTY
    };

    AS_IS: bool, Command = Input {
        key: "as_is",
        ty: Type::Bool,
        subcommands: &[Command::Flash],
        default: Some(false),
        arg: Some(Arg::long("as-is")),
        summary: "Bake in nothing but access - to tell an image's fault from ours.",
        ..Input::EMPTY
    };

    PLACED: bool, Command = Input {
        key: "placed",
        ty: Type::Bool,
        subcommands: &[Command::Hear, Command::Health],
        default: Some(false),
        arg: Some(Arg::long("placed")),
        summary: "The word that the microphone has been placed by the phone (its keeper told, and agreed).",
        ..Input::EMPTY
    };

    HEAR: bool, Command = Input {
        key: "hear",
        ty: Type::Bool,
        subcommands: &[Command::Health],
        default: Some(false),
        arg: Some(Arg::long("hear")),
        summary: "Listen too: play a tone on each speaker and hear it through the lab's microphone.",
        ..Input::EMPTY
    };

    DOCS: String, Command = Input {
        key: "docs",
        ty: Type::String,
        subcommands: &[Command::Gather],
        arg: Some(Arg { value_name: "DOCS", ..Arg::long("docs") }),
        summary: "File into docs at this place instead (default: the lab itself when it has a devicebase.toml, else ../docs).",
        ..Input::EMPTY
    };

    LAND: bool, Command = Input {
        key: "land",
        ty: Type::Bool,
        subcommands: &[Command::Gather],
        default: Some(false),
        arg: Some(Arg::long("land")),
        summary: "Land it too: in devlab's own clone of the lab (and of docs, with --docs or a lab without a base), through `land`.",
        ..Input::EMPTY
    };

    RUNNER: String, Command = Input {
        key: "runner",
        ty: Type::String,
        subcommands: &[Command::Suite],
        arg: Some(Arg { value_name: "FILE", ..Arg::long("runner") }),
        summary: "A runner built elsewhere, copied with the generic checks - to try what is not released yet. (Alone, the built runner is now --built-runner.) Without --runner and --built-runner, the Alpine package (lpoco-health).",
        ..Input::EMPTY
    };

    BUILT_RUNNER: bool, Command = Input {
        key: "built_runner",
        ty: Type::Bool,
        subcommands: &[Command::Suite],
        default: Some(false),
        arg: Some(Arg::long("built-runner")),
        summary: "The runner built (health/build.sh) from the lab-tools this lpoco-lab was built from, whose generic checks it carries.",
        ..Input::EMPTY
    };

    HERE: bool, Command = Input {
        key: "here",
        ty: Type::Bool,
        subcommands: &[Command::Suite],
        default: Some(false),
        arg: Some(Arg::long("here")),
        summary: "The lab's checks and manifest as this clone holds them, not as its main has them - to try a check before it lands.",
        ..Input::EMPTY
    };

    REASON: String, Command = Input {
        key: "reason",
        ty: Type::String,
        subcommands: &[Command::Suite],
        arg: Some(Arg { value_name: "REASON", ..Arg::long("reason") }),
        summary: "Why: a test is never switched off without one.",
        ..Input::EMPTY
    };

    BY: String, Command = Input {
        key: "by",
        ty: Type::String,
        subcommands: &[Command::Suite],
        arg: Some(Arg { value_name: "BY", ..Arg::long("by") }),
        summary: "Who asks (devlab by default).",
        ..Input::EMPTY
    };

    CMDLINE: String, Command = Input {
        key: "cmdline",
        ty: Type::String,
        subcommands: &[Command::Kernel],
        arg: Some(Arg { value_name: "CMDLINE", ..Arg::long("cmdline") }),
        summary: "Extra kernel options for this entry only (e.g. dyndbg=\"file drivers/soundwire/* +p\").",
        ..Input::EMPTY
    };

    BOOTLOADER: bool, Command = Input {
        key: "bootloader",
        ty: Type::Bool,
        subcommands: &[Command::Fastboot],
        default: Some(false),
        arg: Some(Arg::long("bootloader")),
        summary: "Come back into fastboot instead of leaving it.",
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

    // --- the environment it reads (the readers are in flash.rs, provision.rs, watch.rs, phone.rs, hear.rs, device.rs) ---

    DEVLAB_ROOT: String = Input {
        key: "devlab_root",
        ty: Type::String,
        env: Some(Env::new("DEVLAB_ROOT")),
        summary: "The program that runs a root command on this machine, given the command as its arguments (default: sudo -n lab-agent root-exec --).",
        ..Input::EMPTY
    };

    LAB_AGENT: String = Input {
        key: "lab_agent",
        ty: Type::String,
        env: Some(Env::new("LAB_AGENT")),
        summary: "The lab-agent program watch files its board task with (default: lab-agent).",
        ..Input::EMPTY
    };

    PHONE_PASS: String = Input {
        key: "phone_pass",
        ty: Type::String,
        env: Some(Env::new("PHONE_PASS")),
        summary: "The phone user's password, when ssh keys are not set up yet (a private file's value, never in a repository).",
        ..Input::EMPTY
    };

    DEVLAB_MICROPHONE: String = Input {
        key: "devlab_microphone",
        ty: Type::String,
        env: Some(Env::new("DEVLAB_MICROPHONE")),
        summary: "The lab's microphone: the capture device hear listens through.",
        ..Input::EMPTY
    };

    HOME: String = Input {
        key: "home",
        ty: Type::String,
        env: Some(Env::new("HOME")),
        summary: "The lab user's home: devlab's state is kept under ~/.local/state/devlab.",
        ..Input::EMPTY
    };
}

/// The declared environment, read once through the contract (the readers call `env()`, never std::env).
pub struct EnvIn {
    pub devlab_root: String,
    pub lab_agent: String,
    pub phone_pass: Option<String>,
    pub microphone: Option<String>,
    pub home: Option<String>,
}

pub const DEFAULT_ROOT: &str = "sudo -n lab-agent root-exec --";

/// Read the declared variables from a resolved command line; a value the contract refuses is said.
pub fn env_in(r: &argenv_resolve::Resolution) -> Result<EnvIn, String> {
    let root = Contract::DEVLAB_ROOT.get_from(r);
    if root.as_deref().is_some_and(|v| v.trim().is_empty()) {
        return Err(
            "DEVLAB_ROOT is empty: name the program that runs a root command, or unset it".into(),
        );
    }
    Ok(EnvIn {
        devlab_root: root.unwrap_or_else(|| DEFAULT_ROOT.into()),
        lab_agent: Contract::LAB_AGENT
            .get_from(r)
            .unwrap_or_else(|| "lab-agent".into()),
        phone_pass: Contract::PHONE_PASS.get_from(r),
        microphone: Contract::DEVLAB_MICROPHONE
            .get_from(r)
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty()),
        home: Contract::HOME.get_from(r),
    })
}

static ENV: std::sync::OnceLock<EnvIn> = std::sync::OnceLock::new();

/// Keep the environment the command line was resolved with.
pub fn set_env(e: EnvIn) {
    let _ = ENV.set(e);
}

/// The declared environment: from main's parse, else (a unit test) resolved from the process as it stands.
pub fn env() -> &'static EnvIn {
    #[cfg(test)]
    let e = ENV.get_or_init(|| {
        let r = argenv_resolve::parse(&Contract::records());
        env_in(&r).unwrap_or_else(|e| panic!("{e}"))
    });
    #[cfg(not(test))]
    let e = ENV
        .get()
        .expect("cli::set_env runs in parse() before any reader");
    e
}

/// One command or action: what it says, the positionals it takes (keys, in order) and the flags it knows.
pub struct Node {
    pub path: &'static [&'static str],
    pub summary: &'static str,
    pub args: &'static [&'static str],
    pub flags: &'static [&'static str],
}

const fn n(
    path: &'static [&'static str],
    summary: &'static str,
    args: &'static [&'static str],
    flags: &'static [&'static str],
) -> Node {
    Node {
        path,
        summary,
        args,
        flags,
    }
}

/// Every command and action, as clap listed them.
pub const TREE: &[Node] = &[
    n(&["phone"], "Reach the phone", &[], &[]),
    n(&["phone", "find"], "Print the address that answers", &[], &[]),
    n(&["phone", "sh"], "Run the commands on standard input as the user", &[], &[]),
    n(&["phone", "root"], "Run the commands on standard input as root", &[], &[]),
    n(&["phone", "put"], "Copy a local file to the phone", &["local", "remote"], &[]),
    n(&["phone", "reboot"], "Reboot, and wait until the phone answers again", &[], &[]),
    n(
        &["flash"],
        "Flash an image (say `now`): the manifest's fixes baked in, the device tree fingerprinted",
        &["now", "image", "bootloader_file"],
        &["dtb", "as_is"],
    ),
    n(
        &["provision"],
        "Provision a freshly flashed phone: packages, files, Wi-Fi (say `now`: it changes this machine's networking for a while)",
        &["now"],
        &[],
    ),
    n(&["test"], "Run the device's checks now", &[], &[]),
    n(
        &["hear"],
        "Listen: play a tone on each speaker and hear it through the lab's microphone",
        &[],
        &["placed"],
    ),
    n(
        &["health"],
        "The checks against everything that worked before; exit 1 on a regression",
        &[],
        &["hear", "placed"],
    ),
    n(
        &["gather"],
        "Gather (say `now`): every report the phone keeps that the lab lacks, into this lab's tests/results, and every run not yet filed, into test_runs and test_results of the lab's own base (it has a devicebase.toml), else of docs",
        &["now"],
        &["docs", "land"],
    ),
    n(
        &["suite"],
        "The phone's own health suite: put it on the phone, run it, read its latest report",
        &[],
        &[],
    ),
    n(
        &["suite", "put"],
        "Put the suite on the phone (say `now`): the runner, the checks, the manifest's [health], the timer",
        &["action_now"],
        &["runner", "built_runner", "here"],
    ),
    n(
        &["suite", "run"],
        "Run it on the phone now, and say what it found; exit 1 when unhealthy",
        &[],
        &[],
    ),
    n(
        &["suite", "latest"],
        "The latest report the phone keeps, as it is (JSON lines)",
        &[],
        &[],
    ),
    n(
        &["suite", "off"],
        "Switch a test off on the phone, with a reason: the phone's suite keeps it, with who and when",
        &["test"],
        &["reason", "by"],
    ),
    n(&["suite", "on"], "Switch a test on again on the phone", &["test"], &[]),
    n(
        &["suite", "list"],
        "The phone's tests with their switches, and the conditions it finds out about itself now",
        &[],
        &[],
    ),
    n(
        &["mcp"],
        "lpoco-lab's door for agents: an MCP server on standard input and output",
        &[],
        &[],
    ),
    n(
        &["watch"],
        "Watch the phone: health after every boot, silent when well, a board task when not",
        &[],
        &[],
    ),
    n(
        &["fastboot"],
        "The phone over fastboot, by its manifest's serial: read it, boot an image from memory, reboot - nothing written by lpoco-lab, the slots' flags said before and after",
        &[],
        &[],
    ),
    n(
        &["fastboot", "state"],
        "Whether it is in fastboot now, and what its bootloader says of itself (read only)",
        &[],
        &[],
    ),
    n(&["fastboot", "getvar"], "One bootloader variable (read only)", &["name"], &[]),
    n(
        &["fastboot", "boot"],
        "Boot an Android boot image from memory (say `now`): nothing is written; it says where the phone went and what the bootloader did to the slots' flags",
        &["action_now", "fb_image"],
        &[],
    ),
    n(
        &["fastboot", "reboot"],
        "Leave fastboot to the active slot (say `now`), or with --bootloader come back into fastboot",
        &["action_now"],
        &["bootloader"],
    ),
    n(
        &["fastboot", "set-active"],
        "Make SLOT (a or b) the active one (say `now`): metadata only, and only where the manifest says [fastboot] slot_switch = true",
        &["action_now", "slot"],
        &[],
    ),
    n(
        &["keeper"],
        "The phone's keeper: the one session that owns its flow - a lease, so two keepers never run",
        &[],
        &[],
    ),
    n(
        &["keeper", "take"],
        "Take the lease (exit 1, saying who holds it, while another keeper is live)",
        &["name"],
        &[],
    ),
    n(
        &["keeper", "renew"],
        "Renew it as the keeper works: a lease unrenewed for 90 minutes has lapsed",
        &["name"],
        &[],
    ),
    n(&["keeper", "release"], "Give it back, as the keeper ends", &["name"], &[]),
    n(&["keeper", "status"], "Who keeps the phone now; or free", &[], &[]),
    n(
        &["guard"],
        "The ways, held for agents: what Claude Code's hooks on the lab call (needs no manifest)",
        &[],
        &[],
    ),
    n(
        &["guard", "tool"],
        "PreToolUse: the hook's JSON on standard input; ssh/scp to a phone outside lpoco-lab exits 2, saying why",
        &[],
        &[],
    ),
    n(
        &["kernel"],
        "A test kernel beside the stock one: tried once, falls back to stock by itself",
        &[],
        &[],
    ),
    n(
        &["kernel", "install"],
        "Install it as a second boot entry with one try (MODULES.tgz holds lib/modules/RELEASE)",
        &["name", "vmlinuz", "dtb_file", "modules"],
        &["cmdline"],
    ),
    n(
        &["kernel", "try"],
        "Reboot into it once; any later boot is stock (or the kept kernel)",
        &["name"],
        &["dtb"],
    ),
    n(&["kernel", "log"], "What a timed-out try left on the boot partition", &[], &[]),
    n(&["kernel", "remove"], "Remove it and its modules", &["name"], &[]),
    n(
        &["kernel", "keep"],
        "Keep it (say `now`): the default from the next boot, blessed by its first good boot; after 3 bad boots in a row stock boots by itself",
        &["name", "kernel_now"],
        &[],
    ),
    n(
        &["kernel", "unkeep"],
        "Give the default back to stock (say `now`); the kernel stays installed",
        &["name", "kernel_now"],
        &[],
    ),
];

pub const ABOUT: &str = "The device lab for Linux phones, for any device from its manifest";

/// Where a command line's words lead: the command or action to run, the group whose actions to list, or nothing known.
pub enum Found {
    Top,
    Group(&'static [&'static str]),
    Leaf(&'static Node),
    Unknown(String),
}

/// A command with actions has its own node (no args) and children whose path is longer.
fn has_actions(group: &str) -> bool {
    TREE.iter().any(|t| t.path.len() == 2 && t.path[0] == group)
}

pub fn find(words: &[String]) -> Found {
    let Some(first) = words.first() else {
        return Found::Top;
    };
    if has_actions(first) {
        return match words.get(1) {
            None => Found::Group(
                TREE.iter()
                    .find(|t| t.path == [first.as_str()])
                    .unwrap()
                    .path,
            ),
            Some(a) => match TREE.iter().find(|t| t.path == [first.as_str(), a.as_str()]) {
                Some(t) => Found::Leaf(t),
                None => Found::Unknown(format!("{first} {a}")),
            },
        };
    }
    match TREE.iter().find(|t| t.path == [first.as_str()]) {
        Some(t) => Found::Leaf(t),
        None => Found::Unknown(first.clone()),
    }
}

fn record(key: &str) -> Record {
    Contract::records()
        .into_iter()
        .find(|r| r.key == key)
        .unwrap_or_else(|| panic!("cli.rs: {key} is not declared"))
}

fn flag_line(key: &str) -> String {
    let r = record(key);
    let sig = r.usage();
    format!("  {sig:<26} {}\n", r.summary.unwrap_or_default())
}

/// The usage `--help` prints, read from the declaration and the tree.
pub fn usage(found: &Found) -> String {
    match found {
        Found::Top | Found::Unknown(_) => {
            let mut out = format!("Usage: lpoco-lab [OPTIONS] COMMAND\n\n{ABOUT}\n\nCommands:\n");
            for t in TREE.iter().filter(|t| t.path.len() == 1) {
                out.push_str(&format!("  {:<10} {}\n", t.path[0], t.summary));
            }
            out.push_str("\nOptions:\n");
            for k in ["device", "help", "version"] {
                out.push_str(&flag_line(k));
            }
            let mut envs = String::new();
            for r in Contract::records() {
                for e in r.env_names() {
                    envs.push_str(&format!(
                        "  {e:<26} {}\n",
                        r.summary.clone().unwrap_or_default()
                    ));
                }
            }
            out.push_str(&format!("\nEnvironment:\n{envs}"));
            out
        }
        Found::Group(path) => {
            let g = TREE.iter().find(|t| t.path == *path).unwrap();
            let mut out = format!(
                "Usage: lpoco-lab {} [OPTIONS] ACTION\n\n{}\n\nActions:\n",
                g.path[0], g.summary
            );
            for t in TREE
                .iter()
                .filter(|t| t.path.len() == 2 && t.path[0] == g.path[0])
            {
                out.push_str(&format!("  {:<10} {}\n", t.path[1], t.summary));
            }
            out.push_str("\nOptions:\n");
            for k in ["device", "help"] {
                out.push_str(&flag_line(k));
            }
            out
        }
        Found::Leaf(t) => {
            let words: Vec<String> = t
                .args
                .iter()
                .map(|k| record(k).arg.and_then(|a| a.value_name).unwrap_or_default())
                .collect();
            let mut out = format!(
                "Usage: lpoco-lab {} [OPTIONS] {}\n\n{}\n",
                t.path.join(" "),
                words.join(" "),
                t.summary
            );
            if !t.args.is_empty() {
                out.push_str("\nArguments:\n");
                for (k, w) in t.args.iter().zip(&words) {
                    out.push_str(&format!(
                        "  {w:<26} {}\n",
                        record(k).summary.unwrap_or_default()
                    ));
                }
            }
            out.push_str("\nOptions:\n");
            for k in t.flags.iter().copied().chain(["device", "help"]) {
                out.push_str(&flag_line(k));
            }
            out
        }
    }
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
    fn the_tree_names_only_what_is_declared() {
        let keys: Vec<String> = Contract::records().into_iter().map(|r| r.key).collect();
        for t in TREE {
            for k in t.args.iter().chain(t.flags) {
                assert!(keys.iter().any(|x| x == k), "{:?} names {k}", t.path);
            }
        }
    }

    #[test]
    fn every_command_token_has_a_node() {
        for tok in Command::TOKENS {
            assert!(TREE.iter().any(|t| t.path == [*tok]), "{tok}");
        }
    }
}
