//! Watching the phone from the lab machine. After every new boot the phone measures its own health
//! (devlab-health.timer); the watch waits for that report, says the phone's own verdict on the board -
//! silent while all is well, one task naming what is unhealthy when not, closed again once well - and
//! gathers: the report brought to the lab and filed into the picture's database through tad, both
//! landed, so the face is drawn again. Nobody asks for any of it.

use crate::device::Device;
use crate::phone::Phone;
use crate::{gather, suite};
use anyhow::Result;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

/// What the board task says: the phone's verdict when it is unhealthy, or nothing when all is well.
pub fn alert_text(codename: &str, well: bool, verdict: &str) -> Option<String> {
    (!well).then(|| format!("{codename}: {verdict}"))
}

fn board(args: &[&str]) {
    let lab = std::env::var("LAB_AGENT").unwrap_or_else(|_| "lab-agent".into());
    let _ = Command::new(lab).arg("board").args(args).status();
}

pub fn watch(device: &Device, manifest_dir: &Path) -> Result<()> {
    let codename = &device.device.codename;
    let task = format!("health-{codename}");
    let mut last_boot: Option<String> = None;
    let mut alerting: Option<String> = None;
    println!(
        "devlab watch: {codename} - after every boot, its own health report, gathered and landed"
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
                                println!("devlab watch: {t}");
                                board(&["set", &task, t, "--project", "linux-phone-oco"]);
                            }
                            (None, Some(_)) => {
                                println!("devlab watch: all is well again");
                                board(&["done", &task]);
                            }
                            _ => {}
                        }
                        alerting = text;
                        match gather::gather(device, manifest_dir, None, true) {
                            Ok(said) => println!("devlab watch: {said}"),
                            Err(e) => {
                                eprintln!("devlab watch: gathering did not go through: {e:#}")
                            }
                        }
                        last_boot = boot;
                    }
                    Ok(None) => {
                        println!(
                            "devlab watch: the suite is not on the phone - devlab suite put now"
                        );
                        last_boot = boot;
                    }
                    // no report yet: asked again at the next turn
                    Err(e) => eprintln!("devlab watch: {e:#}"),
                }
            }
        }
        std::thread::sleep(Duration::from_secs(60));
    }
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
