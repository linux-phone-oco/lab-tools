// SPDX-License-Identifier: MIT OR Apache-2.0
//! Watching the phone from the lab machine. After every new boot the phone measures its own health
//! (lpoco-health.timer); the watch waits for that report and says the phone's own verdict in its journal -
//! silent while all is well, one line naming what is unhealthy when not, one when well again - and
//! gathers, once the phone has gone QUIET without a new boot: the reports brought to the lab and filed
//! into the picture's database through tad, both
//! landed, so the face is drawn again. Nobody asks for any of it.

use crate::device::Device;
use crate::phone::Phone;
use crate::{gather, suite};
use anyhow::Result;
use std::path::Path;
use std::time::{Duration, Instant};

/// What the journal says about a boot: the phone's verdict when it is unhealthy, or nothing when all is well.
pub fn alert_text(codename: &str, well: bool, verdict: &str) -> Option<String> {
    (!well).then(|| format!("{codename}: {verdict}"))
}

pub fn watch(device: &Device, manifest_dir: &Path) -> Result<()> {
    let codename = &device.device.codename;
    let mut last_boot: Option<String> = None;
    let mut alerting: Option<String> = None;
    let mut due: Option<Instant> = None;
    println!(
        "lpoco-lab watch: {codename} - after every boot, its own health report, gathered and landed"
    );
    loop {
        if let Ok(phone) = Phone::find(device) {
            let boot = phone.boot_id();
            if boot.is_some() && boot != last_boot {
                match suite::of_this_boot(&phone) {
                    Ok(Some(ran)) => {
                        let text = alert_text(codename, ran.well, &ran.verdict);
                        match (&text, &alerting) {
                            (Some(t), a) if a.as_ref() != Some(t) => {
                                println!("lpoco-lab watch: {t}");
                            }
                            (None, Some(_)) => {
                                println!("lpoco-lab watch: all is well again");
                            }
                            _ => {}
                        }
                        alerting = text;
                        // gathered once the phone is quiet: a burst of boots is one landing
                        due = Some(Instant::now() + QUIET);
                        last_boot = boot;
                    }
                    Ok(None) => {
                        println!(
                            "lpoco-lab watch: the suite is not on the phone - lpoco-lab suite put now"
                        );
                        last_boot = boot;
                    }
                    // no report yet: asked again at the next turn
                    Err(e) => eprintln!("lpoco-lab watch: {e:#}"),
                }
            }
        }
        if due.is_some_and(|d| Instant::now() >= d) {
            due = None;
            gather_landed(device, manifest_dir);
        }
        std::thread::sleep(Duration::from_secs(60));
    }
}

/// How long the phone must go without a new boot before its reports are gathered and landed: a run of
/// kernel tries is many boots, and each landing is a pull request, a CI round and a drawing of the face.
pub const QUIET: Duration = Duration::from_secs(15 * 60);
/// A landing can find main moved under it (another session landed meanwhile): gathering is done again
/// from main as it then is, this many times in all.
pub const TRIES: u32 = 3;

fn gather_landed(device: &Device, manifest_dir: &Path) {
    for attempt in 1..=TRIES {
        match gather::gather(device, manifest_dir, None, true) {
            Ok(said) => {
                println!("lpoco-lab watch: {said}");
                return;
            }
            Err(e) => eprintln!(
                "lpoco-lab watch: gathering did not go through (try {attempt} of {TRIES}): {e:#}"
            ),
        }
    }
    eprintln!("lpoco-lab watch: left for the next quiet period - what is unfiled is filed then");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_when_well_and_the_phones_own_words_when_not() {
        assert_eq!(
            alert_text("xiaomi-surya", true, "r: healthy: 29 pass"),
            None
        );
        assert_eq!(
            alert_text(
                "xiaomi-surya",
                false,
                "r: UNHEALTHY: 1 failing of 34 - wifi/x (d)"
            )
            .as_deref(),
            Some("xiaomi-surya: r: UNHEALTHY: 1 failing of 34 - wifi/x (d)")
        );
    }
}
