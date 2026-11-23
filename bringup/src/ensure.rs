// SPDX-License-Identifier: MIT OR Apache-2.0
//! Image ensure: an image of `[images.NAME]` by its declared sha256, from Nix's store, else from the file://
//! cache, else from a seed folder (a file that is the image), else built from its recipe in Nix's sandbox as a
//! fixed-output derivation, so Nix itself refuses a result that is not the declared sha256 (decision
//! 2026-11-23-image-recipes-on-nix-fixed-output-paths). What is built or seeded is copied to the cache.
//! The path an image has in the store is computed from its sha256 and name alone (`nix-store --print-fixed-path`).

use lpoco_manifest::{images::Recipe, Manifest};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const GENERIC_DERIVATION: &str = include_str!("ensure.nix");
const NIXPKGS_PIN: &str = include_str!("../../nixpkgs.nix");
const NIX: [&str; 2] = ["--extra-experimental-features", "nix-command"];

/// Where an image can come from besides the store and its recipe.
pub struct Sources<'a> {
    /// a folder that is a file:// binary cache of the lab
    pub cache: Option<&'a Path>,
    /// a folder of files; one whose sha256 is a wanted one is taken into the store
    pub seed: Option<&'a Path>,
    /// the lab-tools source the sandbox builds lpoco-bringup from (the release's tag when none)
    pub source: Option<&'a Path>,
    pub version: &'a str,
}

/// Something wanted by content: its sha256 and the name it has in the store.
struct Wanted {
    sha256: String,
    name: String,
}

impl Wanted {
    fn path(&self) -> Result<PathBuf, String> {
        let out = run(
            "nix-store",
            &["--print-fixed-path", "sha256", &self.sha256, &self.name],
        )?;
        Ok(PathBuf::from(out.trim()))
    }
}

/// The image `name`: its store path, and a line for each step taken (the last of them says where it came from).
pub fn ensure(
    manifest: &Manifest,
    name: &str,
    sources: &Sources,
    say: &mut dyn FnMut(String),
) -> Result<PathBuf, String> {
    let (wanted, recipe) = image(manifest, name)?;
    if let Some(path) = held(&wanted, sources, say, name)? {
        return Ok(path);
    }
    let recipe = recipe.ok_or_else(|| {
        format!(
            "{name}: sha256 {} is not in the store, the cache or the seed, and the image has no recipe",
            wanted.sha256
        )
    })?;
    let path = build(manifest, name, &wanted, recipe, sources, say)?;
    publish(&path, sources.cache)?;
    say(format!(
        "{name}: built from its recipe, equal to its declared sha256"
    ));
    Ok(path)
}

fn image<'a>(manifest: &'a Manifest, name: &str) -> Result<(Wanted, Option<&'a Recipe>), String> {
    let i = manifest
        .images
        .as_ref()
        .and_then(|i| i.named.get(name))
        .ok_or_else(|| format!("{name}: no [images.{name}] in the manifest"))?;
    let sha256 = i
        .sha256
        .clone()
        .ok_or_else(|| format!("{name}: [images.{name}] has no sha256 to be known by"))?;
    let file = Path::new(&i.path)
        .file_name()
        .and_then(|f| f.to_str())
        .ok_or_else(|| format!("{name}: path {:?} names no file", i.path))?;
    Ok((
        Wanted {
            sha256,
            name: file.to_string(),
        },
        i.recipe.as_ref(),
    ))
}

/// In the store, in the cache or in the seed: its path; none of them: `None`.
fn held(
    w: &Wanted,
    sources: &Sources,
    say: &mut dyn FnMut(String),
    label: &str,
) -> Result<Option<PathBuf>, String> {
    let path = w.path()?;
    if valid(&path) {
        say(format!("{label}: in the store"));
        return Ok(Some(path));
    }
    if let Some(cache) = sources.cache {
        let _ = run(
            "nix",
            &[
                NIX[0],
                NIX[1],
                "copy",
                "--from",
                &url(cache)?,
                path.to_str().unwrap(),
            ],
        );
        if valid(&path) {
            say(format!("{label}: from the cache"));
            return Ok(Some(path));
        }
    }
    if let Some(file) = sources
        .seed
        .map(|d| find(d, &w.sha256))
        .transpose()?
        .flatten()
    {
        add(&file, w)?;
        publish(&path, sources.cache)?;
        say(format!("{label}: taken from the seed {}", file.display()));
        return Ok(Some(path));
    }
    Ok(None)
}

