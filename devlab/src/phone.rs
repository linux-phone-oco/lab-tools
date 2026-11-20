// SPDX-License-Identifier: MIT OR Apache-2.0
//! Reaching the phone over SSH: commands travel as a script on standard input (`sh -s`), never
//! inside quotes on a command line, so quotes, pipes and $ in them need no escaping.

use crate::device::{candidates, Device};
use anyhow::{bail, Context, Result};
use lpoco_manifest::Transport;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::process::{Command, Stdio};

#[derive(Clone)]
pub struct Phone {
    /// Where it was reached: user@address (ssh), address:port (telnet), adb:SERIAL
    pub target: String,
    reach: Reach,
    /// the manifest, for what must never be touched on this phone
    device: Device,
}

const SSH_OPTS: [&str; 8] = [
    "-o",
    "StrictHostKeyChecking=no",
    "-o",
    "UserKnownHostsFile=/dev/null",
    "-o",
    "LogLevel=ERROR",
    "-o",
    "ConnectTimeout=5",
];

/// The ssh arguments to reach `target` and run `remote`.
pub fn ssh_args(target: &str, remote: &str) -> Vec<String> {
    let mut a: Vec<String> = SSH_OPTS.iter().map(|s| s.to_string()).collect();
    a.push(target.to_string());
    a.push(remote.to_string());
    a
}

/// As the user: the session's runtime directory set, the script from standard input.
pub const AS_USER: &str = "export XDG_RUNTIME_DIR=/run/user/$(id -u); sh -s";
/// As root: sudo reads exactly one line - the password - and the shell it starts reads the rest.
pub const AS_ROOT: &str = "sudo -S -p '' sh -s";

/// How this phone is reached, as its device.toml declares ([connection] transport and login).
#[derive(Clone)]
enum Reach {
    /// ssh with the phone user's password
    Ssh { password: String },
    /// the phone's own shell on a TCP port, no login
    Telnet { address: String, port: u16 },
    /// adb by serial, no login
    Adb { serial: String },
}

impl Phone {
    /// The manifest this phone was reached by.
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// The phone at one given address, if it answers there (ssh: the address; telnet and adb: the declared one).
    pub fn at(device: &Device, addr: &str) -> Result<Phone> {
        let p = Phone::reaching(device, addr)?;
        if !p.answers() {
            bail!("{} does not answer at {addr}", device.device.codename);
        }
        Ok(p)
    }

    /// The first address that answers: the manifest's, then the remembered Wi-Fi one; a telnet or adb
    /// phone has the one place its shell is declared at.
    pub fn find(device: &Device) -> Result<Phone> {
        let places = match device.connection.transport {
            Transport::Ssh => {
                let remembered =
                    std::fs::read_to_string(device.state_dir().join("wifi-address")).ok();
                candidates(&device.connection.addresses, remembered.as_deref())
            }
            Transport::Telnet | Transport::Adb => vec![String::new()],
        };
        for addr in places {
            let p = Phone::reaching(device, &addr)?;
            if p.answers() {
                return Ok(p);
            }
        }
        bail!(
            "{} does not answer at any known address",
            device.device.codename
        )
    }

    /// The phone as its transport reaches it, not yet asked whether it answers.
    fn reaching(device: &Device, addr: &str) -> Result<Phone> {
        let c = &device.connection;
        let (target, reach) = match c.transport {
            Transport::Ssh => (
                format!("{}@{addr}", c.user),
                Reach::Ssh {
                    password: password(device)?,
                },
            ),
            Transport::Telnet => {
                let address = c
                    .shell_address
                    .clone()
                    .or_else(|| c.addresses.first().cloned())
                    .context("device.toml: transport telnet needs shell_address (or an address)")?;
                let port = c
                    .shell_port
                    .context("device.toml: transport telnet needs shell_port")?;
                (format!("{address}:{port}"), Reach::Telnet { address, port })
            }
            Transport::Adb => {
                let serial = device.adb_serial()?;
                (format!("adb:{serial}"), Reach::Adb { serial })
            }
        };
        Ok(Phone {
            target,
            reach,
            device: device.clone(),
        })
    }

