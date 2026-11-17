//! The phone's own health suite (lab-tools/health): put onto the phone from the device's manifest, run
//! there, and its reports read back. The generic checks are built into devlab; a phone's own checks
//! come from its lab repository ([health] checks); what the phone is expected to have is [health].
//! Until the package conveyor publishes it as an Alpine package, this is how it reaches a phone.

use crate::device::Device;
use crate::phone::Phone;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The suite's own files, built in: (path under /, contents, mode).
const FILES: &[(&str, &str, &str)] = &[
    (
        "usr/lib/devlab/health/lib.sh",
        include_str!("../../health/suite/lib.sh"),
        "644",
    ),
    (
        "usr/lib/devlab/health/checks/base/10-storage.sh",
        include_str!("../../health/suite/checks/base/10-storage.sh"),
        "644",
    ),
    (
        "usr/lib/devlab/health/checks/base/20-kernel.sh",
        include_str!("../../health/suite/checks/base/20-kernel.sh"),
        "644",
    ),
    (
        "usr/lib/devlab/health/checks/base/30-processors.sh",
        include_str!("../../health/suite/checks/base/30-processors.sh"),
        "644",
    ),
    (
        "usr/lib/devlab/health/checks/base/40-sound.sh",
        include_str!("../../health/suite/checks/base/40-sound.sh"),
        "644",
    ),
    (
        "usr/lib/devlab/health/checks/base/50-wifi.sh",
        include_str!("../../health/suite/checks/base/50-wifi.sh"),
        "644",
    ),
    (
        "usr/lib/devlab/health/checks/base/60-modem.sh",
        include_str!("../../health/suite/checks/base/60-modem.sh"),
        "644",
    ),
    (
        "usr/lib/devlab/health/checks/base/70-bluetooth.sh",
        include_str!("../../health/suite/checks/base/70-bluetooth.sh"),
        "644",
    ),
    (
        "usr/lib/devlab/health/checks/base/80-display-input.sh",
        include_str!("../../health/suite/checks/base/80-display-input.sh"),
        "644",
    ),
    (
        "usr/lib/devlab/health/checks/base/85-parts.sh",
        include_str!("../../health/suite/checks/base/85-parts.sh"),
        "644",
    ),
    (
        "usr/lib/devlab/health/checks/base/87-sensors.sh",
        include_str!("../../health/suite/checks/base/87-sensors.sh"),
        "644",
    ),
    (
        "usr/lib/devlab/health/checks/base/90-sleep.sh",
        include_str!("../../health/suite/checks/base/90-sleep.sh"),
        "644",
    ),
    (
        "usr/lib/devlab/health/checks/shell/10-sound-output.sh",
        include_str!("../../health/suite/checks/shell/10-sound-output.sh"),
        "644",
    ),
    (
        "usr/lib/devlab/health/checks/telephony/10-mobile-data.sh",
        include_str!("../../health/suite/checks/telephony/10-mobile-data.sh"),
        "644",
    ),
    (
        "etc/systemd/system/devlab-health.service",
        include_str!("../../health/suite/devlab-health.service"),
        "644",
    ),
    (
        "etc/systemd/system/devlab-health.timer",
        include_str!("../../health/suite/devlab-health.timer"),
        "644",
    ),
];

/// A finding of the suite, as its report keeps it.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub layer: String,
    pub component: String,
    pub test: String,
    pub outcome: String,
    pub detail: String,
}

/// A report's lines (JSON), each a row; other lines are ignored.
pub fn rows(report: &str) -> Vec<Row> {
    report
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| {
            let s = |k: &str| v[k].as_str().map(str::to_string);
            Some(Row {
                layer: s("layer").unwrap_or_default(),
                component: s("component")?,
                test: s("test")?,
                outcome: s("outcome")?,
                detail: s("detail").unwrap_or_default(),
            })
        })
        .collect()
}

