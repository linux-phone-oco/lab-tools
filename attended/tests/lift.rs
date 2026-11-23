// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco-attended lift`: every device repo's procedures/steps beside the library, near-duplicates flagged by
//! body (decision 2026-11-19-procedures-reflect-and-lift). The program under test is the real binary.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lpoco-lift-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn steps(root: &Path, repo: &str, file: &str, text: &str) -> PathBuf {
    let dir = root.join(repo).join("procedures/steps/src");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(file), text).unwrap();
    root.join(repo)
}

fn lift(root: &Path, repos: &[&str], extra: &[&str]) -> String {
    let list: Vec<String> = repos
        .iter()
        .map(|r| root.join(r).display().to_string())
        .collect();
    let o = Command::new(env!("CARGO_BIN_EXE_lpoco-attended"))
        .args(["lift", "--format", "csv", "--repos", &list.join(",")])
        .args(extra)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).into_owned()
}

const WAIT: &str = "pub fn wait_for_phone() -> Step {\n    // poll\n    retry(30, 2, || phone().sh(\"true\"))\n}\n";
const WAIT_RENAMED: &str =
    "pub fn shell_answers() -> Step {\n    retry(30,   2, || phone().sh(\"true\"))\n}\n";

fn row<'a>(out: &'a str, step: &str) -> Vec<&'a str> {
    out.lines().filter(|l| l.contains(step)).collect()
}

#[test]
fn a_step_copied_and_renamed_into_two_repos_is_one_flagged_pair_with_both_places() {
    let d = tmp("pair");
    steps(&d, "walleye", "lib.rs", WAIT);
    steps(&d, "surya", "lib.rs", WAIT_RENAMED);
    let out = lift(&d, &["walleye", "surya"], &[]);
    let flagged: Vec<&str> = out.lines().filter(|l| l.starts_with("lift,")).collect();
    assert_eq!(flagged.len(), 1, "{out}");
    assert!(
        flagged[0].contains("walleye") && flagged[0].contains("surya"),
        "{out}"
    );
    assert!(
        flagged[0].contains("wait_for_phone") && flagged[0].contains("shell_answers"),
        "{out}"
    );
}

#[test]
fn a_step_the_library_already_has_is_flagged_use_the_library() {
    let d = tmp("library");
    steps(&d, "walleye", "lib.rs", WAIT_RENAMED);
    fs::create_dir_all(d.join("lib")).unwrap();
    fs::write(d.join("lib/steps.rs"), WAIT).unwrap();
    let out = lift(
        &d,
        &["walleye"],
        &["--library", d.join("lib").to_str().unwrap()],
    );
    let r = row(&out, "shell_answers");
    assert_eq!(r.len(), 1, "{out}");
    assert!(r[0].starts_with("use-library,"), "{out}");
    assert!(r[0].contains("wait_for_phone"), "{out}");
}

#[test]
fn a_step_in_one_repo_only_is_listed_not_flagged() {
    let d = tmp("single");
    steps(
        &d,
        "walleye",
        "lib.rs",
        "pub fn only_here() -> Step {\n    step(\"x\", || 1)\n}\n",
    );
    steps(&d, "surya", "lib.rs", WAIT);
    let out = lift(&d, &["walleye", "surya"], &[]);
    let r = row(&out, "only_here");
    assert_eq!(r.len(), 1, "{out}");
    assert!(r[0].starts_with("listed,"), "{out}");
    assert!(!out.lines().any(|l| l.starts_with("lift,")), "{out}");
}

#[test]
fn three_differing_copies_of_a_name_are_flagged_two_differing_are_listed() {
    let d = tmp("differ");
    let v = |n: u32| {
        format!("pub fn boot_once() -> Step {{\n    retry({n}, 2, || phone().sh(\"true\"))\n}}\n")
    };
    steps(&d, "a", "lib.rs", &v(1));
    steps(&d, "b", "lib.rs", &v(2));
    let two = lift(&d, &["a", "b"], &[]);
    assert!(
        row(&two, "boot_once")
            .iter()
            .all(|l| l.starts_with("listed,")),
        "{two}"
    );
    steps(&d, "c", "lib.rs", &v(3));
    let three = lift(&d, &["a", "b", "c"], &[]);
    let r = row(&three, "boot_once");
    assert_eq!(r.len(), 1, "{three}");
    assert!(
        r[0].starts_with("lift-differing,") && r[0].contains('a') && r[0].contains('c'),
        "{three}"
    );
}

#[test]
fn a_quote_or_brace_in_a_char_literal_or_raw_string_loses_no_step() {
    let d = tmp("literals");
    let text = "pub fn p() { let c = '\"'; let b = '{'; let r = r#\"a \" }\"#; 1 }\npub fn q() -> u8 { 2 }\n";
    steps(&d, "a", "lib.rs", text);
    steps(&d, "b", "lib.rs", text);
    let out = lift(&d, &["a", "b"], &[]);
    assert!(
        row(&out, "q")
            .iter()
            .any(|l| l.starts_with("lift,") && l.contains("a:q|b:q")),
        "{out}"
    );
    assert!(
        row(&out, "p").iter().any(|l| l.starts_with("lift,")),
        "{out}"
    );
}

