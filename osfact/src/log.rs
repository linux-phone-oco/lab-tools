// SPDX-License-Identifier: MIT OR Apache-2.0
//! The run's log: every operation an install run asks of the device - each shell line sent and the device's answer
//! with its status, the push, the boot, the way back - written to one file as it happens, each with the seconds
//! since the run began. A run that stops says which file to read (osfact's second phone run could not tell why its
//! check was not true: the device's side of it was kept nowhere).

use crate::install::{Device, Shell, WayBack};
use std::io::Write;
use std::time::Instant;

/// A device whose every operation is written to `out` before its result is handed back.
pub struct Logged<'a> {
    inner: &'a mut dyn Device,
    out: Box<dyn Write + 'a>,
    began: Instant,
}

impl<'a> Logged<'a> {
    pub fn new(inner: &'a mut dyn Device, out: Box<dyn Write + 'a>) -> Logged<'a> {
        Logged {
            inner,
            out,
            began: Instant::now(),
        }
    }

    fn note(&mut self, text: &str) -> Result<(), String> {
        let at = self.began.elapsed().as_secs_f32();
        writeln!(self.out, "[{at:7.1} s] {text}")
            .and_then(|()| self.out.flush())
            .map_err(|e| format!("the run's log: {e}"))
    }

    /// An operation's result noted, then returned as it was: a result that failed is noted with its error.
    fn after<T>(
        &mut self,
        done: &str,
        r: Result<T, String>,
        show: impl Fn(&T) -> String,
    ) -> Result<T, String> {
        match &r {
            Ok(v) => self.note(&format!("{done} {}", show(v)))?,
            Err(e) => self.note(&format!("{done} failed: {e}"))?,
        }
        r
    }
}

impl Device for Logged<'_> {
    fn boot(&mut self, file: &str) -> Result<(), String> {
        self.note(&format!("> fastboot boot {file}"))?;
        let r = self.inner.boot(file);
        self.after("< boot", r, |()| "ended".into())
    }

    fn connect(&mut self, wait_s: u32) -> Result<(), String> {
        self.note(&format!("> connect (up to {wait_s} s)"))?;
        let r = self.inner.connect(wait_s);
        self.after("< connect", r, |()| "answered".into())
    }

    fn shell(&mut self, command: &str, wait_s: u32) -> Result<Shell, String> {
        self.note(&format!("> {command}"))?;
        let r = self.inner.shell(command, wait_s);
        let r = self.after(&format!("< (up to {wait_s} s) shell"), r, |s| {
            format!("status {}", s.status)
        });
        if let Ok(s) = &r {
            for line in s.text.lines().filter(|l| !l.trim().is_empty()) {
                self.note(&format!("< {}", line.trim_end()))?;
            }
        }
        r
    }

    fn send(&mut self, port: u16, file: &str, wait_s: u32) -> Result<u64, String> {
        self.note(&format!("> send {file} to port {port} (up to {wait_s} s)"))?;
        let r = self.inner.send(port, file, wait_s);
        self.after("< send", r, |n| format!("{n} bytes"))
    }

    fn way_back(&mut self, back: &WayBack) -> Result<(), String> {
        self.note("> way back")?;
        let r = self.inner.way_back(back);
        self.after("< way back", r, |()| "sent".into())
    }

    fn back_on(&mut self, argv: &[String], wait_s: u32) -> Result<(), String> {
        self.note(&format!("> back on: {} (up to {wait_s} s)", argv.join(" ")))?;
        let r = self.inner.back_on(argv, wait_s);
        self.after("< back on", r, |()| "answered".into())
    }
}
