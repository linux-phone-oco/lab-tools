// SPDX-License-Identifier: MIT OR Apache-2.0
//! lpoco-layer: a phone's device layer ([layer] in device.toml, typed in lpoco-manifest) as packages. The
//! declared files, and kernel-install's devicetree file, go into `NAME-config`, the same in every format; the glue a format's base needs - the
//! systemd preset, the initramfs module list in the initramfs tool's own form - goes into
//! `NAME-boot`, which apk does not get (its initramfs tool, mkinitfs, is not known yet). Each package is an nfpm
//! config; nfpm packs it. The kernel package (`linux-NAME`) and the dependency token map are not here yet.

use lpoco_manifest::{BootMethod, FileKind, Layer};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Apk,
    Deb,
    Rpm,
    Archlinux,
}

impl Format {
    pub const TOKENS: &'static [&'static str] = &["apk", "deb", "rpm", "archlinux"];

    pub fn parse(s: &str) -> Option<Format> {
        Some(match s.trim() {
            "apk" => Format::Apk,
            "deb" => Format::Deb,
            "rpm" => Format::Rpm,
            "archlinux" => Format::Archlinux,
            _ => return None,
        })
    }

    pub fn token(self) -> &'static str {
        Self::TOKENS[self as usize]
    }

    /// The file a package of this format is written to, in the form each format's tools expect.
    pub fn file_name(self, name: &str, version: &str) -> String {
        match self {
            Format::Apk => format!("{name}-{version}-r0.apk"),
            Format::Deb => format!("{name}_{version}_all.deb"),
            Format::Rpm => format!("{name}-{version}-1.noarch.rpm"),
            Format::Archlinux => format!("{name}-{version}-1-any.pkg.tar.zst"),
        }
    }
}

/// Where a package's file comes from: the layer's tree (relative to the manifest) or text this crate writes.
#[derive(Clone, PartialEq, Debug)]
pub enum Source {
    Tree(String),
    Text(String),
}

