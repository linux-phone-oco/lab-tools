// SPDX-License-Identifier: MIT OR Apache-2.0
//! The package-format interface: what a format must say for osfact to make a root tree from a profile without
//! root. A format is a name in a profile (`format = "apk"`); its adapter lives in its own module (`apk`), so a
//! second format (deb) is one more adapter and one more line in `for_format`, and nothing else changes.

use crate::distro::Profile;
use std::path::{Path, PathBuf};

/// One download the format's tools come from: a pinned package, unpacked into `WORK/into`.
pub struct Fetch {
    pub package: String,
    pub file: String,
    pub url: String,
    pub into: &'static str,
}

/// The tools a root tree is made with, once their downloads are unpacked under WORK.
pub struct Tools {
    pub installer: PathBuf,
    pub keys: PathBuf,
}

pub trait PackageFormat {
    /// The downloads (each must be pinned in the profile), for a host of the given machine kind.
    fn fetches(&self, profile: &Profile, host_arch: &str) -> Result<Vec<Fetch>, String>;
    /// Where the tools are after the downloads were unpacked under WORK.
    fn tools(&self, work: &Path) -> Tools;
    /// The command (program first) that installs the profile's packages into an empty TREE, as an ordinary user.
    fn root_tree(&self, profile: &Profile, tools: &Tools, tree: &Path, arch: &str) -> Vec<String>;
    /// The owners and modes the install could not apply as an ordinary user (a setuid file came out 0111), read from
    /// the format's own database in the tree: commands (program first) to run under fakeroot, in order.
    fn owners_and_modes(&self, tree: &Path) -> Result<Vec<Vec<String>>, String>;
    /// Files the install leaves that name the build, not the system: removed from the tree, relative to it.
    fn build_records(&self) -> &'static [&'static str];
}

pub fn for_format(format: &str) -> Result<Box<dyn PackageFormat>, String> {
    match format {
        "apk" => Ok(Box::new(crate::apk::Apk)),
        other => Err(format!(
            "osfact knows no package format '{other}' (known: apk)"
        )),
    }
}
