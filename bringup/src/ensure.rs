// SPDX-License-Identifier: MIT OR Apache-2.0
//! Image ensure: an image of `[images.NAME]` by its declared sha256, from Nix's store, else from the file://
//! cache, else from a seed folder (a file that is the image), else built from its recipe in Nix's sandbox as a
//! fixed-output derivation, so Nix itself refuses a result that is not the declared sha256 (decision
//! 2026-11-23-image-recipes-on-nix-fixed-output-paths). What is built or seeded is copied to the cache.
//! The path an image has in the store is computed from its sha256 and name alone (`nix-store --print-fixed-path`).

use lpoco_manifest::{images::Recipe, KernelBuild, Manifest, PartBuild};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const GENERIC_DERIVATION: &str = include_str!("ensure.nix");
const KERNEL_DERIVATION: &str = include_str!("kernel.nix");
const PART_DERIVATION: &str = include_str!("part.nix");
const KERNEL_REPO: &str = "https://github.com/linux-phone-oco/linux.git";
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
    if !manifest
        .images
        .as_ref()
        .is_some_and(|i| i.named.contains_key(name))
    {
        if let Some(k) = manifest.kernel_builds.get(name) {
            return ensure_kernel(name, k, sources, say);
        }
        if let Some(p) = manifest.part_builds.get(name) {
            return ensure_part(name, p, sources, say, false);
        }
    }
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
    let path = build(manifest, name, &wanted, recipe, sources, say, false)?;
    publish(&path, sources.cache)?;
    say(format!(
        "{name}: built from its recipe, equal to its declared sha256"
    ));
    Ok(path)
}

/// The image `name` proved against its recipe: the recipe built in the sandbox, ignoring the store, the cache and
/// the seed (decision 2026-11-24-a-recipe-sha-is-proved-by-a-rebuild), and its sha256 compared to the declared
/// one. A result that is not the declared one fails naming the image and both sha256s; nothing is published.
/// Each input of the recipe that has a recipe of its own is proved first, the same way; one that has none (a
/// seed, hand-built) is said to be not proved.
pub fn rebuild(
    manifest: &Manifest,
    name: &str,
    sources: &Sources,
    say: &mut dyn FnMut(String),
    proving: &mut Proving,
) -> Result<PathBuf, String> {
    if proving.active.iter().any(|n| n == name) {
        return Err(format!("{name}: its inputs lead back to {name} itself"));
    }
    proving.active.push(name.to_string());
    let made = rebuild_one(manifest, name, sources, say, proving);
    proving.active.pop();
    made
}

/// What a run of rebuilds has done: the images proved (an input two images share is rebuilt once, also across
/// the images `--changed-since` chooses) and the ones being proved now (an input that leads back is a cycle).
#[derive(Default)]
pub struct Proving {
    done: Vec<String>,
    active: Vec<String>,
}

/// [`rebuild`] of one image, its inputs proved first.
fn rebuild_one(
    manifest: &Manifest,
    name: &str,
    sources: &Sources,
    say: &mut dyn FnMut(String),
    proving: &mut Proving,
) -> Result<PathBuf, String> {
    let named = manifest
        .images
        .as_ref()
        .is_some_and(|i| i.named.contains_key(name));
    if !named && manifest.kernel_builds.contains_key(name) {
        return Err(format!(
            "{name}: --rebuild is for an image with a recipe; {name} is a kernel build"
        ));
    }
    if let Some(p) = manifest.part_builds.get(name).filter(|_| !named) {
        let path = ensure_part(name, p, sources, say, true)?;
        proving.done.push(name.to_string());
        return Ok(path);
    }
    let (wanted, recipe) = image(manifest, name)?;
    let recipe = recipe.ok_or_else(|| format!("{name}: no recipe to rebuild it from"))?;
    prove_inputs(manifest, name, recipe, sources, say, proving)?;
    let path = build(manifest, name, &wanted, recipe, sources, say, true)?;
    let built = sum(&path)?;
    if built != wanted.sha256 {
        return Err(format!(
            "{name}: its recipe makes sha256 {built}, not the declared {}",
            wanted.sha256
        ));
    }
    proving.done.push(name.to_string());
    say(format!(
        "{name}: rebuilt from its recipe, equal to its declared sha256"
    ));
    Ok(path)
}

