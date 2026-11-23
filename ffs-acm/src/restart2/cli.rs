// SPDX-License-Identifier: MIT OR Apache-2.0
//! restart2's invocation surface, declared once with argenv.
//!   restart2 --command STRING [--dry-run]

use argenv_resolve::*;

contract! {
    COMMAND: String = Input {
        key: "command",
        ty: Type::String,
        arg: Some(Arg { value_name: "STRING", ..Arg::long("command") }),
        env: Some(Env::new("RESTART2_COMMAND")),
        summary: "The string handed to reboot(2) RESTART2, which the bootloader reads from the reboot reason (required; only `bootloader` is allowed: it stops the phone in fastboot).",
        ..Input::EMPTY
    };

    DRY_RUN: bool = Input {
        key: "dry_run",
        ty: Type::Bool,
        arg: Some(Arg::long("dry-run")),
        summary: "Say what would be passed to reboot(2) and stop: nothing is synced, nothing restarts.",
        ..Input::EMPTY
    };

    HELP: bool = Input {
        key: "help",
        ty: Type::Bool,
        arg: Some(Arg { short: Some('h'), ..Arg::long("help") }),
        summary: "Print this usage and stop.",
        ..Input::EMPTY
    };
}

/// The usage `--help` prints, read from the declaration above.
pub fn usage() -> String {
    let mut out = String::from(
        "Usage: restart2 --command STRING [--dry-run]\n\nSync the disks, then restart the machine through reboot(2) LINUX_REBOOT_CMD_RESTART2 with STRING as its argument.\n\nArguments:\n",
    );
    for r in &Contract::records() {
        let sig = r.usage();
        if !sig.is_empty() {
            out.push_str(&format!(
                "  {sig:<24} {}\n",
                r.summary.clone().unwrap_or_default()
            ));
        }
    }
    out
}
