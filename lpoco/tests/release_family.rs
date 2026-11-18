//! The family releases as one: every release bumps the version and tags what landed (proof:
//! release-tags-every-change). A release PR that only rewrites a changelog leaves the code that
//! landed under no tag, so the checks refuse what lets that happen.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn read(rel: &str) -> String {
    fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// (directory, package name, version) of every workspace member.
fn members() -> Vec<(String, String, String)> {
    let ws = read("Cargo.toml");
    let list = ws
        .split("members = [")
        .nth(1)
        .unwrap()
        .split(']')
        .next()
        .unwrap();
    let dirs: Vec<String> = list
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect();
    dirs.into_iter()
        .map(|d| {
            let text = read(&format!("{d}/Cargo.toml"));
            let package = text.split("[package]").nth(1).unwrap();
            let field = |key: &str| {
                package
                    .lines()
                    .find_map(|l| l.strip_prefix(key)?.trim_start().strip_prefix('='))
                    .map(|v| v.trim().trim_matches('"').to_string())
                    .unwrap_or_else(|| panic!("{d}: no {key}"))
            };
            (d.clone(), field("name"), field("version"))
        })
        .collect()
}

#[test]
fn release_plz_names_exactly_the_workspace_packages() {
    let named: BTreeSet<String> = read("release-plz.toml")
        .lines()
        .filter_map(|l| l.trim().strip_prefix("name = \""))
        .map(|v| v.trim_end_matches('"').to_string())
        .collect();
    let packages: BTreeSet<String> = members().into_iter().map(|m| m.1).collect();
    assert_eq!(
        named, packages,
        "release-plz.toml and the workspace disagree"
    );
}

#[test]
fn the_family_shares_one_version() {
    let versions: BTreeSet<String> = members().into_iter().map(|m| m.2).collect();
    assert_eq!(
        versions.len(),
        1,
        "the family has more than one version: {versions:?}"
    );
}

#[test]
fn a_changelog_names_each_version_once_and_starts_at_the_package_version() {
    for (dir, name, version) in members() {
        let log = read(&format!("{dir}/CHANGELOG.md"));
        let headings: Vec<&str> = log
            .lines()
            .filter_map(|l| l.strip_prefix("## ["))
            .filter_map(|l| l.split(']').next())
            .filter(|v| *v != "Unreleased")
            .collect();
        let unique: BTreeSet<&&str> = headings.iter().collect();
        assert_eq!(
            unique.len(),
            headings.len(),
            "{name}: a version is named twice in {dir}/CHANGELOG.md"
        );
        assert_eq!(
            headings.first().copied(),
            Some(version.as_str()),
            "{name}: changelog first version is not the package version"
        );
    }
}

#[test]
fn the_commit_kind_pattern_accepts_kinds_and_refuses_the_rest() {
    let pattern = read(".github/commit-kind.regex");
    let re = regex::Regex::new(pattern.trim()).unwrap();
    for ok in [
        "feat: x",
        "fix(host): x",
        "docs(a/b): x",
        "chore: release v0.3.5",
        "refactor!: x",
        "perf: x",
        "test: x",
        "ci: x",
        "build: x",
        "feat(Host): x",
    ] {
        assert!(re.is_match(ok), "should accept: {ok}");
    }
    for bad in [
        "bringup: image swap",
        "Fix: x",
        "feat x",
        "feat:",
        "feat:x",
        "wip: x",
        "",
    ] {
        assert!(!re.is_match(bad), "should refuse: {bad}");
    }
}

#[test]
fn the_workflow_reads_the_pattern_checks_commits_and_stops() {
    let flow = read(".github/workflows/pr-title.yml");
    assert!(flow.contains("pull_request"));
    assert!(flow.contains(".github/commit-kind.regex"));
    assert!(flow.contains("git log"), "commit titles are not checked");
    assert!(flow.contains("exit 1"), "it prints and does not stop");
    assert!(
        flow.contains(".github/check-changelog.sh"),
        "the changelog rule is not run"
    );
}
