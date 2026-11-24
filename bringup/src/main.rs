// SPDX-License-Identifier: MIT OR Apache-2.0
//! lpoco-bringup: the host side of bringing a phone up (run as `lpoco bringup ...`). It makes files on the
//! host; devlab puts them on the phone. See cli.rs for the invocation surface.

mod cli;

use argenv_resolve::{InputResolveExt, OnProblems};
use cli::{Command, Contract, Verb};
use lpoco_bringup::boot::Boot;
use lpoco_bringup::ensure::{self, Sources};
use lpoco_bringup::image::{self, Verdict};
use lpoco_bringup::initramfs;
use lpoco_bringup::swap::{self, Parts};
use std::fs;
use std::path::Path;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let r = Contract::parse_and_lint(OnProblems::FailOnError);
    if Contract::HELP.get_from(&r).unwrap_or(false) {
        print!(
            "{}",
            cli::usage(
                "lpoco-bringup",
                "Boot images and bring-up steps. Commands: image info|unpack|pack|check|swap|bisect|ensure; dt-check binding|dtbs."
            )
        );
        return;
    }
    if Contract::VERSION.get_from(&r).unwrap_or(false) {
        println!("lpoco-bringup {VERSION}");
        return;
    }
    let code = match run(&r) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("lpoco-bringup: {e}");
            2
        }
    };
    std::process::exit(code);
}

fn need(v: Option<String>, what: &str) -> Result<String, String> {
    v.ok_or_else(|| format!("{what} is needed"))
}

fn read(path: &str) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))
}

