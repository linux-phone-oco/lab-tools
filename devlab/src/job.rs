// SPDX-License-Identifier: MIT OR Apache-2.0
//! A long job that must outlive the session that started it - a kernel build, an image, a sleep test -
//! is a user unit of the lab's own, `lpoco-job-NAME.service`, started with `systemd-run --user`. The
//! unit holds the lab awake (an idle and sleep inhibitor, held by the job's own process) for as long as
//! the job runs and no longer: nothing else keeps the machine up. systemd is the runner: its journal is the
//! log, a unit name is claimed once (a name in use, running or finished and not yet stopped, is refused),
//! `status` is its state, `stop` ends it and frees the name. No daemon, no queue, no polling.
//!
//!   devlab job start NAME SCRIPT   run the executable SCRIPT as the unit (its output is the log)
//!   devlab job status NAME         running since T | finished ok at T | failed (exit N) at T | not found
//!   devlab job log NAME            the output of the job's current run, from the journal
//!   devlab job stop NAME           end it if it runs, and free the name
//!   devlab job list                every job and its state
//!
//! The unit does not inherit the caller's environment but PATH; a script that needs more sources it
//! (`. ~/Projects/grenudi/ideal-agent/scripts/agent-env`). It runs in the directory `start` was called from.

use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::{Command, Output};

const PREFIX: &str = "lpoco-job-";

/// The unit of a job; a name is letters, digits, `.`, `_`, `-`, at most 64, so it can be a unit name and nothing else.
pub fn unit(name: &str) -> Result<String> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        bail!("a job name is letters, digits, '.', '_' and '-', at most 64: {name:?}");
    }
    Ok(format!("{PREFIX}{name}.service"))
}

fn run(program: &str, args: &[&str]) -> Result<Output> {
    Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("running {program}"))
}

