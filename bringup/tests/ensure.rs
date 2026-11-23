// SPDX-License-Identifier: MIT OR Apache-2.0
//! image-recipe-ensure-z21: `lpoco-bringup image ensure NAME` gives an image by its declared sha256 - from Nix's
//! store, else from the file:// cache, else built from its recipe in Nix's sandbox as a fixed-output derivation
//! (decision 2026-11-23-image-recipes-on-nix-fixed-output-paths). The recipe here is the shape of z21's: a control
//! image (an [images] name, by sha256) with parts swapped, each part named by sha256. Where nix or avbtool is
//! absent the test says "not measured". The expected sha256 is what `image swap` makes of the same inputs, the
//! way z21's declared sha256 was measured (research image-recipes-how-it-stands, measurement 1).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const ID: std::ops::Range<usize> = 576..608; // the header's id field
const PART: &str = "131072";

fn fixture(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .to_str()
        .unwrap()
        .to_string()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "lpoco-bringup-ensure-{}-{name}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn have(tool: &str, arg: &str) -> bool {
    Command::new(tool)
        .arg(arg)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn bringup(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lpoco-bringup"))
        .args(args)
        .output()
        .unwrap()
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn sha256(path: &Path) -> String {
    let o = Command::new("sha256sum").arg(path).output().unwrap();
    text(&o.stdout)
        .split_whitespace()
        .next()
        .unwrap()
        .to_string()
}

/// The path Nix keys a file of this sha256 and name to (nix-store's own computation).
fn fixed_path(sha: &str, name: &str) -> String {
    let o = Command::new("nix-store")
        .args(["--print-fixed-path", "sha256", sha, name])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", text(&o.stderr));
    text(&o.stdout).trim().to_string()
}

fn forget(path: &str) {
    let _ = Command::new("nix-store").args(["--delete", path]).output();
}

/// The control: a packed image, its id set to a stock-like value, then an AVB hash footer (as tests/swap.rs).
fn control(dir: &Path) -> PathBuf {
    let device = dir.join("small.toml");
    let t = fs::read_to_string(fixture("device.toml"))
        .unwrap()
        .replace("0x2000000", "0x20000");
    fs::write(&device, t).unwrap();
    let img = dir.join("control.img");
    let o = bringup(&[
        "image",
        "pack",
        "--device",
        s(&device),
        "--kernel",
        &fixture("kernel"),
        "--dtb",
        &fixture("appended.dtb"),
        "--ramdisk",
        &fixture("ramdisk"),
        "--out",
        s(&img),
    ]);
    assert!(o.status.success(), "pack: {}", text(&o.stderr));
    let mut b = fs::read(&img).unwrap();
    b[ID].fill(0xAA);
    fs::write(&img, b).unwrap();
    let o = Command::new("avbtool")
        .args([
            "add_hash_footer",
            "--image",
            s(&img),
            "--partition_size",
            PART,
            "--partition_name",
            "boot",
            "--algorithm",
            "NONE",
            "--salt",
            "aabbccdd",
            "--prop",
            "com.example:one",
        ])
        .output()
        .unwrap();
    assert!(o.status.success(), "avbtool: {}", text(&o.stderr));
    img
}

/// A manifest with the control, and `out` made of it with a new ramdisk (named by sha256) and command line.
fn manifest(dir: &Path, control_sha: &str, ramdisk_sha: &str, out_sha: &str) -> PathBuf {
    let t = format!(
        "[images.control]\npath = \"control.img\"\nsha256 = \"{control_sha}\"\n\n\
         [images.out]\npath = \"out.img\"\nsha256 = \"{out_sha}\"\n\n\
         [images.out.recipe]\ncontrol = \"control\"\nramdisk = \"{ramdisk_sha}\"\ncmdline = \"console=ttyMSM0 ensure=1\"\n"
    );
    let base = fs::read_to_string(fixture("device.toml")).unwrap();
    let p = dir.join("device.toml");
    fs::write(&p, format!("{base}\n{t}")).unwrap();
    p
}

fn ensure(device: &Path, cache: &Path, seed: &Path, name: &str) -> Output {
    // the family's own source is what the sandbox builds lpoco-bringup from
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    bringup(&[
        "image",
        "ensure",
        name,
        "--device",
        s(device),
        "--cache",
        s(cache),
        "--seed",
        s(seed),
        "--source",
        s(&source),
    ])
}

#[test]
fn an_image_is_served_by_its_sha256_from_the_store_or_the_cache_else_built_from_its_recipe_and_checked(
) {
    // the sandbox pins avbtool 1.4.0 (ensure.nix): the expected image is made with the same one
    if !have("nix-store", "--version") || !avbtool_is_1_4() {
        eprintln!("not measured: no nix, or no avbtool 1.4 (the sandbox's)");
        return;
    }
    let dir = tmp("z");
    let seed = dir.join("seed");
    let cache = dir.join("cache");
    fs::create_dir_all(&seed).unwrap();
    let ctl = control(&dir);
    fs::rename(&ctl, seed.join("control.img")).unwrap();
    fs::write(seed.join("ramdisk.new"), vec![0x5Au8; 5000]).unwrap();
    let (control_sha, ramdisk_sha) = (
        sha256(&seed.join("control.img")),
        sha256(&seed.join("ramdisk.new")),
    );

    // the expectation: what `image swap` makes of the same inputs
    let want = dir.join("want.img");
    let o = bringup(&[
        "image",
        "swap",
        s(&seed.join("control.img")),
        "--ramdisk",
        s(&seed.join("ramdisk.new")),
        "--cmdline",
        "console=ttyMSM0 ensure=1",
        "--out",
        s(&want),
    ]);
    assert!(o.status.success(), "swap: {}", text(&o.stderr));
    let out_sha = sha256(&want);
    let device = manifest(&dir, &control_sha, &ramdisk_sha, &out_sha);
    let out_path = fixed_path(&out_sha, "out.img");
    for p in [
        &out_path,
        &fixed_path(&control_sha, "control.img"),
        &fixed_path(&ramdisk_sha, &format!("ramdisk-{}", &ramdisk_sha[..12])),
    ] {
        forget(p);
    }

    // 1. nowhere: built from the recipe; Nix itself holds it to the declared sha256
    let o = ensure(&device, &cache, &seed, "out");
    assert!(
        o.status.success(),
        "ensure (build): {}{}",
        text(&o.stdout),
        text(&o.stderr)
    );
    let said = text(&o.stderr);
    assert_eq!(
        text(&o.stdout).trim(),
        out_path,
        "stdout is the store path alone"
    );
    assert!(said.contains("built"), "{said}");
    assert!(said.contains("built from its recipe"), "{said}");
    assert_eq!(
        sha256(Path::new(&out_path)),
        out_sha,
        "the built image is not the declared one"
    );

    // 2. in the store: nothing runs
    let o = ensure(&device, &cache, &seed, "out");
    assert!(
        text(&o.stderr).contains("in the store"),
        "{}{}",
        text(&o.stderr),
        text(&o.stderr)
    );
    assert!(!text(&o.stdout).contains("built"));

    // 3. gone from the store, in the cache: served from it, the builder not run
    forget(&out_path);
    let o = ensure(&device, &cache, &seed, "out");
    assert!(
        o.status.success(),
        "ensure (cache): {}{}",
        text(&o.stderr),
        text(&o.stderr)
    );
    let said = text(&o.stderr);
    assert!(
        said.contains("from the cache") && !said.contains("built"),
        "{said}"
    );
    assert_eq!(sha256(Path::new(&out_path)), out_sha);

    // 4. a declared sha256 the recipe does not give: refused by Nix, nothing in the store
    let wrong = "0".repeat(64);
    let device = manifest(&dir, &control_sha, &ramdisk_sha, &wrong);
    let o = ensure(&device, &cache, &seed, "out");
    assert!(
        !o.status.success(),
        "a recipe that does not give the declared sha256 passed"
    );
    assert!(
        !valid(&fixed_path(&wrong, "out.img")),
        "a refused build is in the store"
    );
    // Nix leaves the refused file at the path, not registered: a second ensure must not take it for the image
    let o = ensure(&device, &cache, &seed, "out");
    assert!(
        !o.status.success() && !text(&o.stderr).contains("out: in the store"),
        "{}",
        text(&o.stderr)
    );
}

fn valid(path: &str) -> bool {
    Command::new("nix-store")
        .args(["--check-validity", path])
        .output()
        .unwrap()
        .status
        .success()
}

fn avbtool_is_1_4() -> bool {
    Command::new("avbtool")
        .arg("version")
        .output()
        .is_ok_and(|o| text(&o.stdout).trim().ends_with(" 1.4.0"))
}

/// image-ensure-in-attended: `image ensure NAME --dry-run` says where the image would come from - the store, the
/// cache, the seed or its recipe's build - and changes nothing (a procedure's dry run asks it before a boot).
#[test]
fn a_dry_run_says_where_the_image_would_come_from_and_puts_nothing_in_the_store() {
    if !have("nix-store", "--version") {
        eprintln!("not measured: no nix");
        return;
    }
    let dir = tmp("plan");
    let (seed, cache) = (dir.join("seed"), dir.join("cache"));
    fs::create_dir_all(&seed).unwrap();
    // a file of its own bytes: this run's, so no earlier run's store path is taken for it
    let body = format!(
        "plan {} {:?}",
        std::process::id(),
        std::time::SystemTime::now()
    );
    fs::write(seed.join("plain.img"), &body).unwrap();
    let sha = sha256(&seed.join("plain.img"));
    let built = "7".repeat(64);
    let t = format!(
        "{}\n[images.plain]\npath = \"plain.img\"\nsha256 = \"{sha}\"\n\n\
         [images.built]\npath = \"built.img\"\nsha256 = \"{built}\"\n\n\
         [images.built.recipe]\ncontrol = \"plain\"\ncmdline = \"x\"\n",
        fs::read_to_string(fixture("device.toml")).unwrap()
    );
    let device = dir.join("device.toml");
    fs::write(&device, t).unwrap();
    let path = fixed_path(&sha, "plain.img");
    forget(&path);
    let plan = |name: &str, extra: &[&str]| {
        let mut a = vec!["image", "ensure", name, "--dry-run", "--device", s(&device)];
        a.extend_from_slice(extra);
        bringup(&a)
    };

    // the seed holds it: said, and not taken into the store
    let o = plan("plain", &["--seed", s(&seed)]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert!(
        text(&o.stdout).contains("from the seed"),
        "{}",
        text(&o.stdout)
    );
    assert!(!valid(&path), "a dry run put the image in the store");

    // really ensured once: in the store and the cache; then gone from the store
    let o = ensure(&device, &cache, &seed, "plain");
    assert!(o.status.success(), "{}", text(&o.stderr));
    forget(&path);

    // the cache alone holds it: said, and not copied back
    let o = plan("plain", &["--cache", s(&cache)]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert!(
        text(&o.stdout).contains("from the cache"),
        "{}",
        text(&o.stdout)
    );
    assert!(!valid(&path), "a dry run copied from the cache");

    // in the store: said
    let o = ensure(&device, &cache, &seed, "plain");
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert!(text(&plan("plain", &[]).stdout).contains("in the store"));

    // nowhere, with a recipe: it would be built; without one: refused
    let o = plan("built", &[]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert!(
        text(&o.stdout).contains("built from its recipe"),
        "{}",
        text(&o.stdout)
    );
    forget(&path);
    let o = plan("plain", &[]);
    assert!(
        !o.status.success(),
        "an image nothing holds and no recipe makes passed"
    );
    assert!(text(&o.stderr).contains("no recipe"), "{}", text(&o.stderr));
}

/// A flag called dry-run never writes: the verbs that write refuse it.
#[test]
fn dry_run_is_refused_by_every_verb_but_ensure() {
    let o = bringup(&[
        "image",
        "swap",
        "x.img",
        "--out",
        "/nonexistent/o.img",
        "--dry-run",
    ]);
    assert_eq!(o.status.code(), Some(2), "{}", text(&o.stderr));
    assert!(
        text(&o.stderr).contains("--dry-run is for image ensure"),
        "{}",
        text(&o.stderr)
    );
}
