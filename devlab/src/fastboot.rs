//! A phone reached over fastboot - before Linux runs on it, or while it sits in its bootloader - by its
//! manifest's `[fastboot] serial`, never "whichever phone is in fastboot" (a lab may hold two).
//!   devlab fastboot state             is it in fastboot now, and what its bootloader says of itself
//!   devlab fastboot getvar NAME       one variable, read only
//!   devlab fastboot boot now IMAGE    an Android boot image booted from memory: nothing written, both
//!                                     slots as they were; the next power cycle comes back to them
//!   devlab fastboot reboot now [--bootloader]   leave fastboot to the active slot, or back to fastboot
//! No word here writes, erases or changes the active slot: writing is devlab flash's, and only where
//! the manifest says `writes = true` (decision 2026-11-17-walleye-fastboot-first).
use crate::device::Device;
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

/// The variables `state` reads: what a keeper needs before any try.
const STATE: &[&str] = &[
    "product",
    "serialno",
    "unlocked",
    "slot-count",
    "current-slot",
    "version-bootloader",
    "version-baseband",
];

/// The phone's serial, as its manifest says it: there is no reaching it by fastboot without one.
pub fn serial(device: &Device) -> Result<&str> {
    let s = device.fastboot.serial.trim();
    if s.is_empty() {
        bail!(
            "{}'s device.toml says no [fastboot] serial - fastboot is reached only by serial (fastboot devices lists them)",
            device.device.codename
        );
    }
    Ok(s)
}

/// Whether `fastboot devices` lists this serial in fastboot mode.
pub fn listed(devices: &str, serial: &str) -> bool {
    devices.lines().any(|l| {
        let mut w = l.split_whitespace();
        w.next() == Some(serial) && w.next() == Some("fastboot")
    })
}

/// Whether the phone answers in fastboot now.
pub fn present(serial: &str) -> Result<bool> {
    let out = Command::new("fastboot")
        .arg("devices")
        .output()
        .context("running fastboot (android-tools)")?;
    Ok(listed(&String::from_utf8_lossy(&out.stdout), serial))
}

fn fastboot(serial: &str, args: &[&str]) -> Result<(bool, String)> {
    let out = Command::new("timeout")
        .arg("60")
        .arg("fastboot")
        .args(["-s", serial])
        .args(args)
        .output()
        .context("running fastboot (android-tools)")?;
    // fastboot speaks on standard error
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    Ok((out.status.success(), said))
}

/// A getvar answer, from what fastboot said (`NAME: VALUE` on standard error).
pub fn answer(said: &str, name: &str) -> Option<String> {
    said.lines()
        .find_map(|l| l.strip_prefix(&format!("{name}:")))
        .map(|v| v.trim().to_string())
}

fn need(serial: &str) -> Result<()> {
    if !present(serial)? {
        bail!("{serial} is not in fastboot now (fastboot devices does not list it)");
    }
    Ok(())
}

/// One variable, read only.
pub fn getvar(device: &Device, name: &str) -> Result<String> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_:".contains(c))
    {
        bail!("{name:?}: not a fastboot variable name");
    }
    let s = serial(device)?;
    need(s)?;
    let (ok, said) = fastboot(s, &["getvar", name])?;
    match answer(&said, name) {
        Some(v) if ok => Ok(v),
        _ => bail!("fastboot getvar {name} said: {}", said.trim()),
    }
}

/// Whether it is in fastboot, and what its bootloader says of itself.
pub fn state(device: &Device) -> Result<String> {
    let s = serial(device)?;
    if !present(s)? {
        return Ok(format!(
            "{}: {s} is not in fastboot now (running a system, off, or not plugged in)",
            device.device.codename
        ));
    }
    let mut out = format!("{}: {s} is in fastboot\n", device.device.codename);
    for name in STATE {
        let (_, said) = fastboot(s, &["getvar", name])?;
        let v = answer(&said, name).unwrap_or_else(|| "not said".into());
        out.push_str(&format!("  {name}: {v}\n"));
    }
    Ok(out)
}

/// Whether the file is an Android boot image (its magic), the only thing `fastboot boot` takes here.
pub fn is_boot_image(head: &[u8]) -> bool {
    head.starts_with(b"ANDROID!")
}

