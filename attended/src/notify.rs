// SPDX-License-Identifier: MIT OR Apache-2.0
//! The author reached directly: a pending prompt is also published to his ntfy topic (step, what to do, what to
//! check, until when), with a button that posts `ID ok` to the reply topic; a reply on that topic whose first word
//! is the prompt's id is the answer, the rest of it the words (`ID a blank screen`). The server, the two topics and
//! an access token are in one mode-600 file on the lab machine, `key: value` lines (server, topic, reply_topic,
//! token), never in a repository. The calls are curl's (ntfy is plain HTTP: docs.ntfy.sh/publish and
//! docs.ntfy.sh/subscribe/api), the token handed over on curl's standard input, never in its arguments.

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::prompt::{Answer, Prompt};

/// How often the reply topic is asked.
const EVERY: Duration = Duration::from_secs(3);

/// Where the author is reached.
pub struct Notify {
    server: String,
    topic: String,
    reply_topic: String,
    token: Option<String>,
    asked: Option<Instant>,
    since: u64,
    said: bool,
}

impl Notify {
    /// The notify file, or why there is none to use: the run says it and goes on with the terminal and the answer file.
    pub fn read(path: &Path) -> Result<Notify, String> {
        let meta =
            std::fs::metadata(path).map_err(|_| format!("no notify file at {}", path.display()))?;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err(format!(
                "{} is readable by others (it holds a token): chmod it to mode 600",
                path.display()
            ));
        }
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let get = |key: &str| {
            text.lines()
                .find_map(|l| l.strip_prefix(key)?.strip_prefix(':'))
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        let need = |key: &str| get(key).ok_or(format!("{} has no {key}", path.display()));
        Ok(Notify {
            server: need("server")?.trim_end_matches('/').to_string(),
            topic: need("topic")?,
            reply_topic: need("reply_topic")?,
            token: Some(need("token")?),
            asked: None,
            since: 0,
            said: false,
        })
    }

    /// Publish the prompt once, under its id; the answer it says it wants goes to the reply topic.
    pub fn publish(&mut self, id: &str, p: &Prompt, until: &str) -> Result<(), String> {
        let body = format!(
            "do: {}\ncheck: {}\nuntil: {until}\nreply: {id} ok, or {id} and what you saw instead",
            p.to_do, p.to_check
        );
        let button = format!(
            "http, ok, {}/{}, method=POST, body={id} ok, clear=true",
            self.server, self.reply_topic
        );
        let cfg = format!(
            "header = \"Title: {}\"\nheader = \"Actions: {}\"\ndata-raw = \"{}\"\nurl = \"{}/{}\"\n{}",
            quote(&line(p.step)),
            quote(&button),
            quote(&body),
            self.server,
            self.topic,
            self.auth()
        );
        let sent = self.curl(&cfg)?;
        // replies are asked for from the server's own stamp of the publish, never the lab clock
        self.since = serde_json::from_str::<serde_json::Value>(&sent)
            .ok()
            .and_then(|m| m.get("time")?.as_u64())
            .unwrap_or_else(crate::prompt::now);
        Ok(())
    }

    /// The reply for this prompt, when one has come: asked at most every few seconds.
    pub fn reply(&mut self, id: &str) -> Option<Answer> {
        if self.asked.is_some_and(|t| t.elapsed() < EVERY) {
            return None;
        }
        self.asked = Some(Instant::now());
        let cfg = format!(
            "url = \"{}/{}/json?poll=1&since={}\"\n{}",
            self.server,
            self.reply_topic,
            self.since,
            self.auth()
        );
        let lines = match self.curl(&cfg) {
            Ok(l) => l,
            Err(e) => {
                if !std::mem::replace(&mut self.said, true) {
                    eprintln!("  (the reply topic cannot be read: {e})");
                }
                return None;
            }
        };
        lines.lines().find_map(|l| {
            let m: serde_json::Value = serde_json::from_str(l).ok()?;
            if m.get("event")?.as_str()? != "message" {
                return None;
            }
            let text = m.get("message")?.as_str()?.trim();
            let (first, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
            (first == id && !rest.trim().is_empty()).then(|| Answer {
                text: rest.trim().to_string(),
                who: "ntfy".into(),
            })
        })
    }

    fn auth(&self) -> String {
        self.token
            .as_ref()
            .map(|t| format!("header = \"Authorization: Bearer {}\"\n", quote(t)))
            .unwrap_or_default()
    }

    /// One curl call, its whole config (url, headers, token, body) on its standard input.
    fn curl(&self, cfg: &str) -> Result<String, String> {
        let mut c = Command::new("curl")
            .args([
                "-sS",
                "--fail",
                "--connect-timeout",
                "2",
                "--max-time",
                "5",
                "-K",
                "-",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("curl: {e}"))?;
        c.stdin
            .take()
            .ok_or("curl: no stdin")?
            .write_all(cfg.as_bytes())
            .map_err(|e| format!("curl: {e}"))?;
        let o = c.wait_with_output().map_err(|e| format!("curl: {e}"))?;
        if o.status.success() {
            Ok(String::from_utf8_lossy(&o.stdout).into_owned())
        } else {
            Err(format!(
                "curl: {}",
                String::from_utf8_lossy(&o.stderr).trim()
            ))
        }
    }
}

/// A header value is one line: control characters become spaces.
fn line(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// A value inside curl's double quotes, as curl's config file reads them.
fn quote(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_is_one_line_whatever_the_step_says() {
        assert_eq!(line("a\r\nX-Injected: 1\tb"), "a  X-Injected: 1 b");
    }

    #[test]
    fn a_notify_file_without_a_token_is_refused() {
        let f = std::env::temp_dir().join(format!("notify-notoken-{}", std::process::id()));
        std::fs::write(&f, "server: https://x\ntopic: a\nreply_topic: b\n").unwrap();
        std::fs::set_permissions(&f, PermissionsExt::from_mode(0o600)).unwrap();
        let e = Notify::read(&f).err().unwrap();
        assert!(e.contains("no token"), "{e}");
    }
}
