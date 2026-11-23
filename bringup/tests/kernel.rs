// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco-bringup kernel assemble` and `kernel publish` (proof: lab-tools bringup tests, backlog item
//! kernel-assemble-from-topics): a throwaway kernel repository, a topics.toml, a `tad` that prints the base's
//! carried items and a `rhizoid` that writes down how it was called. A topic with no upstream-item row or a
//! disagreeing FROMLIST subject is refused before rhizoid runs; a clean list calls rhizoid assemble; the int/ tag
//! is pushed, and rhizoid's worktree ended, once an image names it.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "lpoco-bringup-kernel-{}-{name}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn git(dir: &Path, args: &[&str]) {
    let o = Command::new("git")
        .current_dir(dir)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.org",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

fn script(dir: &Path, name: &str, body: &str) -> String {
    let p = dir.join(name);
    fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    p.to_str().unwrap().to_string()
}

struct Lab {
    dir: PathBuf,
    repo: PathBuf,
    origin: PathBuf,
    tad: String,
    rhizoid: String,
    lists: String,
    called: PathBuf,
}

/// A kernel repository whose topics carry the subjects given, a tad that says item `one` is FROMLIST and
/// item `two` OURS, and a rhizoid that appends its arguments to `called`.
fn lab(name: &str, subject_one: &str) -> Lab {
    let dir = tmp(name);
    let repo = dir.join("linux");
    let origin = dir.join("origin.git");
    fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    fs::write(repo.join("f"), "base\n").unwrap();
    git(&repo, &["add", "f"]);
    git(&repo, &["commit", "-q", "-m", "base"]);
    git(&repo, &["tag", "v7.1"]);
    for (item, subject) in [("one", subject_one), ("two", "OURS(two): a try")] {
        git(
            &repo,
            &["checkout", "-q", "-b", &format!("topic/{item}"), "v7.1"],
        );
        fs::write(repo.join(item), "x\n").unwrap();
        git(&repo, &["add", item]);
        git(&repo, &["commit", "-q", "-m", subject]);
    }
    git(&repo, &["checkout", "-q", "main"]);
    fs::create_dir_all(&origin).unwrap();
    git(&origin, &["init", "-q", "--bare"]);
    git(
        &repo,
        &["remote", "add", "origin", origin.to_str().unwrap()],
    );
    let called = dir.join("rhizoid.called");
    let tad = script(
        &dir,
        "tad",
        "echo 'id,upstream,carried_in,state,tag,applied_upstream,last_update,fork_does,title'\n\
         echo 'one,lkml,linux/topic/one,sent,FROMLIST,NULL,2026-11-23,wait,\"[PATCH 1/1] one, a title\"'\n\
         echo 'two,lkml,linux/topic/two,withdrawn,OURS,NULL,2026-11-23,ask,two'",
    );
    let rhizoid = script(
        &dir,
        "rhizoid",
        &format!(
            "echo \"$@\" >> {}\n\
             case \"$1\" in assemble) case \"$2\" in --done) : ;; *) echo '{{\"tag\":\"int/surya/ufs-off/v7.1/1\"}}' ;; esac ;; esac",
            called.display()
        ),
    );
    let lists = dir.join("topics.toml");
    fs::write(
        &lists,
        "name = \"surya\"\n[lists.ufs-off]\nbase = \"v7.1\"\n\
         [[lists.ufs-off.entries]]\nbranch = \"topic/one\"\nfor = \"one\"\nstate = \"trying\"\n\
         [[lists.ufs-off.entries]]\nbranch = \"topic/two\"\nfor = \"two\"\nstate = \"trying\"\n",
    )
    .unwrap();
    Lab {
        dir,
        repo,
        origin,
        tad,
        rhizoid,
        lists: lists.to_str().unwrap().into(),
        called,
    }
}

fn records(l: &Lab) -> String {
    l.dir.to_str().unwrap().to_string()
}

fn bringup(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lpoco-bringup"))
        .args(args)
        .output()
        .unwrap()
}

