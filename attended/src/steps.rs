// SPDX-License-Identifier: MIT OR Apache-2.0
//! The library's steps: small, device-agnostic functions that return a [`Step`]; a lane's procedure only chains
//! them with its own values. Here: ask, dmesg_clean, check_probe, wait_for_phone, read_dmesg, wait_for_usb, boot_once,
//! back_to_known_good. More come as a second device needs one, each keyed to its bring-up step id and taking its device values from
//! device.toml / [layer], never hard-coded.

use crate::bench::{self, Act, ActKind};
use crate::prompt::Prompt;
use crate::{Kind, Step};
use lpoco_manifest::is_kernel_name;

/// The author does something and looks at something ("what to do / what to check"); "ok" goes on, anything
/// else is what was seen and stops the run; nobody answering stops it too. The same as [`confirm`].
pub fn ask(what: &str) -> Step {
    confirm(what)
}

/// The author's own words, "what to do / what to check", done by the person at the bench (a bench may claim it).
pub fn confirm(what: &str) -> Step {
    act(Act::new(ActKind::Confirm(what.to_string()), ""))
}

/// Press the power button, then check `check`.
pub fn press_power(check: &str) -> Step {
    act(Act::new(ActKind::PressPower, check))
}

/// Hold a button (`"volume down"`) until the check holds.
pub fn hold_button(button: &str, check: &str) -> Step {
    act(Act::new(ActKind::HoldButton(button.to_string()), check))
}

/// Unplug the USB cable.
pub fn unplug_usb(check: &str) -> Step {
    act(Act::new(ActKind::UnplugUsb, check))
}

/// Plug the USB cable back in.
pub fn replug_usb(check: &str) -> Step {
    act(Act::new(ActKind::ReplugUsb, check))
}

/// Touch something on the screen (`"the Settings icon"`).
pub fn touch(target: &str, check: &str) -> Step {
    act(Act::new(ActKind::Touch(target.to_string()), check))
}

/// Swipe (`"up from the bottom edge"`).
pub fn swipe(way: &str, check: &str) -> Step {
    act(Act::new(ActKind::Swipe(way.to_string()), check))
}

/// Look at the phone for something (`"the boot logo"`).
pub fn look_for(what: &str) -> Step {
    act(Act::new(ActKind::LookFor(what.to_string()), ""))
}

/// Listen to the phone for something (`"the ringtone"`).
pub fn listen_for(what: &str) -> Step {
    act(Act::new(ActKind::ListenFor(what.to_string()), ""))
}

/// A physical act as a step: the bench that claims it does it, the person otherwise. The doer is taken now, so put
/// a [`bench`](crate::bench) on the bench before the steps it should do.
pub fn act(act: Act) -> Step {
    let name = act.name();
    match bench::doer(&act) {
        Some(b) => {
            let doer = b.name().to_string();
            Step::new(Kind::Ask, &name, move || b.do_act(&act)).by(&doer)
        }
        None => Step::new(Kind::Ask, &name, move || person(&act)).by(bench::PERSON),
    }
}

/// The person is told what to do and what to check, answers `ok` or says what was seen: on the terminal or in the
/// answer file, whichever comes first.
fn person(act: &Act) -> Result<(), String> {
    let (to_do, to_check) = (act.to_do(), act.to_check());
    println!("  ATTENDED - please {to_do}.");
    println!("  Check that {to_check}. Answer ok, or say what you saw instead:");
    let prompt = Prompt {
        step: &act.name(),
        to_do: &to_do,
        to_check: &to_check,
    };
    let answer = crate::ask_person(&prompt)?;
    crate::note(
        "asked",
        &act.name(),
        &format!(
            "check: {to_check}; answer: {}; by: {}",
            answer.text, answer.who
        ),
    );
    if answer.text.eq_ignore_ascii_case("ok") {
        Ok(())
    } else {
        Err(format!(
            "expected that {to_check}, the author saw: {}",
            answer.text
        ))
    }
}

