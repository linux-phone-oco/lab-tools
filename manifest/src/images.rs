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
            if let Some(s) = &i.sha256 {
                let ok = s.len() == 64 && s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'));
                if !ok {
                    return Err(format!("{at} sha256 {s:?} is not 64 lowercase hex digits"));
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
