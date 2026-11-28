// SPDX-License-Identifier: MIT OR Apache-2.0
//! Where the author's answer can come from: the terminal, or a file another session writes. A pending prompt is
//! written to DIR/prompt (mode 0600: what to do, what to check, until when it waits) and DIR/answer is polled;
//! whichever answers first is taken and says who answered. DIR is ~/.cache/lpoco-attended/DEVICE.

use std::io::{BufRead, IsTerminal};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// How long a prompt waits on its answer file when there is no terminal and no limit was given.
const NO_TERMINAL_LIMIT: Duration = Duration::from_secs(300);
const POLL: Duration = Duration::from_millis(200);
static PROMPTS: AtomicU64 = AtomicU64::new(0);

/// What the author said, and by which way it came.
pub struct Answer {
    pub text: String,
    pub who: String,
}

/// What the prompt is: shown on the terminal and written to the prompt file.
pub struct Prompt<'a> {
    pub step: &'a str,
    pub to_do: &'a str,
    pub to_check: &'a str,
}

/// The two files of one device, in a folder only the user may enter.
pub struct Files {
    dir: PathBuf,
    notify: Option<PathBuf>,
}

impl Files {
    pub fn new(dir: PathBuf, notify: Option<PathBuf>) -> Files {
        Files { dir, notify }
    }

    fn prompt(&self) -> PathBuf {
        self.dir.join("prompt")
    }

    fn answer(&self) -> PathBuf {
        self.dir.join("answer")
    }

    /// File the prompt; the id it was filed under (an answer may say `for: ID` to be sure it is for this prompt).
    fn open(&self, p: &Prompt, limit: Option<Duration>) -> Result<String, String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
        std::fs::set_permissions(
            &self.dir,
            std::os::unix::fs::PermissionsExt::from_mode(0o700),
        )
        .map_err(|e| format!("{}: {e}", self.dir.display()))?;
        // an answer left from an earlier prompt is never taken for this one
        let _ = std::fs::remove_file(self.answer());
        let until = limit.map_or("none".to_string(), |l| (now() + l.as_secs()).to_string());
        let id = format!(
            "{}-{}",
            std::process::id(),
            PROMPTS.fetch_add(1, Ordering::Relaxed)
        );
        self.write_prompt(
            p,
            &format!("id: {id}\nwaits-until: {until}\nstate: waiting\n"),
        )?;
        Ok(id)
    }

    fn write_prompt(&self, p: &Prompt, tail: &str) -> Result<(), String> {
        let text = format!(
            "step: {}\ndo: {}\ncheck: {}\nanswer: write ok, or what you saw instead, to {}\n{tail}",
            p.step,
            p.to_do,
            p.to_check,
            self.answer().display()
        );
        let tmp = self.dir.join("prompt.tmp");
        let _ = std::fs::remove_file(&tmp);
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::io::Write::write_all(&mut f, text.as_bytes())
            .map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, self.prompt())
            .map_err(|e| format!("{}: {e}", self.prompt().display()))
    }

    /// The answer file, taken (and removed) when one is there: its first line is the answer, an optional
    /// `by: NAME` line says who wrote it, else its owner's user id does.
    fn take(&self, id: &str) -> Option<Answer> {
        let path = self.answer();
        let meta = std::fs::symlink_metadata(&path).ok()?;
        if !meta.is_file() {
            return None;
        }
        let text = std::fs::read_to_string(&path).ok()?;
        // a writer that has not finished (no newline yet) is waited for, never taken half
        if !text.ends_with('\n') {
            return None;
        }
        let _ = std::fs::remove_file(&path);
        let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
        let answer = lines.next()?.to_string();
        let rest: Vec<&str> = lines.collect();
        if rest
            .iter()
            .filter_map(|l| l.strip_prefix("for:"))
            .any(|f| f.trim() != id)
        {
            return None; // meant for another prompt: dropped
        }
        let by = rest
            .iter()
            .find_map(|l| l.strip_prefix("by:"))
            .map(|b| b.trim().to_string());
        let who = by.unwrap_or_else(|| format!("user {}", meta.uid()));
        Some(Answer {
            text: answer,
            who: format!("file, written by {who}"),
        })
    }

    fn close(&self, timed_out: bool, p: &Prompt) {
        let _ = std::fs::remove_file(self.answer());
        if timed_out {
            let _ = self.write_prompt(p, "state: timed out\n");
        } else {
            let _ = std::fs::remove_file(self.prompt());
        }
    }
}

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The terminal's lines, read by one thread for the whole run so a line typed late is never lost to a prompt
/// that has ended; `None` is the end of input.
fn lines() -> &'static Mutex<Receiver<Option<String>>> {
    static LINES: OnceLock<Mutex<Receiver<Option<String>>>> = OnceLock::new();
    LINES.get_or_init(|| {
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            loop {
                line.clear();
                match std::io::stdin().lock().read_line(&mut line) {
                    Ok(n) if n > 0 => {
                        if tx.send(Some(line.trim().to_string())).is_err() {
                            return;
                        }
                    }
                    _ => {
                        let _ = tx.send(None);
                        return;
                    }
                }
            }
        });
        Mutex::new(rx)
    })
}

