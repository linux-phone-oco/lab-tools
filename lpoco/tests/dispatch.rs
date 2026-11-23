// SPDX-License-Identifier: MIT OR Apache-2.0
//! The dispatcher's behaviour, through the built binary: `lpoco NAME args...` runs `lpoco-NAME`.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A fixture program `lpoco-fake` in `dir`: reports `version` for --version, otherwise prints its
/// arguments and exits 7.
fn fixture(dir: &Path, version: &str) {
    let p = dir.join("lpoco-fake");
    fs::write(
        &p,
        format!(
            "#!/bin/sh\nif [ \"$1\" = --version ]; then echo 'lpoco-fake {version}'; exit 0; fi\necho \"fake got: $*\"\nexit 7\n"
        ),
    )
    .unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
}

fn tmp(name: &str) -> std::path::PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("lpoco-test-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn lpoco(path_dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lpoco"))
        .args(args)
        .env("PATH", path_dir)
        .output()
        .unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

#[test]
fn lpoco_runs_subcommand_by_path() {
    let d = tmp("runs");
    fixture(&d, VERSION);
    let o = lpoco(&d, &["fake", "x", "--odd-flag"]);
    assert_eq!(o.status.code(), Some(7), "{}", text(&o.stderr));
    assert_eq!(text(&o.stdout), "fake got: x --odd-flag\n");
}

#[test]
fn a_missing_name_stops_with_one_line_naming_what_was_searched() {
    let d = tmp("missing");
    let o = lpoco(&d, &["nothere"]);
    assert_ne!(o.status.code(), Some(0));
    let err = text(&o.stderr);
    assert_eq!(err.lines().count(), 1, "{err}");
    assert!(err.contains("lpoco-nothere"), "{err}");
    assert!(err.contains(d.to_str().unwrap()), "{err}");
}

#[test]
fn lpoco_refuses_mixed_versions() {
    let d = tmp("mixed");
    fixture(&d, "9.9.9");
    let o = lpoco(&d, &["fake", "x"]);
    assert_ne!(o.status.code(), Some(0));
    assert_eq!(text(&o.stdout), "", "the subcommand must not have run");
    let err = text(&o.stderr);
    assert!(err.contains("9.9.9") && err.contains(VERSION), "{err}");
}

#[test]
fn version_list_completions_and_man_come_from_the_declaration() {
    let d = tmp("builtins");
    fixture(&d, VERSION);
    let o = lpoco(&d, &["--version"]);
    assert_eq!(text(&o.stdout), format!("lpoco {VERSION}\n"));
    let o = lpoco(&d, &["list"]);
    assert!(text(&o.stdout).contains("fake"), "{}", text(&o.stdout));
    let o = lpoco(&d, &["completions", "bash"]);
    assert!(text(&o.stdout).contains("complete -F _lpoco lpoco"));
    let o = lpoco(&d, &["man"]);
    assert!(text(&o.stdout).starts_with(".TH LPOCO 1"));
}

/// lpoco-attended is released only when it changes (release-plz.toml), so its version is its own:
/// the dispatcher runs it whatever lpoco's version is (proof: attended-releases-when-it-changes).
#[test]
fn lpoco_attended_runs_at_its_own_version() {
    let d = tmp("attended-own");
    let p = d.join("lpoco-attended");
    fs::write(
        &p,
        "#!/bin/sh\nif [ \"$1\" = --version ]; then echo 'lpoco-attended 0.1.0'; exit 0; fi\necho \"attended got: $*\"\nexit 7\n",
    )
    .unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    // a copy of lpoco beside the fixture: the real lpoco-attended next to the test binary must not answer
    let own = d.join("lpoco");
    link_lpoco(&own);
    let o = Command::new(&own)
        .args(["attended", "check"])
        .env("PATH", &d)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(7), "{}", text(&o.stderr));
    assert_eq!(text(&o.stdout), "attended got: check\n");
}

#[test]
fn list_shows_lpoco_attended_at_its_own_version_without_a_refusal() {
    let d = tmp("attended-list");
    let p = d.join("lpoco-attended");
    fs::write(&p, "#!/bin/sh\necho 'lpoco-attended 0.1.0'\n").unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    let own = d.join("lpoco");
    link_lpoco(&own);
    let o = Command::new(&own)
        .arg("list")
        .env("PATH", &d)
        .output()
        .unwrap();
    let out = text(&o.stdout);
    let line = out.lines().find(|l| l.starts_with("attended")).expect(&out);
    assert!(
        line.contains("0.1.0") && !line.contains("refuses"),
        "{line}"
    );
}

/// `lpoco` beside the fixtures, as a hard link of the built binary (a copy still being written when a
/// parallel test forks is refused to exec: text file busy). It lives in the target folder, the
/// binary's filesystem, and `current_exe` names this path, so the real lpoco-attended next to the
/// built binary is not what answers.
fn link_lpoco(own: &Path) {
    let _ = fs::remove_file(own);
    fs::hard_link(env!("CARGO_BIN_EXE_lpoco"), own).unwrap_or_else(|e| panic!("{own:?}: {e}"));
}

/// lpoco-manifest is tagged only when manifest/ changes (proof: manifest-releases-when-it-changes),
/// so `lpoco manifest` runs it at its own version too.
#[test]
fn lpoco_manifest_runs_at_its_own_version() {
    let d = tmp("manifest-own");
    let p = d.join("lpoco-manifest");
    fs::write(
        &p,
        "#!/bin/sh\nif [ \"$1\" = --version ]; then echo 'lpoco-manifest 0.1.0'; exit 0; fi\necho \"manifest got: $*\"\nexit 7\n",
    )
    .unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    let own = d.join("lpoco");
    link_lpoco(&own);
    let o = Command::new(&own)
        .args(["manifest", "schema"])
        .env("PATH", &d)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(7), "{}", text(&o.stderr));
    assert_eq!(text(&o.stdout), "manifest got: schema\n");
}
