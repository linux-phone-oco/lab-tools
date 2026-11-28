// SPDX-License-Identifier: MIT OR Apache-2.0
//! The root file system of an image, made without root: a root tree installed by the profile's package format,
//! an ext4 image of it (mke2fs -d from a tar read under fakeroot, so every file is root's), built twice and
//! compared. `--dry-run` prints each command and runs none. Source of the image steps: see tests/rootfs_apk.rs.

use crate::distro::{self, Profile};
use crate::pkgformat::{self, PackageFormat, Tools};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Every file's time, so the image is the same twice (2026-01-01 UTC).
const FILE_EPOCH: &str = "1767225600";
/// The image's file system id and hash seed: fixed, so the image is the same twice.
const FS_UUID: &str = "7e1c3a52-0b6d-4f0e-9a41-2d8a6c1e5b90";

pub struct Request {
    pub distro_file: PathBuf,
    pub arch: String,
    pub out: PathBuf,
    pub size_mib: u64,
    pub dry_run: bool,
}

/// What the procedure does, in order; each step is one named call below.
pub fn make(req: &Request) -> Result<(), String> {
    let text = fs::read_to_string(&req.distro_file)
        .map_err(|e| format!("{}: {e}", req.distro_file.display()))?;
    let profile = distro::read(&text)?;
    let format = pkgformat::for_format(&profile.distro.format)?;
    let sh = Shell { dry: req.dry_run };
    let work = req.out.join("work");
    sh.run(&["mkdir", "-p", path(&work)], &[])?;
    let tools = fetch_tools(&sh, &*format, &profile, &work)?;
    let first = build_image(&sh, &*format, &profile, &tools, req, 1)?;
    let second = build_image(&sh, &*format, &profile, &tools, req, 2)?;
    publish(&sh, req, &first, &second)
}

/// The pinned downloads, each checked by its sha256 before it is unpacked into WORK/<into>.
fn fetch_tools(
    sh: &Shell,
    format: &dyn PackageFormat,
    profile: &Profile,
    work: &Path,
) -> Result<Tools, String> {
    let dl = work.join("dl");
    sh.run(&["mkdir", "-p", path(&dl)], &[])?;
    for f in format.fetches(profile, std::env::consts::ARCH)? {
        let file = dl.join(&f.file);
        let pin = profile.pinned(&f.package)?;
        if !sh.dry && file.exists() && sha256(&file)? == pin.sha256 {
            say(&format!("pinned: {} already downloaded", f.file));
        } else {
            sh.run(
                &[
                    "curl",
                    "-sSfL",
                    "--proto",
                    "=https",
                    "--proto-redir",
                    "=https",
                    "--retry",
                    "3",
                    &f.url,
                    "-o",
                    path(&file),
                ],
                &[],
            )?;
        }
        sh.check(&format!("{} sha256 is {}", f.file, pin.sha256), || {
            let got = sha256(&file)?;
            (got == pin.sha256).then_some(()).ok_or(format!(
                "{}: sha256 is {got}, the profile pins {}",
                f.file, pin.sha256
            ))
        })?;
        let into = work.join(f.into);
        sh.run(&["rm", "-rf", path(&into)], &[])?;
        sh.run(&["mkdir", "-p", path(&into)], &[])?;
        sh.run(&["tar", "-xzf", path(&file), "-C", path(&into)], &[])?;
    }
    Ok(format.tools(work))
}

/// One whole build: the tree, then the image of it. Returns the image's path.
fn build_image(
    sh: &Shell,
    format: &dyn PackageFormat,
    profile: &Profile,
    tools: &Tools,
    req: &Request,
    n: u32,
) -> Result<PathBuf, String> {
    let work = req.out.join("work");
    let tree = work.join(format!("tree-{n}"));
    let img = work.join(format!("build-{n}.ext4"));
    say(&format!(
        "build {n}: install the packages into {}",
        tree.display()
    ));
    sh.run(&["rm", "-rf", path(&tree)], &[])?;
    sh.run(&["mkdir", "-p", path(&tree)], &[])?;
    // the root directory's mode goes into the image: fixed, never the caller's umask
    sh.run(&["chmod", "0755", path(&tree)], &[])?;
    let install = format.root_tree(profile, tools, &tree, &req.arch);
    sh.run_all(&install, &[])?;
    for record in format.build_records() {
        sh.run(&["rm", "-f", path(&tree.join(record))], &[])?;
    }
    say(&format!("build {n}: make the ext4 image"));
    make_image(sh, format, profile, &tree, &img, req.size_mib, n)?;
    Ok(img)
}

