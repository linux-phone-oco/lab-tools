// SPDX-License-Identifier: MIT OR Apache-2.0
//! The real device of an install run (`device::Live`) against a stand-in on the loopback: a shell that sends
//! telnet negotiation bytes, echoes the lines it is sent as a pty does, ends each command with its marker and
//! answers the few commands the run asks; and a receiver that keeps what is pushed to it. It proves the shell's
//! framing, the stream's end and the run's order over real sockets; the boot (fastboot) is not here - the
//! stand-in's `boot` is the only part replaced - and the phone's own run is the keeper's, witnessed.

use osfact::device::{clean, fastboot_args, Live};
use osfact::install::{self, Device, Shell};
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

type Held = Arc<Mutex<Vec<u8>>>;

/// The receiver: one connection, read to the end (it ends only when the sender closes its side).
fn receiver() -> (u16, Held, std::thread::JoinHandle<()>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let held: Held = Arc::default();
    let keep = held.clone();
    let t = std::thread::spawn(move || {
        let (mut c, _) = l.accept().unwrap();
        let mut got = Vec::new();
        c.read_to_end(&mut got).unwrap();
        *keep.lock().unwrap() = got;
    });
    (port, held, t)
}

/// The shell: negotiation bytes first, then every line echoed, the answers of the commands the run asks.
fn shell_server(held: Held) -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (mut c, _) = l.accept().unwrap();
        c.write_all(&[0xff, 0xfb, 0x01, 0xff, 0xfb, 0x03]).unwrap();
        let mut lines = BufReader::new(c.try_clone().unwrap());
        let mut line = String::new();
        let mut status = 0;
        while {
            line.clear();
            lines.read_line(&mut line).unwrap_or(0) > 0
        } {
            let cmd = line.trim_end().to_string();
            let mut say = format!("{cmd}\r\n"); // the pty's echo
            if cmd.starts_with("printf '\\n__OS''FACT") {
                say.push_str(&format!("\r\n/ # __OSFACT {status}\r\n")); // after a prompt
            } else if let Some(path) = cmd.strip_prefix("sha256sum ") {
                say.push_str(&format!("{}  {path}\r\n", hex(&held.lock().unwrap())));
            } else if cmd.starts_with("cat ") && cmd.ends_with(".status") {
                say.push_str("0\r\n");
            } else if cmd.starts_with("mountpoint ") {
                status = 1;
            } else if cmd.starts_with("false") {
                status = 3;
            }
            if !cmd.starts_with("false")
                && !cmd.starts_with("mountpoint ")
                && !cmd.starts_with("printf '\\n__OS")
            {
                status = 0;
            }
            c.write_all(say.as_bytes()).unwrap();
        }
    });
    port
}

fn live(shell_port: u16) -> Live {
    Live::new(
        "SERIAL".into(),
        Some(install::Connection {
            address: "127.0.0.1".into(),
            shell_port,
            wait: 5,
        }),
    )
}

#[test]
fn the_shell_ends_each_command_at_its_marker_and_reports_its_status() {
    let mut d = live(shell_server(Held::default()));
    d.connect(5).unwrap();
    let ok = d.shell("echo hello", 5).unwrap();
    assert_eq!(ok.status, 0);
    assert!(ok.text.contains("hello"), "{}", ok.text);
    let Shell { status, .. } = d.shell("false", 5).unwrap();
    assert_eq!(status, 3, "the command's status is not carried");
}

#[test]
fn telnet_negotiation_bytes_are_dropped_and_a_doubled_ff_is_one() {
    assert_eq!(
        clean(&[b'a', 0xff, 0xfb, 0x01, b'b', 0xff, 0xff, b'c']),
        vec![b'a', b'b', 0xff, b'c']
    );
}

#[test]
fn fastboot_always_names_the_serial() {
    assert_eq!(
        fastboot_args("S1", &["boot", "x.img"]),
        ["-s", "S1", "boot", "x.img"]
    );
}

