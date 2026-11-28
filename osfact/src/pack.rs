// SPDX-Liclllet decl = declaration::check(declaration)?;t d = declaration::check(&text).map_err(|e| format!("{}: {e}", file.display()))?;t d = declaration::check(&text).map_err(|e| format!("{}: {e}", file.display()))?;nse-Identifier: MIT OR Apache-2.0
//! The pack: one device's declaration and one distro profile become the images and the plan that installs them -
//! the root file system (`rootfs`), the boot image (`boot`, in the format the declaration names) and install.toml
//! (`install`), each stage the module that already proves it. The images' sha256 are printed and written into
//! install.toml, and the last line is the one command that installs: `install: osfact install now --plan FILE`.
//! `--dry-run` says each step and writes nothing.

use crate::boot::{self, Boot};
use crate::declaration::{self, Arch, Root};
use crate::install::{
    self, Compression, Enter, Image, Install, Plan, Step, StepAction, Transport, WayBack,
    WayBackAction,
};
use crate::rootfs;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Request {
    pub device_file: PathBuf,
    pub distro_file: PathBuf,
    pub parts: PathBuf,
    pub out: PathBuf,
    pub size_mib: u64,
    pub dry_run: bool,
}

struct Declared {
    file: PathBuf,
    text: String,
    arch: Arch,
    boot: Boot,
}

/// What the pack does, in order; each step is one named call below.
pub fn make(req: &Request) -> Result<(), String> {
    let declared = read_declaration(&req.device_file)?;
    rootfs::make(&rootfs::Request {
        distro_file: req.distro_file.clone(),
        arch: declared.arch.as_str().to_string(),
        out: req.out.clone(),
        size_mib: req.size_mib,
        dry_run: req.dry_run,
    })?;
    let (boot_image, boot_sum) = pack_boot(req, &declared)?;
    let (root_image, root_sum) = pack_root(req)?;
    let file = |p: &Path| p.to_string_lossy().into_owned();
    let packed = Packed {
        rootfs: (file(&req.out.join("rootfs.img")), rootfs_sum(req)?),
        boot: (file(&boot_image), boot_sum),
        root: (file(&root_image), root_sum),
    };
    write_plan(req, &declared, &packed)
}

