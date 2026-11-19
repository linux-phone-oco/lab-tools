// SPDX-License-Identifier: MIT OR Apache-2.0
//! The device repo the procedures belong to: the folder of its device.toml, its codename, its procedures/.

use lpoco_manifest::Manifest;
use std::path::{Path, PathBuf};

const FILE: &str = "device.toml";

pub struct Repo {
    pub manifest: PathBuf,
    pub root: PathBuf,
    pub codename: String,
}

/// The repo of `--device FILE`, else of the first device.toml from the current folder upwards.
pub fn find(given: Option<&str>) -> Result<Repo, String> {
    let manifest = match given {
        Some(p) => PathBuf::from(p),
        None => {
            let here = std::env::current_dir().map_err(|e| format!("no current folder: {e}"))?;
            here.ancestors()
                .map(|d| d.join(FILE))
                .find(|f| f.is_file())
                .ok_or_else(|| {
                    format!(
                        "no {FILE} here or above {} (or pass --device FILE): a device repo's procedures are named for its device",
                        here.display()
                    )
                })?
        }
    };
    let manifest =
        std::path::absolute(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let text = std::fs::read_to_string(&manifest)
        .map_err(|e| format!("cannot read {}: {e}", manifest.display()))?;
    let m = Manifest::parse(&text).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let codename = m
        .device
        .map(|d| d.codename)
        .ok_or_else(|| format!("{} has no [device] codename", manifest.display()))?;
    valid_name(&codename).map_err(|_| {
        format!(
            "the codename {codename:?} in {} is not one word of lowercase letters, digits and hyphens; a procedure file would not be safe to write",
            manifest.display()
        )
    })?;
    let root = manifest
        .parent()
        .expect("a file has a folder")
        .to_path_buf();
    Ok(Repo {
        manifest,
        root,
        codename,
    })
}

/// A procedure's name is a file name: lowercase words, digits and hyphens, nothing that climbs or hides.
pub fn valid_name(name: &str) -> Result<(), String> {
    let ok = !name.is_empty()
        && name != "steps"
        && !name.ends_with('-')
        && !name.contains("--")
        && name.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if ok {
        Ok(())
    } else {
        Err(format!(
            "the procedure name {name:?} is not a name: lowercase letters, digits and hyphens, as in calls or boot-pmicwdt"
        ))
    }
}

impl Repo {
    fn dir(&self) -> PathBuf {
        self.root.join("procedures")
    }

    pub fn procedure(&self, name: &str) -> PathBuf {
        self.dir().join(format!("{name}.rs"))
    }

    /// The procedure's file, or an error naming where it looked and what is there.
    pub fn existing(&self, name: &str) -> Result<PathBuf, String> {
        let f = self.procedure(name);
        if f.is_file() {
            return Ok(f);
        }
        let have = self.procedures().unwrap_or_default();
        Err(format!(
            "no procedures/{name}.rs in {} (procedures there: {})",
            self.root.display(),
            if have.is_empty() {
                "none".into()
            } else {
                have.join(", ")
            }
        ))
    }

    /// Every procedure, by name, in order.
    pub fn procedures(&self) -> Result<Vec<String>, String> {
        let dir = self.dir();
        let rd = std::fs::read_dir(&dir).map_err(|_| {
            format!(
                "no procedures/ in {} (lpoco attended new NAME makes the first)",
                self.root.display()
            )
        })?;
        let mut names: Vec<String> = rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "rs"))
            .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .filter(|n| valid_name(n).is_ok())
            .collect();
        names.sort();
        if names.is_empty() {
            return Err(format!(
                "procedures/ in {} holds no procedure",
                self.root.display()
            ));
        }
        Ok(names)
    }

    /// The package name of the repo's own steps crate, when procedures/steps is one.
    pub fn steps_crate(&self) -> Result<Option<String>, String> {
        let manifest = self.dir().join("steps/Cargo.toml");
        if !manifest.is_file() {
            return Ok(None);
        }
        package_name(&manifest).map(Some)
    }
}

fn package_name(manifest: &Path) -> Result<String, String> {
    let text = std::fs::read_to_string(manifest)
        .map_err(|e| format!("cannot read {}: {e}", manifest.display()))?;
    let v: toml::Table = text
        .parse()
        .map_err(|e| format!("{}: {e}", manifest.display()))?;
    v.get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .map(str::to_string)
        .ok_or_else(|| format!("{} has no [package] name", manifest.display()))
}
