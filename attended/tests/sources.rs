// SPDX-License-Identifier: MIT OR Apache-2.0
//! A check names its source (examples/sources.rs, run by rust-script against this very checkout): the dry run
//! refuses, with exit 1 naming the check, one that names none, a file that is not there, a file with nothing in it
//! and a text file outside a research folder (a typed stand-in, not text a phone printed); one that names a research
//! row, a doc or a captured-text file passes. Source of the rule: ideal-agent decision
//! 2026-11-23-a-stop-is-read-before-the-fix (two walleye runs lost on 2026-11-23 to a check typed as a stand-in).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// A device repo in a folder of its own: its device.toml, a doc, a file a phone printed, and files that are not that.
fn repo(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lpoco-sources-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(d.join("research/phone")).unwrap();
    fs::create_dir_all(d.join("docs")).unwrap();
    fs::create_dir_all(d.join("tests")).unwrap();
    fs::write(
        d.join("device.toml"),
        "[device]\ncodename = \"xiaomi-surya\"\n",
    )
    .unwrap();
    fs::write(
        d.join("research/phone/uname.txt"),
        "6.12.0-pmicwdt #1 SMP PREEMPT aarch64\n",
    )
    .unwrap();
    fs::write(d.join("research/phone/empty.txt"), "").unwrap();
    fs::write(
        d.join("docs/boot.md"),
        "# Boot\nuname -r names the kernel.\n",
    )
    .unwrap();
    fs::write(d.join("tests/typed.txt"), "6.12.0-pmicwdt\n").unwrap();
    fs::create_dir_all(d.join("tests/research")).unwrap();
    fs::write(d.join("tests/research/typed.txt"), "6.12.0-pmicwdt\n").unwrap();
    let outside = d.with_extension("outside.txt");
    fs::write(&outside, "text from beyond the repository\n").unwrap();
    std::os::unix::fs::symlink(&outside, d.join("research/phone/out.txt")).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = fs::read_to_string(root.join("examples/sources.rs")).unwrap();
    let git = src
        .lines()
        .find(|l| l.starts_with("//! lpoco-attended = { git"))
        .expect("the example depends on lpoco-attended by git tag");
    let local = format!("//! lpoco-attended = {{ path = \"{}\" }}", root.display());
    fs::write(d.join("sources.rs"), src.replace(git, &local)).unwrap();
    d
}

/// The dry run of the example's check `part`, in `dir`.
fn dry(dir: &Path, part: &str) -> (Output, String) {
    let o = Command::new("rust-script")
        .arg(dir.join("sources.rs"))
        .args(["--dry-run", "--part", part, "--log"])
        .arg(dir.join("run.log"))
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .expect("rust-script is on the lab machine");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    (o, said)
}

fn refused(part: &str, why: &str) {
    let dir = repo(part);
    let (o, said) = dry(&dir, part);
    assert_eq!(o.status.code(), Some(1), "{said}");
    assert!(
        said.contains("\"the kernel is named\""),
        "the check is not named: {said}"
    );
    assert!(said.contains(why), "{why:?} not said: {said}");
}

fn passes(part: &str) {
    let dir = repo(part);
    let (o, said) = dry(&dir, part);
    assert_eq!(o.status.code(), Some(0), "{said}");
    assert!(said.contains("the kernel is named"), "{said}");
}

#[test]
fn a_check_that_names_no_source_is_refused_by_the_dry_run() {
    refused("none", "names no source");
}

#[test]
fn a_source_that_is_empty_words_is_refused_as_none() {
    refused("blank", "names no source");
}

#[test]
fn a_source_file_that_is_not_there_is_refused_and_named() {
    refused("missing", "research/phone/gone.txt");
}

#[test]
fn a_captured_text_file_with_nothing_in_it_is_refused() {
    refused("empty", "has nothing in it");
}

#[test]
fn a_text_file_outside_a_research_folder_is_not_captured_text() {
    refused("typed", "not under a research folder");
}

#[test]
fn a_research_row_a_doc_and_a_captured_text_file_each_pass() {
    passes("research");
    passes("doc");
    passes("captured");
}

#[test]
fn the_refusal_is_filed_in_the_log() {
    let dir = repo("log");
    let (_, said) = dry(&dir, "none");
    let log = fs::read_to_string(dir.join("run.log")).unwrap();
    assert!(
        log.contains("FAIL") && log.contains("the kernel is named"),
        "{log}\n{said}"
    );
}

#[test]
fn a_link_out_of_the_repository_is_not_a_source() {
    refused("link", "not a file inside the repository");
}

#[test]
fn a_research_folder_that_is_not_the_repositorys_own_does_not_make_captured_text() {
    refused("nested", "not under a research folder");
}
