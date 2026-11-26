// SPDX-License-Identifier: MIT OR Apache-2.0
//! lpoco-attended (run as `lpoco attended ...`): a device repo's procedures/NAME.rs, scaffolded, run and
//! dry-run by name (decision 2026-11-19-procedures-one-shape). See cli.rs for the invocation surface.

mod cli;
mod export;
mod lift;
mod repo;
mod template;

use argenv_resolve::{InputResolveExt, OnProblems, Resolution};
use cli::{Contract, Verb};
use std::path::Path;
use std::process::Command;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let r = Contract::parse_and_lint(OnProblems::FailOnError);
    if Contract::HELP.get_from(&r).unwrap_or(false) {
        print!("{}", cli::usage("lpoco-attended"));
        return;
    }
    if Contract::VERSION.get_from(&r).unwrap_or(false) {
        println!("lpoco-attended {VERSION}");
        return;
    }
    let code = run(&r).unwrap_or_else(|e| {
        eprintln!("lpoco-attended: {e}");
        2
    });
    std::process::exit(code);
}

fn run(r: &Resolution) -> Result<i32, String> {
    let verb = Contract::VERB
        .get_from(r)
        .ok_or("usage: lpoco attended new NAME | run NAME [--dry-run] | check [--host]")?;
    if let Some(extra) = Contract::EXTRA.get_from(r) {
        return Err(format!("{extra:?} is one word too many: usage: lpoco attended new NAME | run NAME [--dry-run] | check [--host]"));
    }
    let device = Contract::DEVICE.get_from(r);
    let repo = || repo::find(device.as_deref());
    let rust_script = Contract::RUST_SCRIPT
        .get_from(r)
        .unwrap_or_else(|| "rust-script".into());
    match verb {
        Verb::New => new(r, &repo()?),
        Verb::Run => run_one(r, &repo()?, &rust_script),
        Verb::Check => check(r, &repo()?, &rust_script, device.as_deref()),
        Verb::Lift => lift_verb(r),
        Verb::Export => export_verb(r, &repo()?, &rust_script),
    }
}

fn name_of(r: &Resolution) -> Result<String, String> {
    let name = Contract::NAME
        .get_from(r)
        .ok_or("a procedure NAME is needed (lpoco attended new|run NAME)")?;
    repo::valid_name(&name)?;
    Ok(name)
}

fn new(r: &Resolution, repo: &repo::Repo) -> Result<i32, String> {
    let name = name_of(r)?;
    let file = repo.procedure(&name);
    if file.exists() {
        return Err(format!(
            "{} already exists; nothing written",
            file.display()
        ));
    }
    let lib = Contract::LIB.get_from(r);
    let text = template::procedure(&name, &repo.codename, lib.as_deref(), repo.steps_crate()?);
    write_new(&file, &text).map_err(|e| format!("cannot write {}: {e}", file.display()))?;
    println!(
        "wrote {}: try  lpoco attended run {name} --dry-run",
        file.display()
    );
    Ok(0)
}

/// The options a run hands the procedure, in the order its own `--help` lists them.
fn passed_on(r: &Resolution) -> Vec<String> {
    let mut a = Vec::new();
    if Contract::DRY_RUN.get_from(r).unwrap_or(false) {
        a.push("--dry-run".to_string());
    }
    for (flag, v) in [
        ("--log", Contract::LOG.get_from(r)),
        ("--device", Contract::DEVICE.get_from(r)),
        ("--until", Contract::UNTIL.get_from(r)),
        ("--part", Contract::PART.get_from(r)),
        ("--from", Contract::FROM.get_from(r)),
        ("--call-limit", Contract::CALL_LIMIT.get_from(r)),
        ("--tail-limit", Contract::TAIL_LIMIT.get_from(r)),
        ("--answer-limit", Contract::ANSWER_LIMIT.get_from(r)),
    ] {
        if let Some(v) = v {
            a.push(flag.to_string());
            a.push(v);
        }
    }
    a
}

fn run_one(r: &Resolution, repo: &repo::Repo, rust_script: &str) -> Result<i32, String> {
    let name = name_of(r)?;
    let file = repo.existing(&name)?;
    let status = Command::new(rust_script)
        .arg(&file)
        .args(passed_on(r))
        .status()
        .map_err(|e| format!("cannot run {rust_script}: {e}"))?;
    Ok(status.code().unwrap_or(1))
}