/// The phone's own checks in its lab repository: (path under /, local file).
fn device_checks(device: &Device, manifest_dir: &Path) -> Result<Vec<(String, PathBuf)>> {
    let Some(dir) = &device.health.checks else {
        return Ok(Vec::new());
    };
    let mut v = Vec::new();
    for layer in ["base", "shell", "telephony"] {
        let d = manifest_dir.join(dir).join(layer);
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "sh") {
                let name = p
                    .file_name()
                    .context("a check's name")?
                    .to_string_lossy()
                    .into_owned();
                v.push((format!("usr/lib/devlab/health/device/{layer}/{name}"), p));
            }
        }
    }
    v.sort();
    Ok(v)
}

/// Where the suite is published as an Alpine package (the package conveyor, from packaging/), for the
/// Alpine the phones run, and the key it is signed with.
pub const REPOSITORY: &str = "https://linux-phone-oco.github.io/lab-tools/alpine/v3.24/lab-tools";
pub const KEY_NAME: &str = "linux-phone-oco-lab-tools-20261117.rsa.pub";
const KEY: &str = include_str!("../../packaging/alpine/linux-phone-oco-lab-tools-20261117.rsa.pub");

/// What a root shell on the phone does to take the suite from its repository: the key and the
/// repository added once, the package installed or brought up to date - and then what devlab put by
/// hand before the package is taken away, so that the package's files are the only ones (a unit in
/// /etc/systemd/system would hide the package's own).
pub fn from_the_package() -> String {
    format!(
        "[ -f /etc/apk/keys/{KEY_NAME} ] || cat > /etc/apk/keys/{KEY_NAME} <<'KEY'\n{}KEY\n\
         grep -qxF '{REPOSITORY}' /etc/apk/repositories || echo '{REPOSITORY}' >> /etc/apk/repositories\n\
         apk add -q --update-cache --upgrade devlab-health\n\
         rm -f /etc/systemd/system/devlab-health.service /etc/systemd/system/devlab-health.timer \
         /etc/systemd/system/timers.target.wants/devlab-health.timer\n\
         echo put: $(apk list --installed devlab-health | cut -d' ' -f1) from {REPOSITORY}\n",
        KEY.trim_end().to_string() + "\n"
    )
}

/// Puts the suite on the phone, so the phone checks itself after every boot from now on: the runner and
/// the generic checks from their Alpine package - or, given a runner, as built here and copied, to try
/// what is not released yet - then the phone's own checks and its manifest's expectations, and the timer
/// enabled.
pub fn put(
    device: &Device,
    manifest_dir: &Path,
    phone: &Phone,
    runner: Option<&Path>,
) -> Result<()> {
    let head = match runner {
        Some(runner) => {
            let head =
                std::fs::read(runner).with_context(|| format!("reading {}", runner.display()))?;
            if head.get(..4) != Some(b"\x7fELF") || head.get(18) != Some(&0xb7) {
                bail!("{} is not an aarch64 program (build it: cargo build -p devlab-health --release --target aarch64-unknown-linux-musl)", runner.display());
            }
            Some(head)
        }
        None => None,
    };
    let stage = tempdir()?;
    let write = |rel: &str, bytes: &[u8], mode: &str| -> Result<()> {
        let p = stage.join(rel);
        std::fs::create_dir_all(p.parent().context("a parent")?)?;
        std::fs::write(&p, bytes)?;
        let m = u32::from_str_radix(mode, 8)?;
        std::fs::set_permissions(&p, std::os::unix::fs::PermissionsExt::from_mode(m))?;
        Ok(())
    };
    if let Some(head) = &head {
        write("usr/bin/devlab-health", head, "755")?;
        for (rel, text, mode) in FILES {
            write(rel, text.as_bytes(), mode)?;
        }
    }
    for (rel, local) in device_checks(device, manifest_dir)? {
        write(&rel, &std::fs::read(&local)?, "644")?;
    }
    write(
        "etc/devlab/health.env",
        format!(
            "{}{}",
            device.health.env(&device.device.codename),
            device.hear.env()
        )
        .as_bytes(),
        "644",
    )?;
    let tarball = stage.with_extension("tar");
    let ok = Command::new("tar")
        .args(["--owner=0", "--group=0", "-cf"])
        .arg(&tarball)
        .arg("-C")
        .arg(&stage)
        .args(["usr", "etc"])
        .status()?
        .success();
    if !ok {
        bail!("packing the suite failed");
    }
    phone.put(&tarball.to_string_lossy(), "/tmp/devlab-health.tar")?;
    let _ = std::fs::remove_dir_all(&stage);
    let _ = std::fs::remove_file(&tarball);
    // the phone's own checks are replaced whole: one taken out of the repository leaves the phone too
    let take = if head.is_some() {
        String::new()
    } else {
        from_the_package()
    };
    let (ok, out) = phone.root(&format!(
        "set -e\n{take}\
         rm -rf /usr/lib/devlab/health/device\n\
         tar -xf /tmp/devlab-health.tar -C / --no-same-owner\n\
         rm -f /tmp/devlab-health.tar\n\
         systemctl daemon-reload\n\
         systemctl enable devlab-health.timer\n\
         echo put: $(ls /usr/lib/devlab/health/checks/*/*.sh /usr/lib/devlab/health/device/*/*.sh 2>/dev/null | wc -l) checks\n",
    ))?;
    if !ok {
        bail!("installing the suite on the phone failed: {out}");
    }
    eprint!("{out}");
    Ok(())
}

