// SPDX-License-Identifier: MIT OR Apache-2.0
//! The `[layer]` table of a device's manifest (device.toml): the phone's device layer - kernel, dtbs, firmware,
//! UCM, config and boot glue - declared once, so each package format is generated from it, not rewritten by hand
//! (decision 2026-11-19-device-layer-declared-once). Every table is declared in lib.rs.

use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    /// the packages' stem: NAME-config, NAME-boot, linux-NAME
    pub name: String,
    /// dependency tokens ("qrtr", "tqftpserv>=1.2"); a token map names each per base
    #[serde(default)]
    pub needs: Vec<String>,
    /// the token map: per format, the package name a token has there when it is not the token itself
    #[serde(default)]
    pub names: LayerNames,
    pub kernel: Option<LayerKernel>,
    pub boot: Option<LayerBoot>,
    pub firmware: Option<LayerFirmware>,
    pub initramfs: Option<LayerInitramfs>,
    /// the files the layer installs, each with its kind and a destination under /usr
    #[serde(default)]
    pub file: Vec<LayerFile>,
}

/// `[layer.names.FORMAT]`: token = "that format's package name"; a token not listed keeps its own name.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayerNames {
    #[serde(default)]
    pub apk: BTreeMap<String, String>,
    #[serde(default)]
    pub deb: BTreeMap<String, String>,
    #[serde(default)]
    pub rpm: BTreeMap<String, String>,
    #[serde(default)]
    pub archlinux: BTreeMap<String, String>,
}

impl LayerNames {
    pub fn of(&self, format: &str) -> Option<&BTreeMap<String, String>> {
        Some(match format {
            "apk" => &self.apk,
            "deb" => &self.deb,
            "rpm" => &self.rpm,
            "archlinux" => &self.archlinux,
            _ => return None,
        })
    }
}

/// `[layer.kernel]`: where the kernel comes from and what it builds.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayerKernel {
    /// the git tree
    pub tree: String,
    /// the tag built
    pub tag: String,
    /// a config fragment over the architecture's defconfig, relative to the manifest
    pub config: Option<String>,
    /// device trees installed, relative to the kernel's dts directory
    #[serde(default)]
    pub dtbs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayerBoot {
    pub method: BootMethod,
}

/// How the phone's kernel is started; `android-bootimg` takes its addresses from `[boot]`.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum BootMethod {
    EfiKernelInstall,
    AndroidBootimg,
}

#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayerFirmware {
    pub method: FirmwareMethod,
    /// The device tree `compatible` string the extractor (droid-juicer) names its per-device config after
    /// (`xiaomi,surya`); needed with sources.
    #[serde(default)]
    pub compatible: Option<String>,
    /// What is read from which partition of the phone: the extractor copies these files into the firmware folder.
    /// Only names: no vendor file is ever in a repository or a package.
    #[serde(default)]
    pub source: Vec<FirmwareSource>,
}

/// `[[layer.firmware.source]]`: files of one folder of one vendor partition.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FirmwareSource {
    /// the partition's name under /dev/disk/by-partlabel
    pub partition: String,
    /// the folder on it, relative
    pub origin: String,
    /// the folder under the firmware directory the files are copied to, relative
    pub destination: String,
    pub files: Vec<FirmwareFile>,
}

/// One file to extract: its name on the partition, and the name it is given when that differs.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FirmwareFile {
    pub name: String,
    #[serde(default)]
    pub rename: Option<String>,
}

/// Where firmware comes from: the OEM's is never packaged, only extracted from the phone's own partitions.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum FirmwareMethod {
    ExtractFromPartitions,
    Files,
}

/// `[layer.initramfs]`: one module list, a drop-in per initramfs tool.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayerInitramfs {
    #[serde(default)]
    pub modules: Vec<String>,
}

/// `[[layer.file]]`
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayerFile {
    /// relative to the manifest
    pub from: String,
    /// absolute, under /usr, no `..`
    pub to: String,
    pub kind: FileKind,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    Data,
    Config,
    Unit,
    Script,
}

