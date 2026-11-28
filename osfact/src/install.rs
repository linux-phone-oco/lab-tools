// SPDX-License-Identifier: MIT OR Apache-2.0
//! install.toml: how one packed image reaches a device, read, listed (`commands`) and run (`run`).
//! The skeleton knows one way and nothing is written to the device: boot an image from RAM over fastboot (step
//! `boot-from-ram`), push the root image - compressed, its sha256 checked on the device - into RAM over the
//! declared connection (`push`), enter it (`enter`, by chroot), and on any failure the declared way back.
//! What it does not allow (a write to the device, an unknown transport, an entry it does not run) does not parse:
//! a refused plan fails at `read`, naming the value (a later item, osfact-install-flash, adds the writes).
//!
//! The device is reached through the `Device` trait: `device::Live` is the real one (fastboot, a root shell over
//! TCP, a pushed stream), a test brings a stand-in.

use crate::rootfs;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub install: Install,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Install {
    pub device: String,
    /// The environment variable that holds the device's serial; every fastboot call names it.
    pub serial_env: String,
    /// How the host reaches the booted image (a root shell and a port to push to); needed by `push` and `enter`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<Connection>,
    pub image: Vec<Image>,
    pub step: Vec<Step>,
    pub way_back: WayBack,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    pub address: String,
    /// The port of a root shell with no login (telnetd); one command at a time, its end marked.
    pub shell_port: u16,
    /// Seconds the shell may take to answer after the boot.
    pub wait: u32,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Image {
    pub name: String,
    pub file: String,
    /// The sha256 of the file as it is sent (a compressed image: the compressed file's).
    pub sha256: String,
    /// How the file is compressed, if it is: the device unpacks it as `enter` says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compression: Option<Compression>,
}

#[derive(Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum Compression {
    Gzip,
    Xz,
}