/// The kernel messages that mean the kernel itself is unhappy: an oops, a BUG, a panic, a lockup, a stall.
const TROUBLE: [&str; 9] = [
    "Oops",
    "BUG:",
    "Kernel panic",
    "Call trace:",
    "Call Trace:",
    "Unable to handle kernel",
    "Internal error:",
    "lockup",
    "rcu: INFO",
];

/// The lines of a dmesg that show kernel trouble.
pub fn trouble_lines(dmesg: &str) -> Vec<&str> {
    dmesg
        .lines()
        .filter(|l| TROUBLE.iter().any(|t| l.contains(t)))
        .collect()
}

/// The kernel log (the ring buffer: an early oops that wrapped out is not seen) shows no oops, BUG, panic or
/// lockup (bring-up step dmesg-clean).
pub fn dmesg_clean() -> Step {
    Step::new(Kind::Check, "dmesg shows no oops, BUG or panic", || {
        let log = crate::phone().root("dmesg")?;
        let bad = trouble_lines(&log);
        if bad.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "{} line(s) of trouble, first: {}",
                bad.len(),
                bad[0].trim()
            ))
        }
    })
    .keyed("dmesg-clean")
}

/// A driver is bound: its folder is under /sys/bus/*/drivers. Positive evidence: a dmesg line that only names
/// the driver, or says "deferred probe pending", proves nothing. The name is letters, digits, `_` and `-`.
pub fn check_probe(driver: &str) -> Step {
    let driver = driver.to_string();
    Step::new(
        Kind::Check,
        &format!("the {driver} driver is bound"),
        move || {
            let word = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
            if driver.is_empty() || !driver.chars().all(word) {
                return Err(format!("not a driver name: {driver:?}"));
            }
            crate::phone().sh(&format!("ls -d /sys/bus/*/drivers/{driver} >/dev/null"))
        },
    )
}

/// The phone answers through devlab, tried `tries` times `every` seconds apart. After a reboot command, wait
/// for the phone to go down first: the old instance still answers.
pub fn wait_for_phone(tries: u32, every: u64) -> Step {
    Step::new(Kind::Check, "the phone answers", move || {
        crate::retry(tries, every, || crate::phone().sh("true"))
    })
}

/// The kernel log (as root) holds a line with this text; the failure names the text. A positive look at what the
/// kernel said, as `check_probe` is for a driver (bring-up steps read it for their own message).
pub fn read_dmesg(text: &str) -> Step {
    let text = text.to_string();
    Step::new(Kind::Check, &format!("dmesg says \"{text}\""), move || {
        if text.trim().is_empty() {
            return Err("not a dmesg text: it is empty".into());
        }
        let log = crate::phone().root("dmesg")?;
        if log.contains(&text) {
            Ok(())
        } else {
            Err(format!("no dmesg line holds \"{text}\""))
        }
    })
}

/// The phone is on this computer's USB bus as `vendor:product` (four hex digits each, e.g. `18d1:4ee0`), tried
/// `tries` times `every` seconds apart. The host's own view (sysfs), so it needs no phone answer: it tells a
/// phone that booted to a gadget from one that did not. `LPOCO_ATTENDED_SYSFS_USB` points at another bus folder
/// (the tests' fake). Keyed to bring-up step gadget-enumerates, but it proves only the vendor:product: the serial
/// half of that step is not checked, and the id is the lane's to give (our own kernel's gadget, not the stock one).
pub fn wait_for_usb(id: &str, tries: u32, every: u64) -> Step {
    let id = id.to_ascii_lowercase();
    Step::new(
        Kind::Check,
        &format!("USB device {id} is on the bus"),
        move || usb_on_bus(&id, tries, every),
    )
    .keyed("gadget-enumerates")
}

