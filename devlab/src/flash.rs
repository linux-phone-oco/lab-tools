// SPDX-License-Identifier: MIT OR Apache-2.0
//! Flashing a phone, for any device: the image copied, the manifest's fixes baked into the copy (files,
//! masked services, services enabled on first boot), then fastboot. The bake needs no root and no mount: the
//! image's two partitions are written in place with e2fsprogs' `debugfs` (the ext4 root) and mtools (the
//! vfat esp), found by their offsets in the image's partition table. The image's device tree is
//! fingerprinted before anything is flashed; `--as-is` bakes in nothing but access.

use crate::device::Device;
use anyhow::{bail, Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Where a manifest path points inside the image: `esp:/x` or `root:/x`.
#[derive(Debug, PartialEq, Clone)]
pub enum Spot {
    Esp(String),
    Root(String),
}

impl Spot {
    fn path(&self) -> &str {
        match self {
            Spot::Esp(p) | Spot::Root(p) => p,
        }
    }
}

/// A manifest place, `esp:PATH` or `root:PATH`. A bare path could point anywhere on the lab: refused. So is a
/// path that climbs out (`..`) or holds a space, a quote or a control character (it becomes a tool's input).
pub fn spot(spec: &str) -> Option<Spot> {
    let (part, path) = spec.split_once(':')?;
    if path
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || c == '"' || c == '\'')
        || path.split('/').any(|c| c == "..")
    {
        return None;
    }
    let path = format!("/{}", path.trim_start_matches('/'));
    match part {
        "esp" => Some(Spot::Esp(path)),
        "root" => Some(Spot::Root(path)),
        _ => None,
    }
}

/// The ancestors of a path, shortest first: `/a/b/c` gives `/a`, `/a/b`.
fn parents(path: &str) -> Vec<String> {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    (1..parts.len())
        .map(|n| format!("/{}", parts[..n].join("/")))
        .collect()
}

/// The copy of the image being baked: two partitions at byte offsets.
struct Image {
    file: PathBuf,
    esp: u64,
    root: u64,
}

fn tool(program: &str, args: &[&str]) -> Result<std::process::Output> {
    Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("running {program} (is it installed?)"))
}

impl Image {
    /// The partitions' byte offsets, from `sfdisk -J` (it reads a file, as the lab user).
    fn open(file: &Path, sector: u32, esp_part: u32, root_part: u32) -> Result<Image> {
        let f = file.to_str().context("path")?;
        let o = tool("sfdisk", &["-J", "--sector-size", &sector.to_string(), f])?;
        if !o.status.success() {
            bail!(
                "sfdisk could not read the image's partition table: {}",
                String::from_utf8_lossy(&o.stderr).trim()
            );
        }
        let v: serde_json::Value = serde_json::from_slice(&o.stdout)?;
        let parts = v["partitiontable"]["partitions"]
            .as_array()
            .context("the image has no partition table")?;
        let at = |n: u32| -> Result<u64> {
            let p = parts
                .get(n.checked_sub(1).context("partitions count from 1")? as usize)
                .with_context(|| format!("the image has no partition {n}"))?;
            Ok(p["start"].as_u64().context("a partition without a start")? * u64::from(sector))
        };
        Ok(Image {
            file: file.to_path_buf(),
            esp: at(esp_part)?,
            root: at(root_part)?,
        })
    }

    fn ext(&self) -> String {
        format!("{}?offset={}", self.file.display(), self.root)
    }

    fn vfat(&self) -> String {
        format!("{}@@{}", self.file.display(), self.esp)
    }

    /// The bytes of a file in the image, or None when it is not there.
    fn cat(&self, at: &Spot) -> Result<Option<Vec<u8>>> {
        if !self.stat_ok(at)? {
            return Ok(None);
        }
        let p = at.path();
        let o = match at {
            Spot::Root(_) => tool("debugfs", &["-R", &format!("cat {p}"), &self.ext()])?,
            Spot::Esp(_) => tool("mtype", &["-i", &self.vfat(), &format!("::{p}")])?,
        };
        Ok(o.status.success().then_some(o.stdout))
    }

    fn stat_ok(&self, at: &Spot) -> Result<bool> {
        let p = at.path();
        Ok(match at {
            Spot::Root(_) => {
                let o = tool("debugfs", &["-R", &format!("stat {p}"), &self.ext()])?;
                String::from_utf8_lossy(&o.stdout).contains("Inode:")
            }
            Spot::Esp(_) => tool("mdir", &["-i", &self.vfat(), &format!("::{p}")])?
                .status
                .success(),
        })
    }

    pub fn exists(&self, at: &Spot) -> Result<bool> {
        self.stat_ok(at)
    }