    /// Runs `script` on the phone; returns (succeeded, stdout). `how` and `prefix` are ssh's: how the
    /// script is run there, and what is sent before it (sudo's password line).
    pub fn run(&self, script: &str, how: &str, prefix: &str) -> Result<(bool, String)> {
        if let Some(no) = self.device.hazard_in(script) {
            bail!(no);
        }
        match &self.reach {
            Reach::Ssh { password } => {
                let mut child = Command::new("sshpass")
                    .arg("-e")
                    .arg("ssh")
                    .args(ssh_args(&self.target, how))
                    .env("SSHPASS", password)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::inherit())
                    .spawn()
                    .context("starting sshpass (is it installed?)")?;
                let mut stdin = child.stdin.take().context("ssh stdin")?;
                stdin.write_all(prefix.as_bytes())?;
                stdin.write_all(script.as_bytes())?;
                drop(stdin);
                let out = child.wait_with_output()?;
                Ok((
                    out.status.success(),
                    String::from_utf8_lossy(&out.stdout).into_owned(),
                ))
            }
            Reach::Telnet { address, port } => telnet_run(address, *port, script),
            Reach::Adb { serial } => {
                let out = adb_shell(serial, script, Stdio::inherit())?;
                Ok((
                    out.status.success(),
                    String::from_utf8_lossy(&out.stdout).into_owned(),
                ))
            }
        }
    }

    /// Whether this address answers - quietly: an address that does not is no error worth showing.
    fn answers(&self) -> bool {
        match &self.reach {
            Reach::Ssh { password } => Command::new("sshpass")
                .arg("-e")
                .arg("ssh")
                .args(ssh_args(&self.target, "true"))
                .env("SSHPASS", password)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success()),
            Reach::Telnet { address, port } => telnet_connect(address, *port).is_ok(),
            Reach::Adb { serial } => {
                adb_shell(serial, "true\n", Stdio::null()).is_ok_and(|o| o.status.success())
            }
        }
    }

    pub fn sh(&self, script: &str) -> Result<(bool, String)> {
        self.run(script, AS_USER, "")
    }

    /// As root: ssh asks sudo with the password; a shell with no login (telnet, adb) is the phone's own and
    /// runs the script as it is.
    pub fn root(&self, script: &str) -> Result<(bool, String)> {
        match &self.reach {
            Reach::Ssh { password } => self.run(script, AS_ROOT, &format!("{password}\n")),
            Reach::Telnet { .. } | Reach::Adb { .. } => self.run(script, "", ""),
        }
    }

    /// A root command's output as bytes (a file off the phone); fails if the command fails.
    pub fn root_bytes(&self, script: &str) -> Result<Vec<u8>> {
        if let Some(no) = self.device.hazard_in(script) {
            bail!(no);
        }
        let out = match &self.reach {
            Reach::Ssh { password } => {
                let mut child = Command::new("sshpass")
                    .arg("-e")
                    .arg("ssh")
                    .args(ssh_args(&self.target, AS_ROOT))
                    .env("SSHPASS", password)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::inherit())
                    .spawn()
                    .context("starting sshpass")?;
                let mut stdin = child.stdin.take().context("ssh stdin")?;
                stdin.write_all(format!("{password}\n{script}").as_bytes())?;
                drop(stdin);
                child.wait_with_output()?
            }
            Reach::Adb { serial } => adb_shell(serial, script, Stdio::inherit())?,
            Reach::Telnet { .. } => bail!("a file is not carried over telnet: it is text only"),
        };
        if !out.status.success() {
            bail!("the phone's command failed");
        }
        Ok(out.stdout)
    }

    /// Copies a local file to the phone.
    pub fn put(&self, local: &str, remote: &str) -> Result<()> {
        let ok = match &self.reach {
            Reach::Ssh { password } => {
                let mut a: Vec<String> = SSH_OPTS.iter().map(|s| s.to_string()).collect();
                a.push(local.to_string());
                a.push(format!("{}:{remote}", self.target));
                Command::new("sshpass")
                    .arg("-e")
                    .arg("scp")
                    .args(a)
                    .env("SSHPASS", password)
                    .status()?
                    .success()
            }
            Reach::Adb { serial } => Command::new("adb")
                .args(["-s", serial, "push", local, remote])
                .status()
                .context("starting adb")?
                .success(),
            Reach::Telnet { .. } => bail!("a file is not carried over telnet: it is text only"),
        };
        if !ok {
            bail!("copying {local} to the phone failed");
        }
        Ok(())
    }

    /// This boot's identity: a random value the kernel makes anew at every boot.
    pub fn boot_id(&self) -> Option<String> {
        let (ok, out) = self.sh("cat /proc/sys/kernel/random/boot_id").ok()?;
        ok.then(|| out.trim().to_string()).filter(|s| !s.is_empty())
    }

    pub fn uptime(&self) -> Option<u64> {
        let (ok, out) = self.sh("cut -d. -f1 /proc/uptime").ok()?;
        ok.then(|| out.trim().parse().ok()).flatten()
    }
}