fn check(
    r: &Resolution,
    repo: &repo::Repo,
    rust_script: &str,
    device: Option<&str>,
) -> Result<i32, String> {
    let names = repo.procedures()?;
    let mut failed = 0;
    let dir = scratch_dir()?;
    for name in &names {
        let log = dir.join(format!("{name}.log"));
        let out = dry_run(rust_script, &repo.procedure(name), &log, device)?;
        if out.status.success() {
            println!("{name}: ok");
        } else {
            failed += 1;
            println!("{name}: FAILED (exit {})", out.status.code().unwrap_or(-1));
            for line in String::from_utf8_lossy(&out.stdout)
                .lines()
                .chain(String::from_utf8_lossy(&out.stderr).lines())
            {
                println!("    {line}");
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    if Contract::HOST.get_from(r).unwrap_or(false) {
        failed += host_line(r, &repo.manifest);
    }
    Ok(if failed == 0 { 0 } else { 1 })
}

fn dry_run(
    rust_script: &str,
    file: &Path,
    log: &Path,
    device: Option<&str>,
) -> Result<std::process::Output, String> {
    Command::new(rust_script)
        .arg(file)
        .args(["--dry-run", "--log"])
        .arg(log)
        .args(device.into_iter().flat_map(|d| ["--device", d]))
        .output()
        .map_err(|e| format!("cannot run {rust_script}: {e}"))
}

/// Write `text` as a new, runnable file: never through an existing file or a link, mode 0755 (it has a shebang).
fn write_new(file: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::create_dir_all(file.parent().expect("a file in procedures/"))?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o755)
        .open(file)?
        .write_all(text.as_bytes())
}

/// A folder of this run's own for the throw-away logs: made new (never one that was there), so a planted link is never followed.
fn scratch_dir() -> Result<std::path::PathBuf, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "lpoco-attended-check-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir(&dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    Ok(dir)
}

/// The `host:` line of `check --host`: 0 when the installed devlab reads the manifest, else 1.
fn host_line(r: &Resolution, manifest: &Path) -> i32 {
    let devlab = Contract::DEVLAB
        .get_from(r)
        .unwrap_or_else(|| "devlab".into());
    match host_ready(&devlab, manifest) {
        Ok(said) => {
            println!("host: ok ({said})");
            0
        }
        Err(e) => {
            println!("host: FAILED\n    {e}");
            1
        }
    }
}

/// How long each read-only call to the installed devlab may take: a lease folder that hangs must not hang the gate.
const HOST_CALL_LIMIT: std::time::Duration = std::time::Duration::from_secs(30);

fn devlab_call(devlab: &str, args: &[&std::ffi::OsStr]) -> Result<std::process::Output, String> {
    let mut cmd = Command::new(devlab);
    cmd.args(args);
    lpoco_attended::call::run_limited(
        cmd,
        "",
        Some(HOST_CALL_LIMIT),
        lpoco_attended::call::DEFAULT_TAIL,
    )
    .map_err(|e| format!("{devlab}: {e}"))
}

/// The installed devlab, run for real on this device's manifest with a read-only command: ready only when it
/// reads the file. A refusal names the binary, its version and what it said.
fn host_ready(devlab: &str, manifest: &Path) -> Result<String, String> {
    use std::ffi::OsStr;
    let v = devlab_call(devlab, &[OsStr::new("--version")])?;
    let version = String::from_utf8_lossy(&v.stdout).trim().to_string();
    if !v.status.success() || version.is_empty() {
        return Err(format!(
            "{devlab} does not say its version (--version): it is not the lab's devlab"
        ));
    }
    let out = devlab_call(
        devlab,
        &[
            OsStr::new("--device"),
            manifest.as_os_str(),
            OsStr::new("keeper"),
            OsStr::new("status"),
        ],
    )?;
    if out.status.success() {
        return Ok(format!("{devlab}, {version}"));
    }
    let said = format!(
        "{} {}",
        String::from_utf8_lossy(&out.stdout).trim(),
        String::from_utf8_lossy(&out.stderr).trim()
    );
    Err(format!(
        "the installed {devlab} ({version}) cannot read {}: {}",
        manifest.display(),
        said.trim()
    ))
}

/// `lift`: every named repo's steps beside the library, as a table (it needs no one device).
fn lift_verb(r: &Resolution) -> Result<i32, String> {
    let repos = Contract::REPOS
        .get_from(r)
        .ok_or("lift needs --repos DIR,DIR: the device repos whose procedures/steps to read")?;
    let repos: Vec<std::path::PathBuf> = repos
        .split(',')
        .filter(|d| !d.is_empty())
        .map(std::path::PathBuf::from)
        .collect();
    let library = Contract::LIBRARY.get_from(r).map(std::path::PathBuf::from);
    let (mine, lib) = lift::read(&repos, library.as_deref())?;
    print!("{}", lift::csv(&lift::lift(&mine, &lib)));
    Ok(0)
}

/// `export NAME`: the procedure walks itself with `--export` (nothing is reached), its log is read for the checks.
fn export_verb(r: &Resolution, repo: &repo::Repo, rust_script: &str) -> Result<i32, String> {
    let name = name_of(r)?;
    let file = repo.existing(&name)?;
    let dir = scratch_dir()?;
    let log = dir.join(format!("{name}.log"));
    let mut cmd = Command::new(rust_script);
    cmd.arg(&file)
        .args(["--export", "--log"])
        .arg(&log)
        .arg("--device")
        .arg(&repo.manifest);
    // a check that never ends must not hang the export; a first build of the procedure takes minutes
    let out = lpoco_attended::call::run_limited(
        cmd,
        "",
        Some(EXPORT_LIMIT),
        lpoco_attended::call::DEFAULT_TAIL,
    )
    .map_err(|e| format!("cannot run {rust_script}: {e}"))?;
    if !out.status.success() {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(format!(
            "{name} does not walk (exit {}): {}{}",
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).trim(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let text = std::fs::read_to_string(&log)
        .map_err(|e| format!("{name} left no log {}: {e}", log.display()))?;
    let _ = std::fs::remove_dir_all(&dir);
    let cases = export::cases(&text);
    if cases.is_empty() {
        return Err(format!(
            "{name} has no check to export (a step that only changes the phone is not a test case)"
        ));
    }
    print!("{}", export::definition(&name, &repo.codename, &cases));
    Ok(0)
}

/// The longest an export may take, the procedure's first build included.
const EXPORT_LIMIT: std::time::Duration = std::time::Duration::from_secs(900);