fn read_declaration(file: &Path) -> Result<Declared, String> {
    let text = fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let d = declaration::check(&text).map_err(|e| format!("{}: {e}", file.display()))?;
    let arch = d.device.arch.ok_or_else(|| {
        format!(
            "{}: the declaration's [device] says no arch (the machine kind of its packages, e.g. arch = \"aarch64\")",
            file.display()
        )
    })?;
    let boot = Boot::from_declaration(&text).map_err(|e| format!("{}: {e}", file.display()))?;
    boot.back_wait.ok_or_else(|| {
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
    // the plan is made once before any build, only to refuse a declaration it cannot plan from
    plan_text(&text, &Packed::unmeasured()).map_err(|e| format!("{}: {e}", file.display()))?;
    Ok(Declared {
        file: file.to_path_buf(),
        text,
        arch,
        boot,
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

/// The root image as the device receives it: the root tree's tar (the one the ext4 image was made from), gzipped
/// without a name or a time in it, so the same tree gives the same bytes. Returns the file and its sha256.
fn pack_root(req: &Request) -> Result<(PathBuf, String), String> {
    let tar = rootfs::root_tar(&req.out);
    let image = req.out.join("root.tar.gz");
    say(&format!(
        "root image: gzip of {} to {}",
        tar.display(),
        image.display()
    ));
    if req.dry_run {
        return Ok((image, String::new()));
    }
    // gzipped twice and compared, as the ext4 image is built twice: this host's gzip must give the same bytes
    let (first, second) = (req.out.join("root.tar.gz.1"), req.out.join("root.tar.gz.2"));
    gzip(&tar, &first)?;
    gzip(&tar, &second)?;
    let (a, b) = (rootfs::sha256(&first)?, rootfs::sha256(&second)?);
    let _ = fs::remove_file(&second);
    if a != b {
        let _ = fs::remove_file(&first);
        return Err(format!(
            "the two gzips of {} differ: {a} and {b}",
            tar.display()
        ));
    }
    fs::rename(&first, &image).map_err(|e| format!("{}: {e}", image.display()))?;
    say(&format!("root.tar.gz sha256 {a}"));
    Ok((image, a))
}

/// gzip without a name or a time in it, to a file that is removed if gzip fails.
fn gzip(tar: &Path, to: &Path) -> Result<(), String> {
    let out = fs::File::create(to).map_err(|e| format!("{}: {e}", to.display()))?;
    let status = Command::new("gzip")
        .args(["-n", "-9", "-c"])
        .arg(tar)
        .stdout(out)
        .status()
        .map_err(|e| format!("gzip: {e}"))?;
    if status.success() {
        return Ok(());
    }
    let _ = fs::remove_file(to);
    Err(format!("gzip {} ended with {status}", tar.display()))
}

/// One image the plan names: its file and the sha256 the pack measured.
pub type Measured = (String, String);

/// The images the pack made, each with the sha256 install.toml carries.
pub struct Packed {
    pub rootfs: Measured,
    pub boot: Measured,
    pub root: Measured,
}

/// install.toml: the images with the sha256 the pack measured, and the steps the declaration's [install] allows -
/// the boot from RAM alone, or with [install.connection] the push of the root image and its entry too.
fn write_plan(req: &Request, d: &Declared, packed: &Packed) -> Result<(), String> {
    let plan_file = req.out.join("install.toml");
    let command = format!("osfact install now --plan {}", plan_file.display());
    say(&format!("install plan: {}", plan_file.display()));
    if req.dry_run {
        say(&format!("install: {command}"));
        return Ok(());
    }
    let text = plan_text(&d.text, packed)?;
    fs::write(&plan_file, text).map_err(|e| format!("{}: {e}", plan_file.display()))?;
    say(&format!("install: {command}"));
    Ok(())
}

/// The text of install.toml for one declaration and the images made: what is returned is what `install::read` accepts.
pub fn plan_text(declaration: &str, packed: &Packed) -> Result<String, String> {
    let decl = declaration::check(declaration)?;
    let boot = Boot::from_declaration(declaration)?;
    let back_wait = boot
        .back_wait
        .ok_or("[boot] says no back_wait (seconds the way back waits)")?;
    let image = |name: &str, (file, sha256): &Measured, compression| Image {
        name: name.into(),
        file: file.clone(),
        sha256: sha256.clone(),
        compression,
    };
    let serial_env = decl
        .device
        .serial_env
        .clone()
        .unwrap_or_else(|| serial_env(&decl.device.codename));
    if let Some(i) = decl.install.as_ref().filter(|i| i.connection.is_none()) {
        for (table, declared) in [
            ("[install.root]", i.root.is_some()),
            ("[install.way_back]", i.way_back.is_some()),
        ] {
            if declared {
                return Err(format!("{table} is declared and [install.connection] is not (the push and the entry need the connection)"));
            }
        }
    }
    let install = match decl.install.as_ref().and_then(|i| i.connection.clone()) {
        None => Install {
            device: decl.device.codename.clone(),
            serial_env,
            connection: None,
            image: vec![
                image("rootfs", &packed.rootfs, None),
                image("boot", &packed.boot, None),
            ],
            step: vec![Step::boot_from_ram("boot")],
            way_back: WayBack {
                via: Transport::Fastboot,
                action: WayBackAction::Reboot,
                wait: back_wait,
                command: None,
                back_on: None,
            },
        },
        Some(connection) => {
            let i = decl.install.as_ref().expect("a connection came from it");
            let root = i
                .root
                .as_ref()
                .ok_or("[install.connection] is declared and [install.root] is not (the push and the entry of the root image)")?;
            let way_back = i
                .way_back
                .as_ref()
                .ok_or("[install.connection] is declared and [install.way_back] is not (its command: after the boot the device is no longer in fastboot)")?;
            Install {
                device: decl.device.codename.clone(),
                serial_env,
                connection: Some(connection),
                image: vec![
                    image("boot", &packed.boot, None),
                    image("root", &packed.root, Some(Compression::Gzip)),
                ],
                step: vec![
                    Step::boot_from_ram("boot"),
                    push_step(root),
                    enter_step(root),
                ],
                way_back: WayBack {
                    via: Transport::Shell,
                    action: WayBackAction::Reboot,
                    wait: back_wait,
                    command: Some(way_back.command.clone()),
                    back_on: Some(way_back.back_on.clone().ok_or("[install.way_back] declares no back_on (the host command, as argv, that ends with status 0 once the device is on its known good: the way back is confirmed by it)")?),
                },
            }
        }
    };
    let text = toml::to_string(&Plan { install }).map_err(|e| format!("install.toml: {e}"))?;
    install::read(&text)?; // what is written is what install now reads
    Ok(text)
}

/// The push of the root image: the receiver on the device, where it writes, the seconds it may take.
fn push_step(r: &Root) -> Step {
    Step {
        action: StepAction::Push,
        via: Transport::Network,
        port: Some(r.port),
        receive: Some(r.receive.clone()),
        to: Some(r.to.clone()),
        wait: Some(r.wait),
        ..Step::boot_from_ram("root")
    }
}

/// The entry of the pushed root: a chroot in a tmpfs, the command started, the check that it runs.
fn enter_step(r: &Root) -> Step {
    Step {
        action: StepAction::Enter,
        via: Transport::Shell,
        how: Some(Enter::Chroot),
        root: Some(r.path.clone()),
        size_mib: Some(r.size_mib),
        command: Some(r.command.clone()),
        check: r.check.clone(),
        check_wait: r.check_wait,
        binds: r.binds.clone(),
        ..Step::boot_from_ram("root")
    }
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

/// The sha256 of rootfs.img, once it is built (a dry run builds nothing).
fn rootfs_sum(req: &Request) -> Result<String, String> {
    if req.dry_run {
        return Ok(String::new());
    }
    rootfs::sha256(&req.out.join("rootfs.img"))
}

impl Packed {
    /// Names and no sums: enough for `plan_text` to refuse a declaration before anything is built.
    fn unmeasured() -> Packed {
        let named = |f: &str| (f.to_string(), "0".repeat(64));
        Packed {
            rootfs: named("rootfs.img"),
            boot: named("boot.img"),
            root: named("root.tar.gz"),
        }
    }
}
