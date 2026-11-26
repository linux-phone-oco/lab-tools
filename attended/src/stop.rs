// SPDX-License-Identifier: MIT OR Apache-2.0
//! A run ended by a signal (SIGINT, SIGTERM, SIGHUP) is a run that failed: the handlers only note which signal came,
//! and the run reads it between its calls (a call waited on, a prompt waited on, the start of a step), so the way
//! backs owed run on the run's own thread as they do for a step that returned an error. Once the way backs have
//! begun, a signal changes nothing: they are not cut short, and the calls they make leave this run's process
//! group so a Ctrl-C meant for the run does not reach them.

use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

const SIGNALS: [(i32, &str); 3] = [(SIGINT, "SIGINT"), (SIGTERM, "SIGTERM"), (SIGHUP, "SIGHUP")];

static CAME: OnceLock<Vec<Arc<AtomicBool>>> = OnceLock::new();
static LEAVING: AtomicBool = AtomicBool::new(false);

/// From now on a signal ends the run through its way backs instead of killing it. A handler that cannot be
/// installed stops the run: a run that cannot be stopped safely is not started.
pub(crate) fn catch() -> Result<(), String> {
    if CAME.get().is_some() {
        return Ok(());
    }
    let mut flags = Vec::new();
    for (sig, name) in SIGNALS {
        let flag = Arc::new(AtomicBool::new(false));
        signal_hook::flag::register(sig, Arc::clone(&flag))
            .map_err(|e| format!("cannot catch {name}: {e}"))?;
        flags.push(flag);
    }
    let _ = CAME.set(flags);
    Ok(())
}

/// The first signal that came (SIGINT before SIGTERM before SIGHUP when several did), as its number and name;
/// none once the way backs have begun.
pub(crate) fn came() -> Option<(i32, &'static str)> {
    if LEAVING.load(Ordering::SeqCst) {
        return None;
    }
    let flags = CAME.get()?;
    SIGNALS
        .iter()
        .zip(flags)
        .find(|(_, f)| f.load(Ordering::SeqCst))
        .map(|(s, _)| *s)
}

/// The way backs begin: no signal ends the run from here, and calls leave the run's process group.
pub(crate) fn leaving() {
    LEAVING.store(true, Ordering::SeqCst);
}

pub(crate) fn is_leaving() -> bool {
    LEAVING.load(Ordering::SeqCst)
}

/// Sleep `d`, ending early with the error a stopped run gives when a signal comes: a wait that tries again (a phone
/// that is coming back, a bus that is filling) never outlasts the signal.
pub(crate) fn sleep(d: std::time::Duration) -> Result<(), String> {
    let end = std::time::Instant::now() + d;
    loop {
        if let Some((_, name)) = came() {
            return Err(format!("stopped by {name} while waiting"));
        }
        let left = end.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            return Ok(());
        }
        std::thread::sleep(left.min(std::time::Duration::from_millis(100)));
    }
}
