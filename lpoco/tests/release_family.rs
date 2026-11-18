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

/// release-plz counts every commit since the last tag as a change; its own release commit would
/// open the next release PR at once, for ever. `release_commits` names the kinds that release.
#[test]
fn release_plz_ignores_its_own_release_commits() {
    let toml = read("release-plz.toml");
    let line = toml
        .lines()
        .find_map(|l| l.trim().strip_prefix("release_commits"))
        .expect(
            "release-plz.toml has no release_commits: its release commit opens the next release PR",
        );
    let pattern = line
        .trim_start()
        .strip_prefix('=')
        .unwrap()
        .trim()
        .trim_matches('\'');
    let re = regex::Regex::new(pattern).unwrap();
    for refused in [
        "chore: release v0.3.6 (#117)",
        "chore: release v0.3.7",
        "chore(release): v0.3.7",
    ] {
        assert!(
            !re.is_match(refused),
            "a release commit releases: {refused}"
        );
    }
    for released in [
        "feat: x",
        "fix(host): x",
        "perf: x",
        "refactor!: x",
        "docs: x",
        "build: x",
        "ci: x",
        "test: x",
    ] {
        assert!(re.is_match(released), "a change must release: {released}");
    }
}

/// A family release is one release PR: when only one member changed, release-plz must still write
/// every member's changelog at the new version (proof: release-pr-writes-every-changelog). Runs
/// release-plz on a fixture workspace shaped like this one, with this repository's own
/// release-plz.toml package settings.
#[test]
fn a_release_where_one_member_changed_writes_every_changelog() {
    use std::process::Command;
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("lpoco-release-fixture");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let run = |prog: &str, args: &[&str]| {
        let out = Command::new(prog)
            .args(args)
            .current_dir(&dir)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .output()
            .unwrap_or_else(|e| panic!("{prog}: {e} (CI installs release-plz: ci.yml)"));
        assert!(
            out.status.success(),
            "{prog} {args:?}: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    };
    let names = ["fam-a", "fam-b", "fam-c"];
    // the fixture's release-plz.toml is the repository's own, packages renamed to the fixture's
    let real = read("release-plz.toml");
    let head = real.split("[[package]]").next().unwrap();
    let block = real.split("[[package]]").nth(1).unwrap();
    let mut toml = head.to_string();
    for n in names {
        toml.push_str("[[package]]\n");
        let mut b = String::new();
        for l in block.lines() {
            if l.starts_with("name =") {
                b.push_str(&format!("name = \"{n}\"\n"));
            } else if l.starts_with("changelog_include") {
                let others: Vec<String> = names
                    .iter()
                    .filter(|o| **o != n)
                    .map(|o| format!("\"{o}\""))
                    .collect();
                b.push_str(&format!("changelog_include = [{}]\n", others.join(", ")));
            } else if l.starts_with("git_tag_name") {
                b.push_str(&format!("git_tag_name = \"{n}-v{{{{ version }}}}\"\n"));
            } else {
                b.push_str(l);
                b.push('\n');
            }
        }
        toml.push_str(&b);
    }
    fs::write(dir.join("release-plz.toml"), toml).unwrap();
    fs::write(
        dir.join("Cargo.toml"),
        "[workspace]\nresolver = \"2\"\nmembers = [\"fam-a\", \"fam-b\", \"fam-c\"]\n",
    )
    .unwrap();
    for n in names {
        fs::create_dir_all(dir.join(n).join("src")).unwrap();
        fs::write(
            dir.join(n).join("Cargo.toml"),
            format!("[package]\nname = \"{n}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
        )
        .unwrap();
        fs::write(dir.join(n).join("src/lib.rs"), "").unwrap();
        // each member already has a changelog at its tag, as in this repository
        fs::write(
            dir.join(n).join("CHANGELOG.md"),
            "# Changelog\n\n## [Unreleased]\n\n## [0.1.0] - 2026-01-01\n\n### Other\n\n- start\n",
        )
        .unwrap();
    }
    run("git", &["init", "-q", "-b", "main"]);
    run("git", &["add", "."]);
    run("git", &["commit", "-q", "-m", "feat: start"]);
    for n in names {
        run("git", &["tag", &format!("{n}-v0.1.0")]);
    }
    fs::write(dir.join("fam-a/src/lib.rs"), "pub fn x() {}\n").unwrap();
    run("git", &["commit", "-q", "-am", "feat: only a changes"]);
    run("release-plz", &["update", "--config", "release-plz.toml"]);
    for n in names {
        let log = fs::read_to_string(dir.join(n).join("CHANGELOG.md"))
            .unwrap_or_else(|_| panic!("{n}: no CHANGELOG.md after the release"));
        let version = fs::read_to_string(dir.join(n).join("Cargo.toml")).unwrap();
        assert!(version.contains("version = \"0.1.1\""), "{n}: not bumped");
        assert!(
            log.contains("## [0.1.1]"),
            "{n}: the release PR leaves its changelog without 0.1.1:\n{log}"
        );
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn every_member_includes_the_others_in_its_changelog() {
    let toml = read("release-plz.toml");
    let names: BTreeSet<String> = members().into_iter().map(|m| m.1).collect();
    for block in toml.split("[[package]]").skip(1) {
        let field = |key: &str| {
            block
                .lines()
                .find_map(|l| l.trim().strip_prefix(key)?.trim_start().strip_prefix('='))
                .unwrap_or_else(|| panic!("a package has no {key}"))
                .to_string()
        };
        let name = field("name").trim().trim_matches('"').to_string();
        let included: BTreeSet<String> = field("changelog_include")
            .trim()
            .trim_matches(|c| c == '[' || c == ']')
            .split(',')
            .map(|s| s.trim().trim_matches('"').to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let others: BTreeSet<String> = names.iter().filter(|n| **n != name).cloned().collect();
        assert_eq!(
            included, others,
            "{name}: changelog_include is not every other member"
        );
    }
}
