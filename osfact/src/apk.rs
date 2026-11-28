// SPDX-License-Identifier: MIT OR Apache-2.0
//! The apk adapter of the package-format interface: apk-tools-static installs into a directory as an ordinary user
//! (--usermode), with no scripts (they need root or an emulator), checked against the distro's keys.
//! The source of every argument is named in tests/rootfs_apk.rs (its header).

use crate::distro::Profile;
use crate::pkgformat::{Fetch, PackageFormat, Tools};
use std::path::Path;

pub struct Apk;

const TOOLS_PACKAGE: &str = "apk-tools-static";

impl PackageFormat for Apk {
    fn fetches(&self, profile: &Profile, host_arch: &str) -> Result<Vec<Fetch>, String> {
        let keys = profile.distro.keys_package.as_deref().ok_or(
            "an apk profile names the package of its signing keys: keys_package = \"...\"",
        )?;
        let base = format!(
            "{}/v{}/{}/{host_arch}",
            profile.distro.mirror,
            profile.distro.release,
            first_repository(profile)?
        );
        [(TOOLS_PACKAGE, "tools"), (keys, "keys-pkg")]
            .into_iter()
            .map(|(package, into)| {
                let pin = profile.pinned(package)?;
                let file = format!("{package}-{}.apk", pin.version);
                Ok(Fetch {
                    package: package.to_string(),
                    url: format!("{base}/{file}"),
                    file,
                    into,
                })
            })
            .collect()
    }

    fn tools(&self, work: &Path) -> Tools {
        Tools {
            installer: work.join("tools/sbin/apk.static"),
            keys: work.join("keys-pkg/usr/share/apk/keys"),
        }
    }

    fn root_tree(&self, p: &Profile, tools: &Tools, tree: &Path, arch: &str) -> Vec<String> {
        let d = &p.distro;
        let mut cmd: Vec<String> = [
            &*tools.installer.to_string_lossy(),
            "--usermode",
            "--root",
            &tree.to_string_lossy(),
            "--keys-dir",
            &tools.keys.to_string_lossy(),
            "--initdb",
            "--no-scripts",
            "--no-cache",
            "--no-interactive",
            "--arch",
            arch,
        ]
        .map(String::from)
        .to_vec();
        for repo in &d.repositories {
            cmd.push("-X".into());
            cmd.push(format!("{}/v{}/{repo}", d.mirror, d.release));
        }
        cmd.push("add".into());
        cmd.extend(d.packages.iter().cloned());
        cmd
    }

    fn owners_and_modes(&self, tree: &Path) -> Result<Vec<Vec<String>>, String> {
        let db = std::fs::read_to_string(tree.join("lib/apk/db/installed"))
            .map_err(|e| format!("the apk database of the tree: {e}"))?;
        let present: Vec<(String, u32, u32, u32)> = database_files(&db)?
            .into_iter()
            .filter(|(p, ..)| {
                std::fs::symlink_metadata(tree.join(p)).is_ok_and(|m| !m.file_type().is_symlink())
            })
            .collect();
        Ok(owner_commands(tree, &present))
    }

    fn build_records(&self) -> &'static [&'static str] {
        // apk's log names the build's own directory and times: a build record, not part of the system
        &["var/log/apk.log"]
    }
}

/// Every path of apk's installed database with the owner and mode apk records, octal: a file's from its `a:UID:GID:MODE`
/// line (after `R:`), a directory's from its `M:` line (after `F:`).
pub fn database_files(db: &str) -> Result<Vec<(String, u32, u32, u32)>, String> {
    let mut out = Vec::new();
    let mut dir = String::new();
    let mut target: Option<String> = None;
    for line in db.lines() {
        if let Some(d) = line.strip_prefix("F:") {
            dir = d.to_string();
            target = Some(dir.clone());
        } else if let Some(f) = line.strip_prefix("R:") {
            target = Some(if dir.is_empty() {
                f.to_string()
            } else {
                format!("{dir}/{f}")
            });
        } else if line.starts_with("P:") {
            dir.clear();
            target = None;
        } else if let Some(a) = line.strip_prefix("a:").or_else(|| line.strip_prefix("M:")) {
            let Some(t) = target.clone() else { continue };
            let w: Vec<&str> = a.split(':').collect();
            let parsed = match &w[..] {
                [u, g, m] => (u.parse(), g.parse(), u32::from_str_radix(m, 8)),
                _ => {
                    return Err(format!(
                        "apk database line `{line}` for {t} is not UID:GID:MODE"
                    ))
                }
            };
            let (Ok(u), Ok(g), Ok(m)) = parsed else {
                return Err(format!("apk database line `{line}` for {t} is not numeric"));
            };
            out.push((t, u, g, m));
        }
    }
    Ok(out)
}

/// One chown and one chmod per (owner, mode) group, 200 files to a command.
pub fn owner_commands(tree: &Path, files: &[(String, u32, u32, u32)]) -> Vec<Vec<String>> {
    let mut groups: std::collections::BTreeMap<(u32, u32, u32), Vec<String>> = Default::default();
    for (p, u, g, m) in files {
        groups
            .entry((*u, *g, *m))
            .or_default()
            .push(tree.join(p).to_string_lossy().into_owned());
    }
    let mut out = Vec::new();
    for ((uid, gid, mode), paths) in groups {
        for chunk in paths.chunks(200) {
            for (program, first, extra) in [
                ("chown", format!("{uid}:{gid}"), Some("-h")),
                ("chmod", format!("{mode:o}"), None),
            ] {
                let mut cmd = vec![program.to_string()];
                cmd.extend(extra.map(String::from));
                cmd.push(first);
                cmd.extend(chunk.iter().cloned());
                out.push(cmd);
            }
        }
    }
    out
}

fn first_repository(p: &Profile) -> Result<&str, String> {
    p.distro
        .repositories
        .first()
        .map(String::as_str)
        .ok_or_else(|| "the profile lists no repository".to_string())
}