/// Boots an image from memory: nothing is written, the slots stay as they were. Says whether the phone
/// left fastboot (the image was taken and started), within a minute.
pub fn boot(device: &Device, image: &Path) -> Result<String> {
    let s = serial(device)?;
    let mut head = [0u8; 8];
    {
        use std::io::Read;
        std::fs::File::open(image)
            .with_context(|| format!("reading {}", image.display()))?
            .read_exact(&mut head)
            .with_context(|| format!("{} is too short for a boot image", image.display()))?;
    }
    if !is_boot_image(&head) {
        bail!(
            "{} is not an Android boot image (no ANDROID! magic): make one with mkbootimg or pmbootstrap export",
            image.display()
        );
    }
    let _held = crate::hold::hold(device)?;
    need(s)?;
    let path = image.to_str().context("the image's path")?;
    let (ok, said) = fastboot(s, &["boot", path])?;
    if !ok {
        bail!(
            "fastboot boot refused it - nothing written: {}",
            said.trim()
        );
    }
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(60) {
        if !present(s)? {
            return Ok(format!(
                "booted {} from memory - nothing written, slots as they were; {s} left fastboot after {} s. \
                 A power cycle (or devlab fastboot reboot once it is back in fastboot) returns to the active slot.",
                image.display(),
                start.elapsed().as_secs()
            ));
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    Ok(format!(
        "fastboot took {} but {s} is still in fastboot after a minute - nothing written; it did not start",
        image.display()
    ))
}

/// Leaves fastboot to the active slot, or reboots back into fastboot.
pub fn reboot(device: &Device, bootloader: bool) -> Result<String> {
    let s = serial(device)?;
    let _held = crate::hold::hold(device)?;
    need(s)?;
    let args: &[&str] = if bootloader {
        &["reboot", "bootloader"]
    } else {
        &["reboot"]
    };
    let (ok, said) = fastboot(s, args)?;
    if !ok {
        bail!("fastboot {} said: {}", args.join(" "), said.trim());
    }
    Ok(if bootloader {
        format!("{s} reboots into fastboot")
    } else {
        format!("{s} reboots to its active slot")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_phone_is_found_by_its_own_serial_only() {
        let two = "FA81N1A00790\tfastboot\n4e6b3c1a\tfastboot\n";
        assert!(listed(two, "FA81N1A00790"));
        assert!(listed(two, "4e6b3c1a"));
        assert!(!listed(two, "FA81N1A0079"), "a prefix is not the serial");
        assert!(!listed("FA81N1A00790\tsideload\n", "FA81N1A00790"));
        assert!(!listed("", "FA81N1A00790"));
    }

    #[test]
    fn getvar_answers_are_read_off_what_fastboot_says() {
        let said = "current-slot: b\nFinished. Total time: 0.002s\n";
        assert_eq!(answer(said, "current-slot").as_deref(), Some("b"));
        assert_eq!(answer(said, "unlocked"), None);
    }

    #[test]
    fn only_a_boot_image_is_booted() {
        assert!(is_boot_image(b"ANDROID!\x00\x00"));
        assert!(!is_boot_image(b"\x7fELF"));
        assert!(!is_boot_image(b"MZ"));
    }

    #[test]
    fn no_serial_no_fastboot() {
        let d = Device::parse(
            "[device]\ncodename = \"x\"\n[connection]\nuser = \"u\"\naddresses = [\"a\"]\n",
        )
        .unwrap();
        assert!(serial(&d).is_err());
        assert!(!d.fastboot.writes, "writing is never assumed");
        let d = Device::parse(
            "[device]\ncodename = \"google-walleye\"\n[connection]\nuser = \"u\"\naddresses = [\"a\"]\n\
             [fastboot]\nserial = \"FA81N1A00790\"\n",
        )
        .unwrap();
        assert_eq!(serial(&d).unwrap(), "FA81N1A00790");
        assert!(Device::parse(
            "[device]\ncodename = \"x\"\n[connection]\nuser = \"u\"\naddresses = [\"a\"]\n\
             [fastboot]\nserial = \"s\"\nslot = \"a\"\n"
        )
        .is_err());
    }
}