/// [`wait_for_usb`]'s work, shared with [`boot_once`]: the id (lowercase) is on the bus within the tries.
fn usb_on_bus(id: &str, tries: u32, every: u64) -> Result<(), String> {
    let ok = id.split_once(':').is_some_and(|(v, p)| {
        [v, p]
            .iter()
            .all(|h| h.len() == 4 && h.chars().all(|c| c.is_ascii_hexdigit()))
    });
    if !ok {
        return Err(format!("not a USB id (vendor:product): {id:?}"));
    }
    let root = crate::sysfs_usb();
    crate::retry(tries, every, || {
        usb_ids(root)?
            .contains(&id.to_string())
            .then_some(())
            .ok_or(format!("{id} is not on the USB bus"))
    })
}

/// [`wait_for_usb`] for the id the device declares: the first `[[host.gadget]] usb_id` of its device.toml. A device that
/// declares none ends the run (exit 2, dry run too) naming the key.
pub fn wait_for_gadget(tries: u32, every: u64) -> Step {
    let id = crate::declared("[[host.gadget]] usb_id", "wait_for_gadget", |m| {
        m.host.gadget.iter().find_map(|g| g.usb_id.clone())
    });
    wait_for_usb(&id, tries, every)
}

/// Boot the named test kernel once. A device with `[kernels] boot = "fastboot"` boots its `[images.NAME]` from memory
/// (`devlab fastboot boot now`, nothing written; the phone must be in fastboot) and waits for its gadget. Any other goes
/// through devlab's `kernel try` (never around it): devlab refuses an entry with
/// no try left, reboots, waits for the phone and measures the boot it came to; its output is the step's.
/// Any later boot is the default again. The name is the kernel's own (lowercase letters, digits, `-`: devlab's
/// rule, so a `+`, which is systemd-boot's try counter, is refused here before the phone is reached) and one the
/// device lists under `[kernels] test`: a name it does not list ends the run (exit 2, dry run too) naming the key.
pub fn boot_once(name: &str) -> Step {
    let name = name.to_string();
    declared_test_kernel(&name);
    let memory = memory_boot_of(&name);
    Step::new(Kind::Do, &format!("boot {name} once"), move || {
        if !is_kernel_name(&name) {
            return Err(format!(
                "not a test kernel name: {name:?} (lowercase letters, digits and -)"
            ));
        }
        if phone_runs(&name) {
            return already_running(&format!("boot {name} once"));
        }
        let out = match &memory {
            None => crate::devlab(&["kernel", "try", &name])?,
            Some(m) => boot_from_memory(m)?,
        };
        print!("{out}");
        Ok::<(), String>(())
    })
}

/// What a boot from memory reads, taken from the device at the step's making: the image, the gadget, the wait.
struct FromMemory {
    image: String,
    usb_id: String,
    tries: u32,
}

/// The fastboot boot of a test kernel, when the device declares `[kernels] boot = "fastboot"`: its `[images.NAME]`
/// under `[images] dir`, in the device's folder, and the gadget it then shows. The manifest has judged every piece present.
fn memory_boot_of(name: &str) -> Option<FromMemory> {
    let c = crate::config();
    let m = &c.manifest;
    if m.kernels.as_ref()?.boot != lpoco_manifest::KernelBoot::Fastboot || !is_kernel_name(name) {
        return None;
    }
    // the manifest judged these present; were one missing, the run ends (exit 2) rather than boot by another way
    let image = crate::declared(&format!("[images.{name}]"), "boot_once", |m| {
        m.images.as_ref()?.named.get(name).map(|i| i.path.clone())
    });
    let dir = m
        .images
        .as_ref()
        .and_then(|i| i.dir.clone())
        .unwrap_or_default();
    let usb_id = crate::declared("[[host.gadget]] usb_id", "boot_once", |m| {
        m.host.gadget.iter().find_map(|g| g.usb_id.clone())
    });
    let folder = c.device.parent().unwrap_or(std::path::Path::new("."));
    let wait = m.boot.as_ref().and_then(|b| b.device_wait).unwrap_or(60);
    Some(FromMemory {
        image: folder.join(dir).join(image).display().to_string(),
        usb_id,
        tries: wait / 2 + 1,
    })
}