fn run(r: &argenv_resolve::Resolution) -> Result<i32, String> {
    let Some(verb) = Contract::VERB.get_from(r) else {
        return Err(
            "usage: lpoco bringup image info|unpack|pack|check|swap|bisect|ensure ... (lpoco-bringup --version)"
                .into(),
        );
    };
    let command = Contract::COMMAND.get_from(r);
    if Contract::DRY_RUN.get_from(r).unwrap_or(false) && !matches!(verb, Verb::Ensure) {
        return Err(
            "--dry-run is for image ensure: lpoco-bringup image ensure NAME --dry-run".into(),
        );
    }
    if Contract::REBUILD.get_from(r).unwrap_or(false) {
        if !matches!(verb, Verb::Ensure) {
            return Err(
                "--rebuild is for image ensure: lpoco-bringup image ensure NAME --rebuild".into(),
            );
        }
        if Contract::DRY_RUN.get_from(r).unwrap_or(false) {
            return Err("--rebuild builds; --dry-run changes nothing: give one".into());
        }
    }
    if matches!(verb, Verb::Capture | Verb::Diff) != (command == Some(Command::Reference)) {
        return Err("capture and diff are for the reference: lpoco-bringup reference capture --via COMMAND --out DIR | reference diff PART --live DIR --ours DIR".into());
    }
    if (command == Some(Command::DtCheck)) != matches!(verb, Verb::Binding | Verb::Dtbs) {
        return Err("binding and dtbs are for dt-check: lpoco-bringup dt-check binding|dtbs SRC --image NAME@sha256:DIGEST --out SCRATCH".into());
    }
    if (command == Some(Command::Kernel)) != matches!(verb, Verb::Assemble | Verb::Publish) {
        return Err("assemble and publish are for the kernel: lpoco-bringup kernel assemble LIST --lists FILE --repo DIR --records DIR | kernel publish TAG --repo DIR".into());
    }
    if command == Some(Command::Kernel) {
        return kernel(verb, r);
    }
    if command == Some(Command::DtCheck) {
        return dt_check(verb, r);
    }
    if command == Some(Command::Reference) {
        return reference(verb, r);
    }
    if matches!(verb, Verb::Build | Verb::Init | Verb::Files)
        != (command == Some(Command::Initramfs))
    {
        return Err(match verb {
            Verb::Build | Verb::Init | Verb::Files => {
                "build, init and files are for the initramfs: lpoco-bringup initramfs build --spec FILE --out FILE | initramfs init [--device FILE] | initramfs files --out DIR"
                    .into()
            }
            _ => "image verbs go with image: lpoco-bringup image info|unpack|pack|check|swap|bisect|ensure ..."
                .into(),
        });
    }
    let device = Contract::DEVICE
        .get_from(r)
        .unwrap_or_else(|| "device.toml".into());
    lpoco_bringup::pin::check(Path::new(&device), VERSION)?;
    match verb {
        Verb::Info => {
            let bytes = read(&need(Contract::IMAGE.get_from(r), "an image (IMG)")?)?;
            print!("{}", image::read(&bytes)?.info());
            Ok(0)
        }
        Verb::Unpack => {
            let bytes = read(&need(Contract::IMAGE.get_from(r), "an image (IMG)")?)?;
            let out = need(Contract::OUT.get_from(r), "--out DIR")?;
            let img = image::read(&bytes)?;
            fs::create_dir_all(&out).map_err(|e| format!("cannot make {out}: {e}"))?;
            let put = |name: &str, data: &[u8]| {
                fs::write(Path::new(&out).join(name), data)
                    .map_err(|e| format!("cannot write {out}/{name}: {e}"))
            };
            put("kernel", img.kernel)?;
            put("ramdisk", img.ramdisk)?;
            if !img.second.is_empty() {
                put("second", img.second)?;
            }
            if let Some(dtb) = img.dtb() {
                put("appended.dtb", dtb)?;
            }
            put("boot.toml", img.info().as_bytes())?;
            Ok(0)
        }
        Verb::Pack => {
            let boot = Boot::read(Path::new(&device))?;
            let mut kernel = read(&need(Contract::KERNEL.get_from(r), "--kernel FILE")?)?;
            if let Some(dtb) = Contract::DTB.get_from(r) {
                kernel.extend(read(&dtb)?);
            }
            let mut ramdisk = read(&need(Contract::RAMDISK.get_from(r), "--ramdisk FILE")?)?;
            if let Some(exe) = boot.hooks.as_ref().and_then(|h| h.get("parts")) {
                (kernel, ramdisk) = hook_parts(exe, Path::new(&device), kernel, ramdisk)?;
            }
            let out = need(Contract::OUT.get_from(r), "--out IMG")?;
            let img = image::pack(&boot, &kernel, &ramdisk)?;
            if let Some(max) = boot.partition_size {
                if img.len() as u64 > max {
                    return Err(format!(
                        "the image is {} bytes; the boot partition holds {max}",
                        img.len()
                    ));
                }
            }
            fs::write(&out, &img).map_err(|e| format!("cannot write {out}: {e}"))?;
            Ok(0)
        }
        Verb::Swap => {
            let control = read(&need(
                Contract::IMAGE.get_from(r),
                "the control image (CONTROL)",
            )?)?;
            let out = need(Contract::OUT.get_from(r), "--out IMG")?;
            let opt = |v: Option<String>| v.map(|p| read(&p)).transpose();
            let parts = Parts {
                kernel: opt(Contract::KERNEL.get_from(r))?,
                dtb: opt(Contract::DTB.get_from(r))?,
                ramdisk: opt(Contract::RAMDISK.get_from(r))?,
                cmdline: Contract::CMDLINE.get_from(r),
                no_footer: false,
            };
            let (img, said) = swap::swap(&control, &parts)?;
            fs::write(&out, &img).map_err(|e| format!("cannot write {out}: {e}"))?;
            for line in said {
                println!("{line}");
            }
            Ok(0)
        }
        Verb::Ensure => {
            let name = need(Contract::IMAGE.get_from(r), "the image's name (NAME)")?;
            image_ensure(&name, &device, r)
        }
        Verb::Bisect => {
            let series = need(Contract::IMAGE.get_from(r), "the series file (SERIES)")?;
            let out = need(Contract::OUT.get_from(r), "--out DIR")?;
            let report = lpoco_bringup::bisect::run(Path::new(&series), Path::new(&out))?;
            for line in &report.lines {
                println!("{line}");
            }
            println!(
                "{} images: {} ok, {} failed",
                report.ok + report.failed,
                report.ok,
                report.failed
            );
            Ok(if report.failed == 0 { 0 } else { 1 })
        }
        Verb::Build => {
            let spec = need(Contract::SPEC.get_from(r), "--spec FILE")?;
            let out = need(Contract::OUT.get_from(r), "--out FILE")?;
            let net = net_of(r)?;
            let codec = initramfs::Codec::parse(
                &Contract::CODEC.get_from(r).unwrap_or_else(|| "gzip".into()),
            )?;
            let root = Contract::ROOT.get_from(r);
            let img =
                initramfs::build(Path::new(&spec), root.as_deref().map(Path::new), net, codec)?;
            fs::write(&out, &img).map_err(|e| format!("cannot write {out}: {e}"))?;
            println!("{out}: {} bytes ({codec:?}, net {net})", img.len());
            Ok(0)
        }
        Verb::Init => {
            let net = net_of(r)?;
            let boot = Boot::read(Path::new(&device))?;
            let gadget = boot.gadget.ok_or_else(|| {
                format!("{device}: no [boot.gadget] table (serial and product say how the phone names itself on the cable)")
            })?;
            let text = lpoco_bringup::init::make(&gadget, net)?;
            match Contract::OUT.get_from(r) {
                None => print!("{text}"),
                Some(out) => {
                    fs::write(&out, &text).map_err(|e| format!("cannot write {out}: {e}"))?;
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&out, fs::Permissions::from_mode(0o755))
                        .map_err(|e| format!("cannot make {out} executable: {e}"))?;
                    println!("{out}: {} bytes (net {net})", text.len());
                }
            }
            Ok(0)
        }
        Verb::Files => {
            let out = need(Contract::OUT.get_from(r), "--out DIR")?;
            let net = net_of(r)?;
            // the self-test checks the device's console; without a manifest or a [boot.gadget] it is the ACM one
            let console = Boot::read(Path::new(&device))
                .ok()
                .and_then(|b| b.gadget)
                .map(|g| g.console)
                .unwrap_or_default();
            let names = lpoco_bringup::init::write_files(Path::new(&out), net, console)?;
            println!("{out}: {} (net {net})", names.join(", "));
            Ok(0)
        }
        Verb::Capture
        | Verb::Diff
        | Verb::Binding
        | Verb::Dtbs
        | Verb::Assemble
        | Verb::Publish => {
            unreachable!("handled above")
        }
        Verb::Check => {
            let path = need(Contract::IMAGE.get_from(r), "an image (IMG)")?;
            let bytes = read(&path)?;
            let boot = Boot::read(Path::new(&device))?;
            let findings = image::check(&bytes, &boot);
            let (mut ok, mut fail) = (0, 0);
            for f in &findings {
                println!("{}", f.line());
                match f.verdict {
                    Verdict::Ok(_) => ok += 1,
                    Verdict::Fail(_) => fail += 1,
                    Verdict::NotMeasured(_) => {}
                }
            }
            println!("{path}: {ok} ok, {fail} failed");
            Ok(if fail == 0 { 0 } else { 1 })
        }
    }
}

