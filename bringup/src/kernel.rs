// SPDX-License-Identifier: MIT OR Apache-2.0
//! The kernel of a phone from its named list of topics (decisions 2026-11-23-structure-kernel-topics-and-integration
//! and 2026-11-23-structure-git-flows-run-on-rhizoid): the record checks lab-tools owns run first, then
//! `rhizoid assemble` merges the list into an int/ tag; once an image names that tag it is pushed and rhizoid's
//! worktree ended. The merging, rerere and tagging are rhizoid's, never done here.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

/// The tags a carried item's state gives, from `tad q carried --format csv` (id first, tag fifth).
pub fn carried(csv: &str) -> BTreeMap<String, String> {
    csv.lines()
        .skip(1)
        .filter_map(|l| {
            let mut f = l.splitn(6, ',');
            let (id, tag) = (f.next()?, f.nth(3)?);
            Some((id.to_string(), tag.to_string()))
        })
        .collect()
}

/// The upstream item a topic branch carries: `topic/ITEM` or `backport/ITEM/BASE`.
pub fn item_of(branch: &str) -> Option<&str> {
    let mut p = branch.split('/');
    match (p.next()?, p.next()?) {
        ("topic", item) | ("backport", item) if !item.is_empty() => Some(item),
        _ => None,
    }
}

/// What a carried commit's subject says: `TAG(item): ...`.
fn tagged(subject: &str) -> Option<(&str, &str)> {
    let (tag, rest) = subject.split_once('(')?;
    let (item, after) = rest.split_once(')')?;
    let known = ["UPSTREAM", "BACKPORT", "FROMGIT", "FROMLIST", "OURS"].contains(&tag);
    (known && after.starts_with(':')).then_some((tag, item))
}

/// The refusals for one commit subject against the tags the base's items give.
pub fn judge(hash: &str, subject: &str, carried: &BTreeMap<String, String>) -> Option<String> {
    let Some((tag, item)) = tagged(subject) else {
        return Some(format!("{hash}: no tag: {subject}"));
    };
    let Some(want) = carried.get(item) else {
        return Some(format!(
            "{hash}: {tag}({item}) names an item this base does not carry (tad q carried): {subject}"
        ));
    };
    let backport_ok = tag == "BACKPORT" && matches!(want.as_str(), "UPSTREAM" | "FROMGIT");
    (tag != want && !backport_ok)
        .then(|| format!("{hash}: {tag}({item}) but the item says {want}: {subject}"))
}

/// The branches of one list, with the base: from the lists file in rhizoid's format.
pub fn entries(lists_text: &str, list: &str) -> Result<(String, Vec<String>), String> {
    let t: toml::Table = lists_text
        .parse()
        .map_err(|e| format!("the lists file is not TOML: {e}"))?;
    let l = t
        .get("lists")
        .and_then(|l| l.get(list))
        .ok_or_else(|| format!("the lists file has no list {list:?}"))?;
    let base = l
        .get("base")
        .and_then(|b| b.as_str())
        .ok_or_else(|| format!("list {list:?} has no base"))?;
    let branches = l
        .get("entries")
        .and_then(|e| e.as_array())
        .into_iter()
        .flatten()
        .filter_map(|e| e.get("branch")?.as_str().map(String::from))
        .collect();
    Ok((base.to_string(), branches))
}

fn run(program: &str, dir: &Path, args: &[&str]) -> Result<String, String> {
    let o = Command::new(program)
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(|e| format!("cannot run {program}: {e}"))?;
    if !o.status.success() {
        return Err(format!(
            "{program} {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&o.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

/// The commits a branch carries over the base, as (hash, subject): the local branch, else origin's.
fn commits(repo: &Path, base: &str, branch: &str) -> Result<Vec<(String, String)>, String> {
    let tip = if run(
        "git",
        repo,
        &[
            "rev-parse",
            "--verify",
            "-q",
            &format!("refs/heads/{branch}"),
        ],
    )
    .is_ok()
    {
        branch.to_string()
    } else {
        format!("origin/{branch}")
    };
    let out = run(
        "git",
        repo,
        &["log", "--format=%h%x09%s", &format!("{base}..{tip}")],
    )
    .map_err(|e| format!("{branch}: {e}"))?;
    Ok(out
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(h, s)| (h.to_string(), s.to_string()))
        .collect())
}

/// Every refusal of the record checks over one list: a branch of no upstream-item row, a commit with no
/// tag or the wrong one. Empty when the list may go to rhizoid.
pub fn check(
    lists_text: &str,
    list: &str,
    repo: &Path,
    carried: &BTreeMap<String, String>,
) -> Result<Vec<String>, String> {
    let (base, branches) = entries(lists_text, list)?;
    let mut refused = Vec::new();
    for branch in &branches {
        let Some(item) = item_of(branch) else {
            refused.push(format!(
                "{branch}: not a topic/ITEM or backport/ITEM/BASE branch"
            ));
            continue;
        };
        if !carried.contains_key(item) {
            refused.push(format!(
                "{branch}: no upstream-item row for {item} (tad q carried), so no topic or backport of it"
            ));
            continue;
        }
        for (h, s) in commits(repo, &base, branch)? {
            if let Some(r) = judge(&h, &s, carried) {
                refused.push(format!("{branch}: {r}"));
            }
        }
    }
    Ok(refused)
}

/// The base's carried items, from its `tad q carried`.
pub fn carried_from(tad: &str, records: &Path) -> Result<BTreeMap<String, String>, String> {
    Ok(carried(&run(
        tad,
        records,
        &["q", "carried", "--format", "csv"],
    )?))
}

/// `rhizoid assemble LIST`: its report as it printed it, and its exit code (1 is a conflict or a difference, whose
/// report names the entries; any other failure is an error).
pub fn assemble(
    rhizoid: &str,
    list: &str,
    lists: &Path,
    repo: &Path,
) -> Result<(i32, String), String> {
    let o = Command::new(rhizoid)
        .current_dir(repo)
        .args([
            "assemble",
            list,
            "--lists",
            &lists.to_string_lossy(),
            "--repo",
            &repo.to_string_lossy(),
            "--json",
        ])
        .output()
        .map_err(|e| format!("cannot run {rhizoid}: {e}"))?;
    let said = String::from_utf8_lossy(&o.stdout).into_owned();
    match o.status.code() {
        Some(c @ (0 | 1)) => Ok((c, said)),
        _ => Err(format!(
            "{rhizoid} assemble {list}: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        )),
    }
}

/// The int/ tags the images of a device.toml name.
pub fn image_tags(device_text: &str) -> Result<Vec<String>, String> {
    let m = lpoco_manifest::Manifest::parse(device_text)?;
    Ok(m.images
        .iter()
        .flat_map(|i| i.named.values())
        .filter_map(|i| i.kernel_tag.clone())
        .collect())
}

/// Push the tag an image names, then end rhizoid's worktree for it (its image is built, its recipe pushed).
/// Ok(false): no image names it, and nothing was done.
pub fn publish(
    rhizoid: &str,
    tag: &str,
    repo: &Path,
    image_tags: &[String],
) -> Result<bool, String> {
    if !image_tags.iter().any(|t| t == tag) {
        return Ok(false);
    }
    run(
        "git",
        repo,
        &["push", "origin", &format!("refs/tags/{tag}")],
    )?;
    let repo_arg = repo.to_string_lossy();
    run(
        rhizoid,
        repo,
        &["assemble", "--done", tag, "--repo", &repo_arg, "--force"],
    )?;
    Ok(true)
}