/// The inputs of `recipe` (the control by name; the kernel, dtb and ramdisk by sha256): one that is an image with a
/// recipe is rebuilt; any other is said to be not proved, and is not a failure.
fn prove_inputs(
    manifest: &Manifest,
    name: &str,
    recipe: &Recipe,
    sources: &Sources,
    say: &mut dyn FnMut(String),
    proving: &mut Proving,
) -> Result<(), String> {
    let images = manifest.images.as_ref().map(|i| &i.named);
    // two images may share a sha256: the one with a recipe is the one to prove
    let by_sha = |sha: &String| {
        let same = |want: bool| {
            images.and_then(|m| {
                m.iter()
                    .find(|(_, i)| i.sha256.as_ref() == Some(sha) && i.recipe.is_some() == want)
                    .map(|(n, _)| n.clone())
            })
        };
        same(true)
            .map(|n| (n, true))
            .or_else(|| same(false).map(|n| (n, false)))
    };
    let seed = "it has no recipe (a seed)".to_string();
    let mut inputs = vec![(
        recipe.control.clone(),
        images
            .and_then(|m| m.get(&recipe.control))
            .is_some_and(|i| i.recipe.is_some()),
        seed.clone(),
    )];
    for (part, sha) in [
        ("kernel", &recipe.kernel),
        ("dtb", &recipe.dtb),
        ("ramdisk", &recipe.ramdisk),
    ] {
        if let Some(sha) = sha {
            let (input, has_recipe) = by_sha(sha).unwrap_or((format!("{name} {part}"), false));
            let made = manifest
                .kernel_builds
                .iter()
                .find(|(_, k)| part == "kernel" && k.sha256 == *sha);
            let why = made.map_or(seed.clone(), |(n, _)| {
                format!("it is made by [kernel_builds.{n}], which --rebuild does not run")
            });
            inputs.push((input, has_recipe, why));
        }
    }
    for (input, has_recipe, why) in inputs {
        if !has_recipe {
            say(format!("{input}: not proved, {why}"));
        } else if !proving.done.contains(&input) {
            rebuild(manifest, &input, sources, say, proving)?;
        }
    }
    Ok(())
}

/// Why an image is chosen by `--changed-since`.
#[derive(Debug, PartialEq)]
pub enum Change {
    /// the revision declares no such image
    New,
    Sha256,
    Recipe,
    /// its control, or the control's control, has another sha256 than at the revision
    Control,
}

impl std::fmt::Display for Change {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(match self {
            Change::New => "not declared there",
            Change::Sha256 => "its sha256",
            Change::Recipe => "its recipe",
            Change::Control => "its control",
        })
    }
}

/// The images with a recipe that `now` declares differently from `then` (the manifest at a revision): their names,
/// in the manifest's order, and why. An image without a recipe has nothing to rebuild and is never chosen.
pub fn changed_since(then: &Manifest, now: &Manifest) -> Vec<(String, Change)> {
    let old = |name: &str| then.images.as_ref().and_then(|i| i.named.get(name));
    let Some(images) = now.images.as_ref() else {
        return Vec::new();
    };
    images
        .named
        .iter()
        .filter(|(_, i)| i.recipe.is_some())
        .filter_map(|(name, i)| {
            let change = match old(name) {
                None => Change::New,
                Some(o) if o.sha256 != i.sha256 => Change::Sha256,
                Some(o) if o.recipe != i.recipe => Change::Recipe,
                Some(_) if control_differs(then, now, i) => Change::Control,
                Some(_) => return None,
            };
            Some((name.clone(), change))
        })
        .collect()
}

/// Whether the control an image's recipe starts from, or that control's own control, is another file than at the
/// revision: a recipe names its control by name, so only the sha256s say it changed.
fn control_differs(then: &Manifest, now: &Manifest, image: &lpoco_manifest::images::Image) -> bool {
    let get = |m: &Manifest, n: &str| m.images.as_ref().and_then(|i| i.named.get(n)).cloned();
    let mut next = image.recipe.as_ref().map(|r| r.control.clone());
    // the manifest refuses a loop; the bound is only so a bug cannot spin here
    for _ in 0..32 {
        let Some(name) = next else { return false };
        let (n, o) = (get(now, &name), get(then, &name));
        match (n, o) {
            (Some(n), Some(o)) if n.sha256 == o.sha256 => {
                next = n.recipe.map(|r| r.control);
            }
            _ => return true,
        }
    }
    true
}

/// Where an image would come from.
#[derive(Debug, PartialEq)]
pub enum Origin {
    Store,
    Cache,
    Seed,
    Build,
}