/// How a wait ended.
enum Ended {
    Answered(Answer),
    TimedOut(Duration),
    NobodyAnswered,
    Stopped(&'static str),
}

/// Ask: the terminal and the answer file, whichever answers first. At the end of the terminal's input (EOF is
/// no answer) the wait goes on for the file while the prompt is filed, up to the limit - 300 s when there is no
/// terminal and none was given; past it the run fails and the prompt file says so.
pub fn ask(files: &Files, p: &Prompt, limit: Option<Duration>) -> Result<Answer, String> {
    let no_terminal = !std::io::stdin().is_terminal();
    // with no terminal the end of input is no answer: the file is the way, and it is waited on for a default time
    let limit = limit.or(no_terminal.then_some(NO_TERMINAL_LIMIT));
    // a file that cannot be written is said and the terminal still asks: the file is the extra way
    let id = files
        .open(p, limit)
        .map_err(|e| eprintln!("  (no prompt file: {e})"))
        .ok();
    if no_terminal && id.is_some() {
        eprintln!("  (no terminal, waiting on the answer file)");
    }
    // the author is also reached on his phone when a notify file says where; its absence was said at the start
    let mut notify = files
        .notify
        .as_deref()
        .and_then(|f| crate::notify::Notify::read(f).ok());
    if let (Some(n), Some(id)) = (notify.as_mut(), id.as_deref()) {
        let until = limit.map_or("none".to_string(), |l| {
            format!("{} s from now", l.as_secs())
        });
        if let Err(e) = n.publish(id, p, &until) {
            eprintln!("  (not published to the author: {e})");
        }
    }
    let began = Instant::now();
    let rx = lines().lock().unwrap_or_else(|e| e.into_inner());
    if std::io::stdin().is_terminal() {
        // what was typed before this prompt was shown is not an answer to it
        while rx.try_recv().is_ok() {}
    }
    let mut ended = false;
    let outcome = loop {
        if let Some((_, name)) = crate::stop::came() {
            break Ended::Stopped(name);
        }
        if let Some(a) = id.as_deref().and_then(|id| files.take(id)) {
            break Ended::Answered(a);
        }
        if let Some(a) = notify
            .as_mut()
            .zip(id.as_deref())
            .and_then(|(n, id)| n.reply(id))
        {
            break Ended::Answered(a);
        }
        match limit {
            Some(l) if began.elapsed() >= l => break Ended::TimedOut(l),
            _ if ended && (limit.is_none() || id.is_none()) => break Ended::NobodyAnswered,
            _ => {}
        }
        if ended {
            std::thread::sleep(POLL);
            continue;
        }
        match rx.recv_timeout(POLL) {
            Ok(Some(l)) if !l.is_empty() => {
                break Ended::Answered(Answer {
                    text: l,
                    who: "terminal".into(),
                })
            }
            Ok(Some(_)) | Err(RecvTimeoutError::Timeout) => {}
            Ok(None) | Err(RecvTimeoutError::Disconnected) => ended = true,
        }
    };
    if id.is_some() {
        files.close(matches!(outcome, Ended::TimedOut(_)), p);
    }
    match outcome {
        Ended::Answered(a) => Ok(a),
        Ended::TimedOut(l) => Err(format!("nobody answered within {} s", l.as_secs())),
        Ended::NobodyAnswered => Err("nobody answered".to_string()),
        Ended::Stopped(name) => Err(format!("stopped by {name} while waiting for an answer")),
    }
}

/// The folder of one device's prompt files under `base`.
pub fn dir_for(base: &Path, codename: &str) -> PathBuf {
    base.join(codename)
}