    /// `install -D -m MODE FROM TO`, as root:root, read back and compared.
    fn put(&self, from: &Path, to: &Spot, mode: &str) -> Result<()> {
        let want = std::fs::read(from).with_context(|| format!("reading {}", from.display()))?;
        let src = from.to_str().context("path")?;
        if src
            .chars()
            .any(|c| c.is_whitespace() || c == '"' || c == '\'')
        {
            bail!("{src} holds a space or a quote: move the file somewhere plain, the image tools take no quoting");
        }
        match to {
            Spot::Root(dest) => {
                let m = u32::from_str_radix(mode, 8)
                    .with_context(|| format!("a mode is octal: {mode:?}"))?;
                let mut script = String::new();
                for d in parents(dest) {
                    script.push_str(&format!("mkdir {d}\n"));
                }
                script.push_str(&format!(
                    "rm {dest}\nwrite {src} {dest}\nset_inode_field {dest} mode 0100{m:o}\nset_inode_field {dest} uid 0\nset_inode_field {dest} gid 0\n"
                ));
                self.debugfs(&script)?;
            }
            Spot::Esp(dest) => {
                for d in parents(dest) {
                    let _ = tool("mmd", &["-i", &self.vfat(), &format!("::{d}")]);
                }
                let o = tool(
                    "mcopy",
                    &["-o", "-i", &self.vfat(), src, &format!("::{dest}")],
                )?;
                if !o.status.success() {
                    bail!(
                        "mcopy to {dest} failed: {}",
                        String::from_utf8_lossy(&o.stderr).trim()
                    );
                }
            }
        }
        if self.cat(to)?.as_deref() != Some(&want[..]) {
            bail!("{} did not read back from the image as written", to.path());
        }
        Ok(())
    }

    /// `ln -sf TARGET LINK` on the root partition.
    fn symlink(&self, link: &str, target: &str) -> Result<()> {
        let mut script = String::new();
        for d in parents(link) {
            script.push_str(&format!("mkdir {d}\n"));
        }
        script.push_str(&format!("rm {link}\nsymlink {link} {target}\n"));
        self.debugfs(&script)?;
        let o = tool("debugfs", &["-R", &format!("stat {link}"), &self.ext()])?;
        let said = String::from_utf8_lossy(&o.stdout);
        if !said.contains("Type: symlink") || !said.contains(target) {
            bail!("{link} is not a link to {target} in the image");
        }
        Ok(())
    }

    /// Runs debugfs commands, writing. Its errors are lines, not an exit status, so the only line it may
    /// say of a failure that does not matter is a directory that exists or a file that is not there to remove;
    /// each caller reads what it wrote back.
    fn debugfs(&self, script: &str) -> Result<()> {
        let mut c = Command::new("debugfs")
            .args(["-w", "-f", "-", &self.ext()])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("running debugfs (is e2fsprogs installed?)")?;
        c.stdin
            .take()
            .context("stdin")?
            .write_all(script.as_bytes())?;
        let o = c.wait_with_output()?;
        if !o.status.success() {
            bail!(
                "debugfs failed: {}",
                String::from_utf8_lossy(&o.stderr).trim()
            );
        }
        Ok(())
    }
}

/// The SHA-256 of some bytes, as `sha256sum` says it.
fn sha256(bytes: &[u8]) -> Option<String> {
    let mut c = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    c.stdin.take()?.write_all(bytes).ok()?;
    let o = c.wait_with_output().ok()?;
    String::from_utf8_lossy(&o.stdout)
        .split_whitespace()
        .next()
        .map(str::to_string)
}