fn text(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

/// The command line `systemd-run` gets: the unit, the job's own inhibitor, the script.
pub fn start_args(name: &str, script: &Path, dir: &Path) -> Result<Vec<String>> {
    let unit = unit(name)?;
    let s = |x: &str| x.to_string();
    Ok(vec![
        s("--user"),
        format!("--unit={}", unit.trim_end_matches(".service")),
        format!("--description=lpoco job {name}"),
        s("-p"),
        s("RemainAfterExit=yes"),
        format!("--working-directory={}", dir.display()),
        // no value: systemd-run takes PATH from the caller's environment
        s("--setenv=PATH"),
        s("systemd-inhibit"),
        s("--what=idle:sleep"),
        s("--who=lpoco-lab"),
        format!("--why=job {name}"),
        s("--mode=block"),
        script.display().to_string(),
    ])
}

pub fn start(name: &str, script: &Path) -> Result<String> {
    let unit = unit(name)?;
    let script = script
        .canonicalize()
        .with_context(|| format!("no such script: {}", script.display()))?;
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(&script)?;
        if !meta.is_file() || meta.permissions().mode() & 0o111 == 0 {
            bail!("{} is not an executable file", script.display());
        }
    }
    if state(&unit)?.is_some() {
        bail!(
            "the job {name} is claimed ({}): `devlab job stop {name}` frees the name",
            describe(name)?
        );
    }
    let dir = std::env::current_dir()?;
    let args = start_args(name, &script, &dir)?;
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let o = run("systemd-run", &refs)?;
    if !o.status.success() {
        bail!(
            "systemd-run refused the job {name}: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        );
    }
    Ok(format!(
        "started {name} ({unit}); `devlab job status {name}`, `devlab job log {name}`, `devlab job stop {name}`"
    ))
}

/// What `systemctl show` says of a unit.
#[derive(Debug, PartialEq)]
pub struct Shown {
    pub active: String,
    pub sub: String,
    pub result: String,
    pub exit: String,
    pub since: String,
    pub until: String,
    pub invocation: String,
}

pub fn parse_show(out: &str) -> Shown {
    let get = |k: &str| {
        out.lines()
            .find_map(|l| l.strip_prefix(&format!("{k}=")))
            .unwrap_or("")
            .to_string()
    };
    Shown {
        active: get("ActiveState"),
        sub: get("SubState"),
        result: get("Result"),
        exit: get("ExecMainStatus"),
        since: get("ExecMainStartTimestamp"),
        until: get("ExecMainExitTimestamp"),
        invocation: get("InvocationID"),
    }
}

/// The unit's state, or None when systemd has no such unit loaded.
fn state(unit: &str) -> Result<Option<Shown>> {
    let o = run(
        "systemctl",
        &[
            "--user",
            "show",
            unit,
            "-p",
            "LoadState,ActiveState,SubState,Result,ExecMainStatus,ExecMainStartTimestamp,ExecMainExitTimestamp,InvocationID",
        ],
    )?;
    let out = text(&o);
    if out.lines().any(|l| l == "LoadState=not-found") {
        return Ok(None);
    }
    Ok(Some(parse_show(&out)))
}

/// The one line that says how a job stands.
pub fn say(name: &str, s: &Shown) -> String {
    match (s.active.as_str(), s.sub.as_str()) {
        ("active", "running") | ("activating", _) => format!("{name}: running since {}", s.since),
        ("active", "exited") => format!("{name}: finished ok at {}", s.until),
        ("failed", _) => format!(
            "{name}: failed (exit {}, {}) at {}",
            s.exit, s.result, s.until
        ),
        _ => format!("{name}: {} ({})", s.active, s.sub),
    }
}

fn describe(name: &str) -> Result<String> {
    Ok(match state(&unit(name)?)? {
        Some(s) => say(name, &s),
        None => format!("{name}: not found"),
    })
}

pub fn status(name: &str) -> Result<String> {
    describe(name)
}

pub fn log(name: &str) -> Result<String> {
    let unit = unit(name)?;
    let Some(s) = state(&unit)? else {
        bail!("{name}: not found");
    };
    let filter = format!("_SYSTEMD_INVOCATION_ID={}", s.invocation);
    let o = run(
        "journalctl",
        &["--user", "--no-pager", "-o", "cat", &filter],
    )?;
    Ok(String::from_utf8_lossy(&o.stdout).trim_end().to_string())
}

pub fn stop(name: &str) -> Result<String> {
    let unit = unit(name)?;
    if state(&unit)?.is_none() {
        return Ok(format!("{name}: not found, nothing to stop"));
    }
    let o = run("systemctl", &["--user", "stop", &unit])?;
    if !o.status.success() {
        bail!(
            "{name}: could not stop: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        );
    }
    let _ = run("systemctl", &["--user", "reset-failed", &unit]);
    Ok(format!("{name}: stopped, the name is free"))
}

pub fn list() -> Result<String> {
    let o = run(
        "systemctl",
        &[
            "--user",
            "list-units",
            "--all",
            "--no-legend",
            "--plain",
            &format!("{PREFIX}*.service"),
        ],
    )?;
    let mut lines = Vec::new();
    for l in text(&o).lines() {
        let Some(u) = l.split_whitespace().next() else {
            continue;
        };
        if let Some(name) = u
            .strip_prefix(PREFIX)
            .and_then(|n| n.strip_suffix(".service"))
        {
            lines.push(describe(name)?);
        }
    }
    Ok(if lines.is_empty() {
        "no jobs".into()
    } else {
        lines.join("\n")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_only_a_unit_name() {
        assert_eq!(
            unit("kernel-build-5").unwrap(),
            "lpoco-job-kernel-build-5.service"
        );
        for bad in ["", "a b", "a/b", "a;b", "$(x)", &"x".repeat(65)] {
            assert!(unit(bad).is_err(), "{bad:?} must be refused");
        }
    }

    #[test]
    fn the_job_holds_the_lab_awake_by_its_own_process_and_nothing_else_does() {
        let a = start_args("b1", Path::new("/x/build.sh"), Path::new("/w")).unwrap();
        let at = a.iter().position(|x| x == "systemd-inhibit").unwrap();
        assert_eq!(a[at + 1], "--what=idle:sleep");
        assert_eq!(a.last().unwrap(), "/x/build.sh");
        assert!(a.contains(&"--unit=lpoco-job-b1".to_string()));
        assert!(a.contains(&"RemainAfterExit=yes".to_string()));
    }

    #[test]
    fn systemd_state_is_said_in_one_line() {
        let show = |active: &str, sub: &str, result: &str, exit: &str| {
            parse_show(&format!(
                "ActiveState={active}\nSubState={sub}\nResult={result}\nExecMainStatus={exit}\nExecMainStartTimestamp=T0\nExecMainExitTimestamp=T1\n"
            ))
        };
        assert_eq!(
            say("b", &show("active", "running", "success", "0")),
            "b: running since T0"
        );
        assert_eq!(
            say("b", &show("active", "exited", "success", "0")),
            "b: finished ok at T1"
        );
        assert_eq!(
            say("b", &show("failed", "failed", "exit-code", "2")),
            "b: failed (exit 2, exit-code) at T1"
        );
    }

    /// The real thing, on a lab with a user manager: `cargo test -- --ignored job_runs`.
    #[test]
    #[ignore]
    fn job_runs_logs_finishes_and_frees_its_name() {
        let dir = std::env::temp_dir().join(format!("lpoco-job-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("j.sh");
        std::fs::write(&script, "#!/bin/sh\necho hello-from-job\nsleep 2\n").unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let name = format!("selftest-{}", std::process::id());
        start(&name, &script).unwrap();
        assert!(start(&name, &script).is_err(), "a claimed name is refused");
        assert!(status(&name).unwrap().contains("running"));
        std::thread::sleep(std::time::Duration::from_secs(4));
        assert!(
            status(&name).unwrap().contains("finished ok"),
            "{}",
            status(&name).unwrap()
        );
        assert!(log(&name).unwrap().contains("hello-from-job"));
        assert!(list().unwrap().contains(&name));
        stop(&name).unwrap();
        assert!(status(&name).unwrap().contains("not found"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