impl Compression {
    /// The device's command that writes the unpacked tar to its stdout.
    fn unpack(self, file: &str) -> String {
        match self {
            Compression::Gzip => format!("gzip -dc {file}"),
            Compression::Xz => format!("xz -dc {file}"),
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    #[serde(rename = "do")]
    pub action: StepAction,
    /// The image the step sends (boot-from-ram: the boot image; push, enter: the root image).
    pub write: String,
    pub via: Transport,
    /// push: the device's port the receiver listens on, the receiver's command, where it writes the file, and the
    /// seconds the whole push may take to end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receive: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait: Option<u32>,
    /// enter: how (chroot), the folder the root is unpacked into (a tmpfs of `size_mib`), the command started in it
    /// and a command that must succeed once it is started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub how: Option<Enter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_mib: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<String>,
}

impl Step {
    pub fn boot_from_ram(image: &str) -> Step {
        Step {
            action: StepAction::BootFromRam,
            write: image.into(),
            via: Transport::Fastboot,
            port: None,
            receive: None,
            to: None,
            wait: None,
            how: None,
            root: None,
            size_mib: None,
            command: None,
            check: None,
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum StepAction {
    BootFromRam,
    Push,
    Enter,
}

#[derive(Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum Transport {
    Fastboot,
    /// A stream to a port of the booted image.
    Network,
    /// Commands in the booted image's root shell.
    Shell,
}

/// How a pushed root is entered. Only the chroot runs: a switch_root replaces the init the host's shell belongs to
/// and ends the connection the run reads (decision 2026-11-28-osfact-enter-by-chroot).
#[derive(Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum Enter {
    Chroot,
    SwitchRoot,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WayBack {
    pub via: Transport,
    #[serde(rename = "do")]
    pub action: WayBackAction,
    pub wait: u32,
    /// via shell: the command the booted image is told to restart the device with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WayBackAction {
    Reboot,
}

pub fn read(text: &str) -> Result<Plan, String> {
    let plan: Plan =
        toml::from_str(text).map_err(|e| format!("osfact refuses this install.toml: {e}"))?;
    let refuse = |why: String| Err(format!("osfact refuses this install.toml: {why}"));
    let i = &plan.install;
    if i.step.is_empty() || i.step[0].action != StepAction::BootFromRam {
        return Err("osfact refuses this install.toml: it has no boot step first".into());
    }
    for (n, s) in i.step.iter().enumerate() {
        let at = format!("step {} ({:?} {})", n + 1, s.action, s.write);
        if !i.image.iter().any(|m| m.name == s.write) {
            return refuse(format!(
                "{at} writes image '{}', which install.toml does not list",
                s.write
            ));
        }
        let need = |v: bool, what: &str| -> Result<(), String> {
            if v {
                Ok(())
            } else {
                Err(format!(
                    "osfact refuses this install.toml: {at} needs {what}"
                ))
            }
        };
        match s.action {
            StepAction::BootFromRam => need(s.via == Transport::Fastboot, "via = \"fastboot\"")?,
            StepAction::Push => {
                need(i.connection.is_some(), "[install.connection]")?;
                need(s.via == Transport::Network, "via = \"network\"")?;
                need(
                    s.port.is_some() && s.receive.is_some() && s.to.is_some() && s.wait.is_some(),
                    "port, receive, to and wait",
                )?;
            }
            StepAction::Enter => {
                need(i.connection.is_some(), "[install.connection]")?;
                need(s.via == Transport::Shell, "via = \"shell\"")?;
                need(
                    s.root.is_some() && s.size_mib.is_some() && s.command.is_some(),
                    "root, size_mib and command",
                )?;
                match s.how {
                    Some(Enter::Chroot) => {}
                    Some(Enter::SwitchRoot) => {
                        return refuse(format!(
                            "{at}: how = \"switch-root\" is not run yet (it ends the shell the host reads); use \"chroot\""
                        ))
                    }
                    None => need(false, "how")?,
                }
                need(
                    i.step[..n]
                        .iter()
                        .any(|p| p.action == StepAction::Push && p.write == s.write),
                    "a push of the same image before it",
                )?;
            }
        }
    }
    if i.way_back.via == Transport::Shell && i.way_back.command.is_none() {
        return refuse("the way back via shell needs a command".into());
    }
    if i.connection.is_some() && i.way_back.via != Transport::Shell {
        return refuse(
            "with a connection the way back is via shell: after the boot the device is not in fastboot".into(),
        );
    }
    if i.way_back.via == Transport::Network {
        return refuse("the way back cannot be via network".into());
    }
    Ok(plan)
}

fn image<'a>(i: &'a Install, name: &str) -> &'a Image {
    i.image
        .iter()
        .find(|m| m.name == name)
        .expect("read checked that every step's image is listed")
}

fn basename(file: &str) -> &str {
    file.rsplit('/').next().unwrap_or(file)
}

/// The commands of a plan in order, the way back last.
pub fn commands(plan: &Plan) -> Result<Vec<String>, String> {
    let i = &plan.install;
    let mut out = Vec::new();
    for s in &i.step {
        let m = image(i, &s.write);
        match s.action {
            StepAction::BootFromRam => {
                out.push(format!("fastboot -s ${} boot {}", i.serial_env, m.file));
                if let Some(c) = &i.connection {
                    out.push(format!(
                        "wait for the connection {}:{} ({} s)",
                        c.address, c.shell_port, c.wait
                    ));
                }
            }
            StepAction::Push => {
                let c = i.connection.as_ref().expect("read checked");
                out.push(format!(
                    "push {} to {}:{} as {}, sha256 {} checked on the device (receiver: {})",
                    basename(&m.file),
                    c.address,
                    s.port.expect("read checked"),
                    s.to.as_deref().expect("read checked"),
                    m.sha256,
                    s.receive.as_deref().expect("read checked"),
                ));
            }
            StepAction::Enter => {
                out.push(format!(
                    "enter {}: chroot {} {} (unpacked into a {} MiB tmpfs)",
                    s.write,
                    s.root.as_deref().expect("read checked"),
                    s.command.as_deref().expect("read checked"),
                    s.size_mib.expect("read checked"),
                ));
            }
        }
    }
    out.push(match i.way_back.via {
        Transport::Shell => format!(
            "way back: in the shell `{}` (wait {} s for {})",
            i.way_back.command.as_deref().unwrap_or_default(),
            i.way_back.wait,
            i.device
        ),
        _ => format!(
            "way back: fastboot -s ${} reboot (wait {} s for {})",
            i.serial_env, i.way_back.wait, i.device
        ),
    });
    Ok(out)
}

/// What a command said in the device's shell: its exit status and its text.
pub struct Shell {
    pub status: i32,
    pub text: String,
}

/// The device as the run sees it. `device::Live` is the real one.
pub trait Device {
    /// `fastboot boot FILE`: the image runs from RAM, nothing is written.
    fn boot(&mut self, file: &str) -> Result<(), String>;
    /// The root shell answers within `wait_s` seconds.
    fn connect(&mut self, wait_s: u32) -> Result<(), String>;
    /// One command in the root shell, ended within `wait_s` seconds.
    fn shell(&mut self, command: &str, wait_s: u32) -> Result<Shell, String>;
    /// The file's bytes to the device's `port`, the stream closed after the last: the bytes sent. The receiver
    /// may start a moment after the call is made: it is tried for `wait_s` seconds.
    fn send(&mut self, port: u16, file: &str, wait_s: u32) -> Result<u64, String>;
    /// The declared way back.
    fn way_back(&mut self, back: &WayBack) -> Result<(), String>;
}

/// Runs the plan's steps in order. A failing step runs the way back and the error names the step and why.
/// `say` gets one line per step as it starts.
pub fn run(plan: &Plan, device: &mut dyn Device, say: &mut dyn FnMut(&str)) -> Result<(), String> {
    let i = &plan.install;
    for (n, s) in i.step.iter().enumerate() {
        let at = format!("step {} ({})", n + 1, step_name(s));
        say(&at);
        if let Err(why) = step(i, s, device) {
            let mut e = format!("{at} failed: {why}");
            if let Err(back) = device.way_back(&i.way_back) {
                e.push_str(&format!("; the way back failed too: {back}"));
            } else {
                e.push_str("; the way back was run");
            }
            return Err(e);
        }
    }
    Ok(())
}

fn step_name(s: &Step) -> String {
    match s.action {
        StepAction::BootFromRam => format!("boot {}", s.write),
        StepAction::Push => format!("push {}", s.write),
        StepAction::Enter => format!("enter {}", s.write),
    }
}

fn step(i: &Install, s: &Step, d: &mut dyn Device) -> Result<(), String> {
    let m = image(i, &s.write);
    match s.action {
        StepAction::BootFromRam => {
            d.boot(&m.file)?;
            if let Some(c) = &i.connection {
                d.connect(c.wait)
                    .map_err(|e| format!("no connection to {}:{}: {e}", c.address, c.shell_port))?;
            }
            Ok(())
        }
        StepAction::Push => push(s, m, d),
        StepAction::Enter => enter(i, s, d),
    }
}

/// The receiver is started on the device (its end noted in a status file), the file is sent, and what the device
/// holds is hashed on the device and compared with the packed sha256: a copy that is not the packed one is never entered.
fn push(s: &Step, m: &Image, d: &mut dyn Device) -> Result<(), String> {
    let (port, receive, to, wait) = (
        s.port.expect("read checked"),
        s.receive.as_deref().expect("read checked"),
        s.to.as_deref().expect("read checked"),
        s.wait.expect("read checked"),
    );
    let here = rootfs::sha256(std::path::Path::new(&m.file))?;
    if here != m.sha256 {
        return Err(format!(
            "{} is not the packed image: its sha256 is {here}, the plan says {}",
            m.file, m.sha256
        ));
    }
    let status = format!("{to}.status");
    d.shell(&format!("rm -f {status} {to}"), 5)?;
    d.shell(
        &format!("( {receive} > {to}; echo $? > {status} ) > /dev/null 2>&1 &"),
        5,
    )?;
    d.send(port, &m.file, wait)?;
    let until = Instant::now() + Duration::from_secs(wait.into());
    loop {
        let got = d.shell(&format!("cat {status}"), 5)?;
        match number_line(&got.text) {
            Some(0) => break,
            Some(n) => return Err(format!("the receiver ended with status {n}")),
            None if Instant::now() >= until => {
                return Err(format!(
                    "the receiver did not end within {wait} s of the push"
                ))
            }
            None => std::thread::sleep(Duration::from_secs(1)),
        }
    }
    let got = d.shell(&format!("sha256sum {to}"), 60)?;
    let there = digest_in(&got.text).ok_or_else(|| {
        format!(
            "sha256sum on the device said no digest: {}",
            got.text.trim()
        )
    })?;
    if there != m.sha256 {
        return Err(format!(
            "sha256 on the device is {there}, the packed image's is {}",
            m.sha256
        ));
    }
    Ok(())
}

/// The pushed root is unpacked into a tmpfs, the file removed, the command started in the chroot, the check run.
fn enter(i: &Install, s: &Step, d: &mut dyn Device) -> Result<(), String> {
    let push = i
        .step
        .iter()
        .find(|p| p.action == StepAction::Push && p.write == s.write)
        .expect("read checked");
    let m = image(i, &s.write);
    let (from, root) = (
        push.to.as_deref().expect("read checked"),
        s.root.as_deref().expect("read checked"),
    );
    let ok = |d: &mut dyn Device, c: &str| -> Result<(), String> {
        let r = d.shell(c, 60)?;
        if r.status == 0 {
            Ok(())
        } else {
            Err(format!(
                "`{c}` ended with status {}: {}",
                r.status,
                r.text.trim()
            ))
        }
    };
    ok(d, &format!("mkdir -p {root}"))?;
    if d.shell(&format!("mountpoint -q {root}"), 10)?.status == 0 {
        return Err(format!("{root} is already mounted: a root is entered once"));
    }
    ok(
        d,
        &format!(
            "mount -t tmpfs -o size={}m tmpfs {root}",
            s.size_mib.expect("read checked")
        ),
    )?;
    let unpack = m
        .compression
        .map_or(format!("cat {from}"), |c| c.unpack(from));
    ok(d, &format!("{unpack} | tar -x -C {root}"))?;
    ok(d, &format!("rm -f {from}"))?;
    ok(
        d,
        &format!(
            "setsid chroot {root} {} > /dev/null 2>&1 &",
            s.command.as_deref().expect("read checked")
        ),
    )?;
    if let Some(check) = &s.check {
        let mut tries = 0;
        while ok(d, check).is_err() {
            tries += 1;
            if tries >= 10 {
                return Err(format!(
                    "the check `{check}` did not hold within 10 s of the start"
                ));
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    Ok(())
}

/// The first line of a shell's text that is only a number (a status file's content, not the echoed command).
fn number_line(text: &str) -> Option<i32> {
    text.lines().find_map(|l| l.trim().parse::<i32>().ok())
}

/// The sha256 a `sha256sum` line holds: a 64-character hex word.
fn digest_in(text: &str) -> Option<String> {
    text.split_whitespace()
        .find(|w| w.len() == 64 && w.bytes().all(|b| b.is_ascii_hexdigit()))
        .map(str::to_lowercase)
}