/// Writing over fastboot is the manifest's to allow, never assumed: a phone whose slots must stay as
/// they are says nothing (google-walleye, decision 2026-11-17-walleye-fastboot-first).
pub fn may_write(device: &Device) -> Result<()> {
    if !device.fastboot.writes {
        bail!(
            "{}'s device.toml does not say [fastboot] writes = true - nothing is flashed on it \
             (lpoco-lab fastboot boot now IMAGE boots from memory without writing)",
            device.device.codename
        );
    }
    Ok(())
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
    may_write(device)?;
    let serial = crate::fastboot::serial(device)?;
    let work = std::env::temp_dir().join(format!("devlab-flash-{}", std::process::id()));
    std::fs::create_dir_all(&work)?;
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
            let img = Image::open(&copy, f.sector_size, f.esp_partition, f.root_partition)?;
            let dtb_at = spot(&f.dtb);
            if let Some(d) = dtb {
                let to = dtb_at
                    .as_ref()
                    .context("flash.dtb must be esp:/... or root:/...")?;
                img.put(d, to, "644")?;
            }
            if let Some(sum) = dtb_at
                .as_ref()
                .and_then(|d| img.cat(d).ok().flatten())
                .and_then(|bytes| sha256(&bytes))
            {
                println!(
                    "device tree in the image: {}",
                    sum.get(..16).unwrap_or("(none)")
                );
            } else {
                println!("device tree in the image: (none)");
            }
            if !as_is {
                for o in &f.overlay {
                    let to =
                        spot(&o.to).context("an overlay's 'to' must be esp:/... or root:/...")?;
                    let from = manifest_dir.join(&o.from);
                    let needed = match o.if_exists.as_deref().map(spot) {
                        Some(Some(p)) => Some(img.exists(&p)?),
                        Some(None) => Some(false),
                        None => None,
                    };
                    if needed == Some(false) || !from.is_file() {
                        println!(
                            "NOT baked in: {} (its condition does not hold, or it is missing)",
                            o.to
                        );
                        continue;
                    }
                    img.put(&from, &to, &o.mode)?;
                    println!("baked in: {}", o.to);
                }
                for s in &f.mask {
                    img.symlink(&format!("/etc/systemd/system/{s}"), "/dev/null")?;
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
                img.put(
                    &preset,
                    &Spot::Root("/etc/systemd/system-preset/10-devlab.preset".into()),
                    "644",
                )?;
                println!("enabled on first boot: {}", f.enable.join(" "));
            }
        }
        // by its own serial only: a lab may hold two phones in fastboot (2026-11-17: google-walleye
        // sat in fastboot beside surya, and `fastboot flash` without -s would have written to it)
        if !crate::fastboot::present(serial)? {
            bail!("{serial} is not in fastboot - nothing flashed");
        }
        for (part, file) in [
            (f.bootloader_partition.as_str(), bootloader),
            (f.image_partition.as_str(), copy.as_path()),
        ] {
            let ok = Command::new("fastboot")
                .args(["-s", serial, "flash", part])
                .arg(file)
                .status()?
                .success();
            if !ok {
                bail!("fastboot flash {part} failed");
            }
        }
        let _ = Command::new("timeout")
            .args(["30", "fastboot", "-s", serial, "reboot"])
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
    fn manifest_places_point_inside_the_image() {
        assert_eq!(
            spot("root:/usr/share/x.conf"),
            Some(Spot::Root("/usr/share/x.conf".into()))
        );
        assert_eq!(
            spot("esp:dtbs/a.dtb"),
            Some(Spot::Esp("/dtbs/a.dtb".into()))
        );
        assert_eq!(
            spot("/etc/passwd"),
            None,
            "a bare path could point anywhere: refused"
        );
        assert_eq!(spot("home:/x"), None);
        assert_eq!(spot("root:/a/../../etc/x"), None, "no climbing out");
        assert_eq!(spot("root:/a b"), None, "no space reaches a tool");
        assert_eq!(spot("root:/a\"b"), None);
    }

    #[test]
    fn parents_run_from_the_top() {
        assert_eq!(parents("/a/b/c"), vec!["/a", "/a/b"]);
        assert!(parents("/x").is_empty());
    }

    /// The bake itself on a real, small image: an ext4 root and a vfat esp in a GPT with 4096-byte sectors,
    /// made here without root. `cargo test -- --ignored the_bake` (needs sfdisk, mke2fs, mkfs.vfat, mtools, debugfs).
    #[test]
    #[ignore]
    fn the_bake_writes_files_links_and_the_dtb_without_a_mount() {
        let dir = std::env::temp_dir().join(format!("devlab-bake-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sh = |s: &str| {
            assert!(
                Command::new("sh")
                    .arg("-c")
                    .arg(s)
                    .current_dir(&dir)
                    .status()
                    .unwrap()
                    .success(),
                "{s}"
            );
        };
        // 4 KiB sectors: partition 1 (esp) at sector 2048, partition 2 (root) at 4096, 8 MiB each
        sh("truncate -s 40M disk.img && printf 'label: gpt\\nunit: sectors\\nsector-size: 4096\\n2048,2048,U\\n4096,2048,L\\n' | sfdisk --sector-size 4096 disk.img >/dev/null");
        sh("truncate -s 8M esp.img && mkfs.vfat -S 4096 esp.img >/dev/null && truncate -s 8M root.img && mke2fs -q -t ext4 -d /dev/null root.img 2>/dev/null || mke2fs -q -t ext4 root.img");
        sh("dd if=esp.img of=disk.img bs=4096 seek=2048 conv=notrunc 2>/dev/null && dd if=root.img of=disk.img bs=4096 seek=4096 conv=notrunc 2>/dev/null");
        std::fs::write(dir.join("hello"), b"hello\n").unwrap();
        std::fs::write(dir.join("a.dtb"), b"dtb-bytes").unwrap();
        let img = Image::open(&dir.join("disk.img"), 4096, 1, 2).unwrap();
        img.put(
            &dir.join("hello"),
            &Spot::Root("/usr/local/bin/hello".into()),
            "755",
        )
        .unwrap();
        img.symlink("/etc/systemd/system/x.service", "/dev/null")
            .unwrap();
        img.put(
            &dir.join("a.dtb"),
            &Spot::Esp("/dtbs/qcom/a.dtb".into()),
            "644",
        )
        .unwrap();
        assert!(img
            .exists(&Spot::Root("/usr/local/bin/hello".into()))
            .unwrap());
        assert!(!img.exists(&Spot::Root("/nope".into())).unwrap());
        assert_eq!(
            img.cat(&Spot::Esp("/dtbs/qcom/a.dtb".into()))
                .unwrap()
                .unwrap(),
            b"dtb-bytes"
        );
        let o = tool("debugfs", &["-R", "stat /usr/local/bin/hello", &img.ext()]).unwrap();
        let s = String::from_utf8_lossy(&o.stdout);
        assert!(
            s.contains("Mode:  0755") && s.contains("User:     0"),
            "{s}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