/// A connection to a phone's own shell, quietly: a phone that does not answer is no error worth showing.
fn telnet_connect(address: &str, port: u16) -> Result<TcpStream> {
    let place = (address, port)
        .to_socket_addrs()?
        .next()
        .context("no address for the phone's shell")?;
    Ok(TcpStream::connect_timeout(
        &place,
        std::time::Duration::from_secs(5),
    )?)
}

/// How long a phone's shell may be silent before the lab gives up on it, in seconds.
const TELNET_SILENCE: u64 = 300;

/// Runs `script` in the phone's shell on a TCP port (telnet, no login). The script is bracketed by two
/// markers made for this run, so what the shell echoes back of the commands (or a prompt before them) is not
/// mistaken for their output, and the exit status of the script's last command is read from the end marker.
/// A script that ends the shell itself (`exit`) or reads the standard input never reaches its end marker: that
/// is an error saying so, never a pass; so is a shell that stays silent for `TELNET_SILENCE` seconds.
fn telnet_run(address: &str, port: u16, script: &str) -> Result<(bool, String)> {
    let mut s = telnet_connect(address, port)?;
    s.set_read_timeout(Some(std::time::Duration::from_secs(TELNET_SILENCE)))?;
    let nonce = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    );
    let (begin, end) = (format!("LPOCO-BEGIN-{nonce}"), format!("LPOCO-RC-{nonce}-"));
    let sent = format!("stty -echo 2>/dev/null\necho {begin}\n{script}\necho {end}$?\nexit\n");
    // written while the answer is read: a shell that echoes its input back must not wait on a full pipe
    let mut writer = s.try_clone()?;
    std::thread::spawn(move || {
        let _ = writer.write_all(sent.as_bytes());
    });
    let (mut raw, mut chunk) = (Vec::new(), [0u8; 4096]);
    loop {
        let n = s.read(&mut chunk).map_err(|e| {
            anyhow::anyhow!(
                "the phone's shell went silent for {TELNET_SILENCE} s ({e}); it had said: {}",
                telnet_text(&raw, &begin).unwrap_or_default()
            )
        })?;
        if n == 0 {
            bail!(
                "the phone's shell closed before the script's end marker (a script that exits the shell, or reads standard input, cannot be run over telnet); it had said: {}",
                telnet_text(&raw, &begin).unwrap_or_default()
            );
        }
        raw.extend_from_slice(&chunk[..n]);
        // the answer ends with its marker: look at the last of what came, not at all of it at each read
        if ends_with_marker(&raw, end.as_bytes()) {
            if let Some(said) = telnet_answer(&raw, &begin, &end) {
                return Ok(said);
            }
        }
    }
}

