// SPDX-License-Identifier: MIT OR Apache-2.0
//! What osfact reads from a device's declaration (device.toml) beyond [boot]: the machine kind of the device's
//! packages (`[device] arch`) and the tables that say how the root image reaches the booted image's RAM
//! (`[install.connection]`, `[install.root]`, `[install.way_back]`). These types are the one definition: `pack` reads
//! them, and a program that validates the whole device.toml (lpoco-manifest) takes them from here, so a key is
//! spelled once. They carry a JSON Schema for that program's schema.

use crate::install::Connection;
use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::BTreeMap;

/// The machine kind of a device's packages, as the package managers name it: never the host's.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, JsonSchema)]
pub enum Arch {
    #[serde(rename = "aarch64")]
    Aarch64,
    #[serde(rename = "armv7")]
    Armv7,
    #[serde(rename = "armhf")]
    Armhf,
    #[serde(rename = "x86_64")]
    X86_64,
    #[serde(rename = "x86")]
    X86,
    #[serde(rename = "riscv64")]
    Riscv64,
}

impl Arch {
    /// The word the declaration and the package managers use.
    pub fn as_str(self) -> &'static str {
        match self {
            Arch::Aarch64 => "aarch64",
            Arch::Armv7 => "armv7",
            Arch::Armhf => "armhf",
            Arch::X86_64 => "x86_64",
            Arch::X86 => "x86",
            Arch::Riscv64 => "riscv64",
        }
    }
}

/// How the packed root image reaches the booted image's RAM and is entered (nothing is written to the device).
/// A key osfact does not know is refused, naming it.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InstallTables {
    /// How the host reaches the booted image: a root shell with no login, its address and port.
    pub connection: Option<Connection>,
    pub root: Option<Root>,
    pub way_back: Option<WayBackCommand>,
    /// The steps that write an image to the device: declared by item osfact-device-declaration, not read here yet.
    #[serde(default)]
    #[schemars(with = "Vec<BTreeMap<String, String>>")]
    pub step: Vec<toml::Value>,
}

/// The push of the root image and its entry.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Root {
    /// The device's port the receiver listens on.
    pub port: u16,
    /// The receiver's command, on the device.
    pub receive: String,
    /// Where the receiver writes the compressed root image.
    pub to: String,
    /// Seconds the whole push may take.
    pub wait: u32,
    /// The folder the root is unpacked into (a tmpfs).
    pub path: String,
    pub size_mib: u64,
    /// Started in the chroot.
    pub command: String,
    /// Must succeed once the command is started.
    pub check: Option<String>,
}

/// How the booted image restarts the device when a step fails or the run ends.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WayBackCommand {
    pub command: String,
}
