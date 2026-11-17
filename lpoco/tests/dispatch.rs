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
    let d = std::env::temp_dir().join(format!("lpoco-test-{}-{name}", std::process::id()));
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
