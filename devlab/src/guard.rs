//! The ways, held for agents: Claude Code's hooks on the lab call `devlab guard`, so that what an agent
//! must not do is refused when it is tried, not remembered. Two events:
//!
//!   devlab guard tool    PreToolUse - the hook's JSON on standard input; a refused command exits 2 and
//!                        says why on standard error (Claude Code gives that to the agent and does not run it)
//!   devlab guard start   SessionStart - the live practices (`lab-agent guide`) and the unread messages
//!                        (`lab-agent inbox`, read, not acknowledged)
//!
//! What is refused, in a Bash command (each simple command of it, `sh -c` and `$(...)` included):
//!   - `git push` that reaches main (or master): every change lands through `land`, on its own branch
//!   - `python`, `pip` and their kin: no new Python on the lab
//!   - `ssh`, `scp`, `sftp`, `rsync`, `sshpass` to a phone's address: the phone is changed through devlab,
//!     which holds it for each change. A phone's addresses are the ones devlab has met: every manifest it
//!     loaded leaves them in its state (STATE/CODENAME/addresses), with the Wi-Fi address it learned.

use anyhow::Result;
use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    let cwd = event["cwd"]
        .as_str()
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();
    let phones = phone_addresses(&state_root());
    if let Some(why) = refused(command, &cwd, &phones, &current_branch) {
        eprintln!("devlab guard: {why}");
        std::process::exit(2);
    }
    Ok(())
}

pub fn start() -> Result<()> {
    for (title, args) in [
        ("The ways (lab-agent guide)", &["guide"][..]),
        (
            "Unread messages (lab-agent inbox; answer each, then --ack)",
            &["inbox"][..],
        ),
    ] {
        println!("## {title}\n");
        match Command::new("lab-agent").args(args).output() {
            Ok(out) => print!("{}", String::from_utf8_lossy(&out.stdout)),
            Err(e) => println!("(lab-agent {}: {e})", args[0]),
        }
        println!();
    }
    println!(
        "The lab's hooks refuse git push to main, python/pip, and ssh/scp to the phone outside devlab (devlab guard)."
    );
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

fn current_branch(dir: &Path) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["symbolic-ref", "--short", "-q", "HEAD"])
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|b| !b.is_empty())
}

/// Why this command is refused, if it is.
pub fn refused(
    command: &str,
    cwd: &Path,
    phones: &BTreeSet<String>,
    branch_of: &dyn Fn(&Path) -> Option<String>,
) -> Option<String> {
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
                    if let Some(why) = refused(inner, cwd, phones, branch_of) {
                        return Some(why);
                    }
                }
            }
            continue;
        }
        if is_python(program) {
            return Some(format!(
                "`{program}` is refused on the lab: no new Python, not even for one-offs - shell to orchestrate, \
                 Rust for logic (rust-script for a single file)."
            ));
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
        if program == "git" {
            if let Some(why) = git_push_to_main(args, cwd, branch_of) {
                return Some(why);
            }
        }
    }
    None
}

fn is_python(program: &str) -> bool {
    let bare = program.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
    matches!(bare, "python" | "pip" | "pipx" | "ipython" | "pypy")
}

/// Whether an argument names this address as its host: `addr`, `user@addr`, `addr:path`, `user@addr:path`.
fn names(arg: &str, addr: &str) -> bool {
    let host = arg.rsplit('@').next().unwrap_or(arg);
    let host = host.split(':').next().unwrap_or(host);
    host == addr || arg.split(['=', ',']).any(|part| part == addr)
}

