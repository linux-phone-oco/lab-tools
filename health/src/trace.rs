// SPDX-License-Identifier: MIT OR Apache-2.0
//! The trace hook of `lpoco-health suspend cycle --trace`: the dynamic_debug sites and irq tracepoints the manifest
//! names ([health] suspend_trace_dyndbg, suspend_trace_irq) are switched on before the suspend and off after the
//! wake, and what they said is collected into the cycle's report. Replaces surya-lab's sleep/bt-trace.sh.
//!
//! Interfaces (the kernel's documentation): tracefs `tracing_on`, `trace`, `events/irq/irq_handler_entry/enable` and
//! `filter` (Documentation/trace/events.rst; a filter is cleared by writing 0), and dynamic debug's `control`
//! (admin-guide/dynamic-debug-howto.rst: `QUERY +p`, `QUERY -p`). Every path lies under the cycle's root.
//!
//! What goes back: tracing_on, the irq event's enable and its filter are written back as they were. A dynamic_debug
//! site is switched off with `-p`: one the phone already had on (a `dyndbg=` boot argument) is off afterwards.
//! A cycle cut off while tracing leaves what it switched on named, with the values to go back to, in `trace.json`:
//! the next cycle puts those back first. The record is written before anything is switched on, or nothing is.

use serde_json::{json, Value};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const TRACING: &str = "sys/kernel/tracing";
const CONTROL: &str = "sys/kernel/debug/dynamic_debug/control";
const EVENT: &str = "events/irq/irq_handler_entry";
const KEPT: &str = "trace.json";
/// How much of the kernel log is kept beside the trace: the sites print there.
const DMESG_LINES: usize = 150;

/// What the manifest asks to be traced. A dynamic_debug query is one query: the env file joins them with `;`, so a
/// `;` cannot arrive inside one; a newline cannot either (the env is one line a value).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Sites {
    pub dyndbg: Vec<String>,
    pub irq: Vec<u32>,
}

impl Sites {
    pub fn is_empty(&self) -> bool {
        self.dyndbg.is_empty() && self.irq.is_empty()
    }

    /// A query that already carries a flag would be two instructions (`file x.c +p` then our `+p`).
    pub fn problems(&self) -> Vec<String> {
        self.dyndbg
            .iter()
            .filter(|q| q.contains(['\n', '\r']) || q.ends_with(['+', '-']) || q.contains(" +") || q.contains(" -"))
            .map(|q| format!("the dynamic_debug query {q:?} is not one query without a flag (the tool adds +p and -p)"))
            .collect()
    }
}

/// What tracefs held before: written back at the end.
#[derive(Debug, Clone)]
struct Held {
    tracing_on: String,
    enable: String,
    filter: String,
}

/// What a traced cycle collected; `problems` are the reads and writes that failed, never hidden.
pub struct Collected {
    pub irq_events: usize,
    pub trace_file: PathBuf,
    pub dmesg_file: PathBuf,
    pub problems: Vec<String>,
}

/// Tracing switched on for one cycle; `stop` (or dropping it) puts every switch back.
pub struct Tracing {
    root: PathBuf,
    out: PathBuf,
    sites: Sites,
    held: Held,
    on: bool,
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// dynamic_debug's control takes one command per write; the file is appended to, never truncated.
fn control(root: &Path, line: &str) -> Result<(), String> {
    let p = root.join(CONTROL);
    let mut f = OpenOptions::new()
        .append(true)
        .open(&p)
        .map_err(|e| format!("{}: {e}", p.display()))?;
    f.write_all(format!("{line}\n").as_bytes())
        .map_err(|e| format!("{}: {e}", p.display()))
}

fn filter_for(irqs: &[u32]) -> String {
    irqs.iter()
        .map(|n| format!("irq == {n}"))
        .collect::<Vec<_>>()
        .join(" || ")
}

impl Tracing {
    /// Records what is to be put back, then switches the sites on.
    pub fn start(root: &Path, out: &Path, sites: Sites) -> Result<Tracing, String> {
        let t = root.join(TRACING);
        let old_filter = read(&t.join(EVENT).join("filter"));
        let held = Held {
            tracing_on: read(&t.join("tracing_on")),
            enable: read(&t.join(EVENT).join("enable")),
            // tracefs says `none` for no filter; it is cleared by writing 0
            filter: if old_filter == "none" {
                "0".into()
            } else {
                old_filter
            },
        };
        let kept = json!({"dyndbg": sites.dyndbg, "irq": sites.irq,
                          "tracing_on": held.tracing_on, "enable": held.enable, "filter": held.filter});
        std::fs::create_dir_all(out)
            .and_then(|_| std::fs::write(out.join(KEPT), kept.to_string()))
            .map_err(|e| {
                format!(
                    "cannot keep the record of what is switched on ({}): {e}",
                    out.join(KEPT).display()
                )
            })?;
        let mut me = Tracing {
            root: root.to_path_buf(),
            out: out.to_path_buf(),
            sites,
            held,
            on: true,
        };
        write(&t.join("tracing_on"), "0")?;
        write(&t.join("trace"), "")?;
        for q in &me.sites.dyndbg {
            control(root, &format!("{q} +p"))?;
        }
        if !me.sites.irq.is_empty() {
            write(&t.join(EVENT).join("filter"), &filter_for(&me.sites.irq))?;
            write(&t.join(EVENT).join("enable"), "1")?;
        }
        write(&t.join("tracing_on"), "1")?;
        me.on = true;
        Ok(me)
    }

