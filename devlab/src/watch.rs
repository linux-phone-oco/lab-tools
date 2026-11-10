//! Watching the phone from the lab machine: after every new boot (and every 6 hours) a health
//! check; silent while all is well, one board task naming what regressed when not, closed again
//! once all is well. The board is lab-agent's, reached through its command line.

use crate::device::Device;
use crate::health;
use crate::phone::Phone;
use anyhow::Result;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

/// What the board task says: the regressions, in order, or nothing when all is well.
pub fn alert_text(codename: &str, regressions: &[String]) -> Option<String> {
    (!regressions.is_empty()).then(|| format!("{codename} regressed: {}", regressions.join("; ")))
}

fn board(args: &[&str]) {
    let lab = std::env::var("LAB_AGENT").unwrap_or_else(|_| "lab-agent".into());
    let _ = Command::new(lab).arg("board").args(args).status();
}

pub fn watch(device: &Device, manifest_dir: &Path) -> Result<()> {
    let task = format!("health-{}", device.device.codename);
    let mut last_boot: Option<String> = None;
    let mut last_check = Instant::now() - Duration::from_secs(24 * 3600);
    let mut alerting: Option<String> = None;
    println!(
        "devlab watch: {} - a health check after every boot and every 6 h",
        device.device.codename
    );
    loop {
        if let Ok(phone) = Phone::find(device) {
            let boot = phone.boot_id();
            let new_boot = boot.is_some() && boot != last_boot;
            if new_boot || last_check.elapsed() >= Duration::from_secs(6 * 3600) {
                if new_boot && phone.uptime().unwrap_or(0) < 90 {
                    std::thread::sleep(Duration::from_secs(90)); // let the boot settle
                }
                match health::health(device, manifest_dir, &phone, false) {
                    Ok(r) => {
                        let text = alert_text(&device.device.codename, &r);
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
                    }
                    Err(e) => eprintln!("devlab watch: the health check could not run: {e:#}"),
                }
                last_boot = boot;
                last_check = Instant::now();
            }
        }
        std::thread::sleep(Duration::from_secs(60));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_when_all_is_well_and_a_named_alert_when_not() {
        assert_eq!(alert_text("xiaomi-surya", &[]), None);
        assert_eq!(
            alert_text("xiaomi-surya", &["a/x (fail: d)".into()]).as_deref(),
            Some("xiaomi-surya regressed: a/x (fail: d)")
        );
    }
}
