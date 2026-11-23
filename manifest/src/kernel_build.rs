// SPDX-License-Identifier: MIT OR Apache-2.0
//! `[kernel_builds.NAME]`: a kernel file made from an `int/` tag in Nix's sandbox, so the same declaration gives the
//! same bytes on any machine (item image-recipe-kernel-from-int-tag; decision
//! 2026-11-23-image-recipes-on-nix-fixed-output-paths). Everything the build reads is declared: the tag and the
//! commit it points at, the config fragments and the tree edits by sha256, the time the build's clock is fixed to.

use schemars::JsonSchema;
use serde::Deserialize;

/// `[kernel_builds.NAME]`: one kernel file and how it is made.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KernelBuild {
    /// the int/ tag the kernel is built from (made by `lpoco-bringup kernel assemble`)
    pub tag: String,
    /// the commit the tag points at, 40 lowercase hex digits: what Nix fetches (a tag can be moved, a commit cannot)
    pub rev: String,
    /// the git repository holding the tag (the kernel fork when none)
    pub repo: Option<String>,
    /// seconds since 1970 the build's clock is fixed to (KBUILD_BUILD_TIMESTAMP; the tag's own time is a good one)
    pub epoch: u64,
    /// `make` target that writes the default config
    #[serde(default = "defconfig")]
    pub defconfig: String,
    /// config fragments merged after the defconfig, each by the sha256 of its file
    #[serde(default)]
    pub configs: Vec<String>,
    /// unified diffs applied to the tree (`patch -p1`), each by the sha256 of its file: the lab's dts and Makefile line
    #[serde(default)]
    pub patches: Vec<String>,
    /// `make` targets built
    #[serde(default = "targets")]
    pub targets: Vec<String>,
    /// DTC_FLAGS for the build (`-@` keeps the symbols an overlay merge needs)
    pub dtc_flags: Option<String>,
    /// the file the kernel is, under the build folder
    #[serde(default = "output")]
    pub output: String,
    /// lowercase hex sha256 the output must have: Nix refuses a build that is not these bytes
    pub sha256: String,
}

fn defconfig() -> String {
    "defconfig".into()
}
fn targets() -> Vec<String> {
    vec!["Image.gz".into()]
}
fn output() -> String {
    "arch/arm64/boot/Image.gz".into()
}

fn hex(s: &str, len: usize) -> bool {
    s.len() == len && s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

/// A word `make` and a shell take as one argument.
fn word(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | '+'))
}

impl KernelBuild {
    pub(crate) fn validate(&self, name: &str) -> Result<(), String> {
        let at = format!("kernel_builds.{name}");
        if !crate::is_kernel_name(name) {
            return Err(format!(
                "{at}: not a name (lowercase letters, digits and -)"
            ));
        }
        if self.tag.strip_prefix("int/").is_none_or(str::is_empty)
            || self.tag.contains(char::is_whitespace)
        {
            return Err(format!("{at} tag {:?} is not an int/ tag", self.tag));
        }
        if !self
            .tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | '+'))
        {
            return Err(format!(
                "{at} tag {:?} holds a character a git ref name does not (letters, digits and / . _ - +)",
                self.tag
            ));
        }
        if let Some(r) = self.repo.as_ref().filter(|r| {
            !["https://", "file://", "ssh://"]
                .iter()
                .any(|s| r.starts_with(s))
                || r.contains(char::is_whitespace)
        }) {
            return Err(format!(
                "{at} repo {r:?} is not an https://, file:// or ssh:// address"
            ));
        }
        if !hex(&self.rev, 40) {
            return Err(format!(
                "{at} rev {:?} is not a commit (40 lowercase hex digits)",
                self.rev
            ));
        }
        if !hex(&self.sha256, 64) {
            return Err(format!(
                "{at} sha256 {:?} is not 64 lowercase hex digits",
                self.sha256
            ));
        }
        for (what, list) in [("configs", &self.configs), ("patches", &self.patches)] {
            if let Some(v) = list.iter().find(|v| !hex(v, 64)) {
                return Err(format!(
                    "{at} {what}: {v:?} is not the 64 lowercase hex digits of a file's sha256"
                ));
            }
        }
        if !word(&self.defconfig) {
            return Err(format!(
                "{at} defconfig {:?} is not one make target",
                self.defconfig
            ));
        }
        if self.targets.is_empty() {
            return Err(format!("{at} targets is empty: nothing would be built"));
        }
        if let Some(t) = self.targets.iter().find(|t| !word(t)) {
            return Err(format!("{at} target {t:?} is not one make target"));
        }
        if !word(&self.output)
            || self.output.starts_with('/')
            || self.output.split('/').any(|c| c == "..")
        {
            return Err(format!(
                "{at} output {:?} must be a path inside the build folder",
                self.output
            ));
        }
        if let Some(f) = self
            .dtc_flags
            .as_ref()
            .filter(|f| f.contains(['\0', '\n', '\'', '"', '\\', '$', '`']))
        {
            return Err(format!(
                "{at} dtc_flags {f:?} holds a character a shell would act on"
            ));
        }
        Ok(())
    }
}
