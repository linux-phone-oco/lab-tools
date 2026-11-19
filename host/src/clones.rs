// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco host clones` (backlog host-clones-fresh): the shared clones under the declared folders kept at origin/main.
//! Only a commons place (an AGENTS.md at its top) is touched: any other repository is named and left. A clone that is
//! clean, on main and behind is fast-forwarded; any other is left exactly as it is and named with why.

use crate::Clones;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const TIMER: &str = "lpoco-host-clones.timer";
pub const SERVICE: &str = "lpoco-host-clones.service";

/// What happened to one clone.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    UpToDate,
    /// behind by this many commits (fast-forwarded, or only said when not asked to)
    Behind(u64),
    /// a repository that is not a commons place (no AGENTS.md at its top): not the lab's to move
    NotCommons,
    /// left as it is, and why
    Left(String),
    /// the fetch from origin failed (the clone was left as it is)
    FetchFailed(String),
}

pub fn timer_text(every: &str) -> String {
    format!("[Unit]\nDescription=lpoco host clones: fast-forward the shared clones\n\n[Timer]\nOnBootSec=3min\nOnUnitActiveSec={every}\n\n[Install]\nWantedBy=timers.target\n")
}

pub fn service_text(lab: Option<&str>) -> String {
    let args = lab
        .map(|l| format!(" --lab {}", crate::follow::unit_quoted(l)))
        .unwrap_or_default();
    format!("[Unit]\nDescription=lpoco host clones: fast-forward each clean main checkout to origin/main, name the others\n\n[Service]\nType=oneshot\nTimeoutStartSec=10min\nExecStart=%h/.local/bin/lpoco-host clones now{args}\n")
}

/// A declaration that cannot reach a unit file or leave the lab user's home.
pub fn valid(c: &Clones) -> Result<(), String> {
    crate::follow::valid_every("[clones]", &c.every)?;
    if c.folders.is_empty() {
        return Err("[clones] folders names no folder".into());
    }
    for f in &c.folders {
        let ok = f.starts_with("~/")
            && f.len() > 2
            && !f.split('/').any(|p| p == "..")
            && !f.chars().any(|ch| ch.is_control());
        if !ok {
            return Err(format!(
                "[clones] folder {f:?} must be under the lab user's home, as ~/Projects"
            ));
        }
    }
    Ok(())
}

/// The shared clones of one folder: the folder itself when it is a clone, else each of its direct subfolders that
/// is (a worktree's `.git` is a file, so a worktree is not one).
pub fn clones_in(folder: &Path) -> Result<Vec<PathBuf>, String> {
    if folder.join(".git").is_dir() {
        return Ok(vec![folder.to_path_buf()]);
    }
    let mut v: Vec<PathBuf> = std::fs::read_dir(folder)
        .map_err(|e| format!("{}: cannot be read ({e})", folder.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join(".git").is_dir())
        .collect();
    v.sort();
    Ok(v)
}

/// A failed git call: its exit status (None when it did not start or was killed) and the first line it said.
struct GitErr {
    code: Option<i32>,
    said: String,
}

/// git, quietly: no optional locks (a status here must never make a session's own `git add` fail), no prompts,
/// and a stalled connection ends.
fn git(dir: &Path, args: &[&str]) -> Result<String, GitErr> {
    let o = Command::new("git")
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "http.lowSpeedLimit=1000",
            "-c",
            "http.lowSpeedTime=15",
        ])
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env(
            "GIT_SSH_COMMAND",
            "ssh -o BatchMode=yes -o ConnectTimeout=10",
        )
        .output()
        .map_err(|e| GitErr {
            code: None,
            said: e.to_string(),
        })?;
    if o.status.success() {
        Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
    } else {
        Err(GitErr {
            code: o.status.code(),
            said: String::from_utf8_lossy(&o.stderr)
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string(),
        })
    }
}

/// Whether the clone is on main, clean (untracked files do not count: git refuses a fast-forward they would hit),
/// else why not.
fn ready(dir: &Path) -> Result<(), String> {
    match git(dir, &["symbolic-ref", "--short", "-q", "HEAD"]) {
        Ok(b) if b == "main" => {}
        Ok(b) => return Err(format!("on {b}, not main")),
        // exit 1 is "not a symbolic ref": detached; anything else is a git that cannot read the clone
        Err(GitErr { code: Some(1), .. }) => return Err("detached".into()),
        Err(e) => return Err(format!("HEAD unreadable: {}", e.said)),
    }
    match git(dir, &["status", "--porcelain", "--untracked-files=no"]) {
        Ok(s) if s.is_empty() => Ok(()),
        Ok(_) => Err("uncommitted changes".into()),
        Err(e) => Err(format!("status failed: {}", e.said)),
    }
}

/// Brings one clone to origin/main when it may be (`apply`), else says what it would do.
pub fn freshen(dir: &Path, apply: bool) -> Outcome {
    // the author's own repositories sit among the commons clones: only a commons place has an AGENTS.md at its top
    if !dir.join("AGENTS.md").is_file() {
        return Outcome::NotCommons;
    }
    if let Err(why) = ready(dir) {
        return Outcome::Left(why);
    }
    // an explicit refspec: origin/main is updated whatever the clone's own fetch configuration says
    if let Err(e) = git(
        dir,
        &[
            "fetch",
            "--quiet",
            "origin",
            "+refs/heads/main:refs/remotes/origin/main",
        ],
    ) {
        return Outcome::FetchFailed(e.said);
    }
    match git(dir, &["merge-base", "--is-ancestor", "HEAD", "origin/main"]) {
        Ok(_) => {}
        Err(GitErr { code: Some(1), .. }) => {
            return Outcome::Left("diverged from origin/main".into())
        }
        Err(e) => return Outcome::Left(format!("merge-base failed: {}", e.said)),
    }
    let behind: u64 = match git(dir, &["rev-list", "--count", "HEAD..origin/main"])
        .map_err(|e| e.said)
        .and_then(|n| {
            n.parse()
                .map_err(|_| format!("count {n:?} is not a number"))
        }) {
        Ok(n) => n,
        Err(e) => return Outcome::Left(format!("rev-list failed: {e}")),
    };
    if behind == 0 {
        return Outcome::UpToDate;
    }
    if apply {
        // the fetch took time: the clone may have changed under a session since it was looked at
        if let Err(why) = ready(dir) {
            return Outcome::Left(format!("{why} (changed during the fetch)"));
        }
        if let Err(e) = git(dir, &["merge", "--ff-only", "--quiet", "origin/main"]) {
            return Outcome::Left(format!("fast-forward failed: {}", e.said));
        }
    }
    Outcome::Behind(behind)
}

/// One line per clone, as the unit's journal and the person read it.
pub fn line(name: &str, o: &Outcome, apply: bool) -> String {
    match o {
        Outcome::UpToDate => format!("{name}: up to date"),
        Outcome::Behind(n) => {
            let s = if *n == 1 { "" } else { "s" };
            let verb = if apply { "updated" } else { "would update" };
            format!("{name}: {verb} {n} commit{s}")
        }
        Outcome::NotCommons => format!("{name}: not a commons place, left"),
        Outcome::Left(why) => format!("{name}: left alone ({why})"),
        Outcome::FetchFailed(why) => format!("{name}: left alone (fetch failed: {why})"),
    }
}
