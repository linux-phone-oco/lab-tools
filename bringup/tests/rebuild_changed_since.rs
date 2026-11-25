// SPDX-License-Identifier: MIT OR Apache-2.0
//! image-rebuild-changed-since (decision 2026-11-24-image-rebuild-runs-at-land): `image ensure --rebuild
//! --changed-since REV` rebuilds only the images whose sha256 or recipe differ from the device.toml at git REV, and
//! passes when none do. The manifests are typed here (a recipe's parts are named by sha256, and what is compared is
//! the declaration, not an image); no build is needed to see which images are chosen, because each chosen image is
//! named before its build starts and an unchosen one is never named.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lpoco-bringup-since-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn fixture(name: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
    )
    .unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn git(dir: &Path, args: &[&str]) {
    let o = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.org"])
        .args(args)
        .output()
        .unwrap();
    assert!(o.status.success(), "git {args:?}: {}", text(&o.stderr));
}

/// Two images made of a control by recipe: `a` and `b`, each with the cmdline given.
fn device(dir: &Path, a_cmdline: &str, b_cmdline: &str, b_sha: &str) {
    let images = format!(
        "[images.control]\npath = \"control.img\"\nsha256 = \"{C}\"\n\n\
         [images.a]\npath = \"a.img\"\nsha256 = \"{A}\"\n[images.a.recipe]\ncontrol = \"control\"\ncmdline = \"{a_cmdline}\"\n\n\
         [images.b]\npath = \"b.img\"\nsha256 = \"{b_sha}\"\n[images.b.recipe]\ncontrol = \"control\"\ncmdline = \"{b_cmdline}\"\n"
    );
    fs::write(
        dir.join("device.toml"),
        format!("{}\n{images}", fixture("device.toml")),
    )
    .unwrap();
}

fn repo(name: &str) -> PathBuf {
    let dir = tmp(name);
    git(&dir, &["init", "-q"]);
    device(&dir, "console=ttyMSM0 a=1", "console=ttyMSM0 b=1", B);
    git(&dir, &["add", "device.toml"]);
    git(&dir, &["commit", "-q", "-m", "declared"]);
    dir
}

fn since(dir: &Path, rev: &str) -> Output {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    Command::new(env!("CARGO_BIN_EXE_lpoco-bringup"))
        .current_dir(dir)
        .args([
            "image",
            "ensure",
            "--rebuild",
            "--changed-since",
            rev,
            "--source",
        ])
        .arg(source)
        .output()
        .unwrap()
}

/// A manifest pair that differs in one image's recipe rebuilds that image only; an identical pair rebuilds none
/// and passes.
#[test]
fn only_an_image_whose_recipe_differs_from_the_revision_is_rebuilt_and_none_changed_passes() {
    let dir = repo("recipe");

    let o = since(&dir, "HEAD");
    assert!(
        o.status.success(),
        "an identical declaration must pass: {}{}",
        text(&o.stdout),
        text(&o.stderr)
    );
    let said = format!("{}{}", text(&o.stdout), text(&o.stderr));
    assert!(said.contains("no image changed since HEAD"), "{said}");
    assert!(!said.contains("a: ") && !said.contains("b: "), "{said}");

    device(&dir, "console=ttyMSM0 a=1", "console=ttyMSM0 b=2", B);
    let o = since(&dir, "HEAD");
    let said = format!("{}{}", text(&o.stdout), text(&o.stderr));
    assert!(
        said.contains("b: changed since HEAD (its recipe)"),
        "the image whose recipe differs is chosen: {said}"
    );
    assert!(
        !said.contains("a: "),
        "an image that did not change is never named: {said}"
    );
    assert!(
        !said.contains("control: changed since"),
        "an image without a recipe is not rebuilt: {said}"
    );
}

/// A changed sha256 chooses the image too, and an image the revision did not have is a change.
#[test]
fn a_changed_sha256_or_an_image_the_revision_lacked_is_rebuilt() {
    let dir = repo("sha");
    device(&dir, "console=ttyMSM0 a=1", "console=ttyMSM0 b=1", C);
    let o = since(&dir, "HEAD");
    let said = format!("{}{}", text(&o.stdout), text(&o.stderr));
    assert!(
        said.contains("b: changed since HEAD (its sha256)"),
        "{said}"
    );
    assert!(!said.contains("a: "), "{said}");

    let dir = repo("new");
    fs::write(
        dir.join("device.toml"),
        fixture("device.toml")
            + "\n[images.control]\npath = \"control.img\"\nsha256 = \""
            + C
            + "\"\n",
    )
    .unwrap();
    git(&dir, &["commit", "-q", "-am", "only the control"]);
    device(&dir, "console=ttyMSM0 a=1", "console=ttyMSM0 b=1", B);
    let o = since(&dir, "HEAD");
    let said = format!("{}{}", text(&o.stdout), text(&o.stderr));
    assert!(
        said.contains("a: changed since HEAD (not declared there)")
            && !said.contains("control: changed since"),
        "{said}"
    );
}

/// A revision git does not know, or one with no device.toml, is refused in words, not taken for "nothing changed".
#[test]
fn a_revision_that_cannot_be_read_is_refused_not_taken_for_no_change() {
    let dir = repo("bad");
    let o = since(&dir, "no-such-rev");
    assert!(!o.status.success());
    assert!(
        text(&o.stderr).contains("cannot read device.toml at no-such-rev"),
        "{}",
        text(&o.stderr)
    );
}

/// `--changed-since` goes with `--rebuild`, and a NAME is one image: not both.
#[test]
fn changed_since_is_refused_without_rebuild_and_with_a_name() {
    let dir = repo("flags");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_lpoco-bringup"))
            .current_dir(&dir)
            .args(args)
            .output()
            .unwrap()
    };
    let o = run(&["image", "ensure", "--changed-since", "HEAD"]);
    assert!(!o.status.success());
    assert!(
        text(&o.stderr).contains("--changed-since goes with --rebuild"),
        "{}",
        text(&o.stderr)
    );
    let o = run(&[
        "image",
        "ensure",
        "a",
        "--rebuild",
        "--changed-since",
        "HEAD",
    ]);
    assert!(!o.status.success());
    assert!(
        text(&o.stderr).contains("--changed-since takes every image"),
        "{}",
        text(&o.stderr)
    );
}