/// Boot the image from the phone's memory (devlab fastboot boot now: nothing is written), then wait for its gadget.
fn boot_from_memory(m: &FromMemory) -> Result<String, String> {
    let out = crate::devlab(&["fastboot", "boot", "now", &m.image])?;
    usb_on_bus(&m.usb_id, m.tries, 2).map_err(|e| format!("{out}{e}"))?;
    Ok(out)
}

/// Whether the phone's running kernel (`uname -r`, one line) is the one of this name: the release is a version
/// (`6.1.0`), a `-`, then the kernel's own name (`6.1.0-pmicwdt`), and the name must be all of what follows. A
/// miss costs one reboot; a wrong hit would leave the test kernel unbooted, so anything less exact is a miss.
pub fn runs_kernel(release: &str, name: &str) -> bool {
    let line = release.lines().next().unwrap_or("").trim();
    let version = line
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(line.len());
    line[version..].strip_prefix('-') == Some(name) && version > 0
}

/// Say a step was skipped because the phone is already in the state it would bring about: said, logged, passed.
fn already_running(name: &str) -> Result<(), String> {
    println!("  already running: {name} - nothing to do");
    crate::note("skip", name, "already running");
    Ok(())
}

/// Whether the phone already runs the kernel `name`: false when it cannot be asked (the step then does its work).
fn phone_runs(name: &str) -> bool {
    crate::phone()
        .sh("uname -r 2>/dev/null")
        .is_ok_and(|release| runs_kernel(&release, name))
}

/// A valid test kernel name must be listed under `[kernels] test` of the device: the run ends (exit 2) naming the key
/// and the file when the table is missing, or the name when it is not listed. A name devlab would refuse is left to
/// the step to refuse.
fn declared_test_kernel(name: &str) {
    if !is_kernel_name(name) {
        return;
    }
    let listed = crate::declared("[kernels] test", "boot_once", |m| {
        m.kernels.as_ref().map(|k| k.test.clone())
    });
    if !listed.iter().any(|k| k == name) {
        let c = crate::config();
        eprintln!(
            "{}: {} does not list {name:?} under [kernels] test, which boot_once reads (it lists: {})",
            c.name,
            c.device.display(),
            if listed.is_empty() { "no kernel".to_string() } else { listed.join(", ") }
        );
        crate::leave_with(2)
    }
}

/// Go back to the phone's known good: tell the phone, as root, to reboot, so the default entry (the stock or the
/// kept kernel; a try is used up) boots. It does not wait: follow it with [`wait_for_phone`], after the phone
/// has gone down, as that step says. The device declares a `[kernels]` table; its `kept` kernel is named in the
/// step, and without one the step says stock. The name is what the device declares, not what the phone is checked to boot.
pub fn back_to_known_good() -> Step {
    // a kernel booted from memory is gone at the next reboot, so only an entry kept on the phone can already be the default
    let (kept, from_memory) = crate::declared("[kernels]", "back_to_known_good", |m| {
        m.kernels.as_ref().map(|k| {
            (
                k.kept.clone(),
                k.boot == lpoco_manifest::KernelBoot::Fastboot,
            )
        })
    });
    let default = kept
        .clone()
        .map_or("stock".to_string(), |k| format!("kept: {k}"));
    let name = format!("reboot into the default kernel ({default})");
    Step::new(Kind::Do, &name.clone(), move || {
        // the kept kernel already runs: a reboot would only be a risk (a stock default cannot be told by name)
        if !from_memory && kept.as_deref().is_some_and(phone_runs) {
            return already_running(&name);
        }
        crate::phone()
            .root("set -e\nsync\nsystemd-run --on-active=2 systemctl reboot >/dev/null\necho rebooting\n")
            .map(|_| ())
    })
}