#[test]
fn the_push_sends_every_byte_and_ends_the_stream() {
    let (port, held, t) = receiver();
    let dir = std::env::temp_dir().join(format!("osfact-live-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("root.tar.gz");
    let bytes: Vec<u8> = (0..400_000u32).map(|i| (i % 249) as u8).collect();
    std::fs::write(&file, &bytes).unwrap();
    let mut d = live(0);
    let n = d.send(port, file.to_str().unwrap(), 5).unwrap();
    t.join().unwrap(); // returns only if the sender closed its side
    assert_eq!(n, bytes.len() as u64);
    assert_eq!(*held.lock().unwrap(), bytes);
}

#[test]
fn a_receiver_that_never_listens_is_named_after_the_wait() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    drop(l);
    let mut d = live(0);
    let e = d.send(port, "/nonexistent", 1).unwrap_err();
    assert!(e.contains("did not answer within"), "{e}");
}

/// Boot is the one operation replaced: everything else is the real device over the loopback.
struct NoFastboot(Live);
impl Device for NoFastboot {
    fn boot(&mut self, _file: &str) -> Result<(), String> {
        Ok(())
    }
    fn connect(&mut self, w: u32) -> Result<(), String> {
        self.0.connect(w)
    }
    fn shell(&mut self, c: &str, w: u32) -> Result<Shell, String> {
        self.0.shell(c, w)
    }
    fn send(&mut self, p: u16, f: &str, w: u32) -> Result<u64, String> {
        self.0.send(p, f, w)
    }
    fn way_back(&mut self, _b: &install::WayBack) -> Result<(), String> {
        Ok(())
    }
    fn back_on(&mut self, a: &[String], w: u32) -> Result<(), String> {
        self.0.back_on(a, w)
    }
}

#[test]
fn a_whole_run_over_the_loopback_pushes_checks_and_enters() {
    let (push_port, held, t) = receiver();
    let shell_port = shell_server(held);
    let dir = std::env::temp_dir().join(format!("osfact-live-run-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let root: Vec<u8> = (0..100_000u32).map(|i| (i % 247) as u8).collect();
    std::fs::write(dir.join("root.tar.gz"), &root).unwrap();
    std::fs::write(dir.join("boot.img"), b"boot").unwrap();
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plans/walleye-root-in-ram.toml"),
    )
    .unwrap()
    .replace("172.16.43.1", "127.0.0.1")
    .replace("shell_port = 23", &format!("shell_port = {shell_port}"))
    .replace("port = 9001", &format!("port = {push_port}"))
    .replace(
        "file = \"boot.img\"",
        &format!("file = \"{}/boot.img\"", dir.display()),
    )
    .replace(
        "file = \"root.tar.gz\"",
        &format!("file = \"{}/root.tar.gz\"", dir.display()),
    )
    .replace(&"0".repeat(64), &hex(b"boot"))
    .replace(&"1".repeat(64), &hex(&root));
    let plan = install::read(&text).unwrap();
    let mut said = Vec::new();
    install::run(&plan, &mut NoFastboot(live(shell_port)), &mut |l| {
        said.push(l.to_string())
    })
    .unwrap_or_else(|e| panic!("{e}\n{said:?}"));
    t.join().unwrap();
    assert_eq!(said.len(), 3, "{said:?}");
}

/// The way back's `back_on` against real host commands (the phone's own answer is the keeper's, witnessed).
#[test]
fn back_on_is_a_host_command_tried_until_it_ends_with_status_zero_and_names_the_serial() {
    let mut d = live(0);
    let argv = |w: &[&str]| w.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    d.back_on(&argv(&["true"]), 5).expect("true answers");
    d.back_on(&argv(&["test", "{serial}", "=", "SERIAL"]), 5)
        .expect("the serial is the device's");
    let e = d.back_on(&argv(&["false"]), 1).unwrap_err();
    assert!(e.contains("false ended with"), "{e}");
    let e = d
        .back_on(&argv(&["no-such-program-osfact"]), 1)
        .unwrap_err();
    assert!(e.contains("no-such-program-osfact"), "{e}");
}

#[test]
fn back_on_is_tried_again_until_it_answers_and_a_hanging_try_is_killed_at_the_bound() {
    let mut d = live(0);
    let dir = std::env::temp_dir().join(format!("osfact-backon-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let flag = dir.join("back");
    let later = flag.clone();
    // the device "comes back" about 3 s after the first try: the first tries fail, a later one passes
    let t = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        std::fs::write(later, b"up").unwrap();
    });
    let argv = vec!["test".to_string(), "-e".into(), flag.display().to_string()];
    let started = std::time::Instant::now();
    d.back_on(&argv, 20).expect("answers once the file exists");
    assert!(
        started.elapsed().as_secs() >= 2,
        "it did not wait for the answer"
    );
    t.join().unwrap();
    // a command that never ends is killed and the wait ends near its bound, not 10 s per try beyond it
    let hang = vec!["sleep".to_string(), "30".into()];
    let started = std::time::Instant::now();
    let e = d.back_on(&hang, 1).unwrap_err();
    assert!(e.contains("did not end"), "{e}");
    assert!(
        started.elapsed().as_secs() < 6,
        "overran: {:?}",
        started.elapsed()
    );
}

// ---- the receiver on a shell with job control -------------------------------------------------------------------
//
// The phone's telnetd gives busybox sh a tty with job control (osfact's first phone run):
// a background job that reads the tty's input is stopped, so a receiver started without its input closed never ends.
// A pipe-fed stand-in hides that. This one is `bash -i` on a pty (util-linux `script`), bridged to a socket: the
// same job control. Its source: the stop reproduced by hand on 2026-11-28 with
// `script -qfc "bash --norc -i" /dev/null` ("[1]+ Stopped"), and the fix a device lab's own shell
// helper made (the command as its own line, `</dev/null`, no markers).

/// A shell on a pty with job control behind a socket: its output to the socket, the socket's bytes to its input.
fn pty_shell_server() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (mut c, _) = l.accept().unwrap();
        let mut child = Command::new("script")
            .args(["-qfc", "bash --norc -i", "/dev/null"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("util-linux `script` (a pty) is needed by this test: install it");
        let (mut to_shell, mut from_shell) =
            (child.stdin.take().unwrap(), child.stdout.take().unwrap());
        let mut out = c.try_clone().unwrap();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = from_shell.read(&mut buf) {
                if n == 0 || out.write_all(&buf[..n]).is_err() {
                    break;
                }
            }
        });
        let mut buf = [0u8; 4096];
        while let Ok(n) = c.read(&mut buf) {
            if n == 0 || to_shell.write_all(&buf[..n]).is_err() {
                break;
            }
        }
        let _ = child.kill();
        let _ = child.wait();
    });
    port
}