/// Whether the marker is within the last few bytes of what came: the status after it is a few digits and a
/// newline, so a marker further back is already complete or not there.
fn ends_with_marker(raw: &[u8], marker: &[u8]) -> bool {
    let tail = &raw[raw.len().saturating_sub(marker.len() + 16)..];
    tail.windows(marker.len()).any(|w| w == marker)
}

/// What a telnet shell said after its begin line, cleaned (negotiation and carriage returns dropped): none
/// until the begin line has come. The begin marker is the end of a line that did not echo it as a command
/// (`echo MARK`) - a prompt before it is ignored.
fn telnet_text(raw: &[u8], begin: &str) -> Option<String> {
    let text = String::from_utf8_lossy(&without_negotiation(raw)).into_owned();
    let start = text.match_indices(begin).find_map(|(at, _)| {
        let after = at + begin.len();
        let ends_a_line = text[after..].starts_with('\n');
        let line_start = text[..at].rfind('\n').map_or(0, |n| n + 1);
        let echoed = text[line_start..at].trim_end().ends_with("echo");
        (ends_a_line && !echoed).then_some(after + 1)
    })?;
    Some(text[start..].to_string())
}

/// The bytes of a telnet stream without its commands: IAC IAC is one 0xff, IAC WILL/WONT/DO/DONT option,
/// IAC SB ... IAC SE and IAC with any other command are dropped, and so is every carriage return.
fn without_negotiation(raw: &[u8]) -> Vec<u8> {
    const IAC: u8 = 0xff;
    const SB: u8 = 250;
    const SE: u8 = 240;
    let mut clean = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        match raw[i] {
            IAC => match raw.get(i + 1) {
                Some(&IAC) => {
                    clean.push(IAC);
                    i += 2;
                }
                Some(251..=254) => i += 3,
                Some(&SB) => {
                    i += 2;
                    while i < raw.len() && !(raw[i] == IAC && raw.get(i + 1) == Some(&SE)) {
                        i += 1;
                    }
                    i += 2;
                }
                _ => i += 2,
            },
            b'\r' => i += 1,
            b => {
                clean.push(b);
                i += 1;
            }
        }
    }
    clean
}

/// The shell's answer once its end marker is whole (marker, exit status, newline): the text before the
/// marker, and whether the status was zero. None while the answer is still coming.
fn telnet_answer(raw: &[u8], begin: &str, end: &str) -> Option<(bool, String)> {
    let body = telnet_text(raw, begin)?;
    body.rmatch_indices(end).find_map(|(at, _)| {
        let after = &body[at + end.len()..];
        let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
        let whole = !digits.is_empty() && after[digits.len()..].starts_with('\n');
        let rc: u32 = digits.parse().ok().filter(|_| whole)?;
        Some((rc == 0, body[..at].to_string()))
    })
}

