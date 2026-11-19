// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco` - one front door for the linux-phone-oco tools. `lpoco NAME args...` finds `lpoco-NAME`
//! beside this program or on PATH and runs it, returning its exit code. The family is released
//! together, so a program whose `--version` differs from lpoco's is refused, naming both versions.
//! The phone gets no dispatcher: it runs `lpoco-health` and the others directly.

mod cli;

use argenv_resolve::generate::{self, Program, Shell};
use argenv_resolve::{InputResolveExt, OnProblems};
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const MAN_DATE: &str = "2026-11-17";

/// Where programs are looked for, in order: beside this program, then each PATH folder.
fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(d) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
    {
        dirs.push(d);
    }
    if let Some(path) = std::env::var_os("PATH") {
        for d in std::env::split_paths(&path) {
            if !d.as_os_str().is_empty() && !dirs.contains(&d) {
                dirs.push(d);
            }
        }
    }
    dirs
}

fn is_exec(p: &Path) -> bool {
    p.metadata()
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// The version a program reports for `--version`: the last word of its first line.
fn version_of(prog: &Path) -> Option<String> {
    let out = Command::new(prog).arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .next()?
        .split_whitespace()
        .last()
        .map(str::to_string)
}

fn run(name: &str, args: &[String]) -> i32 {
    let file = format!("lpoco-{name}");
    let dirs = search_dirs();
    let Some(prog) = dirs.iter().map(|d| d.join(&file)).find(|p| is_exec(p)) else {
        let searched: Vec<String> = dirs.iter().map(|d| d.display().to_string()).collect();
        eprintln!(
            "lpoco: no `{file}` found (searched: {})",
            searched.join(", ")
        );
        return 127;
    };
    match version_of(&prog) {
        Some(v) if v == VERSION => {}
        Some(v) => {
            eprintln!(
                "lpoco: {} is version {v} but lpoco is {VERSION}; the family runs one version, install them together",
                prog.display()
            );
            return 3;
        }
        None => {
            eprintln!(
                "lpoco: {} did not report a version for --version (lpoco is {VERSION}); refusing to run it",
                prog.display()
            );
            return 3;
        }
    }
    match Command::new(&prog).args(args).status() {
        Ok(s) => s.code().unwrap_or(128),
        Err(e) => {
            eprintln!("lpoco: cannot run {}: {e}", prog.display());
            126
        }
    }
}

/// Every lpoco-* program found, the first of each name winning, with its version.
fn found() -> BTreeMap<String, (PathBuf, Option<String>)> {
    let mut map: BTreeMap<String, (PathBuf, Option<String>)> = BTreeMap::new();
    for d in search_dirs() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let fname = e.file_name().to_string_lossy().into_owned();
            let Some(name) = fname.strip_prefix("lpoco-").filter(|n| !n.is_empty()) else {
                continue;
            };
            let p = e.path();
            if is_exec(&p) && !map.contains_key(name) {
                let v = version_of(&p);
                map.insert(name.to_string(), (p, v));
            }
        }
    }
    map
}

fn list() -> i32 {
    println!("lpoco {VERSION}");
    for (name, (path, v)) in found() {
        let (shown, note) = match v {
            Some(v) if v == VERSION => (v, ""),
            Some(v) => (v, "  (differs: lpoco refuses to run it)"),
            None => (
                "unknown".into(),
                "  (no --version: lpoco refuses to run it)",
            ),
        };
        println!("{name:<12} {shown:<10} {}{note}", path.display());
    }
    0
}

fn program() -> Program<'static> {
    Program {
        name: "lpoco",
        version: VERSION,
        about: "one front door for the linux-phone-oco tools: lpoco NAME runs lpoco-NAME",
        commands: &[
            generate::Command {
                name: "list",
                summary: "The lpoco-* programs found, with versions",
            },
            generate::Command {
                name: "completions",
                summary: "Shell completions for lpoco",
            },
            generate::Command {
                name: "mcp",
                summary: "The one MCP server for the family (lab, health and bringup tools)",
            },
            generate::Command {
                name: "man",
                summary: "The manual page, in roff",
            },
        ],
        exit_status: &[
            ("0", "done"),
            ("2", "the invocation is wrong"),
            (
                "3",
                "the program's version differs from lpoco's, or it reports none",
            ),
            ("126", "the program was found but could not be run"),
            ("127", "no lpoco-NAME was found"),
            ("other", "the exit code of lpoco-NAME"),
        ],
        see_also: &["lpoco-health(1)"],
        date: MAN_DATE,
        asked: &[],
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // a built-in or a flag is lpoco's own and read through its declaration; any other first word
    // is a program's name and everything after it belongs to that program
    let own = match args.first().map(String::as_str) {
        None => true,
        Some(a) => a.starts_with('-') || cli::Command::TOKENS.contains(&a),
    };
    if !own {
        std::process::exit(run(&args[0], &args[1..]));
    }
    let r = cli::Contract::parse_and_lint(OnProblems::FailOnError);
    if cli::Contract::HELP.get_from(&r).unwrap_or(false) {
        print!("{}", cli::usage("lpoco", "One front door for the linux-phone-oco tools. Commands: NAME args (runs lpoco-NAME), list, completions SHELL, mcp, man."));
        return;
    }
    if cli::Contract::VERSION.get_from(&r).unwrap_or(false) {
        println!("lpoco {VERSION}");
        return;
    }
    let records = cli::Contract::records();
    let code = match cli::Contract::COMMAND.get_from(&r) {
        Some(cli::Command::List) => list(),
        Some(cli::Command::Completions) => {
            let shell = cli::Contract::SHELL
                .get_from(&r)
                .and_then(|s| Shell::parse(&s));
            match shell {
                Some(sh) => emit(generate::completions(sh, &program(), &records)),
                None => {
                    eprintln!("lpoco: completions needs a shell: bash, zsh, fish or nushell");
                    2
                }
            }
        }
        Some(cli::Command::Mcp) => run("lab", &["mcp".to_string()]),
        Some(cli::Command::Man) => emit(generate::man_page(&program(), &records)),
        None => {
            eprintln!(
                "usage: lpoco NAME [args...] | list | completions SHELL | mcp | man | --version"
            );
            2
        }
    };
    std::process::exit(code);
}

fn emit(r: Result<String, generate::GenerateError>) -> i32 {
    match r {
        Ok(s) => {
            print!("{s}");
            0
        }
        Err(e) => {
            eprintln!("lpoco: {e}");
            1
        }
    }
}
