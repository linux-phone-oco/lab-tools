//! What a test kernel's try came to, said by the phone itself. The try is started, the phone's boot id
//! is watched until a new boot answers, and then the phone is asked which kernel runs, what the test
//! entry's counter shows, and whether the boot timeout left a log - so nobody waits for a phone to go
//! down and come up by hand, and nobody guesses why stock came back (2026-11-11: three wrong theories
//! cost three reboots before the entry's counter was read).
use crate::device::Device;
use crate::kernel;
use crate::phone::{rebooted, Phone};
use anyhow::{bail, Result};
use std::time::{Duration, Instant};

/// How a try came out.
#[derive(Debug, PartialEq)]
pub enum Came {
    /// The test kernel runs.
    Booted { release: String },
    /// The bootloader did not choose the entry: stock runs, the entry still has its try.
    NotChosen { release: String },
    /// Chosen, and stopped before it could log anything: stock came back, the try is spent.
    StoppedEarly { release: String },
    /// It ran for its five minutes without reaching a system; its log is on the boot partition.
    TimedOut { release: String },
    /// Stock runs and the entry is gone or unreadable.
    Unclear { release: String },
}

/// Judges a try from what the phone says after the new boot: the running release, the boot entries
/// listing, and whether the timeout hook's log is there.
pub fn judge(release: &str, name: &str, entries: &str, timed_out: bool) -> Came {
    let release = release.trim().to_string();
    if release.ends_with(&format!("-{name}")) {
        return Came::Booted { release };
    }
    let mine: Vec<&str> = entries
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with(&format!("pmos-{name}+")))
        .collect();
    if timed_out {
        Came::TimedOut { release }
    } else if mine
        .iter()
        .any(|l| l.starts_with(&format!("pmos-{name}+0-")))
    {
        Came::StoppedEarly { release }
    } else if mine
        .iter()
        .any(|l| l.starts_with(&format!("pmos-{name}+1-0")))
    {
        Came::NotChosen { release }
    } else {
        Came::Unclear { release }
    }
}

impl Came {
    /// In plain words, with what to do next.
    pub fn say(&self, name: &str) -> String {
        match self {
            Came::Booted { release } => format!("booted the test kernel {release} - reboot to return to stock"),
            Came::NotChosen { release } => format!("the bootloader did not choose pmos-{name}: stock {release} runs, and the entry still has its try"),
            Came::StoppedEarly { release } => format!("pmos-{name} was chosen and stopped before it could log anything - is it an EFI image? stock {release} came back"),
            Came::TimedOut { release } => format!("pmos-{name} ran for five minutes without reaching a system; its log is saved (devlab kernel log); stock {release} came back"),
            Came::Unclear { release } => format!("stock {release} runs and pmos-{name} is gone from the entries - nothing can be read about the try"),
        }
    }
}

/// Starts the try and reports what it came to, once a new boot answers (ten minutes at most: a try
/// that reaches no system falls back to stock after five).
pub fn try_and_report(device: &Device, phone: &Phone, name: &str) -> Result<String> {
    let before = phone.boot_id();
    kernel::try_once(phone, name)?;
    let start = Instant::now();
    let p = loop {
        std::thread::sleep(Duration::from_secs(10));
        if start.elapsed() > Duration::from_secs(600) {
            bail!("no new boot answered within ten minutes - the phone needs looking at");
        }
        if let Ok(p) = Phone::find(device) {
            if rebooted(before.as_deref(), p.boot_id().as_deref()) {
                break p;
            }
        }
    };
    let (_, release) = p.sh("uname -r\n")?;
    let (_, entries) = p.root(
        "ls /boot/loader/entries/; [ -f /boot/test-boot.log ] && echo TEST-BOOT-LOG; true\n",
    )?;
    let came = judge(&release, name, &entries, entries.contains("TEST-BOOT-LOG"));
    Ok(format!(
        "after {} s: {}",
        start.elapsed().as_secs(),
        came.say(name)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_try_is_judged_by_what_the_phone_says() {
        let r = "7.1.0-rc3-sm7150";
        assert_eq!(
            judge(
                "7.1.0-rc3-sm7150-wcd\n",
                "wcd",
                "pmos-wcd+0-1.conf\npmos.conf",
                false
            ),
            Came::Booted {
                release: "7.1.0-rc3-sm7150-wcd".into()
            }
        );
        assert_eq!(
            judge(r, "wcd", "pmos-wcd+1-0.conf\npmos.conf", false),
            Came::NotChosen { release: r.into() }
        );
        assert_eq!(
            judge(r, "wcd", "pmos-wcd+0-1.conf\npmos.conf", false),
            Came::StoppedEarly { release: r.into() }
        );
        assert_eq!(
            judge(
                r,
                "wcd",
                "pmos-wcd+0-1.conf\npmos.conf\nTEST-BOOT-LOG",
                true
            ),
            Came::TimedOut { release: r.into() }
        );
        assert_eq!(
            judge(r, "wcd", "pmos.conf", false),
            Came::Unclear { release: r.into() }
        );
        assert!(
            judge(r, "wcd", "pmos-wcdx+0-1.conf", false) == Came::Unclear { release: r.into() },
            "another entry's name is not this one's"
        );
    }

    #[test]
    fn each_outcome_says_what_to_do() {
        assert!(Came::StoppedEarly {
            release: "s".into()
        }
        .say("wcd")
        .contains("is it an EFI image?"));
        assert!(Came::TimedOut {
            release: "s".into()
        }
        .say("wcd")
        .contains("devlab kernel log"));
    }
}