#[derive(Clone, PartialEq, Debug)]
pub struct Item {
    pub source: Source,
    pub dst: String,
    pub mode: u32,
    pub config: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Package {
    pub name: String,
    pub description: String,
    pub depends: Vec<String>,
    pub items: Vec<Item>,
}

/// An item with its source made a file on disk.
#[derive(Clone, PartialEq, Debug)]
pub struct Entry {
    pub src: PathBuf,
    pub dst: String,
    pub mode: u32,
    pub config: bool,
}

pub fn packages(layer: &Layer, format: Format) -> Vec<Package> {
    let name = &layer.name;
    let mut out = Vec::new();
    let mut config = Vec::new();
    for f in &layer.file {
        config.push(Item {
            source: Source::Tree(f.from.clone()),
            dst: f.to.clone(),
            mode: if f.kind == FileKind::Script {
                0o755
            } else {
                0o644
            },
            config: f.kind == FileKind::Config,
        });
    }
    let text = |dst: String, body: String, mode: u32| Item {
        source: Source::Text(body),
        dst,
        mode,
        config: false,
    };
    if let (Some(b), Some(k)) = (&layer.boot, &layer.kernel) {
        // kernel-install(8) installs the one dtb this file names; a phone with several variants gets its first.
        // It is in the config package of every format, so the formats hold the same files (Alpine never reads it).
        if b.method == BootMethod::EfiKernelInstall {
            if let Some(dtb) = k.dtbs.first() {
                config.push(text(
                    "/usr/lib/kernel/devicetree".into(),
                    format!("{dtb}\n"),
                    0o644,
                ));
            }
        }
    }
    if !config.is_empty() {
        out.push(Package {
            name: format!("{name}-config"),
            description: format!("{name}: device configuration"),
            depends: layer.needs.clone(),
            items: config,
        });
    }
    if format == Format::Apk {
        return out;
    }
    let mut boot = Vec::new();
    let units: Vec<&str> = layer
        .file
        .iter()
        .filter(|f| f.kind == FileKind::Unit)
        .filter_map(|f| f.to.rsplit('/').next())
        .filter(|u| {
            [".service", ".socket", ".timer", ".path", ".mount"]
                .iter()
                .any(|s| u.ends_with(s))
        })
        .collect();
    if !units.is_empty() {
        let body: String = units.iter().map(|u| format!("enable {u}\n")).collect();
        boot.push(text(
            format!("/usr/lib/systemd/system-preset/50-{name}.preset"),
            body,
            0o644,
        ));
    }
    if let Some(i) = layer.initramfs.as_ref().filter(|i| !i.modules.is_empty()) {
        let mods = &i.modules;
        boot.push(match format {
            Format::Deb => text(
                format!("/usr/share/initramfs-tools/hooks/{name}"),
                format!(
                    "#!/bin/sh\nPREREQ=\"\"\nprereqs() {{ echo \"$PREREQ\"; }}\ncase \"$1\" in prereqs) prereqs; exit 0 ;; esac\n. /usr/share/initramfs-tools/hook-functions\n{}",
                    mods.iter().map(|m| format!("manual_add_modules {m}\n")).collect::<String>()
                ),
                0o755,
            ),
            Format::Rpm => text(
                format!("/usr/lib/dracut/dracut.conf.d/50-{name}.conf"),
                format!("add_drivers+=\" {} \"\n", mods.join(" ")),
                0o644,
            ),
            _ => text(
                format!("/etc/mkinitcpio.conf.d/{name}.conf"),
                format!("MODULES+=({})\n", mods.join(" ")),
                0o644,
            ),
        });
    }
    if !boot.is_empty() {
        out.push(Package {
            name: format!("{name}-boot"),
            description: format!(
                "{name}: boot glue (kernel-install devicetree, systemd preset, initramfs modules)"
            ),
            depends: Vec::new(),
            items: boot,
        });
    }
    out
}

/// Makes every item a file: tree files are checked to exist, text is written under `stage`.
pub fn materialize(pkg: &Package, manifest_dir: &Path, stage: &Path) -> Result<Vec<Entry>, String> {
    let mut out = Vec::new();
    for it in &pkg.items {
        let src = match &it.source {
            Source::Tree(rel) => {
                let p = manifest_dir.join(rel);
                if !p.is_file() {
                    return Err(format!(
                        "{}: {rel} is not a file under {}",
                        pkg.name,
                        manifest_dir.display()
                    ));
                }
                p
            }
            Source::Text(body) => {
                let p = stage.join(&pkg.name).join(it.dst.trim_start_matches('/'));
                fs::create_dir_all(p.parent().unwrap())
                    .map_err(|e| format!("{}: {e}", p.display()))?;
                fs::write(&p, body).map_err(|e| format!("{}: {e}", p.display()))?;
                p
            }
        };
        out.push(Entry {
            src,
            dst: it.dst.clone(),
            mode: it.mode,
            config: it.config,
        });
    }
    Ok(out)
}

/// A dependency token as the format writes it: `tqftpserv>=1.2` is `tqftpserv (>= 1.2)` in a deb, with spaces in an rpm.
fn dependency(token: &str, format: Format) -> String {
    let at = token.find(['<', '>', '=']);
    let Some(at) = at else {
        return token.trim().to_string();
    };
    let (name, rest) = token.split_at(at);
    let (name, rest) = (name.trim(), rest.trim());
    let op_len = rest.chars().take_while(|c| "<>=".contains(*c)).count();
    let (op, ver) = rest.split_at(op_len);
    let ver = ver.trim();
    match format {
        Format::Deb => format!("{name} ({op} {ver})"),
        Format::Rpm => format!("{name} {op} {ver}"),
        _ => format!("{name}{op}{ver}"),
    }
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn nfpm_yaml(pkg: &Package, format: Format, version: &str, entries: &[Entry]) -> String {
    let mut y = format!(
        "name: {}\narch: all\nplatform: linux\nversion: {}\nsection: misc\nmaintainer: linux-phone-oco\ndescription: {}\nlicense: MIT OR Apache-2.0\n",
        pkg.name,
        quote(version),
        quote(&pkg.description)
    );
    if !pkg.depends.is_empty() {
        y.push_str("depends:\n");
        for d in &pkg.depends {
            y.push_str(&format!("- {}\n", dependency(d, format)));
        }
    }
    y.push_str("contents:\n");
    for e in entries {
        y.push_str(&format!(
            "- src: {}\n  dst: {}\n  file_info:\n    mode: {:04o}\n",
            quote(&e.src.to_string_lossy()),
            quote(&e.dst),
            e.mode
        ));
        if e.config {
            y.push_str("  type: config|noreplace\n");
        }
    }
    y
}
