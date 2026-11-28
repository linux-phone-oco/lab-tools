// SPDX-License-Identifier: MIT OR Apache-2.0
//! A distro profile: the data that says which package format a base uses, where its packages are and which ones
//! the image holds. osfact's code names a format (`apk`), never a distro: the distro is this file.

use crate::root_links::LinkList;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub distro: Distro,
    /// Downloads the format's tools come from, each checked by its sha256 before it is unpacked.
    #[serde(default)]
    pub pin: Vec<Pin>,
    /// Lists of paths a package ships that its install script would link, which a root built without running scripts
    /// must link itself.
    #[serde(default)]
    pub link_list: Vec<LinkList>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Distro {
    pub name: String,
    /// The package format: the one word osfact's code knows (`apk`).
    pub format: String,
    pub release: String,
    pub mirror: String,
    pub repositories: Vec<String>,
    pub init: String,
    pub packages: Vec<String>,
    /// The package that carries the distro's signing keys, for formats that check packages against keys.
    pub keys_package: Option<String>,
    /// Paths the finished root must have (relative to it), else the build stops naming the first one missing.
    #[serde(default)]
    pub required_paths: Vec<String>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Pin {
    pub name: String,
    pub version: String,
    pub sha256: String,
}

impl Profile {
    pub fn pinned(&self, name: &str) -> Result<&Pin, String> {
        self.pin
            .iter()
            .find(|p| p.name == name)
            .ok_or_else(|| format!("the profile pins no download for {name}"))
    }
}

pub fn read(text: &str) -> Result<Profile, String> {
    toml::from_str(text).map_err(|e| format!("osfact refuses this distro profile: {e}"))
}