impl std::fmt::Display for Origin {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(match self {
            Origin::Store => "in the store",
            Origin::Cache => "from the cache",
            Origin::Seed => "from the seed",
            Origin::Build => "would be built from its recipe",
        })
    }
}

/// Where [`ensure`] would take the image `name` from, and the store path it would have: it reads the store, the
/// cache and the seed and changes none of them (nothing is copied, added or built).
pub fn plan(
    manifest: &Manifest,
    name: &str,
    sources: &Sources,
) -> Result<(Origin, PathBuf), String> {
    let named = manifest
        .images
        .as_ref()
        .is_some_and(|i| i.named.contains_key(name));
    if !named && manifest.kernel_builds.contains_key(name) {
        return Ok((Origin::Build, PathBuf::new()));
    }
    if let Some(p) = manifest.part_builds.get(name).filter(|_| !named) {
        return plan_held(name, &part_wanted(p), true, sources);
    }
    let (wanted, recipe) = image(manifest, name)?;
    plan_held(name, &wanted, recipe.is_some(), sources)
}

/// [`plan`] of something wanted by content: the store, the cache, the seed, else a build when it has a recipe.
fn plan_held(
    name: &str,
    wanted: &Wanted,
    has_recipe: bool,
    sources: &Sources,
) -> Result<(Origin, PathBuf), String> {
    let path = wanted.path()?;
    if valid(&path) {
        return Ok((Origin::Store, path));
    }
    if let Some(cache) = sources.cache {
        // a cache that is not there holds nothing; one that answers "no" is a miss
        if cache.is_dir()
            && run(
                "nix",
                &[
                    NIX[0],
                    NIX[1],
                    "path-info",
                    "--store",
                    &url(cache)?,
                    path.to_str().unwrap(),
                ],
            )
            .is_ok()
        {
            return Ok((Origin::Cache, path));
        }
    }
    if let Some(dir) = sources.seed {
        if find(dir, &wanted.sha256)?.is_some() {
            return Ok((Origin::Seed, path));
        }
    }
    if has_recipe {
        return Ok((Origin::Build, path));
    }
    Err(format!(
        "{name}: sha256 {} is not in the store, the cache or the seed, and the image has no recipe",
        wanted.sha256
    ))
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
        if let Err(e) = run(
            "nix",
            &[
                NIX[0],
                NIX[1],
                "copy",
                "--from",
                &url(cache)?,
                path.to_str().unwrap(),
            ],
        ) {
            // a path the cache lacks is a miss; any other failure is said
            if !e.contains("no substituter that can build") {
                eprintln!("{label}: the cache did not serve it: {e}");
            }
        }
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
    rebuild: bool,
) -> Result<PathBuf, String> {
    let mut args: Vec<(&str, String)> = vec![
        ("name", wanted.name.clone()),
        ("sha256", wanted.sha256.clone()),
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
            // a kernel no store, cache or seed holds may be one a [kernel_builds] row makes
            let made = manifest
                .kernel_builds
                .iter()
                .find(|(_, k)| part == "kernel" && k.sha256 == *sha);
            let path = match (held(&input, sources, say, &label)?, made) {
                (Some(p), _) => p,
                (None, Some((n, k))) => ensure_kernel(n, k, sources, say)?,
                (None, None) => {
                    return Err(format!(
                        "{label}: sha256 {sha} is not in the store, the cache or the seed (nothing makes it)"
                    ))
                }
            };
            args.push((part, path.display().to_string()));
        }
    }
    // the missing parts are found before the control, which may take a build of its own
    let control = ensure(manifest, &recipe.control, sources, say)?;
    args.push(("control", control.display().to_string()));
    if rebuild {
        args.push(("rebuild", "1".to_string()));
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
    let result = nix_build(&scratch, "ensure.nix", GENERIC_DERIVATION, &args);
    let _ = fs::remove_dir_all(&scratch);
    Ok(PathBuf::from(result?))
}

