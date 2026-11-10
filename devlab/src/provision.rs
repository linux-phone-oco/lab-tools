//! Provisioning a freshly flashed phone, for any device: the manifest's packages over the lab
//! machine's internet connection (a new phone has no network of its own), files the manifest names
//! (installed when their condition holds on the phone), then the user's Wi-Fi from private files.
//!
//! The lab machine shares its connection only while installing - forwarding, a masquerade rule in
//! an nftables table of its own, and with ufw active a route rule - and a guard undoes all of it,
//! and the phone's route and DNS, when it goes out of scope: also on any error.

use crate::device::Device;
use crate::phone::Phone;
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

/// `ip route show default` -> (gateway, interface).
pub fn default_route(text: &str) -> Option<(String, String)> {
    let w: Vec<&str> = text.lines().next()?.split_whitespace().collect();
    let at = |k: &str| {
        w.iter()
            .position(|x| *x == k)
            .and_then(|i| w.get(i + 1))
            .map(|s| s.to_string())
    };
    Some((at("via")?, at("dev")?))
}

/// `ip -o -4 addr` -> (interface, address) of this machine on the phone's subnet (the same first
/// three octets as `phone`).
pub fn local_on_subnet(text: &str, phone: &str) -> Option<(String, String)> {
    let net = phone.rsplit_once('.')?.0;
    text.lines().find_map(|l| {
        let w: Vec<&str> = l.split_whitespace().collect();
        let i = w.iter().position(|x| *x == "inet")?;
        let ip = w.get(i + 1)?.split('/').next()?;
        let iface = w.get(1)?.to_string();
        (ip.rsplit_once('.')?.0 == net && ip != phone).then(|| (iface, ip.to_string()))
    })
}

