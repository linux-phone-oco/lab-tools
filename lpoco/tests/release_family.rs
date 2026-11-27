// SPDX-License-Identifier: MIT OR Apache-2.0
//! The family releases as one: every release bumps the version and tags what landed (proof:
//! release-tags-every-change). A release PR that only rewrites a changelog leaves the code that
//! landed under no tag, so the checks refuse what lets that happen.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

/// The packages released on their own, outside the family (release-plz.toml).
const OWN: &[&str] = &["lpoco-attended", "lpoco-manifest", "osfact"];
/// Of those, the ones the family members depend on by path: their changelogs still list them.
const IN_CHANGELOGS: &[&str] = &["lpoco-manifest"];

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
    let versions: BTreeSet<String> = members()
        .into_iter()
        .filter(|m| !OWN.contains(&m.1.as_str()))
        .map(|m| m.2)
        .collect();
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
        "chore(deps): bump lpoco-manifest to v0.47.0",
        "chore(deps)!: x",
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
        "chore: release v0.48.5 (#393)",
        "chore(lpoco): release v0.3.7",
        "chore(lpoco-attended): release v0.45.3",
        "chore(deps-dev): x",
        "chore: x",
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
        "chore(deps): bump lpoco-manifest to v0.47.0",
        "chore(deps)!: x",
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
    let names: BTreeSet<String> = members()
        .into_iter()
        .map(|m| m.1)
        .filter(|n| !OWN.contains(&n.as_str()) || IN_CHANGELOGS.contains(&n.as_str()))
        .collect();
    for block in toml.split("[[package]]").skip(1) {
        let field = |key: &str| {
            block
                .lines()
                .find_map(|l| l.trim().strip_prefix(key)?.trim_start().strip_prefix('='))
                .unwrap_or_else(|| panic!("a package has no {key}"))
                .to_string()
        };
        let name = field("name").trim().trim_matches('"').to_string();
        if OWN.contains(&name.as_str()) {
            assert!(
                !block.contains("version_group"),
                "{name} is in the family: it is tagged without a change"
            );
            continue;
        }
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

/// lpoco-attended is a library the labs pin by git tag, so it is tagged only when it changes
/// (proof: attended-releases-when-it-changes). Runs release-plz on a fixture shaped like this
/// workspace: family members, one of them a library that a standalone package names by its git
/// tag (not by path: release-plz bumps the dependents of a released path dependency), the
/// standalone package's release-plz.toml block the repository's own for lpoco-attended.
#[test]
fn a_release_moves_lpoco_attended_only_when_attended_changed() {
    use std::process::Command;
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("lpoco-attended-fixture");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("libsrc/src")).unwrap();
    let run_in = |at: &std::path::Path, prog: &str, args: &[&str]| {
        let out = Command::new(prog)
            .args(args)
            .current_dir(at)
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
    let run = |prog: &str, args: &[&str]| run_in(&dir, prog, args);
    // the library the standalone package names by tag, in a repository of its own
    let src = dir.join("libsrc");
    fs::write(
        src.join("Cargo.toml"),
        "[package]\nname = \"famlib\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::write(src.join("src/lib.rs"), "").unwrap();
    run_in(&src, "git", &["init", "-q", "-b", "main"]);
    run_in(&src, "git", &["add", "."]);
    run_in(&src, "git", &["commit", "-q", "-m", "feat: start"]);
    run_in(&src, "git", &["tag", "famlib-v0.1.0"]);
    let real = read("release-plz.toml");
    let head = real.split("[[package]]").next().unwrap();
    let attended = real
        .split("[[package]]")
        .find(|b| b.contains("name = \"lpoco-attended\""))
        .expect("release-plz.toml has no lpoco-attended package");
    let family = ["fam-a", "fam-b", "famlib"];
    let mut toml = head.to_string();
    for n in family {
        let others: Vec<String> = family
            .iter()
            .filter(|o| **o != n)
            .map(|o| format!("\"{o}\""))
            .collect();
        toml.push_str(&format!(
            "[[package]]\nname = \"{n}\"\nchangelog_include = [{}]\nversion_group = \"fam\"\ngit_tag_name = \"{n}-v{{{{ version }}}}\"\n",
            others.join(", ")
        ));
    }
    toml.push_str("[[package]]\n");
    for l in attended.lines() {
        if l.starts_with("name =") {
            toml.push_str("name = \"solo\"\n");
        } else if l.starts_with("git_tag_name") {
            toml.push_str("git_tag_name = \"solo-v{{ version }}\"\n");
        } else {
            toml.push_str(l);
            toml.push('\n');
        }
    }
    fs::write(dir.join("release-plz.toml"), toml).unwrap();
    let members = "[\"fam-a\", \"fam-b\", \"famlib\", \"solo\"]";
    fs::write(
        dir.join("Cargo.toml"),
        format!("[workspace]\nresolver = \"2\"\nmembers = {members}\n"),
    )
    .unwrap();
    let url = format!("file://{}", src.display());
    for n in ["fam-a", "fam-b", "famlib", "solo"] {
        fs::create_dir_all(dir.join(n).join("src")).unwrap();
        let dep = if n == "solo" {
            format!("\n[dependencies]\nfamlib = {{ git = \"{url}\", tag = \"famlib-v0.1.0\" }}\n")
        } else {
            String::new()
        };
        fs::write(
            dir.join(n).join("Cargo.toml"),
            format!("[package]\nname = \"{n}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n{dep}"),
        )
        .unwrap();
        fs::write(dir.join(n).join("src/lib.rs"), "").unwrap();
        fs::write(
            dir.join(n).join("CHANGELOG.md"),
            "# Changelog\n\n## [Unreleased]\n\n## [0.1.0] - 2026-01-01\n\n### Other\n\n- start\n",
        )
        .unwrap();
    }
    // the library's own repository stays out of the fixture workspace's history
    fs::write(dir.join(".gitignore"), "libsrc/\ntarget/\n").unwrap();
    run("git", &["init", "-q", "-b", "main"]);
    run("git", &["add", "."]);
    run("git", &["commit", "-q", "-m", "feat: start"]);
    for n in ["fam-a", "fam-b", "famlib", "solo"] {
        run("git", &["tag", &format!("{n}-v0.1.0")]);
    }
    let version_of = |n: &str| {
        let t = fs::read_to_string(dir.join(n).join("Cargo.toml")).unwrap();
        t.lines()
            .find_map(|l| l.strip_prefix("version = "))
            .unwrap()
            .trim_matches('"')
            .to_string()
    };
    // a change outside attended/, in the library it names as well: the family moves, attended stays
    fs::write(dir.join("fam-a/src/lib.rs"), "pub fn x() {}\n").unwrap();
    fs::write(dir.join("famlib/src/lib.rs"), "pub fn z() {}\n").unwrap();
    run(
        "git",
        &["commit", "-q", "-am", "feat: only the family changes"],
    );
    run("release-plz", &["update", "--config", "release-plz.toml"]);
    for n in family {
        assert_eq!(version_of(n), "0.1.1", "{n}: the family did not move");
    }
    assert_eq!(
        version_of("solo"),
        "0.1.0",
        "a change outside attended/ moves lpoco-attended"
    );
    run("git", &["checkout", "-q", "."]);
    run("git", &["clean", "-fdq", "-e", "libsrc"]);
    // a change inside attended/: it moves
    fs::write(dir.join("solo/src/lib.rs"), "pub fn y() {}\n").unwrap();
    run("git", &["commit", "-q", "-am", "feat: attended changes"]);
    run("release-plz", &["update", "--config", "release-plz.toml"]);
    assert_eq!(
        version_of("solo"),
        "0.1.1",
        "a change inside attended/ does not move lpoco-attended"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// attended names lpoco-manifest by a released git tag, never by path: a path dependency moves it
/// at every family release (the proof above runs the same shape on a fixture).
#[test]
fn lpoco_attended_names_lpoco_manifest_by_tag_not_by_path() {
    let toml = read("attended/Cargo.toml");
    let line = toml
        .lines()
        .find(|l| l.starts_with("lpoco-manifest"))
        .expect("attended/Cargo.toml does not name lpoco-manifest");
    assert!(
        line.contains("git =") && line.contains("tag = \"lpoco-manifest-v"),
        "not a released git tag: {line}"
    );
    assert!(!line.contains("path"), "a path dependency: {line}");
}

/// lpoco-manifest is a library the family names by path and lpoco-attended by released tag, so it is
/// tagged only when manifest/ changes (proof: manifest-releases-when-it-changes). Runs release-plz on
/// a fixture shaped like this workspace - family members that depend on a library by path, the
/// library's release-plz.toml block the repository's own for lpoco-manifest - and checks both ways:
/// a change outside the library leaves its version; a change in it moves the whole family.
#[test]
fn a_release_moves_lpoco_manifest_only_when_manifest_changed() {
    use std::process::Command;
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("lpoco-manifest-fixture");
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
    let real = read("release-plz.toml");
    let head = real.split("[[package]]").next().unwrap();
    let lib_block = real
        .split("[[package]]")
        .find(|b| b.contains("name = \"lpoco-manifest\""))
        .expect("release-plz.toml has no lpoco-manifest package");
    let family = ["fam-a", "fam-b"];
    let mut toml = head.to_string();
    for n in family {
        let others: Vec<String> = family
            .iter()
            .filter(|o| **o != n)
            .map(|o| format!("\"{o}\""))
            .chain(["\"famlib\"".to_string()])
            .collect();
        toml.push_str(&format!(
            "[[package]]\nname = \"{n}\"\nchangelog_include = [{}]\nversion_group = \"lpoco\"\ngit_tag_name = \"{n}-v{{{{ version }}}}\"\n",
            others.join(", ")
        ));
    }
    toml.push_str("[[package]]\n");
    for l in lib_block.lines() {
        if l.starts_with("name =") {
            toml.push_str("name = \"famlib\"\n");
        } else if l.starts_with("git_tag_name") {
            toml.push_str("git_tag_name = \"famlib-v{{ version }}\"\n");
        } else {
            toml.push_str(l);
            toml.push('\n');
        }
    }
    fs::write(dir.join("release-plz.toml"), toml).unwrap();
    fs::write(
        dir.join("Cargo.toml"),
        "[workspace]\nresolver = \"2\"\nmembers = [\"fam-a\", \"fam-b\", \"famlib\"]\n",
    )
    .unwrap();
    for n in ["fam-a", "fam-b", "famlib"] {
        fs::create_dir_all(dir.join(n).join("src")).unwrap();
        let dep = if n == "famlib" {
            String::new()
        } else {
            "\n[dependencies]\nfamlib = { path = \"../famlib\" }\n".to_string()
        };
        fs::write(
            dir.join(n).join("Cargo.toml"),
            format!("[package]\nname = \"{n}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n{dep}"),
        )
        .unwrap();
        fs::write(dir.join(n).join("src/lib.rs"), "").unwrap();
        fs::write(
            dir.join(n).join("CHANGELOG.md"),
            "# Changelog\n\n## [Unreleased]\n\n## [0.1.0] - 2026-01-01\n\n### Other\n\n- start\n",
        )
        .unwrap();
    }
    fs::write(dir.join(".gitignore"), "target/\n").unwrap();
    run("git", &["init", "-q", "-b", "main"]);
    run("git", &["add", "."]);
    run("git", &["commit", "-q", "-m", "feat: start"]);
    for n in ["fam-a", "fam-b", "famlib"] {
        run("git", &["tag", &format!("{n}-v0.1.0")]);
    }
    let version_of = |n: &str| {
        let t = fs::read_to_string(dir.join(n).join("Cargo.toml")).unwrap();
        t.lines()
            .find_map(|l| l.strip_prefix("version = "))
            .unwrap()
            .trim_matches('"')
            .to_string()
    };
    // a change outside the library: the family moves, the library keeps its version (and so its tag)
    fs::write(dir.join("fam-a/src/lib.rs"), "pub fn x() {}\n").unwrap();
    run(
        "git",
        &["commit", "-q", "-am", "feat: only the family changes"],
    );
    run("release-plz", &["update", "--config", "release-plz.toml"]);
    for n in family {
        assert_eq!(version_of(n), "0.1.1", "{n}: the family did not move");
    }
    assert_eq!(
        version_of("famlib"),
        "0.1.0",
        "a change outside manifest/ moves lpoco-manifest"
    );
    run("git", &["checkout", "-q", "."]);
    run("git", &["clean", "-fdq"]);
    // the first change must not stay in history: scenario 2's only change since the tags is the library's
    run("git", &["reset", "-q", "--hard", "HEAD~1"]);
    // a change in the library: it moves, and its dependents with it, to one version
    fs::write(dir.join("famlib/src/lib.rs"), "pub fn y() {}\n").unwrap();
    run("git", &["commit", "-q", "-am", "feat: the library changes"]);
    run("release-plz", &["update", "--config", "release-plz.toml"]);
    assert_eq!(
        version_of("famlib"),
        "0.1.1",
        "a change in manifest/ does not move lpoco-manifest"
    );
    for n in family {
        assert_eq!(
            version_of(n),
            "0.1.1",
            "{n}: a manifest release leaves a family member behind"
        );
    }
    let _ = fs::remove_dir_all(&dir);
}

/// The programs the lab host installs from release assets: host/src/lib.rs FAMILY.
fn host_family() -> Vec<String> {
    let lib = read("host/src/lib.rs");
    lib.split("pub const FAMILY: &[&str] = &[")
        .nth(1)
        .and_then(|rest| rest.split("];").next())
        .expect("host/src/lib.rs has no FAMILY")
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// What `lpoco-host` installs follows a release asset: its package is released (a GitHub release to hold the asset),
/// and the Assets workflow builds it and attaches it. The dispatcher runs `lpoco layer` as the program lpoco-layer
/// found on the host (README, the `layer/` row), so lpoco-layer is among them.
#[test]
fn every_program_the_host_installs_is_released_and_its_asset_attached() {
    let family = host_family();
    assert!(
        family.contains(&"lpoco-layer".to_string()),
        "lpoco layer cannot run on a host that lacks lpoco-layer: {family:?}"
    );
    let plz = read("release-plz.toml");
    let assets = read(".github/workflows/assets.yml");
    for name in &family {
        let block = plz
            .split("[[package]]")
            .find(|b| b.contains(&format!("name = \"{name}\"")))
            .unwrap_or_else(|| panic!("release-plz.toml has no package {name}"));
        assert!(
            block.contains("git_release_enable = true"),
            "{name} has no GitHub release for its asset"
        );
        assert!(
            assets.contains(&format!("-p {name}")),
            "assets.yml does not build {name}"
        );
        assert!(
            assets.contains(&format!(" {name} ")) || assets.contains(&format!(" {name};")),
            "assets.yml does not attach {name}"
        );
    }
}
