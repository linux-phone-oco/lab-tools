//! The phone's way, held for agents: Claude Code's PreToolUse hook on the lab calls `devlab guard tool`,
//! so that changing the phone outside devlab is refused when it is tried, not remembered. The hook's JSON
//! comes on standard input; a refused command exits 2 and says why on standard error (Claude Code gives
//! that to the agent and does not run it).
//!
//! What is refused, in a Bash command (each simple command of it, `sh -c` and `$(...)` included):
//! `ssh`, `scp`, `sftp`, `rsync`, `sshpass` to a phone's address - the phone is changed through devlab, which
//! holds it for each change. A phone's addresses are the ones devlab has met: every manifest it loaded
//! leaves them in its state (STATE/CODENAME/addresses), with the Wi-Fi address it learned.
//!
//! The ways every project shares - no git push to main, no Python, the practices and the inbox at the
//! start of a session - are the ideal-agent plugin's hooks (grenudi/ideal-agent), installed on the lab
//! since 2026-11-17; devlab keeps only what is its phone's.

use anyhow::Result;
use ideal_agent::shell::{simple_commands, unwrapped};
use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};

/// The address postmarketOS gives a phone on USB networking, known before any manifest is loaded.
const USB_NET: &str = "172.16.42.1";

pub fn tool() -> Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let event: serde_json::Value = serde_json::from_str(&input).unwrap_or_default();
    if event["tool_name"].as_str() != Some("Bash") {
        return Ok(());
    }
    let command = event["tool_input"]["command"].as_str().unwrap_or_default();
    let phones = phone_addresses(&state_root());
    if let Some(why) = refused(command, &phones) {
        eprintln!("devlab guard: {why}");
        std::process::exit(2);
    }
    Ok(())
}

fn state_root() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state/devlab")
}

/// Every address devlab has met for a phone, under its state root.
pub fn phone_addresses(root: &Path) -> BTreeSet<String> {
    let mut all = BTreeSet::from([USB_NET.to_string()]);
    for dir in std::fs::read_dir(root).into_iter().flatten().flatten() {
        for name in ["addresses", "wifi-address"] {
            if let Ok(text) = std::fs::read_to_string(dir.path().join(name)) {
                all.extend(text.split_whitespace().map(str::to_string));
            }
        }
    }
    all
}

/// Why this command is refused, if it is.
pub fn refused(command: &str, phones: &BTreeSet<String>) -> Option<String> {
    for words in simple_commands(command) {
        let words = unwrapped(&words);
        let Some(first) = words.first() else { continue };
        let program = first.rsplit('/').next().unwrap_or(first);
        let args = &words[1..];
        if matches!(program, "sh" | "bash" | "zsh" | "dash" | "fish") {
            if let Some(i) = args
                .iter()
                .position(|a| a == "-c" || a == "-lc" || a == "-ec")
            {
                if let Some(inner) = args.get(i + 1) {
                    if let Some(why) = refused(inner, phones) {
                        return Some(why);
                    }
                }
            }
            continue;
        }
        if program == "fastboot" {
            if let Some(why) = fastboot_changes(args) {
                return Some(why);
            }
            continue;
        }
        if matches!(program, "ssh" | "scp" | "sftp" | "rsync" | "sshpass") {
            if let Some(addr) = args
                .iter()
                .find_map(|a| phones.iter().find(|p| names(a, p)))
            {
                return Some(format!(
                    "`{program}` to the phone ({addr}) is refused: change the phone through devlab, which holds it \
                     for each change - devlab phone sh|root (commands on standard input), devlab phone put, \
                     devlab kernel ..., or the MCP door (devlab mcp)."
                ));
            }
        }
    }
    None
}

/// Raw fastboot outside devlab only reads (devices, getvar, help, --version): anything else boots,
/// writes, erases or switches a slot on whichever phone it reaches - devlab names the phone by its
/// serial, holds it, and writes only where its manifest says so (2026-11-17, google-walleye beside surya).
fn fastboot_changes(args: &[String]) -> Option<String> {
    const VALUED: &[&str] = &[
        "-s",
        "-S",
        "-i",
        "-b",
        "-n",
        "-c",
        "-p",
        "--slot",
        "--os-version",
        "--os-patch-level",
    ];
    let mut verb = None;
    let mut skip = false;
    for a in args {
        if skip {
            skip = false;
            continue;
        }
        if a == "-w" || a.starts_with("--set-active") || a == "--wipe-and-use-fbe" {
            verb = Some(a.as_str());
            break;
        }
        if VALUED.contains(&a.as_str()) {
            skip = true;
            continue;
        }
        if a.starts_with('-') {
            continue;
        }
        verb = Some(a.as_str());
        break;
    }
    match verb {
        None | Some("devices" | "getvar" | "help") => None,
        Some(v) => Some(format!(
            "`fastboot {v}` is refused outside devlab: it changes whichever phone it reaches - devlab fastboot \
             state|getvar NAME|boot now IMAGE|reboot now names the phone by its manifest's serial and holds it; \
             writing (devlab flash) only where the manifest says [fastboot] writes = true."
        )),
    }
}