#[test]
fn a_name_in_three_repos_with_differing_bodies_is_flagged_even_when_two_agree() {
    let d = tmp("two-agree");
    let v =
        |n: u32| format!("pub fn q() -> Step {{\n    retry({n}, 2, || phone().sh(\"true\"))\n}}\n");
    steps(&d, "a", "lib.rs", &v(1));
    steps(&d, "b", "lib.rs", &v(1));
    steps(&d, "c", "lib.rs", &v(2));
    let out = lift(&d, &["a", "b", "c"], &[]);
    let r = row(&out, "q");
    assert_eq!(r.len(), 1, "{out}");
    assert!(r[0].starts_with("lift-differing,"), "{out}");
}

#[test]
fn a_file_that_cannot_be_read_whole_stops_the_run_naming_it() {
    let d = tmp("broken");
    steps(
        &d,
        "a",
        "lib.rs",
        "pub fn p() { let s = \"never closed; }\n",
    );
    let o = Command::new(env!("CARGO_BIN_EXE_lpoco-attended"))
        .args(["lift", "--repos", d.join("a").to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("lib.rs"));
}

const READY: &str = "pub fn phone_is_up() -> Group {\n    // reach it\n    group(\"the phone is up\", [wait_for_phone(60, 5), shell_answers()]).way_back(back())\n}\n";
const READY_RENAMED: &str = "pub fn reach_the_phone() -> crate::Group {\n    crate::group(\"the phone is up\", [wait_for_phone(60,   5), shell_answers()]).way_back(back())\n}\n";

#[test]
fn a_group_copied_and_renamed_into_two_repos_is_one_flagged_pair_with_both_places() {
    let d = tmp("group-pair");
    steps(&d, "walleye", "lib.rs", READY);
    steps(&d, "surya", "lib.rs", READY_RENAMED);
    let out = lift(&d, &["walleye", "surya"], &[]);
    let flagged: Vec<&str> = out
        .lines()
        .filter(|l| l.starts_with("lift-group,"))
        .collect();
    assert_eq!(flagged.len(), 1, "{out}");
    assert!(
        flagged[0].contains("walleye:phone_is_up") && flagged[0].contains("surya:reach_the_phone"),
        "{out}"
    );
}

#[test]
fn a_group_the_library_already_has_is_flagged_use_the_library() {
    let d = tmp("group-library");
    steps(&d, "walleye", "lib.rs", READY_RENAMED);
    fs::create_dir_all(d.join("lib")).unwrap();
    fs::write(d.join("lib/steps.rs"), READY).unwrap();
    let out = lift(
        &d,
        &["walleye"],
        &["--library", d.join("lib").to_str().unwrap()],
    );
    let r = row(&out, "reach_the_phone");
    assert_eq!(r.len(), 1, "{out}");
    assert!(r[0].starts_with("use-library,"), "{out}");
    assert!(r[0].contains("phone_is_up"), "{out}");
}

fn kinds(out: &str, step: &str) -> Vec<String> {
    row(out, step)
        .iter()
        .map(|l| l.split(',').next().unwrap().to_string())
        .collect()
}

#[test]
fn a_return_type_that_only_starts_with_group_or_holds_one_in_a_parameter_is_not_a_group() {
    let d = tmp("not-group");
    let kind = "pub fn k() -> GroupKind {\n    GroupKind::new(1)\n}\n";
    let param = "pub fn p(f: impl Fn() -> Group) -> Step {\n    step(\"x\", f)\n}\n";
    steps(&d, "walleye", "lib.rs", &format!("{kind}{param}"));
    steps(&d, "surya", "lib.rs", &format!("{kind}{param}"));
    let out = lift(&d, &["walleye", "surya"], &[]);
    assert_eq!(kinds(&out, "walleye:k"), ["lift"], "{out}");
    assert_eq!(kinds(&out, "walleye:p"), ["lift"], "{out}");
}

#[test]
fn a_group_is_a_group_by_any_path_to_the_type_and_a_renamed_copy_may_come_first() {
    let d = tmp("group-paths");
    let a = "pub fn up() -> lpoco_attended::Group {\n    group(\"up\", [s()])\n}\n";
    let b = "pub fn ready() -> self::Group {\n    group(\"up\", [s()])\n}\n";
    let c = "pub fn alive() -> Group {\n    group(\"up\", [s()])\n}\n";
    steps(&d, "a", "lib.rs", a);
    steps(&d, "b", "lib.rs", b);
    steps(&d, "c", "lib.rs", c);
    let out = lift(&d, &["a", "b", "c"], &[]);
    let flagged: Vec<&str> = out
        .lines()
        .filter(|l| l.starts_with("lift-group,"))
        .collect();
    assert_eq!(flagged.len(), 1, "{out}");
    assert!(
        flagged[0].contains("a:up")
            && flagged[0].contains("b:ready")
            && flagged[0].contains("c:alive"),
        "{out}"
    );
}

#[test]
fn two_calls_that_differ_by_their_module_path_stay_two_different_steps() {
    let d = tmp("paths-differ");
    steps(
        &d,
        "a",
        "lib.rs",
        "pub fn go() -> Step {\n    crate::one::run()\n}\n",
    );
    steps(
        &d,
        "b",
        "lib.rs",
        "pub fn go2() -> Step {\n    crate::two::run()\n}\n",
    );
    let out = lift(&d, &["a", "b"], &[]);
    assert!(
        out.lines().skip(1).all(|l| l.starts_with("listed,")),
        "{out}"
    );
}
