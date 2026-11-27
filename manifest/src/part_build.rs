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
    /// the git repository holding the crate (an https://, file:// or ssh:// address); with `rev`, the crate form
    pub repo: Option<String>,
    /// the commit the crate is read at, 40 lowercase hex digits (a tag can be moved, a commit cannot)
    pub rev: Option<String>,
    /// the crate's folder in the repository, holding its Cargo.toml and Cargo.lock (the repository's root when none)
    #[serde(rename = "crate")]
    pub krate: Option<String>,
    /// the nixpkgs package (one attribute of the pinned nixpkgs' static aarch64 set) whose binary this is: the form
    /// for a program that is not a crate, such as busybox; instead of `repo`, `rev` and `crate`
    pub nixpkgs: Option<String>,
    /// the name of the binary the crate builds, or the file the package leaves in its bin/
    pub bin: String,
    /// lowercase hex sha256 the binary must have: Nix refuses a build that is not these bytes
    pub sha256: String,
}

/// Where a part's bytes come from: the one source its declaration names.
#[derive(Debug, PartialEq)]
pub enum PartSource<'a> {
    /// a crate at a commit of a git repository (`crate` is the folder in it, "." for its root)
    Crate {
        repo: &'a str,
        rev: &'a str,
        krate: &'a str,
    },
    /// a package of the pinned nixpkgs
    Nixpkgs(&'a str),
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
    /// The source this declaration names; `validate` has already refused one that names two or none.
    pub fn source(&self) -> PartSource<'_> {
        match (&self.nixpkgs, &self.repo, &self.rev) {
            (Some(n), _, _) => PartSource::Nixpkgs(n),
            (None, Some(repo), Some(rev)) => PartSource::Crate {
                repo,
                rev,
                krate: self.krate.as_deref().unwrap_or("."),
            },
            _ => unreachable!("validate refuses a part with no source"),
        }
    }

    pub(crate) fn validate(&self, name: &str) -> Result<(), String> {
        let at = format!("part_builds.{name}");
        if !crate::is_kernel_name(name) {
            return Err(format!(
                "{at}: not a name (lowercase letters, digits and -)"
            ));
        }
        if !hex(&self.sha256, 64) {
            return Err(format!(
                "{at} sha256 {:?} is not 64 lowercase hex digits",
                self.sha256
            ));
        }
        if !word(&self.bin) || self.bin == "." || self.bin == ".." {
            return Err(format!(
                "{at} bin {:?} is not one binary name (letters, digits and . _ - +)",
                self.bin
            ));
        }
        match (&self.nixpkgs, &self.repo, &self.rev, &self.krate) {
            (Some(n), None, None, None) => {
                if !word(n) || n.contains('.') {
                    return Err(format!(
                        "{at} nixpkgs {n:?} is not one package attribute (letters, digits and _ - +)"
                    ));
                }
                Ok(())
            }
            (Some(_), ..) => Err(format!(
                "{at}: nixpkgs is a source of its own; repo, rev and crate beside it would be a second one"
            )),
            (None, Some(repo), Some(rev), krate) => {
                if !["https://", "file://", "ssh://"]
                    .iter()
                    .any(|s| repo.starts_with(s))
                    || repo.contains(char::is_whitespace)
                {
                    return Err(format!(
                        "{at} repo {repo:?} is not an https://, file:// or ssh:// address"
                    ));
                }
                if !hex(rev, 40) {
                    return Err(format!(
                        "{at} rev {rev:?} is not a commit (40 lowercase hex digits)"
                    ));
                }
                let krate = krate.as_deref().unwrap_or(".");
                let inside = krate == "."
                    || (!krate.starts_with('/')
                        && krate.split('/').all(|c| word(c) && c != "." && c != ".."));
                if !inside {
                    return Err(format!(
                        "{at} crate {krate:?} must be a folder inside the repository (. for its root), never absolute or with .."
                    ));
                }
                Ok(())
            }
            _ => Err(format!(
                "{at}: names no source (nixpkgs, or repo with rev)"
            )),
        }
    }
}