fn git_push_to_main(
    args: &[String],
    cwd: &Path,
    branch_of: &dyn Fn(&Path) -> Option<String>,
) -> Option<String> {
    let mut dir = cwd.to_path_buf();
    let mut i = 0;
    while i < args.len() && args[i].starts_with('-') {
        match args[i].as_str() {
            "-C" => {
                if let Some(d) = args.get(i + 1) {
                    dir = cwd.join(d);
                }
                i += 2;
            }
            "-c" | "--git-dir" | "--work-tree" | "--namespace" => i += 2,
            _ => i += 1,
        }
    }
    if args.get(i).map(String::as_str) != Some("push") {
        return None;
    }
    let rest = &args[i + 1..];
    let refuse = |what: &str| {
        Some(format!(
            "`git push` {what} is refused: every change gets its own branch and lands with `land` \
             (landit, from a shell that sourced oco-work), which waits for every check."
        ))
    };
    if rest.iter().any(|a| a == "--all" || a == "--mirror") {
        return refuse("of every branch");
    }
    let deleting = rest.iter().any(|a| a == "--delete" || a == "-d");
    let mut positional = Vec::new();
    let mut j = 0;
    while j < rest.len() {
        let a = &rest[j];
        if matches!(
            a.as_str(),
            "-o" | "--push-option" | "--repo" | "--receive-pack" | "--exec"
        ) {
            j += 2;
            continue;
        }
        if !a.starts_with('-') {
            positional.push(a.as_str());
        }
        j += 1;
    }
    let here = || branch_of(&dir);
    let is_main = |b: &str| matches!(b, "main" | "master");
    let refspecs = positional.get(1..).unwrap_or(&[]);
    if refspecs.is_empty() {
        return match here() {
            Some(b) if is_main(&b) => refuse("from main"),
            _ => None,
        };
    }
    if deleting {
        return refspecs
            .iter()
            .any(|r| is_main(r.trim_start_matches("refs/heads/")))
            .then(|| "`git push --delete` of main is refused.".to_string());
    }
    for spec in refspecs {
        let spec = spec.trim_start_matches('+');
        let dest = spec.rsplit(':').next().unwrap_or(spec);
        let dest = dest.trim_start_matches("refs/heads/");
        let dest = if dest == "HEAD" || dest == "@" {
            here().unwrap_or_default()
        } else {
            dest.to_string()
        };
        if is_main(&dest) {
            return refuse("to main");
        }
    }
    None
}

/// Strip what only runs another command: assignments, sudo, env, timeout, nice, nohup, exec, command,
/// time, xargs, stdbuf, and the lab's one grant (`lab-agent root-exec --`).
fn unwrapped(words: &[String]) -> Vec<String> {
    let mut w: Vec<String> = words.to_vec();
    loop {
        let Some(first) = w.first().cloned() else {
            return w;
        };
        let program = first.rsplit('/').next().unwrap_or(&first).to_string();
        let skip = if is_assignment(&first) {
            1
        } else {
            match program.as_str() {
                "sudo" | "doas" => flags_then(
                    &w,
                    1,
                    &["-u", "-g", "-C", "-D", "-h", "-p", "-U", "-r", "-t"],
                ),
                "env" => {
                    let mut k = flags_then(&w, 1, &["-u", "-C", "-S"]);
                    while k < w.len() && is_assignment(&w[k]) {
                        k += 1;
                    }
                    k
                }
                "timeout" => flags_then(&w, 1, &["-s", "-k", "--signal", "--kill-after"]) + 1,
                "nice" => flags_then(&w, 1, &["-n"]),
                "nohup" | "exec" | "command" | "time" | "builtin" => flags_then(&w, 1, &[]),
                "xargs" => flags_then(&w, 1, &["-I", "-n", "-P", "-L", "-d", "-a", "-E", "-s"]),
                "stdbuf" => flags_then(&w, 1, &[]),
                "lab-agent" if w.get(1).map(String::as_str) == Some("root-exec") => {
                    w.iter().position(|a| a == "--").map_or(w.len(), |p| p + 1)
                }
                _ => return w,
            }
        };
        if skip == 0 || skip > w.len() {
            return Vec::new();
        }
        w.drain(..skip);
    }
}

fn flags_then(w: &[String], mut k: usize, with_value: &[&str]) -> usize {
    while k < w.len() && w[k].starts_with('-') {
        if w[k] == "--" {
            return k + 1;
        }
        k += if with_value.contains(&w[k].as_str()) {
            2
        } else {
            1
        };
    }
    k
}

fn is_assignment(word: &str) -> bool {
    match word.split_once('=') {
        Some((name, _)) => {
            !name.is_empty()
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && !name.starts_with(|c: char| c.is_ascii_digit())
        }
        None => false,
    }
}

