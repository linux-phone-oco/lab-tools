// SPDX-License-Identifier: MIT OR Apache-2.0
//! The pack: one device's declaration and one distro profile become the images and the plan that installs them -
//! the root file system (`rootfs`), the boot image (`boot`, in the format the declaration names) and install.toml
//! (`install`), each stage the module that already proves it. The images' sha256 are printed and written into
//! install.toml, and the last line is the one command that installs: `install: osfact install now --plan FILE`.
//! `--dry-run` says each step and writes nothing.

use crate::boot::{self, Boot};
use crate::install::{
    self, Image, Install, Plan, Step, StepAction, Transport, WayBack, WayBackAction,
};
use crate::rootfs;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Request {
    pub device_file: PathBuf,
    pub distro_file: PathBuf,
    pub parts: PathBuf,
    pub out: PathBuf,
    pub size_mib: u64,
    pub dry_run: bool,
}

/// The tables of a declaration that are the device's: the lab's own tables are not osfact's and are not read.
#[derive(Deserialize)]
struct Declaration {
    device: Device,
}

#[derive(Deserialize)]
struct Device {
    codename: String,
    /// The machine kind of the device's packages (aarch64): never the host's.
    arch: Option<String>,
    /// The environment variable that holds the device's serial; by default the codename's words and SERIAL.
    serial_env: Option<String>,
}

struct Declared {
    file: PathBuf,
    device: Device,
    arch: String,
    boot: Boot,
    back_wait: u32,
}

/// What the pack does, in order; each step is one named call below.
pub fn make(req: &Request) -> Result<(), String> {
    let declared = read_declaration(&req.device_file)?;
    rootfs::make(&rootfs::Request {
        distro_file: req.distro_file.clone(),
        arch: declared.arch.clone(),
        out: req.out.clone(),
        size_mib: req.size_mib,
        dry_run: req.dry_run,
    })?;
    let (boot_image, boot_sum) = pack_boot(req, &declared)?;
    write_plan(req, &declared, &boot_image, &boot_sum)
}

fn read_declaration(file: &Path) -> Result<Declared, String> {
    let text = fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let d: Declaration =
        toml::from_str(&text).map_err(|e| format!("{}: the declaration: {e}", file.display()))?;
    let arch = d.device.arch.clone().ok_or_else(|| {
        format!(
            "{}: the declaration's [device] says no arch (the machine kind of its packages, e.g. arch = \"aarch64\")",
            file.display()
        )
    })?;
    let boot = Boot::from_declaration(&text).map_err(|e| format!("{}: {e}", file.display()))?;
    let back_wait = boot.back_wait.ok_or_else(|| {
        format!(
            "{}: [boot] says no back_wait (seconds the way back waits)",
            file.display()
        )
    })?;
    if boot.avb_footer && boot.partition_size.is_none() {
        return Err(format!(
            "{}: [boot] avb_footer = true needs partition_size: the footer sits at the partition's end",
            file.display()
        ));
    }
    Ok(Declared {
        file: file.to_path_buf(),
        device: d.device,
        arch,
        boot,
        back_wait,
    })
}

/// The boot image: the parts packed in the declared format, counted back against the declaration.
fn pack_boot(req: &Request, d: &Declared) -> Result<(PathBuf, String), String> {
    let image = req.out.join("boot.img");
    let format = boot::format(&d.boot.format)?;
    say(&format!(
        "boot image: {} from {} (kernel, ramdisk{}) to {}",
        d.boot.format,
        req.parts.display(),
        d.boot
            .dtb
            .as_ref()
            .map(|p| format!(", {p}"))
            .unwrap_or_default(),
        image.display()
    ));
    if req.dry_run {
        say("check: the boot image is counted back against the declaration's [boot]");
        return Ok((image, String::new()));
    }
    let read = |name: &str| {
        let p = req.parts.join(name);
        fs::read(&p).map_err(|e| format!("the parts folder: {}: {e}", p.display()))
    };
    let mut kernel = read("kernel")?;
    if let Some(dtb) = &d.boot.dtb {
        kernel.extend(read(dtb)?);
    }
    let bytes = format.pack(&d.boot, &kernel, &read("ramdisk")?)?;
    let bad: Vec<String> = format
        .check(&bytes, &d.boot)
        .iter()
        .filter(|f| matches!(f.verdict, boot::Verdict::Fail(_)))
        .map(|f| f.line())
        .collect();
    if !bad.is_empty() {
        return Err(format!(
            "{}: the boot image fails the declaration's [boot]: {}",
            d.file.display(),
            bad.join("; ")
        ));
    }
    fs::write(&image, &bytes).map_err(|e| format!("{}: {e}", image.display()))?;
    let sum = rootfs::sha256(&image)?;
    say(&format!("boot.img sha256 {sum}"));
    Ok((image, sum))
}

/// install.toml: both images with the sha256 the pack measured, the boot from RAM, the boot from RAM and the fastboot reboot are the one way osfact installs so far (the wait is the declaration's).
fn write_plan(
    req: &Request,
    d: &Declared,
    boot_image: &Path,
    boot_sum: &str,
) -> Result<(), String> {
    let plan_file = req.out.join("install.toml");
    let command = format!("osfact install now --plan {}", plan_file.display());
    say(&format!("install plan: {}", plan_file.display()));
    if req.dry_run {
        say(&format!("install: {command}"));
        return Ok(());
    }
    let image = |name: &str, file: &Path| -> Result<Image, String> {
        Ok(Image {
            name: name.into(),
            file: file.to_string_lossy().into_owned(),
            sha256: rootfs::sha256(file)?,
        })
    };
    let plan = Plan {
        install: Install {
            device: d.device.codename.clone(),
            serial_env: d
                .device
                .serial_env
                .clone()
                .unwrap_or_else(|| serial_env(&d.device.codename)),
            image: vec![
                image("rootfs", &req.out.join("rootfs.img"))?,
                Image {
                    name: "boot".into(),
                    file: boot_image.to_string_lossy().into_owned(),
                    sha256: boot_sum.into(),
                },
            ],
            step: vec![Step {
                action: StepAction::BootFromRam,
                write: "boot".into(),
                via: Transport::Fastboot,
            }],
            way_back: WayBack {
                via: Transport::Fastboot,
                action: WayBackAction::Reboot,
                wait: d.back_wait,
            },
        },
    };
    let text = toml::to_string(&plan).map_err(|e| format!("install.toml: {e}"))?;
    install::read(&text)?; // what is written is what install now reads
    fs::write(&plan_file, text).map_err(|e| format!("{}: {e}", plan_file.display()))?;
    say(&format!("install: {command}"));
    Ok(())
}

/// The codename's words, upper case, then SERIAL: a codename "acme-one" gives ACME_ONE_SERIAL.
fn serial_env(codename: &str) -> String {
    let words: String = codename
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("{words}_SERIAL")
}

fn say(line: &str) {
    println!("{line}");
}
