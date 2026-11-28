// SPDX-License-Identifier: MIT OR Apache-2.0
//! osfact's first release, osfact-v0.1.0 (decision 2026-11-28-osfact-is-first-released-with-the-walking-skeleton-s):
//! release-plz releases it on its own tag, and the assets workflow attaches its x86_64 binary and the sha256,
//! as it does for the lpoco family, so a keeper installs a pinned release (item osfact-first-release).

use std::fs;
use std::path::PathBuf;

fn read(rel: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn osfact_block() -> String {
    read("release-plz.toml")
        .split("[[package]]")
        .find(|b| b.contains("name = \"osfact\""))
        .expect("release-plz.toml has no osfact package")
        .to_string()
}

#[test]
fn release_plz_releases_osfact_on_its_own_tag_with_a_github_release() {
    let block = osfact_block();
    let live = |key: &str| {
        block
            .lines()
            .map(str::trim)
            .any(|l| !l.starts_with('#') && l.replace(' ', "") == key)
    };
    assert!(
        !live("release=false"),
        "osfact is still not released:\n{block}"
    );
    assert!(
        live("git_release_enable=true"),
        "no GitHub release to hold the binary:\n{block}"
    );
    assert!(
        block.contains("git_tag_name = \"osfact-v{{ version }}\""),
        "tag prefix:\n{block}"
    );
    assert!(
        !block.contains("version_group"),
        "osfact is not in the lpoco family:\n{block}"
    );
}

#[test]
fn osfact_starts_at_its_first_release_version() {
    let toml = read("osfact/Cargo.toml");
    assert!(
        toml.contains("\nversion = \"0.1.0\""),
        "osfact's first release is 0.1.0:\n{toml}"
    );
    let log = read("osfact/CHANGELOG.md");
    let first = log
        .lines()
        .filter_map(|l| l.strip_prefix("## ["))
        .find(|v| !v.starts_with("Unreleased"));
    assert!(
        first.is_some_and(|v| v.starts_with("0.1.0]")),
        "changelog starts at {first:?}"
    );
}

#[test]
fn the_assets_workflow_attaches_osfact_and_its_sha256_to_its_release() {
    let wf = read(".github/workflows/assets.yml");
    for needle in [
        "osfact-v",
        "osfact-x86_64",
        "osfact-x86_64.sha256",
        "-p osfact",
        "gh release upload",
    ] {
        assert!(wf.contains(needle), "assets.yml does not mention {needle}");
    }
    let sha = wf.find("osfact-x86_64.sha256").unwrap();
    let bin = wf.rfind("dist/osfact-x86_64\"").unwrap_or(0);
    assert!(
        sha < bin || bin == 0,
        "the hash goes up first, then the binary"
    );
}

#[test]
fn osfact_docs_say_how_to_install_the_release() {
    let doc = read("osfact/docs/README.md");
    for needle in [
        "osfact-v",
        "osfact-x86_64",
        "sha256sum",
        "gh release download",
    ] {
        assert!(
            doc.contains(needle),
            "osfact/docs/README.md does not say {needle}"
        );
    }
}
