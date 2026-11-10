//! Flashing a phone, for any device: the image copied, its partitions mounted, the manifest's fixes
//! baked in (files, masked services, services enabled on first boot), then fastboot. The image's
//! device tree is fingerprinted before anything is flashed; `--as-is` bakes in nothing but access.

use crate::device::Device;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

fn host_root(args: &[&str]) -> Result<String> {
    let prefix =
        std::env::var("DEVLAB_ROOT").unwrap_or_else(|_| "sudo -n lab-agent root-exec --".into());
    let mut p = prefix.split_whitespace();
    let out = Command::new(p.next().context("DEVLAB_ROOT is empty")?)
        .args(p)
        .args(args)
        .output()?;
    if !out.status.success() {
        bail!(
            "{} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Where a manifest path points: `esp:/x` or `root:/x` inside the mounted image.
pub fn place(spec: &str, esp: &Path, root: &Path) -> Option<PathBuf> {
    let (part, path) = spec.split_once(':')?;
    let base = match part {
        "esp" => esp,
        "root" => root,
        _ => return None,
    };
    Some(base.join(path.trim_start_matches('/')))
}

/// The image, attached and mounted while it lives; detached on any way out.
struct Mounted {
    dir: PathBuf,
    lo: Option<String>,
}

impl Mounted {
    fn esp(&self) -> PathBuf {
        self.dir.join("esp")
    }
    fn root(&self) -> PathBuf {
        self.dir.join("root")
    }
}

impl Drop for Mounted {
    fn drop(&mut self) {
        for m in ["esp", "root"] {
            let _ = host_root(&["umount", self.dir.join(m).to_str().unwrap_or("")]);
        }
        if let Some(lo) = self.lo.take() {
            let _ = host_root(&["losetup", "-d", &lo]);
        }
    }
}

pub fn flash(
    device: &Device,
    manifest_dir: &Path,
    image: &Path,
    bootloader: &Path,
    dtb: Option<&Path>,
    as_is: bool,
) -> Result<()> {
    let f = &device.flash;
    let work = std::env::temp_dir().join(format!("devlab-flash-{}", std::process::id()));
    std::fs::create_dir_all(work.join("esp"))?;
    std::fs::create_dir_all(work.join("root"))?;
    let copy = work.join("image.img");
    let ok = Command::new("cp")
        .arg("--sparse=always")
        .arg(image)
        .arg(&copy)
        .status()?
        .success();
    if !ok {
        bail!("copying {} failed", image.display());
    }
    let result = (|| -> Result<()> {
        {
            let lo = host_root(&[
                "losetup",
                "--show",
                "-f",
                "--sector-size",
                &f.sector_size.to_string(),
                "-P",
                copy.to_str().context("path")?,
            ])?;
            let m = Mounted {
                dir: work.clone(),
                lo: Some(lo.clone()),
            };
            host_root(&[
                "mount",
                &format!("{lo}p{}", f.esp_partition),
                m.esp().to_str().context("path")?,
            ])?;
            host_root(&[
                "mount",
                &format!("{lo}p{}", f.root_partition),
                m.root().to_str().context("path")?,
            ])?;
            if let Some(d) = dtb {
                let to = place(&f.dtb, &m.esp(), &m.root())
                    .context("flash.dtb must be esp:/... or root:/...")?;
                host_root(&[
                    "cp",
                    d.to_str().context("path")?,
                    to.to_str().context("path")?,
                ])?;
            }
            if let Some(d) = place(&f.dtb, &m.esp(), &m.root()) {
                let sum =
                    host_root(&["sha256sum", d.to_str().context("path")?]).unwrap_or_default();
                println!(
                    "device tree in the image: {}",
                    sum.get(..16).unwrap_or("(none)")
                );
            }
            if !as_is {
                for o in &f.overlay {
                    let to = place(&o.to, &m.esp(), &m.root())
                        .context("an overlay's 'to' must be esp:/... or root:/...")?;
                    let from = manifest_dir.join(&o.from);
                    let needed = o
                        .if_exists
                        .as_deref()
                        .and_then(|p| place(p, &m.esp(), &m.root()));
                    if needed.as_ref().is_some_and(|n| !n.exists()) || !from.is_file() {
                        println!(
                            "NOT baked in: {} (its condition does not hold, or it is missing)",
                            o.to
                        );
                        continue;
                    }
                    host_root(&[
                        "install",
                        "-D",
                        "-m",
                        &o.mode,
                        from.to_str().context("path")?,
                        to.to_str().context("path")?,
                    ])?;
                    println!("baked in: {}", o.to);
                }
                for s in &f.mask {
                    let link = m.root().join("etc/systemd/system").join(s);
                    host_root(&["ln", "-sf", "/dev/null", link.to_str().context("path")?])?;
                    println!("masked: {s}");
                }
            }
            // access on first boot: always, so the lab can reach the phone
            if !f.enable.is_empty() {
                let preset = work.join("10-devlab.preset");
                std::fs::write(
                    &preset,
                    f.enable
                        .iter()
                        .map(|s| format!("enable {s}\n"))
                        .collect::<String>(),
                )?;
                let to = m.root().join("etc/systemd/system-preset/10-devlab.preset");
                host_root(&[
                    "install",
                    "-D",
                    "-m",
                    "644",
                    preset.to_str().context("path")?,
                    to.to_str().context("path")?,
                ])?;
                println!("enabled on first boot: {}", f.enable.join(" "));
            }
        } // unmounted and detached
        let fb = Command::new("fastboot").arg("devices").output()?;
        if !String::from_utf8_lossy(&fb.stdout).contains("fastboot") {
            bail!("the phone is not in fastboot - nothing flashed");
        }
        for (part, file) in [
            (f.bootloader_partition.as_str(), bootloader),
            (f.image_partition.as_str(), copy.as_path()),
        ] {
            let ok = Command::new("fastboot")
                .args(["flash", part])
                .arg(file)
                .status()?
                .success();
            if !ok {
                bail!("fastboot flash {part} failed");
            }
        }
        let _ = Command::new("timeout")
            .args(["30", "fastboot", "reboot"])
            .status();
        println!("flashed; the first boot of a fresh image is slow (it resizes the storage)");
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&work);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_places_point_inside_the_mounted_image() {
        let (e, r) = (Path::new("/w/esp"), Path::new("/w/root"));
        assert_eq!(
            place("root:/usr/share/x.conf", e, r),
            Some(PathBuf::from("/w/root/usr/share/x.conf"))
        );
        assert_eq!(
            place("esp:dtbs/a.dtb", e, r),
            Some(PathBuf::from("/w/esp/dtbs/a.dtb"))
        );
        assert_eq!(
            place("/etc/passwd", e, r),
            None,
            "a bare path could point anywhere: refused"
        );
        assert_eq!(place("home:/x", e, r), None);
    }
}