/// A package list: one per line, blank lines and # comments skipped.
pub fn packages(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

/// A manifest path with the phone's compatible name filled in.
pub fn fill(template: &str, compatible: &str) -> String {
    template.replace("{compatible}", compatible)
}

/// A command as root on this machine: $DEVLAB_ROOT (default: lab-agent's root-exec).
fn host_root(args: &[&str]) -> Result<bool> {
    let prefix =
        std::env::var("DEVLAB_ROOT").unwrap_or_else(|_| "sudo -n lab-agent root-exec --".into());
    let mut p = prefix.split_whitespace();
    let mut c = Command::new(p.next().context("DEVLAB_ROOT is empty")?);
    c.args(p).args(args);
    Ok(c.status()?.success())
}

fn output(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

/// The lab machine's connection, shared with the phone while it lives.
struct Share<'a> {
    phone: &'a Phone,
    usb: String,
    uplink: String,
    pc: String,
    forwarding_was_on: bool,
    ufw: bool,
}

const TABLE: &str = "devlab-provision";

impl<'a> Share<'a> {
    fn start(phone: &'a Phone, phone_addr: &str) -> Result<Share<'a>> {
        let (gw, uplink) = default_route(&output("ip", &["route", "show", "default"]))
            .context("no default route on this machine")?;
        let (usb, pc) = local_on_subnet(&output("ip", &["-o", "-4", "addr"]), phone_addr)
            .context("no address on the phone's USB subnet")?;
        let forwarding_was_on = std::fs::read_to_string("/proc/sys/net/ipv4/ip_forward")
            .map(|s| s.trim() == "1")
            .unwrap_or(false);
        let ufw = Command::new("sh")
            .arg("-c")
            .arg("sudo -n lab-agent root-exec -- ufw status 2>/dev/null | grep -q 'Status: active'")
            .status()
            .is_ok_and(|s| s.success());
        let s = Share {
            phone,
            usb,
            uplink,
            pc,
            forwarding_was_on,
            ufw,
        };
        host_root(&["sysctl", "-qw", "net.ipv4.ip_forward=1"])?;
        if s.ufw {
            host_root(&[
                "ufw", "route", "allow", "in", "on", &s.usb, "out", "on", &s.uplink,
            ])?;
        }
        host_root(&["nft", "add", "table", "ip", TABLE])?;
        host_root(&[
            "nft",
            "add",
            "chain",
            "ip",
            TABLE,
            "out",
            "{ type nat hook postrouting priority 100; }",
        ])?;
        let subnet = format!(
            "{}.0/24",
            phone_addr.rsplit_once('.').context("phone address")?.0
        );
        host_root(&[
            "nft",
            "add",
            "rule",
            "ip",
            TABLE,
            "out",
            "ip",
            "saddr",
            &subnet,
            "oifname",
            &s.uplink,
            "masquerade",
        ])?;
        let (ok, _) = phone.root(&format!(
            "ip route add default via {pc}\ncp /etc/resolv.conf /tmp/resolv.conf.devlab\necho nameserver {gw} > /etc/resolv.conf\nping -c1 -W3 1.1.1.1 >/dev/null\n",
            pc = s.pc
        ))?;
        if !ok {
            bail!("the phone cannot reach the internet through this machine");
        }
        Ok(s)
    }
}

impl Drop for Share<'_> {
    fn drop(&mut self) {
        let _ = self.phone.root(&format!(
            "ip route del default via {} 2>/dev/null\n[ -f /tmp/resolv.conf.devlab ] && cat /tmp/resolv.conf.devlab > /etc/resolv.conf && rm /tmp/resolv.conf.devlab\ntrue\n",
            self.pc
        ));
        let _ = host_root(&["nft", "delete", "table", "ip", TABLE]);
        if self.ufw {
            let _ = host_root(&[
                "ufw",
                "route",
                "delete",
                "allow",
                "in",
                "on",
                &self.usb,
                "out",
                "on",
                &self.uplink,
            ]);
        }
        if !self.forwarding_was_on {
            let _ = host_root(&["sysctl", "-qw", "net.ipv4.ip_forward=0"]);
        }
    }
}

pub fn provision(device: &Device, manifest_dir: &Path) -> Result<()> {
    // sharing the connection needs the USB link: the manifest's first address
    let usb_addr = device
        .connection
        .addresses
        .first()
        .context("no address in the manifest")?
        .clone();
    let phone = Phone::at(device, &usb_addr)?;
    let list = manifest_dir.join(&device.provision.packages);
    let pkgs = packages(
        &std::fs::read_to_string(&list).with_context(|| format!("reading {}", list.display()))?,
    );
    let compatible = phone.sh("tr -c '\\0[:alnum:].,-' '_' < /proc/device-tree/compatible | tr '\\0' '\\n' | head -n1\n")?.1.trim().to_string();
    {
        let _share = Share::start(&phone, &usb_addr)?;
        if !pkgs.is_empty() {
            println!("installing: {}", pkgs.join(" "));
            let (ok, _) =
                phone.root(&format!("apk update -q && apk add -q {}\n", pkgs.join(" ")))?;
            if !ok {
                bail!("installing the packages failed");
            }
        }
    } // the connection is no longer shared
    for f in &device.provision.install {
        let from = manifest_dir.join(fill(&f.from, &compatible));
        let to = fill(&f.to, &compatible);
        if !from.is_file() {
            continue;
        }
        if !f.when.is_empty() && !phone.sh(&format!("{}\n", f.when))?.0 {
            continue;
        }
        phone.put(from.to_str().context("path")?, "/tmp/devlab-file")?;
        let (ok, _) = phone.root(&format!("install -m {} /tmp/devlab-file '{to}'\n", f.mode))?;
        println!("{}: {to}", if ok { "installed" } else { "FAILED" });
    }
    // Wi-Fi, from private files: ~/.config/devlab/CODENAME/wifi-ssid and wifi-password
    let cfg = device.config_dir();
    if let (Ok(ssid), true) = (
        std::fs::read_to_string(cfg.join("wifi-ssid")),
        cfg.join("wifi-password").is_file(),
    ) {
        phone.put(
            cfg.join("wifi-password").to_str().context("path")?,
            "/tmp/.wifi-pw",
        )?;
        let _ = phone.root(&format!(
            "chmod 600 /tmp/.wifi-pw\nnmcli dev wifi connect '{}' password \"$(cat /tmp/.wifi-pw)\" >/dev/null 2>&1\nrm -f /tmp/.wifi-pw\n",
            ssid.trim().replace('\'', "")
        ));
        let addr = phone
            .sh("ip -4 addr show wlan0 | grep -o 'inet [0-9.]*' | cut -d' ' -f2\n")?
            .1
            .trim()
            .to_string();
        if !addr.is_empty() {
            std::fs::create_dir_all(device.state_dir())?;
            std::fs::write(device.state_dir().join("wifi-address"), &addr)?;
            println!("wifi: {addr}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_uplink_and_gateway_come_from_the_default_route() {
        let t = "default via 192.168.178.1 dev enp6s0 proto dhcp src 192.168.178.74 metric 100\n";
        assert_eq!(
            default_route(t),
            Some(("192.168.178.1".into(), "enp6s0".into()))
        );
        assert_eq!(default_route(""), None);
    }

    #[test]
    fn this_machine_is_found_on_the_phones_subnet() {
        let t = "2: enp6s0    inet 192.168.178.74/24 brd 192.168.178.255 scope global\n9: enp10s0f3u1    inet 172.16.42.2/24 brd 172.16.42.255 scope global\n";
        assert_eq!(
            local_on_subnet(t, "172.16.42.1"),
            Some(("enp10s0f3u1".into(), "172.16.42.2".into()))
        );
        assert_eq!(local_on_subnet(t, "10.0.0.1"), None);
    }

    #[test]
    fn a_package_list_skips_comments_and_blanks() {
        assert_eq!(
            packages("# a list\nfirefox\n\n  i2c-tools  \n"),
            ["firefox", "i2c-tools"]
        );
    }

    #[test]
    fn manifest_paths_take_the_phones_name() {
        assert_eq!(
            fill(
                "p/sxmo_deviceprofile_{compatible}.sh",
                "xiaomi,surya-huaxing"
            ),
            "p/sxmo_deviceprofile_xiaomi,surya-huaxing.sh"
        );
    }
}
