//! Reaching the phone over SSH: commands travel as a script on standard input (`sh -s`), never
//! inside quotes on a command line, so quotes, pipes and $ in them need no escaping.

use crate::device::{candidates, Device};
use anyhow::{bail, Context, Result};
use std::io::Write;
use std::process::{Command, Stdio};

pub struct Phone {
    pub target: String,
    password: String,
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

impl Phone {
    /// The first address that answers: the manifest's, then the remembered Wi-Fi one.
    pub fn find(device: &Device) -> Result<Phone> {
        let password = password(device)?;
        let remembered = std::fs::read_to_string(device.state_dir().join("wifi-address")).ok();
        for addr in candidates(&device.connection.addresses, remembered.as_deref()) {
            let p = Phone {
                target: format!("{}@{addr}", device.connection.user),
                password: password.clone(),
            };
            if p.run("true", AS_USER, "").map(|o| o.0).unwrap_or(false) {
                return Ok(p);
            }
        }
        bail!(
            "{} does not answer at any known address",
            device.device.codename
        )
    }

    /// Runs `script` on the phone; returns (succeeded, stdout).
    pub fn run(&self, script: &str, how: &str, prefix: &str) -> Result<(bool, String)> {
        let mut child = Command::new("sshpass")
            .arg("-e")
            .arg("ssh")
            .args(ssh_args(&self.target, how))
            .env("SSHPASS", &self.password)
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

    pub fn sh(&self, script: &str) -> Result<(bool, String)> {
        self.run(script, AS_USER, "")
    }

    pub fn root(&self, script: &str) -> Result<(bool, String)> {
        self.run(script, AS_ROOT, &format!("{}\n", self.password))
    }

    /// Copies a local file to the phone.
    pub fn put(&self, local: &str, remote: &str) -> Result<()> {
        let mut a: Vec<String> = SSH_OPTS.iter().map(|s| s.to_string()).collect();
        a.push(local.to_string());
        a.push(format!("{}:{remote}", self.target));
        let ok = Command::new("sshpass")
            .arg("-e")
            .arg("scp")
            .args(a)
            .env("SSHPASS", &self.password)
            .status()?
            .success();
        if !ok {
            bail!("copying {local} to the phone failed");
        }
        Ok(())
    }

    pub fn uptime(&self) -> Option<u64> {
        let (ok, out) = self.sh("cut -d. -f1 /proc/uptime").ok()?;
        ok.then(|| out.trim().parse().ok()).flatten()
    }
}

/// The phone's password: $PHONE_PASS, else ~/.config/devlab/CODENAME/password (keep it mode 600).
fn password(device: &Device) -> Result<String> {
    if let Ok(p) = std::env::var("PHONE_PASS") {
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

/// Reboots the phone and waits until it answers again, freshly booted.
pub fn reboot(device: &Device, phone: &Phone) -> Result<Phone> {
    phone.root("systemd-run --on-active=2 systemctl reboot >/dev/null\n")?;
    std::thread::sleep(std::time::Duration::from_secs(20));
    for _ in 0..90 {
        if let Ok(p) = Phone::find(device) {
            if p.uptime().is_some_and(|u| u < 300) {
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
