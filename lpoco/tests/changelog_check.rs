//! A changelog never changes without a version, but a follow-up release PR may complete a version
//! the family already carries (proof: release-tags-every-change). The rule lives in
//! .github/check-changelog.sh; these tests run it on small throwaway repositories.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(".github/check-changelog.sh")
}

fn sh(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// A repository with one package `p` at version 0.3.4 whose changelog names 0.3.4, then `change`
/// applied on top; returns whether the check accepts the change.
fn accepts(change: impl Fn(&Path)) -> bool {
    accepts_from("0.3.4", change)
}

/// Like `accepts`, with the package already at `version` in the base (a release whose changelog
/// entry for it is still missing).
fn accepts_from(version: &str, change: impl Fn(&Path)) -> bool {
    let dir = std::env::temp_dir().join(format!(
        "changelog-check-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("p")).unwrap();
    sh(&dir, &["init", "-q", "-b", "main"]);
    fs::write(
        dir.join("p/Cargo.toml"),
        format!("[package]\nname = \"p\"\nversion = \"{version}\"\n"),
    )
    .unwrap();
    fs::write(
        dir.join("p/CHANGELOG.md"),
        "# Changelog\n\n## [Unreleased]\n\n## [0.3.4](u) - d\n",
    )
    .unwrap();
    for name in ["my pkg", "ü"] {
        fs::create_dir_all(dir.join(name)).unwrap();
        fs::write(
            dir.join(name).join("Cargo.toml"),
            "[package]\nname = \"q\"\nversion = \"0.3.4\"\n",
        )
        .unwrap();
        fs::write(
            dir.join(name).join("CHANGELOG.md"),
            "# Changelog\n\n## [0.3.4](u) - d\n",
        )
        .unwrap();
    }
    sh(&dir, &["add", "."]);
    sh(&dir, &["commit", "-q", "-m", "chore: base"]);
    let base = sh(&dir, &["rev-parse", "HEAD"]);
    change(&dir);
    sh(&dir, &["add", "."]);
    sh(&dir, &["commit", "-q", "-m", "chore: change"]);
    let head = sh(&dir, &["rev-parse", "HEAD"]);
    let ok = Command::new("sh")
        .arg(script())
        .args([&base, &head])
        .current_dir(&dir)
        .status()
        .unwrap()
        .success();
    let _ = fs::remove_dir_all(&dir);
    ok
}

#[test]
fn a_change_that_touches_no_changelog_passes() {
    assert!(accepts(
        |d| fs::write(d.join("p/x.rs"), "fn main(){}").unwrap()
    ));
}

#[test]
fn a_changelog_with_a_version_bump_passes() {
    assert!(accepts(|d| {
        fs::write(
            d.join("p/Cargo.toml"),
            "[package]\nname = \"p\"\nversion = \"0.3.5\"\n",
        )
        .unwrap();
        fs::write(
            d.join("p/CHANGELOG.md"),
            "# Changelog\n\n## [Unreleased]\n\n## [0.3.5](u) - d\n\n## [0.3.4](u) - d\n",
        )
        .unwrap();
    }));
}

#[test]
fn a_changelog_alone_that_adds_the_current_version_passes() {
    assert!(accepts_from("0.3.5", |d| {
        fs::write(
            d.join("p/CHANGELOG.md"),
            "# Changelog\n\n## [Unreleased]\n\n## [0.3.5](u) - d\n\n## [0.3.4](u) - d\n",
        )
        .unwrap();
    }));
}

#[test]
fn a_changelog_alone_that_adds_no_current_version_is_refused() {
    assert!(!accepts(|d| {
        fs::write(
            d.join("p/CHANGELOG.md"),
            "# Changelog\n\n## [Unreleased]\n\nnotes\n\n## [0.3.4](u) - d\n",
        )
        .unwrap();
    }));
}

#[test]
fn a_changelog_alone_that_repeats_a_version_is_refused() {
    assert!(!accepts(|d| {
        fs::write(
            d.join("p/CHANGELOG.md"),
            "# Changelog\n\n## [Unreleased]\n\n## [0.3.4](u) - d\n\n## [0.3.4](u) - again\n",
        )
        .unwrap();
    }));
}

#[test]
fn a_dependency_version_line_is_not_a_bump() {
    assert!(!accepts(|d| {
        fs::write(
            d.join("p/Cargo.toml"),
            "[package]\nname = \"p\"\nversion = \"0.3.4\"\n[dependencies.x]\nversion = \"1\"\n",
        )
        .unwrap();
        fs::write(
            d.join("p/CHANGELOG.md"),
            "# Changelog\n\n## [Unreleased]\n\nn\n\n## [0.3.4](u) - d\n",
        )
        .unwrap();
    }));
}

#[test]
fn a_package_with_a_space_or_non_ascii_name_is_checked() {
    for name in ["my pkg", "ü"] {
        assert!(
            !accepts(|d| {
                fs::write(
                    d.join(name).join("CHANGELOG.md"),
                    "# Changelog\n\nedit\n\n## [0.3.4](u) - d\n",
                )
                .unwrap();
            }),
            "{name}: a lone changelog edit passed"
        );
    }
}

#[test]
fn a_package_without_a_literal_version_is_refused() {
    assert!(!accepts(|d| {
        fs::write(
            d.join("p/Cargo.toml"),
            "[package]\nname = \"p\"\nversion.workspace = true\n",
        )
        .unwrap();
        fs::write(d.join("p/CHANGELOG.md"), "# Changelog\n\nn\n").unwrap();
    }));
}
