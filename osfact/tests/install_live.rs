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