fn assemble(l: &Lab) -> Output {
    let (repo, rec) = (l.repo.to_str().unwrap().to_string(), records(l));
    bringup(&[
        "kernel",
        "assemble",
        "ufs-off",
        "--lists",
        &l.lists,
        "--repo",
        &repo,
        "--records",
        &rec,
        "--tad",
        &l.tad,
        "--rhizoid",
        &l.rhizoid,
    ])
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

#[test]
fn a_clean_list_calls_rhizoid_assemble() {
    let l = lab("clean", "FROMLIST(one): a series");
    let o = assemble(&l);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    let said = fs::read_to_string(&l.called).unwrap();
    assert!(said.starts_with("assemble ufs-off --lists "), "{said}");
    assert!(
        said.contains(&format!("--repo {}", l.repo.display())) && said.contains("--json"),
        "{said}"
    );
    assert!(
        text(&o).contains("int/surya/ufs-off/v7.1/1"),
        "the tag is said: {}",
        text(&o)
    );
}

#[test]
fn a_topic_with_no_upstream_item_row_is_refused_before_rhizoid_runs() {
    let l = lab("norow", "FROMLIST(one): a series");
    git(&l.repo, &["branch", "topic/ghost", "topic/one"]);
    let lists = fs::read_to_string(&l.lists).unwrap()
        + "[[lists.ufs-off.entries]]\nbranch = \"topic/ghost\"\nfor = \"g\"\nstate = \"trying\"\n";
    fs::write(&l.lists, lists).unwrap();
    let o = assemble(&l);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    assert!(
        text(&o).contains("topic/ghost") && text(&o).contains("no upstream-item row"),
        "{}",
        text(&o)
    );
    assert!(!l.called.exists(), "rhizoid ran");
}

#[test]
fn a_disagreeing_fromlist_subject_is_refused_before_rhizoid_runs() {
    let l = lab("wrongtag", "FROMGIT(one): a series");
    let o = assemble(&l);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    assert!(
        text(&o).contains("FROMGIT(one) but the item says FROMLIST"),
        "{}",
        text(&o)
    );
    assert!(!l.called.exists(), "rhizoid ran");
}

#[test]
fn an_untagged_subject_is_refused() {
    let l = lab("untagged", "a series with no tag");
    let o = assemble(&l);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    assert!(text(&o).contains("no tag"), "{}", text(&o));
}

fn device(l: &Lab, kernel_tag: &str) -> String {
    let p = l.dir.join("device.toml");
    fs::write(
        &p,
        format!(
            "[device]\ncodename = \"surya\"\n[connection]\nuser = \"u\"\naddresses = [\"a\"]\n\
             [images.ufs-off]\npath = \"images/a.img\"\nkernel_tag = \"{kernel_tag}\"\n"
        ),
    )
    .unwrap();
    p.to_str().unwrap().to_string()
}

fn publish(l: &Lab, dev: &str, tag: &str) -> Output {
    git(&l.repo, &["tag", "int/surya/ufs-off/v7.1/1"]);
    bringup(&[
        "kernel",
        "publish",
        tag,
        "--repo",
        l.repo.to_str().unwrap(),
        "--device",
        dev,
        "--rhizoid",
        &l.rhizoid,
    ])
}

#[test]
fn the_tag_is_pushed_and_its_worktree_ended_once_an_image_names_it() {
    let l = lab("publish", "FROMLIST(one): a series");
    let tag = "int/surya/ufs-off/v7.1/1";
    let o = publish(&l, &device(&l, tag), tag);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    let pushed = Command::new("git")
        .current_dir(&l.origin)
        .args(["tag", "-l", tag])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&pushed.stdout).trim(), tag);
    let said = fs::read_to_string(&l.called).unwrap();
    assert!(
        said.contains(&format!(
            "assemble --done {tag} --repo {} --force",
            l.repo.display()
        )),
        "{said}"
    );
}

#[test]
fn a_tag_no_image_names_is_not_pushed() {
    let l = lab("unnamed", "FROMLIST(one): a series");
    let tag = "int/surya/ufs-off/v7.1/1";
    let o = publish(&l, &device(&l, "int/surya/ufs-on/v7.1/1"), tag);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    let pushed = Command::new("git")
        .current_dir(&l.origin)
        .args(["tag", "-l"])
        .output()
        .unwrap();
    assert!(pushed.stdout.is_empty(), "pushed");
    assert!(!l.called.exists(), "rhizoid ran");
}

#[test]
fn a_conflict_report_is_printed_and_ends_1() {
    let l = lab("conflict", "FROMLIST(one): a series");
    let rh = script(
        &l.dir,
        "rhizoid-conflict",
        "echo '{\"conflict\":{\"entry\":\"topic/two\",\"with\":\"topic/one\"}}'; exit 1",
    );
    let (repo, rec) = (l.repo.to_str().unwrap().to_string(), records(&l));
    let o = bringup(&[
        "kernel",
        "assemble",
        "ufs-off",
        "--lists",
        &l.lists,
        "--repo",
        &repo,
        "--records",
        &rec,
        "--tad",
        &l.tad,
        "--rhizoid",
        &rh,
    ]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    assert!(
        text(&o).contains("topic/two") && text(&o).contains("topic/one"),
        "{}",
        text(&o)
    );
}
