// SPDX-License-Identifier: MIT OR Apache-2.0
//! A phone reached over fastboot - before Linux runs on it, or while it sits in its bootloader - by its
//! manifest's `[fastboot] serial`, never "whichever phone is in fastboot" (a lab may hold two).
//!   devlab fastboot state             is it in fastboot now, what its bootloader says, and its slots' flags
//!   devlab fastboot getvar NAME       one variable, read only
//!   devlab fastboot boot now IMAGE    an Android boot image booted from memory: devlab writes nothing, and
//!                                     says where the phone went and what the slots' flags became
//!   devlab fastboot reboot now [--bootloader]   leave fastboot to the active slot, or back to fastboot
//!   devlab fastboot set-active now SLOT          the active slot (metadata only), only where the manifest
//!                                     says [fastboot] slot_switch = true
//! devlab writes no partition here and erases nothing: writing is devlab flash's, only where the manifest
//! says `writes = true` (decision 2026-11-17-walleye-fastboot-first). But a phone's bootloader keeps A/B
//! flags of its own and may change them on a boot that fails (google-walleye, 2026-11-17: a RAM boot ended
//! with current-slot b and slot a unbootable) - so every boot and switch says the flags before and after.
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
    "version-bootloader",
    "version-baseband",
];

/// The A/B flags the bootloader keeps, read before and after a boot or a switch.
const SLOTS: &[&str] = &[
    "current-slot",
    "slot-unbootable:a",
    "slot-successful:a",
    "slot-retry-count:a",
    "slot-unbootable:b",
    "slot-successful:b",
    "slot-retry-count:b",
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

/// The slots' flags as the bootloader says them now, in SLOTS' order ("not said" where it says none).
fn slots(serial: &str) -> Result<Vec<(&'static str, String)>> {
    SLOTS
        .iter()
        .map(|name| {
            let (_, said) = fastboot(serial, &["getvar", name])?;
            Ok((
                *name,
                answer(&said, name).unwrap_or_else(|| "not said".into()),
            ))
        })
        .collect()
}

/// What changed between two readings of the slots' flags, in words: "slot-unbootable:a no -> yes, ...".
pub fn changed(before: &[(&str, String)], after: &[(&str, String)]) -> String {
    let moved: Vec<String> = before
        .iter()
        .zip(after)
        .filter(|((_, b), (_, a))| b != a)
        .map(|((name, b), (_, a))| format!("{name} {b} -> {a}"))
        .collect();
    if moved.is_empty() {
        "the slots' flags did not change".into()
    } else {
        format!("the slots' flags changed: {}", moved.join(", "))
    }
}

fn said_slots(flags: &[(&str, String)]) -> String {
    flags
        .iter()
        .map(|(n, v)| format!("{n}={v}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether it is in fastboot, what its bootloader says of itself, and its slots' flags.
pub fn state(device: &Device) -> Result<String> {
    let s = serial(device)?;
    if !present(s)? {
        return Ok(format!(
            "{}: {s} is not in fastboot now ({})",
            device.device.codename,
            elsewhere(device, s).unwrap_or_else(|| "gone from USB: neither its serial nor anything on its USB port answers - off, or it needs hands".into())
        ));
    }
    remember_port(device, s);
    let mut out = format!("{}: {s} is in fastboot\n", device.device.codename);
    for name in STATE {
        let (_, said) = fastboot(s, &["getvar", name])?;
        let v = answer(&said, name).unwrap_or_else(|| "not said".into());
        out.push_str(&format!("  {name}: {v}\n"));
    }
    for (name, v) in slots(s)? {
        out.push_str(&format!("  {name}: {v}\n"));
    }
    Ok(out)
}

/// A USB device as sysfs says it: its port path (e.g. "1-4"), vendor:product, and serial ("" where none).
#[derive(Debug, Clone, PartialEq)]
pub struct Usb {
    pub port: String,
    pub id: String,
    pub serial: String,
}

fn usb_devices() -> Vec<Usb> {
    let Ok(dir) = std::fs::read_dir("/sys/bus/usb/devices") else {
        return Vec::new();
    };
    let read = |p: &Path, f: &str| {
        std::fs::read_to_string(p.join(f))
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    };
    dir.flatten()
        .filter_map(|e| {
            let p = e.path();
            let vendor = read(&p, "idVendor");
            (!vendor.is_empty()).then(|| Usb {
                port: e.file_name().to_string_lossy().into_owned(),
                id: format!("{vendor}:{}", read(&p, "idProduct")),
                serial: read(&p, "serial"),
            })
        })
        .collect()
}

/// The USB port this phone sits on while in fastboot, remembered in its state: another phone's gadget
/// never answers for it (2026-11-17: surya's postmarketOS gadget was taken for walleye's Linux).
fn remember_port(device: &Device, serial: &str) {
    if let Some(u) = usb_devices().into_iter().find(|u| u.serial == serial) {
        let _ = std::fs::create_dir_all(device.state_dir());
        let _ = std::fs::write(device.state_dir().join("usb-port"), &u.port);
    }
}

/// Where this phone answers when not in fastboot - found only by its own serial, or on the USB port it
/// sat on in fastboot; never by a vendor:product alone or by an address another phone may share.
pub fn whereabouts(devices: &[Usb], serial: &str, port: Option<&str>) -> Option<String> {
    let says = |u: &Usb| match u.id.as_str() {
        "18d1:4ee7" | "18d1:4ee2" | "18d1:4ee8" => "Android, its adb gadget".to_string(),
        "18d1:d001" => "Linux, the postmarketOS USB gadget".to_string(),
        "18d1:4ee0" | "18d1:d00d" => "its bootloader".to_string(),
        id => format!("a USB device {id}"),
    };
    if let Some(u) = devices.iter().find(|u| u.serial == serial) {
        return Some(format!("{} (its serial, port {})", says(u), u.port));
    }
    let port = port?;
    devices
        .iter()
        .find(|u| u.port == port)
        .map(|u| format!("{} (on its USB port {port})", says(u)))
}

fn elsewhere(device: &Device, serial: &str) -> Option<String> {
    let port = std::fs::read_to_string(device.state_dir().join("usb-port")).ok();
    whereabouts(&usb_devices(), serial, port.as_deref().map(str::trim))
}

/// Whether the file is an Android boot image (its magic), the only thing `fastboot boot` takes here.
pub fn is_boot_image(head: &[u8]) -> bool {
    head.starts_with(b"ANDROID!")
}

/// Boots an image from memory: devlab writes nothing. Reads the slots' flags first, then watches where
/// the phone goes for up to three minutes - back to fastboot, Android, or Linux on its USB network - and,
/// once it is in fastboot again, says what the flags became. fastboot's own exit is not the verdict: a
/// phone that leaves USB at "Booting" can make it fail (walleye's try b was called refused, and ran).
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
    remember_port(device, s);
    let before = slots(s)?;
    let mut out = format!("before: {}\n", said_slots(&before));
    let path = image.to_str().context("the image's path")?;
    let (ok, said) = fastboot(s, &["boot", path])?;
    let start = Instant::now();
    let mut left = None;
    let mut came = false;
    while start.elapsed() < Duration::from_secs(180) {
        let here = present(s)?;
        match left {
            None if !here => {
                left = Some(start.elapsed().as_secs());
                out.push_str(&format!(
                    "left fastboot after {} s\n",
                    start.elapsed().as_secs()
                ));
            }
            None if start.elapsed() > Duration::from_secs(60) => {
                out.push_str(&format!(
                    "still in fastboot after a minute: it did not start (fastboot said: {})\n",
                    said.trim()
                ));
                break;
            }
            Some(_) if here => {
                out.push_str(&format!(
                    "back in fastboot after {} s\n",
                    start.elapsed().as_secs()
                ));
                break;
            }
            Some(_) => {
                if let Some(where_) = elsewhere(device, s) {
                    out.push_str(&format!(
                        "came back as {where_} after {} s\n",
                        start.elapsed().as_secs()
                    ));
                    came = true;
                    break;
                }
            }
            None => {}
        }
        std::thread::sleep(Duration::from_secs(3));
    }
    if left.is_some() && !came && !present(s)? {
        out.push_str(
            "gone from USB after three minutes: neither its serial nor anything on its USB port answers - \
             it needs hands (power, a button), not a guess\n",
        );
    }
    if left.is_some() && !ok {
        out.push_str(&format!(
            "(fastboot itself said it failed - the phone left anyway: {})\n",
            said.lines().last().unwrap_or("").trim()
        ));
    }
    if present(s)? {
        let after = slots(s)?;
        out.push_str(&format!(
            "after: {}\n{}",
            said_slots(&after),
            changed(&before, &after)
        ));
    } else {
        out.push_str(
            "the slots' flags are not known yet: the phone is not in fastboot to say them \
             (lpoco-lab fastboot state once it is back)",
        );
    }
    Ok(out)
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

/// Makes SLOT the active one - the bootloader's metadata only, no partition written - where the manifest
/// says `slot_switch = true` (said only by a recorded decision), and says the flags before and after.
pub fn set_active(device: &Device, slot: &str) -> Result<String> {
    if !device.fastboot.slot_switch {
        bail!(
            "{}'s device.toml does not say [fastboot] slot_switch = true - the active slot is not changed",
            device.device.codename
        );
    }
    if !matches!(slot, "a" | "b") {
        bail!("{slot:?}: a slot is a or b");
    }
    let s = serial(device)?;
    let _held = crate::hold::hold(device)?;
    need(s)?;
    let before = slots(s)?;
    let (ok, said) = fastboot(s, &["set_active", slot])?;
    if !ok {
        bail!("fastboot set_active {slot} said: {}", said.trim());
    }
    let after = slots(s)?;
    Ok(format!(
        "before: {}\nafter: {}\n{}",
        said_slots(&before),
        said_slots(&after),
        changed(&before, &after)
    ))
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
        let said = "slot-unbootable:a: yes\n";
        assert_eq!(answer(said, "slot-unbootable:a").as_deref(), Some("yes"));
    }

    #[test]
    fn what_the_bootloader_changed_is_said() {
        // walleye, 2026-11-17, try b
        let before = vec![
            ("current-slot", "a".to_string()),
            ("slot-unbootable:a", "no".to_string()),
        ];
        let after = vec![
            ("current-slot", "b".to_string()),
            ("slot-unbootable:a", "yes".to_string()),
        ];
        assert_eq!(
            changed(&before, &after),
            "the slots' flags changed: current-slot a -> b, slot-unbootable:a no -> yes"
        );
        assert_eq!(changed(&before, &before), "the slots' flags did not change");
    }

    #[test]
    fn a_phone_is_found_by_its_serial_or_its_port_never_by_another_phones_gadget() {
        let u = |port: &str, id: &str, serial: &str| Usb {
            port: port.into(),
            id: id.into(),
            serial: serial.into(),
        };
        // 2026-11-17: walleye left USB; surya's postmarketOS gadget sat on another port
        let surya = u("3-2", "18d1:d001", "");
        assert_eq!(
            whereabouts(std::slice::from_ref(&surya), "FA81N1A00790", Some("1-4")),
            None,
            "another phone's gadget is not this phone"
        );
        assert_eq!(
            whereabouts(std::slice::from_ref(&surya), "FA81N1A00790", None),
            None
        );
        let adb = u("1-4", "18d1:4ee7", "FA81N1A00790");
        assert!(whereabouts(&[surya.clone(), adb], "FA81N1A00790", None)
            .unwrap()
            .starts_with("Android"));
        let linux = u("1-4", "18d1:d001", "");
        let w = whereabouts(&[surya, linux], "FA81N1A00790", Some("1-4")).unwrap();
        assert!(w.starts_with("Linux") && w.contains("1-4"), "{w}");
    }

    #[test]
    fn only_a_boot_image_is_booted() {
        assert!(is_boot_image(b"ANDROID!\x00\x00"));
        assert!(!is_boot_image(b"\x7fELF"));
        assert!(!is_boot_image(b"MZ"));
    }

    #[test]
    fn no_serial_no_fastboot_and_no_switch_unless_said() {
        let d = Device::parse(
            "[device]\ncodename = \"x\"\n[connection]\nuser = \"u\"\naddresses = [\"a\"]\n",
        )
        .unwrap();
        assert!(serial(&d).is_err());
        assert!(!d.fastboot.writes, "writing is never assumed");
        assert!(!d.fastboot.slot_switch, "switching is never assumed");
        assert!(set_active(&d, "a").is_err());
        let d = Device::parse(
            "[device]\ncodename = \"google-walleye\"\n[connection]\nuser = \"u\"\naddresses = [\"a\"]\n\
             [fastboot]\nserial = \"FA81N1A00790\"\n",
        )
        .unwrap();
        assert_eq!(serial(&d).unwrap(), "FA81N1A00790");
        let e = set_active(&d, "a").unwrap_err().to_string();
        assert!(e.contains("slot_switch = true"), "{e}");
        assert!(Device::parse(
            "[device]\ncodename = \"x\"\n[connection]\nuser = \"u\"\naddresses = [\"a\"]\n\
             [fastboot]\nserial = \"s\"\nslot = \"a\"\n"
        )
        .is_err());
    }
}
