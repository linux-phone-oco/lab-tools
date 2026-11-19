// SPDX-License-Identifier: MIT OR Apache-2.0
//! The invocation surface every attended procedure has, declared once with argenv: its arguments and the
//! environment variables the library reads. A lane's entry file declares none of it.
//!
//!   PROCEDURE [--dry-run] [--log FILE] [--device DEVICE] [--until FIRST_PROMPT] [--part NAME]
//!   PROCEDURE --help

use argenv_resolve::*;

contract! {
    DRY_RUN: bool = Input {
        key: "dry_run",
        ty: Type::Bool,
        default: Some(false),
        arg: Some(Arg::long("dry-run")),
        summary: "Walk every step and say what it would do; reach no phone, ask no one, change nothing.",
        ..Input::EMPTY
    };

    DEVICE: String = Input {
        key: "device",
        ty: Type::String,
        arg: Some(Arg { value_name: "DEVICE", ..Arg::long("device") }),
        summary: "The device manifest the procedure is for (default: device.toml here or in a folder above); devlab is handed this very file.",
        ..Input::EMPTY
    };

    LOG: String = Input {
        key: "log",
        ty: Type::String,
        arg: Some(Arg { value_name: "FILE", ..Arg::long("log") }),
        summary: "The log to write, one line per step (attended-NAME.log in this folder when none is given).",
        ..Input::EMPTY
    };

    UNTIL: String = Input {
        key: "until",
        ty: Type::Enum,
        allowed: &["FIRST_PROMPT"],
        arg: Some(Arg { value_name: "POINT", ..Arg::long("until") }),
        summary: "Stop the run cleanly at a point, never answering it: FIRST_PROMPT is the first ask_author (the steps before it run).",
        ..Input::EMPTY
    };

    PART: String = Input {
        key: "part",
        ty: Type::String,
        arg: Some(Arg { value_name: "NAME", ..Arg::long("part") }),
        summary: "Run one named part of the procedure instead of the whole (the entry file says which it has).",
        ..Input::EMPTY
    };

    DEVLAB: String = Input {
        key: "devlab",
        ty: Type::String,
        env: Some(Env::new("LPOCO_ATTENDED_DEVLAB")),
        summary: "The devlab program that reaches the phone (devlab when none is given); a test points it at a stand-in.",
        ..Input::EMPTY
    };

    SYSFS_USB: String = Input {
        key: "sysfs_usb",
        ty: Type::String,
        env: Some(Env::new("LPOCO_ATTENDED_SYSFS_USB")),
        summary: "The folder of the computer's USB devices wait_for_usb reads (/sys/bus/usb/devices when none is given); a test points it at a fake.",
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
}

/// The usage `--help` prints, read from the declaration above.
pub fn usage(name: &str) -> String {
    let mut out = format!("Usage: {name} [OPTIONS]\n\nAn attended procedure: named steps, each stopping the run when it fails.\n");
    let mut args = String::new();
    let mut envs = String::new();
    for r in &Contract::records() {
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