/// The phone is up on the named test kernel and answers: boot it once ([`boot_once`], so the name is one the device lists
/// under `[kernels] test`), wait for the phone, and see the user's shell answer. Its way back is [`back_to_known_good`]:
/// a step that fails after the boot leaves the phone on the default kernel. A device that declares no `[kernels]` ends the
/// run (exit 2, dry run too). The device's own gate (power, cable, battery) is its procedure's step before this group.
pub fn phone_ready(kernel: &str) -> crate::Group {
    crate::group(
        &format!("the phone is ready on {kernel}"),
        [
            boot_once(kernel),
            wait_for_phone(60, 5),
            Step::new(Kind::Check, "the shell answers", || {
                crate::phone().sh("id -u")
            }),
        ],
    )
    .way_back(back_to_known_good())
}

/// Leave the phone on its known good ([`back_to_known_good`]): reboot into the default kernel, see the phone go down
/// (the old instance still answers for a moment), then see it answer again. The group is settling: once it has passed,
/// the way backs of the groups before it (`phone_ready`) are settled and a later failure does not reboot again.
pub fn leave_known_good() -> crate::Group {
    crate::group(
        "leave the phone on its known good",
        [
            back_to_known_good(),
            Step::new(Kind::Check, "the phone goes down", || {
                crate::retry(30, 1, || {
                    if crate::phone().sh("true").is_err() {
                        Ok(())
                    } else {
                        Err("the phone still answers")
                    }
                })
            }),
            wait_for_phone(60, 5),
        ],
    )
    .settling()
}

/// The `vendor:product` of every device under a sysfs USB devices folder.
fn usb_ids(root: &str) -> Result<Vec<String>, String> {
    let read = |d: &std::path::Path, f: &str| {
        std::fs::read_to_string(d.join(f))
            .ok()
            .map(|s| s.trim().to_ascii_lowercase())
    };
    let dir = std::fs::read_dir(root).map_err(|e| format!("cannot read {root}: {e}"))?;
    Ok(dir
        .flatten()
        .filter_map(|e| {
            Some(format!(
                "{}:{}",
                read(&e.path(), "idVendor")?,
                read(&e.path(), "idProduct")?
            ))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trouble_is_found_in_a_dmesg_and_a_quiet_one_has_none() {
        let log = "[1.0] boot ok\n[2.0] Unable to handle kernel NULL pointer\n[2.1] Call trace:\n[2.2] Oops: 0000\n[2.3] BUG: scheduling\n[2.4] Kernel panic - not syncing\n[2.5] watchdog: BUG: soft lockup\n[2.6] Call Trace:\n[2.7] Internal error: Oops\n[2.8] rcu: INFO: stall\n";
        assert_eq!(trouble_lines(log).len(), 9);
        assert!(trouble_lines("[1.0] all quiet\n[1.1] deferred probe pending\n").is_empty());
    }

    #[test]
    fn a_kernel_runs_only_when_its_name_is_all_that_follows_the_version() {
        assert!(runs_kernel("6.1.0-pmicwdt\n", "pmicwdt"));
        assert!(runs_kernel("6.12.0-mainline-stable", "mainline-stable"));
        for (release, name) in [
            ("6.1.0-pmicwdt", "pmic"),
            ("6.1.0-pmicwdt-2", "pmicwdt"),
            ("6.1.0-xpmicwdt", "pmicwdt"),
            ("6.1.0-pmic-wdt", "wdt"),
            ("6.12.0-mainline-stable", "stable"),
            ("6.1.0-pmicwdt", "1"),
            ("6.1.0-rc1-foo", "rc1"),
            ("6.1.0", "pmicwdt"),
            ("pmicwdt", "pmicwdt"),
        ] {
            assert!(!runs_kernel(release, name), "{release} is not {name}");
        }
    }
}
