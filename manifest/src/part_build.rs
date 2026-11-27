// SPDX-License-Identifier: MIT OR Apache-2.0
//! `[part_builds.NAME]`: a static aarch64 binary made from a crate at a git commit in Nix's sandbox, so the same
//! declaration gives the same bytes on any machine (item recipe-static-binary-part; decision
//! 2026-11-23-image-recipes-on-nix-fixed-output-paths). Everything the build reads is declared: the repository and
//! the commit, the crate's folder in it, the binary's name; the crate's own `Cargo.lock` pins its dependencies.
//! The target is always `aarch64-unknown-linux-gnu`, linked static (`+crt-static`) so the file runs in a ramdisk
//! with no libraries; the std library is built from the pinned toolchain's own source.

use schemars::JsonSchema;
use serde::Deserialize;

/// `[part_builds.NAME]`: one binary and how it is made.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PartBuild {
    /// the git repository holding the crate (an https://, file:// or ssh:// address)
    pub repo: String,
    /// the commit the crate is read at, 40 lowercase hex digits (a tag can be moved, a commit cannot)
    pub rev: String,
    /// the crate's folder in the repository, holding its Cargo.toml and Cargo.lock (the repository's root when none)
    #[serde(rename = "crate", default = "root")]
    pub krate: String,
    /// the name of the binary the crate builds
    pub bin: String,
    /// lowercase hex sha256 the binary must have: Nix refuses a build that is not these bytes
    pub sha256: String,
}

fn root() -> String {
    ".".into()
}

fn hex(s: &str, len: usize) -> bool {
    s.len() == len && s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

/// A word a shell and cargo take as one argument.
fn word(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'))
}

impl PartBuild {
    pub(crate) fn validate(&self, name: &str) -> Result<(), String> {
        let at = format!("part_builds.{name}");
        if !crate::is_kernel_name(name) {
            return Err(format!(
                "{at}: not a name (lowercase letters, digits and -)"
            ));
        }
        if !["https://", "file://", "ssh://"]
            .iter()
            .any(|s| self.repo.starts_with(s))
            || self.repo.contains(char::is_whitespace)
        {
            return Err(format!(
                "{at} repo {:?} is not an https://, file:// or ssh:// address",
                self.repo
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
        let inside = self.krate == "."
            || (!self.krate.starts_with('/')
                && self
                    .krate
                    .split('/')
                    .all(|c| word(c) && c != "." && c != ".."));
        if !inside {
            return Err(format!(
                "{at} crate {:?} must be a folder inside the repository (. for its root), never absolute or with ..",
                self.krate
            ));
        }
        if !word(&self.bin) || self.bin == "." || self.bin == ".." {
            return Err(format!(
                "{at} bin {:?} is not one binary name (letters, digits and . _ - +)",
                self.bin
            ));
        }
        Ok(())
    }
}