    /// A cycle cut off while tracing left its sites on (trace.json names them and what to go back to): put them back.
    pub fn put_right_what_a_cut_off_cycle_left(root: &Path, out: &Path) {
        let kept = out.join(KEPT);
        let Some(v) = std::fs::read_to_string(&kept)
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        else {
            return;
        };
        let strings = |k: &str| -> Vec<String> {
            v[k].as_array()
                .into_iter()
                .flatten()
                .filter_map(|q| q.as_str().map(String::from))
                .collect()
        };
        let sites = Sites {
            dyndbg: strings("dyndbg"),
            irq: v["irq"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|n| n.as_u64().map(|n| n as u32))
                .collect(),
        };
        let text = |k: &str, d: &str| v[k].as_str().unwrap_or(d).to_string();
        let held = Held {
            tracing_on: text("tracing_on", "0"),
            enable: text("enable", "0"),
            filter: text("filter", "0"),
        };
        if Tracing::switch_off(root, &sites, &held).is_empty() {
            let _ = std::fs::remove_file(kept);
        }
    }

    /// Puts each switch back; what could not be is named (the record stays then).
    fn switch_off(root: &Path, sites: &Sites, held: &Held) -> Vec<String> {
        let t = root.join(TRACING);
        let mut problems = Vec::new();
        let mut put = |path: PathBuf, text: &str| {
            if let Err(e) = write(&path, text) {
                problems.push(e);
            }
        };
        put(t.join("tracing_on"), "0");
        if !sites.irq.is_empty() {
            put(t.join(EVENT).join("enable"), "0");
            put(t.join(EVENT).join("filter"), &held.filter);
            put(t.join(EVENT).join("enable"), &held.enable);
        }
        put(t.join("tracing_on"), &held.tracing_on);
        for q in &sites.dyndbg {
            if let Err(e) = control(root, &format!("{q} -p")) {
                problems.push(e);
            }
        }
        problems
    }

    /// Stops tracing, reads what it said into `out` (cycle.trace, cycle.dmesg), puts every switch back.
    pub fn stop(mut self) -> Collected {
        let t = self.root.join(TRACING);
        let mut problems = Vec::new();
        if let Err(e) = write(&t.join("tracing_on"), "0") {
            problems.push(e);
        }
        let trace = match std::fs::read_to_string(t.join("trace")) {
            Ok(s) => s,
            Err(e) => {
                problems.push(format!("the trace could not be read: {e}"));
                String::new()
            }
        };
        let irq_events = trace
            .lines()
            .filter(|l| l.contains("irq_handler_entry"))
            .count();
        let dmesg = match Command::new("dmesg").output() {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
            Ok(o) => {
                problems.push(format!("dmesg failed: {}", o.status));
                String::new()
            }
            Err(e) => {
                problems.push(format!("dmesg not run: {e}"));
                String::new()
            }
        };
        let lines: Vec<&str> = dmesg.lines().collect();
        let tail = lines[lines.len().saturating_sub(DMESG_LINES)..].join("\n");
        let trace_file = self.out.join("cycle.trace");
        let dmesg_file = self.out.join("cycle.dmesg");
        for (file, text) in [(&trace_file, trace), (&dmesg_file, format!("{tail}\n"))] {
            if let Err(e) = std::fs::write(file, text) {
                problems.push(format!("{}: {e}", file.display()));
            }
        }
        problems.extend(self.put_back());
        Collected {
            irq_events,
            trace_file,
            dmesg_file,
            problems,
        }
    }

    fn put_back(&mut self) -> Vec<String> {
        if !self.on {
            return vec![];
        }
        self.on = false;
        let problems = Tracing::switch_off(&self.root, &self.sites, &self.held);
        if problems.is_empty() {
            let _ = std::fs::remove_file(self.out.join(KEPT));
        }
        problems
    }
}

impl Drop for Tracing {
    fn drop(&mut self) {
        // a cycle that leaves early still switches the sites off
        let _ = self.put_back();
    }
}
