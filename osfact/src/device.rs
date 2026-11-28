// SPDX-License-Identifier: MIT OR Apache-2.0
//! The real device of an install run (`install::Device`): `fastboot boot` of the image, a root shell over TCP
//! (a telnetd with no login: one command at a time, its end marked), and a file pushed as a stream to a port.
//! Nothing here is written to the device beyond what the plan's own commands do in its RAM.

use crate::install::{Connection, Device, Shell, WayBack};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The end of a command's text in the shell: spelled in two quoted halves in the line sent, so the shell's echo
/// of that line never holds the whole word and only the command's own output does.
const MARK: &str = "__OSFACT";

/// Seconds a fastboot call may take before it is killed: it waits for a device for ever otherwise.
const FASTBOOT_S: u64 = 60;

pub struct Live {
    serial: String,
    connection: Option<Connection>,
    session: Option<TcpStream>,
    pending: Vec<u8>,
    booted: bool,
}

impl Live {
    pub fn new(serial: String, connection: Option<Connection>) -> Live {
        Live {
            serial,
            connection,
            session: None,
            pending: Vec::new(),
            booted: false,
        }
    }

    fn fastboot(&self, args: &[&str]) -> Result<(), String> {
        let mut child = Command::new("fastboot")
            .args(fastboot_args(&self.serial, args))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("fastboot: {e}"))?;
        let until = Instant::now() + Duration::from_secs(FASTBOOT_S);
        while child
            .try_wait()
            .map_err(|e| format!("fastboot: {e}"))?
            .is_none()
        {
            if Instant::now() >= until {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "fastboot {} did not end within {FASTBOOT_S} s (no device in the bootloader?)",
                    args.join(" ")
                ));
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        let out = child
            .wait_with_output()
            .map_err(|e| format!("fastboot: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!(
                "fastboot {} ended with {}: {}",
                args.join(" "),
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    }

    fn stream(&mut self) -> Result<&mut TcpStream, String> {
        self.session
            .as_mut()
            .ok_or_else(|| "no shell is open: the connection step has not run".to_string())
    }
}

/// fastboot's arguments for one call: the serial always named, so a second device on the bus is never touched.
pub fn fastboot_args<'a>(serial: &'a str, args: &[&'a str]) -> Vec<&'a str> {
    let mut v = vec!["-s", serial];
    v.extend_from_slice(args);
    v
}

fn target(address: &str, port: u16) -> Result<std::net::SocketAddr, String> {
    (address, port)
        .to_socket_addrs()
        .map_err(|e| format!("{address}:{port}: {e}"))?
        .next()
        .ok_or_else(|| format!("{address}:{port} names no address"))
}

fn open(address: &str, port: u16, wait: Duration) -> Result<TcpStream, String> {
    let until = Instant::now() + wait;
    let to = target(address, port)?;
    loop {
        match TcpStream::connect_timeout(&to, Duration::from_secs(3)) {
            Ok(s) => return Ok(s),
            Err(e) if Instant::now() >= until => {
                return Err(format!(
                    "{address}:{port} did not answer within {} s: {e}",
                    wait.as_secs()
                ))
            }
            Err(_) => std::thread::sleep(Duration::from_millis(500)),
        }
    }
}

/// The text of a telnet stream without its option negotiation (IAC and its command and option bytes).
pub fn clean(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len());
    let mut k = 0;
    while k < raw.len() {
        if raw[k] != 0xff {
            out.push(raw[k]);
            k += 1;
        } else if raw.get(k + 1) == Some(&0xff) {
            out.push(0xff);
            k += 2;
        } else if matches!(raw.get(k + 1), Some(251..=254)) {
            k += 3;
        } else {
            k += 2;
        }
    }
    out
}

/// The status after the mark - anywhere in the text, a prompt or unended output before it - once its line is ended, and what came before the mark.
fn marked(text: &str) -> Option<(i32, String)> {
    let at = text.find(&format!("{MARK} "))?;
    let rest = &text[at + MARK.len() + 1..];
    let end = rest.find('\n')?;
    let status = rest[..end].trim().parse().ok()?;
    Some((status, text[..at].to_string()))
}

impl Device for Live {
    fn boot(&mut self, file: &str) -> Result<(), String> {
        self.fastboot(&["boot", file])?;
        self.booted = true;
        Ok(())
    }

    fn connect(&mut self, wait_s: u32) -> Result<(), String> {
        let c = self
            .connection
            .clone()
            .ok_or("the plan declares no connection")?;
        let s = open(&c.address, c.shell_port, Duration::from_secs(wait_s.into()))?;
        s.set_read_timeout(Some(Duration::from_secs(1)))
            .map_err(|e| format!("the shell: {e}"))?;
        self.session = Some(s);
        self.pending.clear();
        Ok(())
    }

    fn shell(&mut self, command: &str, wait_s: u32) -> Result<Shell, String> {
        let (first, second) = MARK.split_at(MARK.len() / 2);
        self.pending.clear();
        let line = format!("{command}\nprintf '\\n{first}''{second} %s\\n' \"$?\"\n");
        self.stream()?
            .write_all(line.as_bytes())
            .map_err(|e| format!("the shell: {e}"))?;
        let until = Instant::now() + Duration::from_secs(wait_s.into());
        loop {
            let text = String::from_utf8_lossy(&clean(&self.pending)).into_owned();
            if let Some((status, before)) = marked(&text) {
                self.pending.clear();
                return Ok(Shell {
                    status,
                    text: before,
                });
            }
            if Instant::now() >= until {
                return Err(format!(
                    "`{command}` did not end within {wait_s} s; its text so far: {}",
                    text.trim()
                ));
            }
            let mut buf = [0u8; 4096];
            match self.stream()?.read(&mut buf) {
                Ok(0) => return Err("the shell closed the connection".into()),
                Ok(n) => self.pending.extend_from_slice(&buf[..n]),
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(e) => return Err(format!("the shell: {e}")),
            }
        }
    }

    fn send(&mut self, port: u16, file: &str, wait_s: u32) -> Result<u64, String> {
        let address = self
            .connection
            .as_ref()
            .ok_or("the plan declares no connection")?
            .address
            .clone();
        let mut stream = open(&address, port, Duration::from_secs(wait_s.into()))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .map_err(|e| format!("the push: {e}"))?;
        let mut f = std::fs::File::open(file).map_err(|e| format!("{file}: {e}"))?;
        let n = std::io::copy(&mut f, &mut stream).map_err(|e| format!("pushing {file}: {e}"))?;
        stream.flush().map_err(|e| format!("pushing {file}: {e}"))?;
        // the write side closed: the receiver's `nc -l` ends on it
        stream
            .shutdown(Shutdown::Write)
            .map_err(|e| format!("closing the push: {e}"))?;
        Ok(n)
    }

    fn way_back(&mut self, back: &WayBack) -> Result<(), String> {
        match back.via {
            crate::install::Transport::Shell if self.booted => {
                let command = back.command.as_deref().ok_or("no way-back command")?;
                if self.session.is_none() {
                    self.connect(3)?;
                }
                // the shell ends with the device: only the sending is waited for
                self.stream()?
                    .write_all(format!("{command}\n").as_bytes())
                    .map_err(|e| format!("the way back: {e}"))
            }
            // not booted yet: the device is still in the bootloader, whatever the plan's way back says
            _ => self.fastboot(&["reboot"]),
        }
    }
}