/// What one run on the phone came to: its report, and the phone's own verdict on it.
pub struct Ran {
    pub report: String,
    pub verdict: String,
    pub well: bool,
}

/// Runs the suite on the phone now. The verdict is the phone's own - it knows its gaps - and is
/// passed on as said, never judged again here.
pub fn run(phone: &Phone) -> Result<Ran> {
    let (well, out) = phone.root(
        "devlab-health 2>/run/devlab-health.said >/dev/null; rc=$?\n\
         devlab-health latest; echo \"# $(tail -n1 /run/devlab-health.said)\"; exit $rc\n",
    )?;
    let verdict = out
        .lines()
        .rev()
        .find_map(|l| l.strip_prefix("# "))
        .unwrap_or_default()
        .to_string();
    if rows(&out).is_empty() {
        bail!("the phone gave no report - is the suite on it? (devlab suite put now)");
    }
    Ok(Ran {
        report: out,
        verdict,
        well,
    })
}

/// Whether a report was written in the boot `boot` (each line carries the boot it was made in).
pub fn of_boot(report: &str, boot: &str) -> bool {
    report
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .any(|v| v["boot"] == boot)
}

/// The report of the boot the phone is in: the one its timer writes after every boot. Waits for it
/// (the timer runs 120 s after boot; six minutes at most); None where the suite is not on the phone.
pub fn of_this_boot(phone: &Phone) -> Result<Option<Ran>> {
    let (has, _) = phone.sh("command -v devlab-health >/dev/null\n")?;
    if !has {
        return Ok(None);
    }
    let boot = phone.boot_id().context("reading the boot id")?;
    let start = std::time::Instant::now();
    loop {
        let (_, report) = phone.root(
            "devlab-health latest; echo \"# $(journalctl -b 0 -t devlab-health -o cat --no-pager | tail -n1)\"\n",
        )?;
        if of_boot(&report, &boot) {
            let verdict = report
                .lines()
                .rev()
                .find_map(|l| l.strip_prefix("# "))
                .unwrap_or_default()
                .to_string();
            let well = verdict.contains(": healthy");
            return Ok(Some(Ran {
                report,
                verdict,
                well,
            }));
        }
        if start.elapsed() > std::time::Duration::from_secs(360) {
            bail!("this boot wrote no health report within six minutes - is devlab-health.timer enabled?");
        }
        std::thread::sleep(std::time::Duration::from_secs(15));
    }
}

/// What every change to the phone ends with: its health, said, and the report brought to the lab.
/// `rebooted`: the change booted the phone anew, so the report is the one that boot writes by itself;
/// otherwise the suite runs now. Returns whether the phone is well (true where it has no suite yet,
/// said as such), and what to say of it.
pub fn after_change(phone: &Phone, rebooted: bool) -> Result<(bool, String)> {
    let ran = if rebooted {
        of_this_boot(phone)?
    } else {
        let (has, _) = phone.sh("command -v devlab-health >/dev/null\n")?;
        if has {
            Some(run(phone)?)
        } else {
            None
        }
    };
    let Some(ran) = ran else {
        return Ok((
            true,
            "health: the suite is not on the phone - devlab suite put now".into(),
        ));
    };
    // the report stays on the phone until the watch gathers it into devlab's own clone and lands it:
    // brought into the clone a command runs from, it rode along in that clone's next landing (2026-11-17)
    let said = format!(
        "health: {} (filed by the watch once the phone is quiet)",
        ran.verdict
    );
    Ok((ran.well, said))
}