impl Layer {
    pub fn validate(&self) -> Result<(), String> {
        let ok_name = !self.name.is_empty()
            && self
                .name
                .starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
            && self
                .name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "+._-".contains(c));
        if !ok_name {
            return Err(format!(
                "layer name {:?} is not a package name ([a-z0-9][a-z0-9+._-]*)",
                self.name
            ));
        }
        if self.needs.iter().any(|n| n.trim().is_empty()) {
            return Err("layer needs holds an empty token".into());
        }
        let wanted: Vec<&str> = self
            .needs
            .iter()
            .map(|n| n.split(['<', '>', '=']).next().unwrap_or("").trim())
            .collect();
        for f in ["apk", "deb", "rpm", "archlinux"] {
            for (token, name) in self.names.of(f).into_iter().flatten() {
                if !wanted.contains(&token.as_str()) {
                    return Err(format!(
                        "layer names.{f} maps {token:?}, which layer needs does not hold"
                    ));
                }
                if name.is_empty()
                    || name.contains(char::is_whitespace)
                    || name.contains(['<', '>', '='])
                {
                    return Err(format!(
                        "layer names.{f} maps {token:?} to {name:?}, which is not one package name"
                    ));
                }
            }
        }
        if let Some(i) = &self.initramfs {
            if i.modules.iter().any(|m| m.trim().is_empty()) {
                return Err("layer initramfs modules holds an empty name".into());
            }
        }
        let relative = |what: &str, p: &str| -> Result<(), String> {
            if p.is_empty() || p.starts_with('/') || p.split('/').any(|s| s == ".." || s.is_empty())
            {
                return Err(format!(
                    "layer {what} {p:?} is not a plain path relative to the manifest"
                ));
            }
            Ok(())
        };
        if let Some(k) = &self.kernel {
            if let Some(c) = &k.config {
                relative("kernel config", c)?;
            }
            for d in &k.dtbs {
                relative("kernel dtb", d)?;
            }
        }
        if let Some(fw) = &self.firmware {
            fw.validate()?;
        }
        for (i, f) in self.file.iter().enumerate() {
            relative("file from", &f.from)?;
            // the OEM's firmware is extracted from the phone, never packaged: no layer file lands in the firmware folder
            if f.to.starts_with("/usr/lib/firmware") {
                return Err(format!(
                    "layer file {}: destination {} is in the firmware folder; vendor firmware is never packaged, name its partition in [layer.firmware] instead",
                    f.from, f.to
                ));
            }
            let rest = f.to.strip_prefix("/usr/").unwrap_or("");
            if rest.is_empty()
                || rest
                    .split('/')
                    .any(|s| s.is_empty() || s == "." || s == "..")
            {
                return Err(format!(
                    "layer file {}: destination {} is not a plain path under /usr",
                    f.from, f.to
                ));
            }
            if self.file[..i].iter().any(|o| o.to == f.to) {
                return Err(format!("layer file destination {} is declared twice", f.to));
            }
        }
        Ok(())
    }
}

impl LayerFirmware {
    /// What goes into the extractor's TOML: one plain word or relative path each, never a quote or a control
    /// character, and a name for the config when sources are declared.
    fn validate(&self) -> Result<(), String> {
        let word = |what: &str, w: &str| -> Result<(), String> {
            if w.is_empty()
                || !w
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_.-+".contains(c))
            {
                return Err(format!(
                    "layer firmware {what} {w:?} is not one plain word (letters, digits and _ . - +)"
                ));
            }
            Ok(())
        };
        let path = |what: &str, p: &str| -> Result<(), String> {
            if p.is_empty()
                || p.starts_with('/')
                || p.split('/').any(|s| s.is_empty() || s == "." || s == "..")
                || !p
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_.-+/,".contains(c))
            {
                return Err(format!(
                    "layer firmware {what} {p:?} is not a plain relative path (letters, digits and _ . - + , /)"
                ));
            }
            Ok(())
        };
        if self.source.is_empty() {
            return Ok(());
        }
        if self.method != FirmwareMethod::ExtractFromPartitions {
            return Err(
                "layer firmware sources are for method = \"extract-from-partitions\"".into(),
            );
        }
        let Some(c) = &self.compatible else {
            return Err("layer firmware sources need `compatible` (the device tree string the extractor names its config after)".into());
        };
        // `vendor,device`: a file name under /usr/share/droid-juicer/configs
        if c.is_empty()
            || !c
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || "_.-+,".contains(ch))
        {
            return Err(format!(
                "layer firmware compatible {c:?} is not a device tree compatible string"
            ));
        }
        for s in &self.source {
            word("partition", &s.partition)?;
            path("origin", &s.origin)?;
            path("destination", &s.destination)?;
            if s.files.is_empty() {
                return Err(format!(
                    "layer firmware source {} names no file",
                    s.partition
                ));
            }
            for f in &s.files {
                word("file name", &f.name)?;
                if let Some(r) = &f.rename {
                    word("rename", r)?;
                }
            }
        }
        Ok(())
    }
}