/// Whether an argument names this address as its host: `addr`, `user@addr`, `addr:path`, `user@addr:path`.
fn names(arg: &str, addr: &str) -> bool {
    let host = arg.rsplit('@').next().unwrap_or(arg);
    let host = host.split(':').next().unwrap_or(host);
    host == addr || arg.split(['=', ',']).any(|part| part == addr)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phones() -> BTreeSet<String> {
        BTreeSet::from([USB_NET.to_string(), "192.168.178.182".to_string()])
    }

    fn says(command: &str) -> Option<String> {
        refused(command, &phones())
    }

    #[test]
    fn a_quoted_heredoc_is_text_not_commands() {
        let phones = BTreeSet::from([USB_NET.to_string()]);
        assert!(refused("cat <<'EOF'\nssh user@172.16.42.1 true\nEOF\n", &phones).is_none());
        assert!(refused("ssh user@172.16.42.1 true <<'EOF'\nuname\nEOF\n", &phones).is_some());
    }

    #[test]
    fn ssh_to_the_phone_is_refused_and_elsewhere_is_not() {
        for c in [
            "ssh user@172.16.42.1 uname -a",
            "scp f user@192.168.178.182:/tmp/",
            "sshpass -f ~/.config/devlab/xiaomi-surya/password ssh -o X=Y user@172.16.42.1",
            "rsync -a d/ user@172.16.42.1:/tmp/d",
            "sudo lab-agent root-exec -- ssh 172.16.42.1",
            "bash -c 'ssh 172.16.42.1'",
        ] {
            assert!(says(c).is_some(), "refused: {c}");
        }
        for c in [
            "ssh git@github.com",
            "scp a user@10.0.0.2:/x",
            "devlab phone sh",
            "ping 172.16.42.1",
            "echo ssh user@172.16.42.1",
        ] {
            assert!(says(c).is_none(), "allowed: {c}");
        }
    }

    #[test]
    fn raw_fastboot_only_reads() {
        for c in [
            "fastboot devices",
            "fastboot -s FA81N1A00790 getvar all",
            "fastboot getvar current-slot 2>&1 | head",
            "fastboot --version",
            "echo fastboot flash boot x.img",
        ] {
            assert!(says(c).is_none(), "reads: {c}");
        }
        for c in [
            "fastboot flash boot boot.img",
            "fastboot -s FA81N1A00790 boot boot.img",
            "fastboot --slot b flash boot x",
            "fastboot erase userdata",
            "fastboot set_active b",
            "fastboot --set-active=b reboot",
            "fastboot -w getvar all",
            "fastboot reboot",
            "fastboot flashing lock",
            "sudo fastboot oem unlock",
            "bash -c 'fastboot flash boot x'",
        ] {
            assert!(says(c).is_some(), "changes: {c}");
        }
    }

    #[test]
    fn the_ways_every_project_shares_are_the_plugins() {
        // no push to main and no Python are held by the ideal-agent plugin's hook, not here
        for c in ["git push origin main", "python3 x.py"] {
            assert!(says(c).is_none(), "not devlab's: {c}");
        }
    }

    #[test]
    fn a_phones_addresses_are_the_ones_devlab_met() {
        let root = std::env::temp_dir().join(format!("devlab-guard-{}", std::process::id()));
        std::fs::create_dir_all(root.join("a")).unwrap();
        std::fs::create_dir_all(root.join("b")).unwrap();
        std::fs::write(root.join("a/addresses"), "10.1.1.1\n10.1.1.2\n").unwrap();
        std::fs::write(root.join("b/wifi-address"), "10.2.2.2").unwrap();
        let met = phone_addresses(&root);
        for a in [USB_NET, "10.1.1.1", "10.1.1.2", "10.2.2.2"] {
            assert!(met.contains(a), "{a}");
        }
        let _ = std::fs::remove_dir_all(root);
    }
}
