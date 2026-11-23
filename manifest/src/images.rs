// SPDX-License-Identifier: MIT OR Apache-2.0
//! `[images]` and `[procedures.NAME]`: the images a procedure puts on the phone, by name, and the values only one
//! procedure uses (decision 2026-11-19-attended-keys-in-the-manifest).

use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::BTreeMap;

/// `[images]`: `dir` is the folder the images live in, relative to the device repo; each `[images.NAME]` is one.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, JsonSchema)]
pub struct Images {
    pub dir: Option<String>,
    #[serde(flatten)]
    pub named: BTreeMap<String, Image>,
}

/// `[images.NAME]`: one image, by its path relative to the device repo and, once known, its checksum.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Image {
    pub path: String,
    /// lowercase hex, 64 digits
    pub sha256: Option<String>,
    /// the int/ tag of the kernel the image was built from (made by `lpoco-bringup kernel assemble`)
    pub kernel_tag: Option<String>,
    /// how the image is made when no store or cache holds it (`lpoco-bringup image ensure`)
    pub recipe: Option<Recipe>,
}

/// `[images.NAME.recipe]`: the image is another image of this manifest (`control`) with parts swapped in, each part
/// named by the sha256 of its file, never by a path on one host (decision 2026-11-23-image-recipes-on-nix-fixed-output-paths).
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    /// the name of the `[images.NAME]` this one is made from; it declares a sha256
    pub control: String,
    /// lowercase hex sha256 of the kernel file
    pub kernel: Option<String>,
    /// lowercase hex sha256 of the device tree file appended to the kernel
    pub dtb: Option<String>,
    /// lowercase hex sha256 of the ramdisk file
    pub ramdisk: Option<String>,
    /// the kernel command line to put in the header
    pub cmdline: Option<String>,
}

fn is_sha256(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

/// One value of `[procedures.NAME]`: a number or a word, nothing nested.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(untagged)]
pub enum ProcValue {
    Int(i64),
    Float(f64),
    Text(String),
}

impl Images {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if let Some(d) = &self.dir {
            relative(&format!("images dir {d:?}"), d)?;
        }
        for (name, i) in &self.named {
            let at = format!("images.{name}");
            if !crate::is_kernel_name(name) {
                return Err(format!(
                    "{at}: not a name (lowercase letters, digits and -)"
                ));
            }
            relative(&format!("{at} path {:?}", i.path), &i.path)?;
            if let Some(t) = &i.kernel_tag {
                if t.strip_prefix("int/").is_none_or(str::is_empty)
                    || t.contains(char::is_whitespace)
                {
                    return Err(format!("{at} kernel_tag {t:?} is not an int/ tag"));
                }
            }
            if let Some(s) = &i.sha256 {
                if !is_sha256(s) {
                    return Err(format!("{at} sha256 {s:?} is not 64 lowercase hex digits"));
                }
            }
            if let Some(r) = &i.recipe {
                if i.sha256.is_none() {
                    return Err(format!(
                        "{at}: a recipe needs the sha256 the image must have"
                    ));
                }
                match self.named.get(&r.control) {
                    Some(c) if c.sha256.is_some() && r.control != *name => {}
                    _ => {
                        return Err(format!(
                            "{at}.recipe control {:?} is not another [images.NAME] with a sha256",
                            r.control
                        ))
                    }
                }
                for (part, v) in [
                    ("kernel", &r.kernel),
                    ("dtb", &r.dtb),
                    ("ramdisk", &r.ramdisk),
                ] {
                    if let Some(v) = v.as_ref().filter(|v| !is_sha256(v)) {
                        return Err(format!(
                            "{at}.recipe {part} {v:?} is not the 64 lowercase hex digits of a file's sha256"
                        ));
                    }
                }
                if let Some(c) = r.cmdline.as_ref().filter(|c| c.contains(['\0', '\n'])) {
                    return Err(format!(
                        "{at}.recipe cmdline {c:?} holds a NUL or a newline"
                    ));
                }
            }
        }
        Ok(())
    }
}

/// A path that stays inside the device repo: not empty, not absolute, no `..`.
fn relative(what: &str, p: &str) -> Result<(), String> {
    let bad = p.is_empty() || p.starts_with('/') || p.split('/').any(|c| c == "..");
    if bad {
        return Err(format!(
            "{what} must be a non-empty path relative to the device repo, never absolute or with .."
        ));
    }
    Ok(())
}

/// The names of `[procedures.NAME]` are table names too.
pub(crate) fn validate_procedures(
    p: &BTreeMap<String, BTreeMap<String, ProcValue>>,
) -> Result<(), String> {
    for (name, values) in p {
        if !crate::is_kernel_name(name) {
            return Err(format!(
                "procedures.{name}: not a name (lowercase letters, digits and -)"
            ));
        }
        if let Some(k) = values.keys().find(|k| k.is_empty() || k.contains('.')) {
            return Err(format!(
                "procedures.{name}: key {k:?} is empty or holds a dot (a procedure reads its keys by name)"
            ));
        }
    }
    Ok(())
}