/// ext4 from the tree, every file root's (fakeroot), every time fixed: the same bytes twice.
fn make_image(
    sh: &Shell,
    format: &dyn PackageFormat,
    profile: &Profile,
    tree: &Path,
    img: &Path,
    size_mib: u64,
    n: u32,
) -> Result<(), String> {
    let work = tree.parent().unwrap_or(tree);
    let state = work.join(format!("fakeroot-{n}.state"));
    let tarball = PathBuf::from(format!("{}.tar", tree.display()));
    let epoch = format!("@{FILE_EPOCH}");
    sh.run(&["rm", "-f", path(img), path(&state)], &[])?;
    sh.run(
        &[
            "find",
            path(tree),
            "-exec",
            "touch",
            "-h",
            "-d",
            &epoch,
            "{}",
            "+",
        ],
        &[],
    )?;
    sh.run(
        &[
            "fakeroot",
            "-s",
            path(&state),
            "--",
            "chown",
            "-R",
            "-h",
            "0:0",
            path(tree),
        ],
        &[],
    )?;
    apply_owners_and_modes(sh, format, tree, &state)?;
    // a tar carries owner, mode and mtime and no ctime: mke2fs copies from it, the same bytes every time
    let mtime = format!("--mtime={epoch}");
    sh.run(
        &[
            "fakeroot",
            "-i",
            path(&state),
            "--",
            "tar",
            "-C",
            path(tree),
            "--sort=name",
            "--numeric-owner",
            "--format=gnu",
            &mtime,
            "-cf",
            path(&tarball),
            ".",
        ],
        &[],
    )?;
    sh.run(&["truncate", "-s", &format!("{size_mib}M"), path(img)], &[])?;
    let seed = format!("hash_seed={FS_UUID}");
    sh.run(
        &[
            "mke2fs",
            "-q",
            "-F",
            "-t",
            "ext4",
            "-O",
            "^has_journal",
            "-L",
            &profile.distro.name,
            "-U",
            FS_UUID,
            "-E",
            &seed,
            "-d",
            path(&tarball),
            path(img),
        ],
        &[("E2FSPROGS_FAKE_TIME", FILE_EPOCH)],
    )
}

/// What the package tool could not apply as an ordinary user, applied through fakeroot's record of the tree.
fn apply_owners_and_modes(
    sh: &Shell,
    format: &dyn PackageFormat,
    tree: &Path,
    state: &Path,
) -> Result<(), String> {
    if sh.dry {
        say("fakeroot -i STATE -s STATE -- chown and chmod, per the package database's owner and mode groups");
        return Ok(());
    }
    for cmd in format.owners_and_modes(tree)? {
        let mut argv = vec!["fakeroot", "-i", path(state), "-s", path(state), "--"];
        argv.extend(cmd.iter().map(String::as_str));
        sh.run(&argv, &[])?;
    }
    Ok(())
}

/// The two builds must be the same bytes; the first becomes OUT/rootfs.img and its sha256 is printed.
fn publish(sh: &Shell, req: &Request, first: &Path, second: &Path) -> Result<(), String> {
    let image = req.out.join("rootfs.img");
    sh.check("the two builds are the same bytes", || {
        let (a, b) = (sha256(first)?, sha256(second)?);
        (a == b)
            .then_some(())
            .ok_or(format!("the two builds differ: {a} and {b}"))
    })?;
    sh.run(&["cp", path(first), path(&image)], &[])?;
    sh.run(
        &[
            "rm",
            "-rf",
            path(&req.out.join("work/tree-1")),
            path(&req.out.join("work/tree-2")),
        ],
        &[],
    )?;
    if !sh.dry {
        say("built twice: equal");
        say(&format!("rootfs.img sha256 {}", sha256(&image)?));
    }
    Ok(())
}

/// Runs a command, or in a dry run prints it: the one place osfact starts a program.
struct Shell {
    dry: bool,
}

impl Shell {
    fn run(&self, argv: &[&str], env: &[(&str, &str)]) -> Result<(), String> {
        let owned: Vec<String> = argv.iter().map(|s| s.to_string()).collect();
        self.run_all(&owned, env)
    }

    fn run_all(&self, argv: &[String], env: &[(&str, &str)]) -> Result<(), String> {
        let prefix: String = env.iter().map(|(k, v)| format!("{k}={v} ")).collect();
        if self.dry {
            say(&format!("{prefix}{}", argv.join(" ")));
            return Ok(());
        }
        let (program, args) = argv.split_first().ok_or("an empty command")?;
        let out = Command::new(program)
            .args(args)
            .envs(env.iter().copied())
            .output()
            .map_err(|e| format!("{program}: {e}"))?;
        if out.status.success() {
            return Ok(());
        }
        Err(format!("{} failed ({}): {}", argv.join(" "), out.status, {
            let err = String::from_utf8_lossy(&out.stderr);
            let said = if err.trim().is_empty() {
                String::from_utf8_lossy(&out.stdout)
            } else {
                err
            };
            said.trim().to_string()
        }))
    }

    /// A check the run makes itself: said as a step in a dry run, run (and fatal when false) otherwise.
    fn check(&self, what: &str, f: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
        if self.dry {
            say(&format!("check: {what}"));
            return Ok(());
        }
        f()
    }
}

fn say(line: &str) {
    println!("{line}");
}

fn path(p: &Path) -> &str {
    p.to_str().unwrap_or("")
}

pub(crate) fn sha256(file: &Path) -> Result<String, String> {
    let bytes = fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
