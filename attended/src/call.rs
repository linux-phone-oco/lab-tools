// SPDX-License-Identifier: MIT OR Apache-2.0
//! One call to a program, with an optional time limit: a call that hangs (a phone that froze under a reboot) is
//! cut, with every process it started that is still attached to it, and fails its step naming the limit. With
//! no limit the call is waited for as it always was. The call stays in this run's own process group, so Ctrl-C
//! reaches it as it reaches the run. (A double-forked daemon is reparented to init and is not found.)

use std::io::{Read, Write};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

fn limit_error(limit: Option<Duration>) -> String {
    format!(
        "no answer within the call limit of {} s; the call was cut",
        limit.map_or(0, |l| l.as_secs())
    )
}

/// Run `cmd` with `stdin` on its standard input; its output when it ends within `limit` (for ever when none).
/// Past the limit the call and the processes it started are killed and the error says the limit. The limit
/// also covers the wait for the output to end: a daemon the call left holding the pipes does not hang the step.
pub fn run_limited(
    mut cmd: Command,
    stdin: &str,
    limit: Option<Duration>,
) -> Result<Output, String> {
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot start {}: {e}", cmd.get_program().to_string_lossy()))?;
    let mut to = child.stdin.take().expect("piped");
    let script = stdin.to_string();
    // a program that never reads its input must not block the limit: the write has its own thread
    std::thread::spawn(move || {
        let _ = to.write_all(script.as_bytes());
    });
    let out = reader(child.stdout.take().expect("piped"));
    let err = reader(child.stderr.take().expect("piped"));
    let began = Instant::now();
    let status = loop {
        match child.try_wait().map_err(|e| format!("cannot wait: {e}"))? {
            Some(s) => break s,
            None if limit.is_some_and(|l| began.elapsed() >= l) => {
                kill_tree(child.id())?;
                let _ = child.wait();
                return Err(limit_error(limit));
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    };
    let stdout = collect(&out, limit, began)?;
    let stderr = collect(&err, limit, began)?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

/// What a reader read, once its pipe ended; past the limit, the limit error.
fn collect(
    from: &Receiver<Vec<u8>>,
    limit: Option<Duration>,
    began: Instant,
) -> Result<Vec<u8>, String> {
    match limit {
        None => Ok(from.recv().unwrap_or_default()),
        Some(l) => match from.recv_timeout(l.saturating_sub(began.elapsed())) {
            Ok(b) => Ok(b),
            Err(RecvTimeoutError::Timeout) => Err(limit_error(limit)),
            Err(RecvTimeoutError::Disconnected) => Ok(Vec::new()),
        },
    }
}

fn reader(mut from: impl Read + Send + 'static) -> Receiver<Vec<u8>> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = from.read_to_end(&mut b);
        let _ = tx.send(b);
    });
    rx
}

/// Every descendant of `pid`, read from `pgrep -P`; a `pgrep` that cannot run is an error, not "no children".
fn descendants(pid: u32) -> Result<Vec<u32>, String> {
    let mut all = Vec::new();
    let mut next = vec![pid];
    while let Some(p) = next.pop() {
        let o = Command::new("pgrep")
            .args(["-P", &p.to_string()])
            .output()
            .map_err(|e| format!("cannot cut the call: pgrep: {e}"))?;
        for kid in String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(|l| l.trim().parse::<u32>().ok())
        {
            all.push(kid);
            next.push(kid);
        }
    }
    Ok(all)
}

/// Kill the call and all it started that is still attached: the root is stopped first, so it cannot start
/// another while its children are listed; then all are killed.
fn kill_tree(pid: u32) -> Result<(), String> {
    let _ = Command::new("kill")
        .args(["-STOP", &pid.to_string()])
        .status();
    let listed = descendants(pid);
    let mut all = listed.clone().unwrap_or_default();
    all.push(pid);
    for p in all.iter().rev() {
        let _ = Command::new("kill")
            .args(["-KILL", &p.to_string()])
            .status();
    }
    listed.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_call_inside_the_limit_returns_its_output_and_one_past_it_is_cut() {
        let mut c = Command::new("sh");
        c.args(["-c", "cat; echo done"]);
        let o = run_limited(c, "hi\n", Some(Duration::from_secs(10))).unwrap();
        assert_eq!(String::from_utf8_lossy(&o.stdout), "hi\ndone\n");
        let mut c = Command::new("sh");
        c.args(["-c", "sleep 30 & sleep 30"]);
        let began = Instant::now();
        let e = run_limited(c, "", Some(Duration::from_millis(200))).unwrap_err();
        assert!(e.contains("call limit"), "{e}");
        assert!(
            began.elapsed() < Duration::from_secs(10),
            "the sleeps were waited out"
        );
    }

    #[test]
    fn a_respawning_child_and_a_daemon_holding_the_pipes_are_both_cut_by_the_limit() {
        let began = Instant::now();
        let mut c = Command::new("sh");
        c.args(["-c", "while :; do sleep 7771; done"]);
        let e = run_limited(c, "", Some(Duration::from_secs(1))).unwrap_err();
        assert!(e.contains("call limit"), "{e}");
        std::thread::sleep(Duration::from_millis(200));
        let left = Command::new("pgrep")
            .args(["-f", "sleep 7771"])
            .output()
            .unwrap();
        assert!(
            left.stdout.is_empty(),
            "a sleep outlived the cut: {}",
            String::from_utf8_lossy(&left.stdout)
        );
        let mut c = Command::new("sh");
        c.args(["-c", "sleep 7773 & echo hi"]);
        let e = run_limited(c, "", Some(Duration::from_secs(1))).unwrap_err();
        assert!(
            e.contains("call limit"),
            "a daemon holding the pipes hung the step: {e}"
        );
        assert!(began.elapsed() < Duration::from_secs(10));
        let _ = Command::new("pkill").args(["-f", "sleep 7773"]).status();
    }

    #[test]
    fn with_no_limit_a_slow_call_is_waited_for_and_stays_in_this_processs_group() {
        let mut c = Command::new("sh");
        c.args(["-c", "sleep 1; ps -o pgid= -p $$"]);
        let o = run_limited(c, "", None).unwrap();
        let theirs: u32 = String::from_utf8_lossy(&o.stdout).trim().parse().unwrap();
        let mine = Command::new("ps")
            .args(["-o", "pgid=", "-p", &std::process::id().to_string()])
            .output()
            .unwrap();
        let mine: u32 = String::from_utf8_lossy(&mine.stdout)
            .trim()
            .parse()
            .unwrap();
        assert_eq!(
            theirs, mine,
            "Ctrl-C reaches the call only inside the run's group"
        );
    }
}
