//! lpoco-lab's command line is declared once with argenv (backlog lpoco-lab-argenv): every command and
//! subcommand answers `--help` with the arguments clap gave it, and a misspelt flag is refused.
//!
//! The table below is what clap's `--help` listed for each path before the move: the words (summary
//! fragment, value names and flags) a reader needs to find in the new usage.

use std::process::Command;

fn run(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_lpoco-lab"))
        .args(args)
        .output()
        .unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

/// (the path, words its usage must hold)
const PATHS: &[(&[&str], &[&str])] = &[
    (
        &[],
        &[
            "--device",
            "--version",
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
        ],
    ),
    (&["phone"], &["find", "sh", "root", "put", "reboot"]),
    (&["phone", "find"], &["address that answers"]),
    (&["phone", "sh"], &["standard input as the user"]),
    (&["phone", "root"], &["standard input as root"]),
    (&["phone", "put"], &["LOCAL", "REMOTE"]),
    (&["phone", "reboot"], &["answers again"]),
    (
        &["flash"],
        &["NOW", "IMAGE", "BOOTLOADER", "--dtb", "--as-is"],
    ),
    (&["provision"], &["NOW", "Wi-Fi"]),
    (&["test"], &["device's checks"]),
    (&["hear"], &["--placed", "microphone"]),
    (&["health"], &["--hear", "--placed", "regression"]),
    (&["gather"], &["NOW", "--docs", "--land", "devicebase.toml"]),
    (&["suite"], &["put", "run", "latest", "off", "on", "list"]),
    (
        &["suite", "put"],
        &["NOW", "--runner", "--built-runner", "--here"],
    ),
    (&["suite", "run"], &["exit 1 when unhealthy"]),
    (&["suite", "latest"], &["JSON lines"]),
    (&["suite", "off"], &["TEST", "--reason", "--by"]),
    (&["suite", "on"], &["TEST"]),
    (&["suite", "list"], &["conditions"]),
    (&["mcp"], &["MCP server"]),
    (&["watch"], &["after every boot"]),
    (
        &["fastboot"],
        &["state", "getvar", "boot", "reboot", "set-active"],
    ),
    (&["fastboot", "state"], &["read only"]),
    (&["fastboot", "getvar"], &["NAME"]),
    (&["fastboot", "boot"], &["NOW", "IMAGE"]),
    (&["fastboot", "reboot"], &["NOW", "--bootloader"]),
    (&["fastboot", "set-active"], &["NOW", "SLOT"]),
    (&["keeper"], &["take", "renew", "release", "status"]),
    (&["keeper", "take"], &["NAME", "lease"]),
    (&["keeper", "renew"], &["NAME", "90 minutes"]),
    (&["keeper", "release"], &["NAME"]),
    (&["keeper", "status"], &["free"]),
    (&["guard"], &["tool"]),
    (&["guard", "tool"], &["PreToolUse"]),
    (
        &["kernel"],
        &["install", "try", "log", "remove", "keep", "unkeep"],
    ),
    (
        &["kernel", "install"],
        &["NAME", "VMLINUZ", "DTB", "MODULES", "--cmdline"],
    ),
    (&["kernel", "try"], &["NAME", "--dtb"]),
    (&["kernel", "log"], &["timed-out try"]),
    (&["kernel", "remove"], &["NAME"]),
    (&["kernel", "keep"], &["NAME", "NOW"]),
    (&["kernel", "unkeep"], &["NAME", "NOW"]),
];

#[test]
fn every_path_answers_help_with_its_arguments() {
    for (path, words) in PATHS {
        for flag in ["--help", "-h"] {
            let mut args: Vec<&str> = path.to_vec();
            args.push(flag);
            let (code, out, err) = run(&args);
            assert_eq!(code, 0, "{args:?}: stderr {err}");
            assert!(out.starts_with("Usage: lpoco-lab"), "{args:?}: {out}");
            for w in *words {
                assert!(out.contains(w), "{args:?} lacks {w:?}:\n{out}");
            }
        }
    }
}

#[test]
fn the_environment_it_reads_is_declared() {
    let (_, out, _) = run(&["--help"]);
    for v in [
        "DEVLAB_ROOT",
        "LAB_AGENT",
        "PHONE_PASS",
        "DEVLAB_MICROPHONE",
    ] {
        assert!(out.contains(v), "{v} missing:\n{out}");
    }
}

#[test]
fn a_flag_for_another_command_is_refused() {
    let (code, _, err) = run(&["phone", "find", "--dtb", "x"]);
    assert_ne!(code, 0, "{err}");
    let (code, _, _) = run(&["test", "--no-such-flag"]);
    assert_ne!(code, 0);
}

#[test]
fn a_missing_or_extra_argument_is_refused_with_the_usage_before_any_manifest_is_read() {
    for args in [
        &["phone", "put", "a"][..],
        &["phone", "put", "a", "b", "c"],
        &["suite", "off", "t"],
        &["suite", "put", "now", "--runner", "a", "--built-runner"],
        &["kernel", "keep", "k"],
        &["kernel", "install", "n", "v", "d", "m", "--dtb", "x"],
        &["suite", "on", "t", "--reason", "x"],
        &["fastboot", "state", "--bootloader"],
        &["phone", "find", "--dtb", "x"],
        &["test", "--no-such-flag"],
        &["bogus"],
        &["suite", "put", "now", "--runner"],
    ] {
        let (code, _, err) = run(args);
        assert_eq!(code, 2, "{args:?}: {err}");
        assert!(err.contains("Usage: lpoco-lab"), "{args:?}: {err}");
    }
    // help wins over a mistake in the words
    let (code, out, _) = run(&["bogus", "--help"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("Usage: lpoco-lab"));
}