/// The simple commands of a shell line, each as its words: split at ; & | && || newlines, parentheses,
/// braces, and command substitutions (whose insides are commands of their own); quotes are kept together.
pub fn simple_commands(line: &str) -> Vec<Vec<String>> {
    let mut all = Vec::new();
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let end_word = |words: &mut Vec<String>, word: &mut String, in_word: &mut bool| {
        if *in_word {
            words.push(std::mem::take(word));
            *in_word = false;
        }
    };
    let end_command = |all: &mut Vec<Vec<String>>, words: &mut Vec<String>| {
        if !words.is_empty() {
            all.push(std::mem::take(words));
        }
    };
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\'' => {
                in_word = true;
                i += 1;
                while i < chars.len() && chars[i] != '\'' {
                    word.push(chars[i]);
                    i += 1;
                }
            }
            '"' => {
                in_word = true;
                i += 1;
                let mut inner = String::new();
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        i += 1;
                    }
                    inner.push(chars[i]);
                    i += 1;
                }
                // a substitution inside double quotes is a command too
                if inner.contains("$(") || inner.contains('`') {
                    all.extend(
                        substitutions(&inner)
                            .iter()
                            .flat_map(|s| simple_commands(s)),
                    );
                }
                word.push_str(&inner);
            }
            '\\' if i + 1 < chars.len() => {
                if chars[i + 1] != '\n' {
                    in_word = true;
                    word.push(chars[i + 1]);
                }
                i += 1;
            }
            '#' if !in_word => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            ' ' | '\t' => end_word(&mut words, &mut word, &mut in_word),
            ';' | '&' | '|' | '\n' | '(' | ')' | '`' | '{' | '}'
                if !(c == '{' || c == '}') || !in_word =>
            {
                if c == '(' && in_word && word.ends_with('$') {
                    word.pop();
                }
                end_word(&mut words, &mut word, &mut in_word);
                end_command(&mut all, &mut words);
            }
            _ => {
                in_word = true;
                word.push(c);
            }
        }
        i += 1;
    }
    end_word(&mut words, &mut word, &mut in_word);
    end_command(&mut all, &mut words);
    all
}

/// The insides of $(...) and `...` in a text.
fn substitutions(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("$(") {
        let after = &rest[start + 2..];
        let mut depth = 1;
        let mut end = after.len();
        for (k, ch) in after.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = k;
                        break;
                    }
                }
                _ => {}
            }
        }
        found.push(after[..end].to_string());
        rest = &after[end.min(after.len())..];
    }
    let mut parts = text.split('`');
    parts.next();
    found.extend(parts.step_by(2).map(str::to_string));
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phones() -> BTreeSet<String> {
        BTreeSet::from([USB_NET.to_string(), "192.168.178.182".to_string()])
    }

    fn on(branch: &'static str) -> impl Fn(&Path) -> Option<String> {
        move |_| Some(branch.to_string())
    }

    fn says(command: &str, branch: &'static str) -> Option<String> {
        refused(command, Path::new("/tmp"), &phones(), &on(branch))
    }

    #[test]
    fn a_push_that_reaches_main_is_refused() {
        for c in [
            "git push origin main",
            "git push -f origin main",
            "git push origin HEAD:main",
            "git push origin +feat:refs/heads/main",
            "cd x && git push",
            "git -C ../docs push origin master",
            "git push --all",
            "GIT_TRACE=1 git push -u origin HEAD",
            "bash -c 'git push origin main'",
        ] {
            assert!(says(c, "main").is_some(), "refused: {c}");
        }
    }

    #[test]
    fn a_push_of_a_branch_is_not() {
        for c in [
            "git push origin feat",
            "git push -u origin HEAD",
            "git push",
            "git push origin --delete old-branch",
            "echo git push origin main",
            "git log main && git status",
            "land",
        ] {
            assert!(says(c, "feat").is_none(), "allowed: {c}");
        }
    }

    #[test]
    fn python_is_refused_wherever_it_runs() {
        for c in [
            "python3 -c 'print(1)'",
            "/usr/bin/python script.py",
            "pip install x",
            "pip3 install --user y",
            "sudo python3 x.py",
            "env FOO=1 python x",
            "timeout 10 python3 x",
            "ls | xargs -n1 python3",
            "echo $(python3 -c 1)",
            "X=\"$(python -V)\"",
            "sh -c \"python3 x\"",
        ] {
            assert!(says(c, "feat").is_some(), "refused: {c}");
        }
        for c in [
            "grep python file",
            "echo 'python3 x'",
            "rg -n pip src",
            "cargo build --features python",
            "ls ~/.python",
        ] {
            assert!(says(c, "feat").is_none(), "allowed: {c}");
        }
    }

    #[test]
    fn ssh_to_the_phone_is_refused_and_elsewhere_is_not() {
        for c in [
            "ssh user@172.16.42.1 uname -a",
            "scp f user@192.168.178.182:/tmp/",
            "sshpass -f ~/.config/devlab/xiaomi-surya/password ssh -o X=Y user@172.16.42.1",
            "rsync -a d/ user@172.16.42.1:/tmp/d",
            "sudo lab-agent root-exec -- ssh 172.16.42.1",
        ] {
            assert!(says(c, "feat").is_some(), "refused: {c}");
        }
        for c in [
            "ssh git@github.com",
            "scp a user@10.0.0.2:/x",
            "devlab phone sh",
            "ping 172.16.42.1",
            "echo ssh user@172.16.42.1",
        ] {
            assert!(says(c, "feat").is_none(), "allowed: {c}");
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
