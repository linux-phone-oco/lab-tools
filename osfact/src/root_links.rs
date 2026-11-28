// SPDX-License-Identifier: MIT OR Apache-2.0
//! What a root made without running the packages' install scripts still needs: the links such a script would make
//! (the root is built on one machine kind for another, so no script of it can run). The profile names each list a
//! package ships (`[[link_list]]`: the list's path in the root and the target every listed path links to) and the
//! paths the root must have (`required_paths`); this module only reads, links and checks. Source of the first
//! list's shape and target: tests/root_links.rs (its header).

use serde::Deserialize;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

/// One list of paths a package ships, each becoming a link to `target`.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct LinkList {
    /// The list's path in the root, relative to it; one path per line, relative to the root.
    pub list: String,
    /// What every listed path links to, an absolute path in the root.
    pub target: String,
}

/// Links every path of the list to the target, leaving a path that already exists; returns how many it made.
pub fn apply(tree: &Path, spec: &LinkList) -> Result<usize, String> {
    let list = tree.join(&spec.list);
    let text = fs::read_to_string(&list)
        .map_err(|e| format!("the link list {} of the root: {e}", spec.list))?;
    let mut made = 0;
    for rel in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let at = tree.join(rel);
        if fs::symlink_metadata(&at).is_ok() {
            continue;
        }
        if let Some(parent) = at.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        symlink(&spec.target, &at).map_err(|e| format!("{}: {e}", at.display()))?;
        made += 1;
    }
    Ok(made)
}

/// Every required path exists in the root (a link counts, whatever it points to); the first one missing is named.
pub fn require(tree: &Path, paths: &[String]) -> Result<(), String> {
    for rel in paths {
        if fs::symlink_metadata(tree.join(rel)).is_err() {
            return Err(format!(
                "the root lacks /{}: the profile requires it",
                rel.trim_start_matches('/')
            ));
        }
    }
    Ok(())
}