/// `adb -s SERIAL shell` with the script on its standard input.
fn adb_shell(serial: &str, script: &str, stderr: Stdio) -> Result<std::process::Output> {
    let mut child = Command::new("adb")
        .args(["-s", serial, "shell"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(stderr)
        .spawn()
        .context("starting adb (is it installed?)")?;
    let mut stdin = child.stdin.take().context("adb stdin")?;
    stdin.write_all(script.as_bytes())?;
    drop(stdin);
    Ok(child.wait_with_output()?)
}

/// The phone's password: $PHONE_PASS, else ~/.config/devlab/CODENAME/password (keep it mode 600).
fn password(device: &Device) -> Result<String> {
    if let Some(p) = crate::cli::env().phone_pass.clone() {
        return Ok(p);
    }
    let f = device.config_dir().join("password");
    Ok(std::fs::read_to_string(&f)
        .with_context(|| {
            format!(
                "no password: set PHONE_PASS or write {} (mode 600)",
                f.display()
            )
        })?
        .trim_end_matches('\n')
        .to_string())
}

/// Whether the phone answering is a new boot: a different boot id than before the reboot. Uptime
/// cannot tell - a young boot still shutting down looks just like a new one.
pub fn rebooted(before: Option<&str>, now: Option<&str>) -> bool {
    matches!((before, now), (Some(b), Some(n)) if b != n)
}

/// Reboots the phone and waits until a new boot answers.
pub fn reboot(device: &Device, phone: &Phone) -> Result<Phone> {
    let before = phone
        .boot_id()
        .context("reading the boot id before rebooting")?;
    phone.root("systemd-run --on-active=2 systemctl reboot >/dev/null\n")?;
    std::thread::sleep(std::time::Duration::from_secs(20));
    for _ in 0..90 {
        if let Ok(p) = Phone::find(device) {
            if rebooted(Some(&before), p.boot_id().as_deref()) {
                return Ok(p);
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(4));
    }
    bail!("the phone did not come back")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_different_boot_id_is_a_new_boot() {
        assert!(rebooted(Some("a"), Some("b")));
        assert!(
            !rebooted(Some("a"), Some("a")),
            "the old boot, still shutting down"
        );
        assert!(!rebooted(Some("a"), None), "no answer yet");
    }

    #[test]
    fn the_script_never_rides_on_the_command_line() {
        let a = ssh_args("user@172.16.42.1", AS_ROOT);
        assert_eq!(a[a.len() - 2], "user@172.16.42.1");
        assert_eq!(
            a[a.len() - 1],
            "sudo -S -p '' sh -s",
            "only the fixed runner - the script goes on stdin"
        );
    }
}

#[cfg(test)]
mod telnet_tests {
    use super::*;

    #[test]
    fn a_shell_that_echoes_its_input_is_not_mistaken_for_its_output() {
        let raw = b"\xff\xfd\x01echo B1\r\necho hi\r\necho E1-$?\r\nB1\r\nhi\r\nE1-0\r\n";
        let (ok, text) = telnet_answer(raw, "B1", "E1-").unwrap();
        assert_eq!((ok, text.as_str()), (true, "hi\n"));
        let (ok, _) = telnet_answer(b"B1\nE1-3\n", "B1", "E1-").unwrap();
        assert!(!ok, "the script's own failure");
    }

    #[test]
    fn an_answer_is_not_whole_until_its_end_marker_is() {
        assert!(telnet_answer(b"B1\nhi\n", "B1", "E1-").is_none());
        assert!(
            telnet_answer(b"B1\nhi\nE1-1", "B1", "E1-").is_none(),
            "the status may still grow"
        );
        assert_eq!(
            telnet_answer(b"B1\nno newline E1-0\n", "B1", "E1-")
                .unwrap()
                .1,
            "no newline "
        );
    }
}

#[cfg(test)]
mod telnet_review_tests {
    use super::*;

    #[test]
    fn a_prompt_before_the_markers_is_ignored_and_the_echoed_command_is_not_the_marker() {
        let raw =
            b"sh$ echo B1\r\nsh$ echo hi\r\nsh$ echo E1-$?\r\nsh$ B1\r\nsh$ hi\r\nsh$ E1-0\r\n";
        let (ok, text) = telnet_answer(raw, "B1", "E1-").unwrap();
        assert!(ok);
        assert!(text.contains("hi") && !text.contains("echo E1-0"), "{text}");
    }

    #[test]
    fn negotiation_of_every_kind_is_dropped_whole() {
        let raw = b"\xff\xfb\x01\xff\xfa\x18\x01\xff\xf0\xff\xf1B1\nok\xff\xffx\nE1-0\n";
        let (ok, text) = telnet_answer(raw, "B1", "E1-").unwrap();
        assert_eq!((ok, text.as_str()), (true, "ok\u{fffd}x\n"));
    }

    #[test]
    fn a_status_split_across_reads_is_found_when_it_is_whole() {
        assert!(ends_with_marker(b"...\nE1-", b"E1-"));
        assert!(ends_with_marker(b"...\nE1-12\n", b"E1-"));
        assert!(!ends_with_marker(&[b'x'; 100], b"E1-"));
    }
}