/// The latest report the phone keeps (after its last boot, or the last run asked for).
pub fn latest(phone: &Phone) -> Result<String> {
    let (_, out) = phone.root("devlab-health latest\n")?;
    Ok(out)
}

/// A report in plain words, a line per test, failures first.
pub fn said(rows: &[Row]) -> String {
    let mut sorted: Vec<&Row> = rows.iter().collect();
    sorted.sort_by_key(|r| match r.outcome.as_str() {
        "fail" => 0,
        "skip" => 2,
        _ => 1,
    });
    let n = |o: &str| rows.iter().filter(|r| r.outcome == o).count();
    let mut s: String = sorted
        .iter()
        .map(|r| {
            format!(
                "{:4} {:5} {}/{} - {}\n",
                r.outcome, r.layer, r.component, r.test, r.detail
            )
        })
        .collect();
    s.push_str(&format!(
        "{} pass, {} fail, {} not measured\n",
        n("pass"),
        n("fail"),
        n("skip")
    ));
    s
}

fn tempdir() -> Result<PathBuf> {
    let p = std::env::temp_dir().join(format!("devlab-suite-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p)?;
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_reads_as_rows_and_noise_is_ignored() {
        let r = "{\"run\":\"r\",\"layer\":\"base\",\"component\":\"wifi\",\"test\":\"wifi-interface\",\"outcome\":\"pass\",\"detail\":\"wlan0\"}\nnoise\n";
        assert_eq!(
            rows(r),
            vec![Row {
                layer: "base".into(),
                component: "wifi".into(),
                test: "wifi-interface".into(),
                outcome: "pass".into(),
                detail: "wlan0".into()
            }]
        );
    }

    #[test]
    fn a_report_is_known_by_the_boot_it_was_made_in() {
        let r = "{\"boot\":\"b1\",\"test\":\"t\"}\n# devlab-health r: healthy\n";
        assert!(of_boot(r, "b1"));
        assert!(
            !of_boot(r, "b2"),
            "a report from before the reboot is not this boot's"
        );
    }

    #[test]
    fn failures_are_said_first() {
        let row = |o: &str| Row {
            layer: "base".into(),
            component: "c".into(),
            test: o.into(),
            outcome: o.into(),
            detail: String::new(),
        };
        let s = said(&[row("pass"), row("skip"), row("fail")]);
        assert!(s.starts_with("fail"));
        assert!(s.ends_with("1 pass, 1 fail, 1 not measured\n"));
    }

    #[test]
    fn the_suite_comes_from_its_signed_repository() {
        let s = from_the_package();
        assert!(s.starts_with(&format!("[ -f /etc/apk/keys/{KEY_NAME} ] || cat > /etc/apk/keys/{KEY_NAME} <<'KEY'\n-----BEGIN PUBLIC KEY-----\n")));
        assert!(
            s.contains("-----END PUBLIC KEY-----\nKEY\n"),
            "the key whole, then its end"
        );
        assert!(s.contains(&format!(
            "grep -qxF '{REPOSITORY}' /etc/apk/repositories || echo"
        )));
        let add = s.find("apk add").unwrap();
        let rm = s
            .find("rm -f /etc/systemd/system/devlab-health.service")
            .unwrap();
        assert!(
            add < rm,
            "what was put by hand goes only once the package is in"
        );
    }

    #[test]
    fn every_suite_file_is_built_in() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../health/suite/checks");
        let on_disk: usize = ["base", "shell", "telephony"]
            .iter()
            .map(|l| std::fs::read_dir(format!("{dir}/{l}")).map_or(0, |d| d.count()))
            .sum();
        let built_in = FILES
            .iter()
            .filter(|(p, _, _)| p.contains("/checks/"))
            .count();
        assert_eq!(
            on_disk, built_in,
            "a check on disk that devlab does not carry"
        );
    }
}