/// The `parts` hook: the kernel and ramdisk are put in a scratch folder (the user's files stay as they are),
/// the manifest's executable is called with their two paths (kernel, ramdisk) and may change them, and what
/// they hold afterwards is what gets packed.
fn hook_parts(
    exe: &str,
    device: &Path,
    kernel: Vec<u8>,
    ramdisk: Vec<u8>,
) -> Result<(Vec<u8>, Vec<u8>), String> {
    let dir = std::env::temp_dir().join(format!("lpoco-bringup-parts-{}", std::process::id()));
    fs::create_dir_all(&dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    let (k, rd) = (dir.join("kernel"), dir.join("ramdisk"));
    let result = (|| {
        fs::write(&k, &kernel).map_err(|e| format!("cannot write {}: {e}", k.display()))?;
        fs::write(&rd, &ramdisk).map_err(|e| format!("cannot write {}: {e}", rd.display()))?;
        let mdir = device
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        lpoco_bringup::hooks::run("parts", exe, mdir, &[&k, &rd])?;
        Ok((read(k.to_str().unwrap())?, read(rd.to_str().unwrap())?))
    })();
    let _ = fs::remove_dir_all(&dir);
    result
}

/// `--net N`: the gadget address 172.16.N.1; the default when none is given.
fn net_of(r: &argenv_resolve::Resolution) -> Result<u8, String> {
    match Contract::NET.get_from(r) {
        None => Ok(initramfs::DEFAULT_NET),
        Some(n) => n
            .trim()
            .parse::<u8>()
            .map_err(|_| format!("--net {n}: a number from 0 to 255")),
    }
}

/// `kernel assemble` and `kernel publish`: a phone's test kernel from its named list, then its tag kept once an image names it.
fn kernel(verb: Verb, r: &argenv_resolve::Resolution) -> Result<i32, String> {
    use lpoco_bringup::kernel as k;
    let rhizoid = Contract::RHIZOID
        .get_from(r)
        .unwrap_or_else(|| "rhizoid".into());
    let repo = need(Contract::REPO.get_from(r), "--repo DIR (the kernel clone)")?;
    let name = need(
        Contract::LIST.get_from(r),
        "the list (assemble) or the tag (publish)",
    )?;
    if verb == Verb::Publish {
        let device = Contract::DEVICE
            .get_from(r)
            .unwrap_or_else(|| "device.toml".into());
        let text = fs::read_to_string(&device).map_err(|e| format!("cannot read {device}: {e}"))?;
        let tags = k::image_tags(&text).map_err(|e| format!("{device}: {e}"))?;
        if !k::publish(&rhizoid, &name, Path::new(&repo), &tags)? {
            println!("{name}: refused, no image names it (an [images.NAME] kernel_tag in {device}); not pushed");
            return Ok(1);
        }
        println!("{name}: pushed, its worktree ended");
        return Ok(0);
    }
    let lists = need(Contract::LISTS.get_from(r), "--lists FILE")?;
    let records = need(Contract::RECORDS.get_from(r), "--records DIR")?;
    let tad = Contract::TAD.get_from(r).unwrap_or_else(|| "tad".into());
    let text = fs::read_to_string(&lists).map_err(|e| format!("cannot read {lists}: {e}"))?;
    let carried = k::carried_from(&tad, Path::new(&records))?;
    let refused = k::check(&text, &name, Path::new(&repo), &carried)?;
    if !refused.is_empty() {
        for l in &refused {
            println!("{l}");
        }
        println!(
            "{name}: refused, {} things to put right before rhizoid runs",
            refused.len()
        );
        return Ok(1);
    }
    let reproduces = Contract::REPRODUCES.get_from(r);
    if reproduces.as_deref() == Some("") {
        return Err("--reproduces needs a commit".into());
    }
    let (code, said) = k::assemble(
        &rhizoid,
        &name,
        Path::new(&lists),
        Path::new(&repo),
        reproduces.as_deref(),
    )?;
    print!("{said}");
    if let Some(commit) = &reproduces {
        let v = k::verdict(&said)?
            .ok_or_else(|| format!("--reproduces {commit} was asked and rhizoid reported no comparison (an rhizoid that does not know the flag?)"))?;
        println!("{v}");
    }
    Ok(code)
}

/// `dt-check binding|dtbs`: the kernel's devicetree checks in a container.
fn dt_check(verb: Verb, r: &argenv_resolve::Resolution) -> Result<i32, String> {
    let src = need(Contract::SRC.get_from(r), "the kernel source (SRC)")?;
    let image = need(Contract::DT_IMAGE.get_from(r), "--image NAME@sha256:DIGEST")?;
    let out = need(Contract::OUT.get_from(r), "--out SCRATCH")?;
    let docker = Contract::DOCKER
        .get_from(r)
        .unwrap_or_else(|| "docker".into());
    let arch = Contract::ARCH.get_from(r).unwrap_or_else(|| "arm64".into());
    lpoco_bringup::dt_check::check_target(&arch)?;
    let (schema, dtbs) = (
        Contract::SCHEMA_FILES.get_from(r),
        Contract::DTBS.get_from(r),
    );
    if (verb == Verb::Binding && dtbs.is_some()) || (verb == Verb::Dtbs && schema.is_some()) {
        return Err("--schema-files goes with binding and --dtbs with dtbs".into());
    }
    let mut make = vec![format!("ARCH={arch}")];
    if verb == Verb::Binding {
        if let Some(f) = schema {
            make.push(format!("DT_SCHEMA_FILES={f}"));
        }
        make.push("dt_binding_check".into());
    } else {
        let dtbs = need(dtbs, "--dtbs A.dtb,B.dtb")?;
        make.push("CHECK_DTBS=y".into());
        for d in dtbs.split(',').map(str::trim) {
            lpoco_bringup::dt_check::check_target(d)?;
            make.push(d.to_string());
        }
    }
    lpoco_bringup::dt_check::run(&docker, &image, Path::new(&src), Path::new(&out), &make)
}

/// `reference capture` and `reference diff`: a stock OS's snapshot, and two snapshots compared for one part.
fn reference(verb: Verb, r: &argenv_resolve::Resolution) -> Result<i32, String> {
    use lpoco_bringup::reference as rf;
    match verb {
        Verb::Capture => {
            let via = need(
                Contract::VIA.get_from(r),
                "--via COMMAND (adb shell su 0 sh, ssh HOST sudo sh)",
            )?;
            let out = need(Contract::OUT.get_from(r), "--out DIR")?;
            for line in rf::capture(&via, Path::new(&out))? {
                println!("{line}");
            }
            Ok(0)
        }
        _ => {
            let part = need(Contract::PART.get_from(r), "the part (PART)")?;
            let live = need(Contract::LIVE.get_from(r), "--live DIR")?;
            let ours = need(Contract::OURS.get_from(r), "--ours DIR")?;
            let words: Vec<String> = Contract::MATCH
                .get_from(r)
                .unwrap_or_else(|| part.clone())
                .split(',')
                .map(|w| w.trim().to_lowercase())
                .filter(|w| !w.is_empty())
                .collect();
            let rep = rf::diff(Path::new(&live), Path::new(&ours), &words)?;
            for l in &rep.lines {
                println!("{l}");
            }
            if rep.differences > 0 {
                println!(
                    "{part}: {} differences in {} lines",
                    rep.differences, rep.matched
                );
                return Ok(1);
            }
            if rep.matched == 0 || rep.unmeasured > 0 {
                println!(
                    "{part}: not known yet ({} lines of it, {} things one side did not give; --match WORDS says what to look for)",
                    rep.matched, rep.unmeasured
                );
                return Ok(2);
            }
            println!("{part}: no difference in {} lines", rep.matched);
            Ok(0)
        }
    }
}

/// `image ensure NAME`: the image's store path, after a line for each step that got it there.
fn image_ensure(name: &str, device: &str, r: &argenv_resolve::Resolution) -> Result<i32, String> {
    let text = fs::read_to_string(device).map_err(|e| format!("cannot read {device}: {e}"))?;
    let manifest = lpoco_manifest::Manifest::parse(&text).map_err(|e| format!("{device}: {e}"))?;
    let (cache, seed, source) = (
        Contract::CACHE.get_from(r),
        Contract::SEED.get_from(r),
        Contract::SOURCE.get_from(r),
    );
    let sources = Sources {
        cache: cache.as_deref().map(Path::new),
        seed: seed.as_deref().map(Path::new),
        source: source.as_deref().map(Path::new),
        version: VERSION,
    };
    if Contract::DRY_RUN.get_from(r).unwrap_or(false) {
        let (origin, path) = ensure::plan(&manifest, name, &sources)?;
        println!("{name}: {origin} ({})", path.display());
        return Ok(0);
    }
    if Contract::REBUILD.get_from(r).unwrap_or(false) {
        let path = ensure::rebuild(&manifest, name, &sources, &mut |line| eprintln!("{line}"))?;
        println!("{}", path.display());
        return Ok(0);
    }
    let path = ensure::ensure(&manifest, name, &sources, &mut |line| eprintln!("{line}"))?;
    println!("{}", path.display());
    Ok(0)
}
