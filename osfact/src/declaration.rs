// SPDX-License-Identifier: MIT OR Apache-2.0
//! What osfact reads from a device's declaration (device.toml) beyond [boot]: the machine kind of the device's
//! packages (`[device] arch`) and the tables that say how the root image reaches the booted image's RAM
//! (`[install.connection]`, `[install.root]`, `[install.way_back]`). These types are the one definition: `pack` reads
//! them, and a program that validates the rest of the device.toml (lpoco-manifest) keeps them unread and calls
//! `check`; only its JSON Schema takes these types, so editors still complete a key. The schema of `[device]` no
//! longer refuses an unknown key (a later key of osfact's is `check`'s to refuse).

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
    /// Kernel filesystems mounted into the root before the chroot, in this order (none when not declared).
    #[serde(default)]
    pub binds: Vec<crate::install::Bind>,
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

/// The tables of a declaration that are the device's: the lab's own tables are not osfact's and are not read.
#[derive(Debug, Deserialize)]
pub struct Declaration {
    pub device: Identity,
    pub install: Option<InstallTables>,
}

/// Whether the tables osfact owns in a declaration (device.toml) are ones it reads: every key spelled as osfact
/// spells it, every value of its kind. The one reader of these tables - a program that reads the rest of the file
/// keeps them unread and calls this (decision 2026-11-28-osfact-s-tables-in-device-toml-are-read-and).
pub fn check(text: &str) -> Result<Declaration, String> {
    toml::from_str(text).map_err(|e| format!("the declaration: {e}"))
}
