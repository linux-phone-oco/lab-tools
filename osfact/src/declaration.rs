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
    /// Seconds the check may take to hold, tried every second (10 when not declared).
    pub check_wait: Option<u32>,
}

/// How the booted image restarts the device when a step fails or the run ends.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WayBackCommand {
    pub command: String,
    /// The host command, as argv (`{serial}` is the device's serial), that ends with status 0 once the device is on its
    /// known good again: the way back is confirmed by it. Needed: a restart that is only sent is never known to have worked.
    pub back_on: Option<Vec<String>>,
}

/// The device's own keys of `[device]`: its name, the machine kind of its packages and the variable that holds its
/// serial. A key osfact does not know is refused, naming it.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    /// The device's codename, as its package manager names it (vendor-model)
    pub codename: String,
    /// The machine kind of the device's packages (osfact reads it): aarch64, armv7, armhf, x86_64, x86 or riscv64
    pub arch: Option<Arch>,
    /// The environment variable that holds the device's fastboot serial; by default the codename's words and SERIAL
    /// (the codename vendor-model gives VENDOR_MODEL_SERIAL)
    pub serial_env: Option<String>,
}