fn nix_build(
    scratch: &Path,
    file: &str,
    derivation: &str,
    args: &[(&str, String)],
) -> Result<String, String> {
    let write = |f: &str, text: &str| {
        fs::write(scratch.join(f), text)
            .map_err(|e| format!("cannot write {}/{f}: {e}", scratch.display()))
    };
    write("nixpkgs.nix", NIXPKGS_PIN)?;
    write(file, derivation)?;
    let expr = scratch.join(file);
    let mut cmd = vec![expr.to_str().unwrap().to_string(), "--no-out-link".into()];
    let rebuild = args.iter().any(|(k, _)| *k == "rebuild");
    if rebuild {
        cmd.extend(["--option".into(), "substitute".into(), "false".into()]);
    }
    for (k, v) in args {
        // paths and text alike go in as strings; the derivation turns a store path into one
        cmd.extend(["--argstr".into(), k.to_string(), v.clone()]);
    }
    let mut cmd: Vec<&str> = cmd.iter().map(String::as_str).collect();
    let mut out = run("nix-build", &cmd)?;
    if rebuild {
        // the output may already be in the store (an earlier build): --check runs the builder again and compares
        cmd.push("--check");
        out = run("nix-build", &cmd)?;
    }
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
        } else if path.is_file() && sum(&path).is_ok_and(|s| s == sha256) {
            // a file that cannot be read is not the wanted one, not a failure of the run
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn sum(path: &Path) -> Result<String, String> {
    let mut file =
        fs::File::open(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut hash = Sha256::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let n = std::io::Read::read(&mut file, &mut chunk)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hash.update(&chunk[..n]);
    }
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
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
    // a new folder only the user can enter: never one another user planted in the shared temp folder
    use std::os::unix::fs::DirBuilderExt;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!(
        "lpoco-bringup-ensure-{}-{nanos}-{what}",
        std::process::id()
    ));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&dir)
        .map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
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
            args.iter()
                .find(|a| !a.starts_with('-') && **a != "nix-command")
                .unwrap_or(&""),
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

/// The name a kernel file has in the store: by its sha256, as a recipe's `kernel` part is looked up.
fn kernel_wanted(k: &KernelBuild) -> Wanted {
    Wanted {
        sha256: k.sha256.clone(),
        name: format!("kernel-{}", &k.sha256[..12]),
    }
}

/// The kernel `[kernel_builds.NAME]`: its store path, held by its sha256 or built from its tag's commit in the sandbox.
fn ensure_kernel(
    name: &str,
    k: &KernelBuild,
    sources: &Sources,
    say: &mut dyn FnMut(String),
) -> Result<PathBuf, String> {
    let wanted = kernel_wanted(k);
    if let Some(path) = held(&wanted, sources, say, name)? {
        return Ok(path);
    }
    let mut args: Vec<(&str, String)> = vec![
        ("name", wanted.name.clone()),
        ("sha256", k.sha256.clone()),
        ("repo", k.repo.clone().unwrap_or_else(|| KERNEL_REPO.into())),
        ("tag", k.tag.clone()),
        ("rev", k.rev.clone()),
        ("epoch", k.epoch.to_string()),
        ("defconfig", k.defconfig.clone()),
        ("targets", k.targets.join(" ")),
        ("output", k.output.clone()),
        ("dtcFlags", k.dtc_flags.clone().unwrap_or_default()),
    ];
    for (key, what, shas) in [
        ("configs", "config", &k.configs),
        ("patches", "patch", &k.patches),
    ] {
        let mut paths = Vec::new();
        for sha in shas {
            let input = Wanted {
                sha256: sha.clone(),
                name: format!("{what}-{}", &sha[..12]),
            };
            let label = format!("{name} {what}");
            let path = held(&input, sources, say, &label)?.ok_or_else(|| {
                format!("{label}: sha256 {sha} is not in the store, the cache or the seed (nothing makes it)")
            })?;
            paths.push(path.display().to_string());
        }
        args.push((key, paths.join(" ")));
    }
    let scratch = scratch(name)?;
    let result = nix_build(&scratch, "kernel.nix", KERNEL_DERIVATION, &args);
    let _ = fs::remove_dir_all(&scratch);
    let path = PathBuf::from(result?);
    publish(&path, sources.cache)?;
    say(format!(
        "{name}: built from its recipe (tag {}), equal to its declared sha256",
        k.tag
    ));
    Ok(path)
}

/// The name a part's binary has in the store: by its sha256, as a kernel file is.
fn part_wanted(p: &PartBuild) -> Wanted {
    Wanted {
        sha256: p.sha256.clone(),
        name: format!("part-{}", &p.sha256[..12]),
    }
}