/// The receiver as a program that reads its input first, as `nc -l` does, then takes what was pushed: the pushed
/// bytes are kept by the test's socket, which marks them complete with a file.
struct Pushed {
    port: u16,
    dir: std::path::PathBuf,
    bytes: Vec<u8>,
}

fn pushed_to_a_stand_in_receiver(tag: &str) -> Pushed {
    let dir = std::env::temp_dir().join(format!("osfact-jc-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (port, held, t) = receiver();
    let (kept, done) = (dir.join("pushed"), dir.join("pushed.done"));
    std::thread::spawn(move || {
        t.join().unwrap();
        std::fs::write(&kept, &*held.lock().unwrap()).unwrap();
        std::fs::write(&done, "").unwrap();
    });
    let bytes = pushed_bytes(tag);
    Pushed { port, dir, bytes }
}

/// The run's own steps against the pty shell: start the receiver with `line`, push, and read the status file.
fn pushed_bytes(tag: &str) -> Vec<u8> {
    let _ = tag;
    (0..300_000u32).map(|i| (i % 241) as u8).collect()
}

fn push_through(line: &dyn Fn(&str, &str, &str) -> String, tag: &str) -> Result<String, String> {
    let p = pushed_to_a_stand_in_receiver(tag);
    let file = p.dir.join("root.tar");
    std::fs::write(&file, &p.bytes).unwrap();
    let (to, status) = (p.dir.join("received"), p.dir.join("received.status"));
    let receive = format!(
        "sh -c 'read x; while [ ! -e {d}/pushed.done ]; do sleep 0.1; done; cat {d}/pushed'",
        d = p.dir.display()
    );
    let mut d = live(pty_shell_server());
    d.connect(5)?;
    d.shell("PS1='$ '", 5)?;
    d.shell(
        &line(&receive, to.to_str().unwrap(), status.to_str().unwrap()),
        5,
    )?;
    d.send(p.port, file.to_str().unwrap(), 5)?;
    for _ in 0..8 {
        let got = d.shell(&format!("cat {}", status.display()), 5)?;
        if got.text.lines().any(|l| l.trim() == "0") {
            let sum = d.shell(&format!("sha256sum {}", to.display()), 10)?;
            return Ok(sum.text);
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    Err("the receiver did not end within 4 s of the push".into())
}

#[test]
fn a_receiver_on_a_shell_with_job_control_takes_the_push() {
    let text = push_through(&|r, to, st| install::receiver_line(r, to, st), "new")
        .expect("the receiver was stopped by job control");
    assert!(text.contains(&hex(&pushed_bytes("new"))), "{text}");
}

#[test]
fn the_old_receiver_line_is_stopped_on_that_shell() {
    let old =
        |r: &str, to: &str, st: &str| format!("( {r} > {to}; echo $? > {st} ) > /dev/null 2>&1 &");
    let e = push_through(&old, "old").unwrap_err();
    assert!(e.contains("did not end"), "{e}");
}

/// The walleye fixture plan, pointed at the loopback stand-ins and a work folder with images of known bytes;
/// `edit` changes the text first (the check, its wait).
fn loopback_plan(
    tag: &str,
    push_port: u16,
    shell_port: u16,
    edit: impl Fn(String) -> String,
) -> (install::Plan, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("osfact-live-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let root: Vec<u8> = (0..100_000u32).map(|i| (i % 247) as u8).collect();
    std::fs::write(dir.join("root.tar.gz"), &root).unwrap();
    std::fs::write(dir.join("boot.img"), b"boot").unwrap();
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plans/walleye-root-in-ram.toml"),
    )
    .unwrap()
    .replace("172.16.43.1", "127.0.0.1")
    // the way back is confirmed by a command that answers at once (the fixture asks adb)
    .replace("\"adb\", \"-s\", \"{serial}\", \"get-state\"", "\"true\"")
    .replace("shell_port = 23", &format!("shell_port = {shell_port}"))
    .replace("port = 9001", &format!("port = {push_port}"))
    .replace(
        "file = \"boot.img\"",
        &format!("file = \"{}/boot.img\"", dir.display()),
    )
    .replace(
        "file = \"root.tar.gz\"",
        &format!("file = \"{}/root.tar.gz\"", dir.display()),
    )
    .replace(&"0".repeat(64), &hex(b"boot"))
    .replace(&"1".repeat(64), &hex(&root));
    (install::read(&edit(text)).unwrap(), dir)
}

#[test]
fn a_run_writes_every_line_it_sends_and_every_answer_to_its_log() {
    let (push_port, held, t) = receiver();
    let shell_port = shell_server(held);
    let (plan, dir) = loopback_plan("log", push_port, shell_port, |t| t);
    let log = dir.join("install.log");
    install::run_logged(&plan, &mut NoFastboot(live(shell_port)), &log, &mut |_| {})
        .unwrap_or_else(|e| panic!("{e}"));
    t.join().unwrap();
    let text = std::fs::read_to_string(&log).unwrap();
    for line in [
        "> mkdir -p /mm",
        "> mount -t tmpfs -o size=400m tmpfs /mm",
        "> gzip -dc /tmp/root.tar.gz | tar -x -C /mm",
        "> rm -f /tmp/root.tar.gz",
        "> setsid chroot /mm /sbin/init > /dev/null 2>&1 &",
        "> test -e /mm/sbin/init",
        "> sha256sum /tmp/root.tar.gz",
    ] {
        assert!(text.contains(line), "the log lacks `{line}`:\n{text}");
    }
    // the device's answer is there too: the digest the stand-in computed, and each command's status
    let digest = hex(&(0..100_000u32)
        .map(|i| (i % 247) as u8)
        .collect::<Vec<u8>>());
    assert!(
        text.contains(&format!("< {digest}")),
        "no answer in the log:\n{text}"
    );
    assert!(text.contains("status 0"), "{text}");
}

#[test]
fn a_check_that_never_holds_fails_naming_the_step_the_wait_and_the_log() {
    let (push_port, held, t) = receiver();
    let shell_port = shell_server(held);
    let (plan, dir) = loopback_plan("never", push_port, shell_port, |t| {
        t.replace(
            "check = \"test -e /mm/sbin/init\"",
            "check = \"false\"\ncheck_wait = 2",
        )
    });
    let log = dir.join("install.log");
    let e = install::run_logged(&plan, &mut NoFastboot(live(shell_port)), &log, &mut |_| {})
        .unwrap_err();
    t.join().unwrap();
    assert!(e.contains("step 3"), "{e}");
    assert!(e.contains("within 2 s"), "{e}");
    assert!(e.contains(log.to_str().unwrap()), "{e}");
    assert!(std::fs::read_to_string(&log).unwrap().contains("> false"));
}

#[test]
fn check_wait_reaches_the_plan_and_the_dry_run_prints_what_enter_runs() {
    let (plan, _) = loopback_plan("wait", 1, 1, |t| {
        t.replace(
            "check = \"test -e /mm/sbin/init\"",
            "check = \"test -e /mm/sbin/init\"\ncheck_wait = 120",
        )
    });
    assert_eq!(plan.install.step[2].check_wait, Some(120));
    let said = install::commands(&plan).unwrap().join("\n");
    for line in [
        "on the device: mkdir -p /mm",
        "on the device: mount -t tmpfs -o size=400m tmpfs /mm",
        "on the device: gzip -dc /tmp/root.tar.gz | tar -x -C /mm",
        "on the device: setsid chroot /mm /sbin/init > /dev/null 2>&1 &",
        "on the device: test -e /mm/sbin/init (tried every second for 120 s)",
    ] {
        assert!(said.contains(line), "the dry run lacks `{line}`:\n{said}");
    }
}

#[test]
fn a_check_wait_without_a_check_or_of_zero_is_refused_at_read() {
    let plan = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plans/walleye-root-in-ram.toml"),
    )
    .unwrap();
    let check = "check = \"test -e /mm/sbin/init\"";
    for (text, named) in [
        (
            plan.replace(check, "check_wait = 5"),
            "check_wait needs a check",
        ),
        (
            plan.replace(check, &format!("{check}\ncheck_wait = 0")),
            "check_wait",
        ),
    ] {
        let e = install::read(&text).err().expect("refused");
        assert!(e.contains("refuses") && e.contains(named), "{named}: {e}");
    }
}

/// A device whose check answers "not yet" until its Nth try: a root that is slow to unpack.
struct SlowRoot {
    inner: NoFastboot,
    tries_left: u32,
}
impl Device for SlowRoot {
    fn boot(&mut self, f: &str) -> Result<(), String> {
        self.inner.boot(f)
    }
    fn connect(&mut self, w: u32) -> Result<(), String> {
        self.inner.connect(w)
    }
    fn shell(&mut self, c: &str, w: u32) -> Result<Shell, String> {
        if c.starts_with("test -e /mm/sbin/init") && self.tries_left > 0 {
            self.tries_left -= 1;
            return Ok(Shell {
                status: 1,
                text: String::new(),
            });
        }
        self.inner.shell(c, w)
    }
    fn send(&mut self, p: u16, f: &str, w: u32) -> Result<u64, String> {
        self.inner.send(p, f, w)
    }
    fn way_back(&mut self, b: &install::WayBack) -> Result<(), String> {
        self.inner.way_back(b)
    }
    fn back_on(&mut self, a: &[String], w: u32) -> Result<(), String> {
        self.inner.back_on(a, w)
    }
}

#[test]
fn a_check_that_holds_on_its_third_try_passes_within_its_wait_and_the_default_is_ten_seconds() {
    let (push_port, held, t) = receiver();
    let shell_port = shell_server(held);
    let (plan, _) = loopback_plan("slow", push_port, shell_port, |t| {
        t.replace(
            "check = \"test -e /mm/sbin/init\"",
            "check = \"test -e /mm/sbin/init\"\ncheck_wait = 5",
        )
    });
    let mut d = SlowRoot {
        inner: NoFastboot(live(shell_port)),
        tries_left: 2,
    };
    install::run(&plan, &mut d, &mut |_| {}).unwrap_or_else(|e| panic!("{e}"));
    t.join().unwrap();
    assert_eq!(d.tries_left, 0, "the check was not tried until it held");
    let (default, _) = loopback_plan("default", 1, 1, |t| t);
    assert!(install::commands(&default)
        .unwrap()
        .join("\n")
        .contains("tried every second for 10 s"));
}
