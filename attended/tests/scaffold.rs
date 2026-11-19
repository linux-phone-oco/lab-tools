// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco-attended new|run|check`: scaffold, run and dry-run a device repo's procedures/NAME.rs.
//! The program under test is the real binary; rust-script is the real one for the end-to-end case and an sh
//! stand-in (it runs the procedure file with sh and records what it was given) for the rest.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lpoco-scaffold-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    fs::write(
        d.join("device.toml"),
        "[device]\ncodename = \"xiaomi-surya\"\n",
    )
    .unwrap();
    d
}

fn lib() -> &'static str {
    env!("CARGO_MANIFEST_DIR")
}

/// A rust-script stand-in: records its arguments one line each in `dir/runs`, then runs the file with sh.
fn stand_in(dir: &Path) -> PathBuf {
    let f = dir.join("fake-rust-script");
    fs::write(
        &f,
        format!(
            "#!/bin/sh\necho \"$*\" >> {}/runs\nexec sh \"$@\"\n",
            dir.display()
        ),
    )
    .unwrap();
    Command::new("chmod").arg("+x").arg(&f).status().unwrap();
    f
}

fn tool(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lpoco-attended"))
        .args(args)
        .current_dir(dir)
        .env("LPOCO_ATTENDED_LIB", lib())
        .env("LPOCO_ATTENDED_RUST_SCRIPT", stand_in(dir))
        .output()
        .unwrap()
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn proc_file(dir: &Path, name: &str, body: &str) {
    fs::create_dir_all(dir.join("procedures")).unwrap();
    fs::write(dir.join(format!("procedures/{name}.rs")), body).unwrap();
}

#[test]
fn new_writes_a_procedure_for_the_repos_device_that_the_real_rust_script_dry_runs_unchanged() {
    let d = tmp("e2e");
    let o = tool(&d, &["new", "calls"]);
    assert!(o.status.success(), "{}", text(&o));
    let file = d.join("procedures/calls.rs");
    let src = fs::read_to_string(&file).unwrap();
    assert!(src.starts_with("#!/usr/bin/env rust-script"), "{src}");
    assert!(
        src.contains("procedure(\"calls\", \"xiaomi-surya\")"),
        "{src}"
    );
    assert!(src.contains(&format!("path = \"{}\"", lib())), "{src}");
    assert!(
        !src.contains("path = \"steps\""),
        "no steps crate here: {src}"
    );

    // the file as written, run by the real rust-script, in a dry run
    let log = d.join("calls.log");
    let r = Command::new("rust-script")
        .arg(&file)
        .args(["--dry-run", "--log"])
        .arg(&log)
        .current_dir(&d)
        .output()
        .expect("rust-script is on the lab machine");
    assert!(r.status.success(), "{}", text(&r));
    assert!(fs::read_to_string(&log).unwrap().contains("calls"));
}

#[test]
fn a_repo_with_a_steps_crate_has_it_in_the_new_files_cargo_block() {
    let d = tmp("steps");
    fs::create_dir_all(d.join("procedures/steps/src")).unwrap();
    fs::write(
        d.join("procedures/steps/Cargo.toml"),
        "[package]\nname = \"surya-steps\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    let o = tool(&d, &["new", "wake"]);
    assert!(o.status.success(), "{}", text(&o));
    let src = fs::read_to_string(d.join("procedures/wake.rs")).unwrap();
    assert!(
        src.contains("//! surya-steps = { path = \"steps\" }"),
        "{src}"
    );
}

#[test]
fn new_refuses_what_it_would_get_wrong_and_writes_nothing() {
    let d = tmp("refuse");
    proc_file(&d, "calls", "# mine\n");
    for (args, why) in [
        (vec!["new", "calls"], "already"),
        (vec!["new", "Bad Name"], "name"),
        (vec!["new", "../x"], "name"),
        (vec!["new"], "NAME"),
    ] {
        let o = tool(&d, &args);
        assert_eq!(o.status.code(), Some(2), "{args:?}: {}", text(&o));
        assert!(text(&o).contains(why), "{args:?}: {}", text(&o));
    }
    assert_eq!(
        fs::read_to_string(d.join("procedures/calls.rs")).unwrap(),
        "# mine\n"
    );
    // no device.toml: no codename to write
    let bare = std::env::temp_dir().join(format!("lpoco-scaffold-{}-bare", std::process::id()));
    let _ = fs::remove_dir_all(&bare);
    fs::create_dir_all(&bare).unwrap();
    let o = tool(&bare, &["new", "calls"]);
    assert_eq!(o.status.code(), Some(2), "{}", text(&o));
    assert!(text(&o).contains("device.toml"), "{}", text(&o));
    assert!(!bare.join("procedures").exists());
}

#[test]
fn run_passes_the_procedures_arguments_through_and_its_exit_code_back() {
    let d = tmp("run");
    proc_file(&d, "calls", "echo \"ran with $*\"\nexit 3\n");
    let o = tool(
        &d,
        &[
            "run",
            "calls",
            "--dry-run",
            "--log",
            "x.log",
            "--part",
            "boot",
        ],
    );
    assert_eq!(o.status.code(), Some(3), "{}", text(&o));
    assert!(
        text(&o).contains("ran with --dry-run --log x.log --part boot"),
        "{}",
        text(&o)
    );
    let o = tool(&d, &["run", "nothing"]);
    assert_eq!(o.status.code(), Some(2), "{}", text(&o));
    assert!(
        text(&o).contains("procedures/nothing.rs") && text(&o).contains("calls"),
        "names the file it looked for and the procedures there are: {}",
        text(&o)
    );
}

#[test]
fn check_dry_runs_every_procedure_and_names_the_one_that_fails() {
    let d = tmp("check");
    proc_file(&d, "boot", "exit 0\n");
    proc_file(&d, "broken", "echo 'wrong device' >&2\nexit 2\n");
    proc_file(&d, "wake", "exit 0\n");
    let o = tool(&d, &["check"]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    let t = text(&o);
    assert!(t.contains("broken") && t.contains("wrong device"), "{t}");
    assert!(t.contains("boot: ok") && t.contains("wake: ok"), "{t}");
    // every file was dry-run, each with --dry-run and a log that is not the lane's
    let runs = fs::read_to_string(d.join("runs")).unwrap();
    assert_eq!(runs.lines().count(), 3, "{runs}");
    assert!(
        runs.lines()
            .all(|l| l.contains("--dry-run") && l.contains("--log")),
        "{runs}"
    );

    proc_file(&d, "broken", "exit 0\n");
    let o = tool(&d, &["check"]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    // a repo with no procedures has nothing to pass
    let e = tmp("check-none");
    let o = tool(&e, &["check"]);
    assert_eq!(o.status.code(), Some(2), "{}", text(&o));
}

#[test]
fn help_says_the_commands() {
    let d = tmp("help");
    let o = tool(&d, &["--help"]);
    assert!(o.status.success());
    let t = text(&o);
    for w in ["new", "run", "check", "LPOCO_ATTENDED_LIB"] {
        assert!(t.contains(w), "{w}: {t}");
    }
}

#[test]
fn a_codename_that_would_write_code_into_the_file_is_refused() {
    for bad in ["a\\\"b", "a\\nfn evil(){}\\n//", "Mixed"] {
        let d = tmp("codename");
        fs::write(
            d.join("device.toml"),
            format!("[device]\ncodename = \"{bad}\"\n"),
        )
        .unwrap();
        let o = tool(&d, &["new", "calls"]);
        assert_eq!(o.status.code(), Some(2), "{bad}: {}", text(&o));
        assert!(!d.join("procedures").exists(), "{bad}");
    }
}

#[test]
fn new_never_writes_through_a_link_and_the_file_is_runnable() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let d = tmp("link");
    fs::create_dir_all(d.join("procedures")).unwrap();
    let outside = d.join("outside.rs");
    symlink(&outside, d.join("procedures/calls.rs")).unwrap();
    let o = tool(&d, &["new", "calls"]);
    assert_eq!(o.status.code(), Some(2), "{}", text(&o));
    assert!(!outside.exists());
    let o = tool(&d, &["new", "wake"]);
    assert!(o.status.success(), "{}", text(&o));
    let mode = fs::metadata(d.join("procedures/wake.rs"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o111, 0o111);
}

#[test]
fn check_and_run_agree_on_what_a_procedure_is_and_check_hands_on_the_device() {
    let d = tmp("agree");
    proc_file(&d, "boot", "exit 0\n");
    proc_file(&d, "Notes", "exit 9\n");
    proc_file(&d, "steps", "exit 9\n");
    let o = tool(&d, &["check", "--device", "device.toml"]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    assert!(
        !text(&o).contains("Notes") && !text(&o).contains("steps:"),
        "{}",
        text(&o)
    );
    let runs = fs::read_to_string(d.join("runs")).unwrap();
    assert!(runs.contains("--device device.toml"), "{runs}");
    let o = tool(
        &d,
        &[
            "run",
            "boot",
            "--device",
            "device.toml",
            "--until",
            "FIRST_PROMPT",
        ],
    );
    assert!(o.status.success(), "{}", text(&o));
    let runs = fs::read_to_string(d.join("runs")).unwrap();
    assert!(
        runs.contains("--device device.toml --until FIRST_PROMPT"),
        "{runs}"
    );
}

#[test]
fn a_stray_argument_is_refused_naming_it_and_nothing_runs() {
    let d = tmp("stray");
    proc_file(&d, "calls", "echo ran\n");
    for args in [vec!["run", "calls", "extra"], vec!["new", "other", "again"]] {
        let o = tool(&d, &args);
        let t = text(&o);
        assert_eq!(o.status.code(), Some(2), "{args:?}: {t}");
        assert!(t.contains(args[args.len() - 1]), "{args:?} names it: {t}");
    }
    assert!(!d.join("runs").exists(), "the procedure was not started");
    assert!(
        !d.join("procedures/other.rs").exists(),
        "nothing was written"
    );
}

#[test]
fn a_flag_that_is_not_declared_is_refused_naming_it_and_nothing_runs() {
    let d = tmp("flag");
    proc_file(&d, "calls", "echo ran >> runs\n");
    let o = tool(&d, &["run", "calls", "--bogus"]);
    assert!(!o.status.success(), "{}", text(&o));
    assert!(text(&o).contains("--bogus"), "{}", text(&o));
    assert!(!d.join("runs").exists(), "the procedure was not started");
}