/// The recipe built in Nix's sandbox: inputs first (each by its own sha256), then the generic derivation.
fn build(
    manifest: &Manifest,
    name: &str,
    wanted: &Wanted,
    recipe: &Recipe,
    sources: &Sources,
    say: &mut dyn FnMut(String),
) -> Result<PathBuf, String> {
    let control = ensure(manifest, &recipe.control, sources, say)?;
    let mut args: Vec<(&str, String)> = vec![
        ("name", wanted.name.clone()),
        ("sha256", wanted.sha256.clone()),
        ("control", control.display().to_string()),
        ("version", sources.version.to_string()),
    ];
    for (part, sha) in [
        ("kernel", &recipe.kernel),
        ("dtb", &recipe.dtb),
        ("ramdisk", &recipe.ramdisk),
    ] {
        if let Some(sha) = sha {
            let input = Wanted {
                sha256: sha.clone(),
                name: format!("{part}-{}", &sha[..12]),
            };
            let label = format!("{name} {part}");
            let path = held(&input, sources, say, &label)?.ok_or_else(|| {
                format!("{label}: sha256 {sha} is not in the store, the cache or the seed (nothing makes it)")
            })?;
            args.push((part, path.display().to_string()));
        }
    }
    if let Some(c) = &recipe.cmdline {
        args.push(("cmdline", c.clone()));
    }
    if let Some(s) = sources.source {
        args.push((
            "source",
            fs::canonicalize(s)
                .map_err(|e| format!("{}: {e}", s.display()))?
                .display()
                .to_string(),
        ));
    }
    let scratch = scratch(name)?;
    let result = nix_build(&scratch, &args);
    let _ = fs::remove_dir_all(&scratch);
    Ok(PathBuf::from(result?))
}

fn nix_build(scratch: &Path, args: &[(&str, String)]) -> Result<String, String> {
    let write = |f: &str, text: &str| {
        fs::write(scratch.join(f), text)
            .map_err(|e| format!("cannot write {}/{f}: {e}", scratch.display()))
    };
    write("nixpkgs.nix", NIXPKGS_PIN)?;
    write("ensure.nix", GENERIC_DERIVATION)?;
    let expr = scratch.join("ensure.nix");
    let mut cmd = vec![expr.to_str().unwrap().to_string(), "--no-out-link".into()];
    for (k, v) in args {
        // paths and text alike go in as strings; the derivation turns a store path into one
        cmd.extend(["--argstr".into(), k.to_string(), v.clone()]);
    }
    let cmd: Vec<&str> = cmd.iter().map(String::as_str).collect();
    let out = run("nix-build", &cmd)?;
    out.lines()
        .last()
        .map(str::to_string)
        .ok_or_else(|| "nix-build printed no path".into())
}

/// A file whose sha256 is `sha256`, anywhere under `dir`.
fn find(dir: &Path, sha256: &str) -> Result<Option<PathBuf>, String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            if let Some(f) = find(&path, sha256)? {
                return Ok(Some(f));
            }
        } else if path.is_file() && sum(&path)? == sha256 {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn sum(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// The file as the store's path of its sha256 and name (a copy under that name first: the store names by file name).
fn add(file: &Path, w: &Wanted) -> Result<(), String> {
    let dir = scratch("add")?;
    let named = dir.join(&w.name);
    let result = fs::copy(file, &named)
        .map_err(|e| format!("cannot copy {}: {e}", file.display()))
        .and_then(|_| {
            run(
                "nix-store",
                &["--add-fixed", "sha256", named.to_str().unwrap()],
            )
        });
    let _ = fs::remove_dir_all(&dir);
    result.map(|_| ())
}

/// A path copied to the cache (nothing when no cache is given).
fn publish(path: &Path, cache: Option<&Path>) -> Result<(), String> {
    let Some(cache) = cache else { return Ok(()) };
    fs::create_dir_all(cache).map_err(|e| format!("cannot make {}: {e}", cache.display()))?;
    run(
        "nix",
        &[
            NIX[0],
            NIX[1],
            "copy",
            "--to",
            &url(cache)?,
            path.to_str().unwrap(),
        ],
    )
    .map(|_| ())
}

fn url(cache: &Path) -> Result<String, String> {
    let abs = fs::canonicalize(cache)
        .or_else(|_| fs::create_dir_all(cache).and_then(|_| fs::canonicalize(cache)));
    Ok(format!(
        "file://{}",
        abs.map_err(|e| format!("{}: {e}", cache.display()))?
            .display()
    ))
}

fn scratch(what: &str) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join(format!(
        "lpoco-bringup-ensure-{}-{what}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    Ok(dir)
}

fn run(tool: &str, args: &[&str]) -> Result<String, String> {
    let o = Command::new(tool)
        .args(args)
        .output()
        .map_err(|e| format!("cannot run {tool}: {e} (ensure needs Nix)"))?;
    if !o.status.success() {
        let err = String::from_utf8_lossy(&o.stderr);
        let tail: Vec<&str> = err.lines().rev().take(12).collect();
        return Err(format!(
            "{tool} {} failed: {}",
            args.first().unwrap_or(&""),
            tail.into_iter().rev().collect::<Vec<_>>().join("\n")
        ));
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

/// A path the store holds and knows. A file left at the path by a build Nix refused (a hash mismatch) is not valid.
fn valid(path: &Path) -> bool {
    Command::new("nix-store")
        .args(["--check-validity", path.to_str().unwrap_or_default()])
        .output()
        .is_ok_and(|o| o.status.success())
}