/// The binary `[part_builds.NAME]`: its store path, held by its sha256 or built from its crate's commit in the
/// sandbox (unit tests first). With `rebuild` the store, cache and seed are ignored, the build is held to nothing but
/// run twice by `nix-build --check`, and its sha256 is compared to the declared one (decision
/// 2026-11-24-a-recipe-sha-is-proved-by-a-rebuild); nothing is published. Any failure names the part.
fn ensure_part(
    name: &str,
    p: &PartBuild,
    sources: &Sources,
    say: &mut dyn FnMut(String),
    rebuild: bool,
) -> Result<PathBuf, String> {
    let wanted = part_wanted(p);
    if !rebuild {
        if let Some(path) = held(&wanted, sources, say, name)? {
            return Ok(path);
        }
    }
    let mut args: Vec<(&str, String)> = vec![
        ("name", wanted.name.clone()),
        ("sha256", p.sha256.clone()),
        ("repo", p.repo.clone()),
        ("rev", p.rev.clone()),
        ("crate", p.krate.clone()),
        ("bin", p.bin.clone()),
    ];
    if rebuild {
        args.push(("rebuild", "1".to_string()));
    }
    let scratch = scratch(name)?;
    let result = nix_build(&scratch, "part.nix", PART_DERIVATION, &args);
    let _ = fs::remove_dir_all(&scratch);
    let path = PathBuf::from(result.map_err(|e| format!("{name}: {e}"))?);
    if rebuild {
        let built = sum(&path)?;
        if built != p.sha256 {
            return Err(format!(
                "{name}: its crate at {} makes sha256 {built}, not the declared {}",
                p.rev, p.sha256
            ));
        }
        say(format!(
            "{name}: rebuilt from its recipe, equal to its declared sha256"
        ));
        return Ok(path);
    }
    publish(&path, sources.cache)?;
    say(format!(
        "{name}: built from its recipe (crate {} at {}), equal to its declared sha256",
        p.krate,
        &p.rev[..12]
    ));
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn manifest(images: &str) -> Manifest {
        let base = include_str!("../tests/fixtures/device.toml");
        let control = format!("[images.c]\npath = \"c.img\"\nsha256 = \"{A}\"\n");
        Manifest::parse(&format!("{base}\n{control}\n{images}")).unwrap()
    }

    fn image(name: &str, sha: &str, cmdline: &str) -> String {
        format!(
            "[images.{name}]\npath = \"{name}.img\"\nsha256 = \"{sha}\"\n[images.{name}.recipe]\ncontrol = \"c\"\ncmdline = \"{cmdline}\"\n"
        )
    }

    #[test]
    fn an_image_is_chosen_for_each_way_its_declaration_can_differ_and_for_no_other() {
        let then = manifest(
            &[
                image("same", A, "x"),
                image("recipe", A, "x"),
                image("sha", A, "x"),
                format!("[images.seed]\npath = \"seed.img\"\nsha256 = \"{A}\"\n"),
            ]
            .join("\n"),
        );
        let now = manifest(
            &[
                image("same", A, "x"),
                image("recipe", A, "y"),
                image("sha", B, "x"),
                image("new", A, "x"),
                format!("[images.seed]\npath = \"seed.img\"\nsha256 = \"{B}\"\n"),
            ]
            .join("\n"),
        );
        assert_eq!(
            changed_since(&then, &now),
            vec![
                ("new".to_string(), Change::New),
                ("recipe".to_string(), Change::Recipe),
                ("sha".to_string(), Change::Sha256),
            ],
            "a seed-only image has nothing to rebuild, whatever changed"
        );
        assert_eq!(changed_since(&now, &now), vec![]);
    }

    #[test]
    fn a_recipe_whose_control_is_another_file_is_chosen_though_its_own_text_is_the_same() {
        let with = |control_sha: &str| {
            manifest(&format!(
                "[images.base]\npath = \"base.img\"\nsha256 = \"{control_sha}\"\n[images.base.recipe]\ncontrol = \"c\"\ncmdline = \"x\"\n\n{}",
                [
                    "[images.top]\npath = \"top.img\"\nsha256 = \"{A}\"\n[images.top.recipe]\ncontrol = \"base\"\ncmdline = \"y\"\n"
                        .replace("{A}", A),
                ]
                .join("")
            ))
        };
        let changed: Vec<_> = changed_since(&with(A), &with(B)).into_iter().collect();
        assert_eq!(
            changed,
            vec![
                ("base".to_string(), Change::Sha256),
                ("top".to_string(), Change::Control)
            ]
        );
    }
}
