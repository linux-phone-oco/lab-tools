// SPDX-License-Identifier: MIT OR Apache-2.0
//! lpoco-layer: the device layer packed (run as `lpoco layer ...`). See cli.rs for the invocation surface.

mod cli;

use argenv_resolve::{InputResolveExt, OnProblems};
use cli::Contract;
use lpoco_layer::{materialize, nfpm_yaml, packages, Format};
use lpoco_manifest::Manifest;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let r = Contract::parse_and_lint(OnProblems::FailOnError);
    if Contract::HELP.get_from(&r).unwrap_or(false) {
        print!("{}", cli::usage("lpoco-layer", "A phone's device layer, declared once in device.toml [layer], packed into packages (run as lpoco layer). Command: pack --format apk|deb|rpm|archlinux writes an nfpm config per package and has nfpm pack it."));
        return;
    }
    if Contract::VERSION.get_from(&r).unwrap_or(false) {
        println!("lpoco-layer {VERSION}");
        return;
    }
    if let Err(e) = pack(&r) {
        eprintln!("lpoco-layer: {e}");
        std::process::exit(2);
    }
}

fn pack(r: &argenv_resolve::Resolution) -> Result<(), String> {
    Contract::COMMAND
        .get_from(r)
        .ok_or("no command: say `pack` (see --help)")?;
    let format = Contract::FORMAT
        .get_from(r)
        .and_then(|f| Format::parse(&f))
        .ok_or("--format is needed: apk, deb, rpm or archlinux")?;
    let manifest = PathBuf::from(
        Contract::MANIFEST
            .get_from(r)
            .unwrap_or_else(|| "device.toml".into()),
    );
    let out = PathBuf::from(Contract::OUT.get_from(r).unwrap_or_else(|| ".".into()));
    let version = Contract::PKG_VERSION
        .get_from(r)
        .unwrap_or_else(|| "0.1.0".into());
    let nfpm = Contract::NFPM.get_from(r).unwrap_or_else(|| "nfpm".into());

    let text = fs::read_to_string(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let m = Manifest::parse(&text).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let layer = m
        .layer
        .as_ref()
        .ok_or_else(|| format!("{} declares no [layer]", manifest.display()))?;
    let dir = manifest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .to_path_buf();
    let dir = std::path::absolute(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let out = std::path::absolute(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let pkgs = packages(layer, format);
    if pkgs.is_empty() {
        return Err("the layer holds no file to pack".into());
    }
    fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let stage = out.join("stage");
    for p in &pkgs {
        let entries = materialize(p, &dir, &stage)?;
        let conf = out.join(format!("{}.nfpm.yaml", p.name));
        fs::write(&conf, nfpm_yaml(p, format, &version, &entries)?)
            .map_err(|e| format!("{}: {e}", conf.display()))?;
        let target = out.join(format.file_name(&p.name, &version));
        let status = Command::new(&nfpm)
            .args(["package", "--packager", format.token(), "--config"])
            .arg(&conf)
            .arg("--target")
            .arg(&target)
            .status()
            .map_err(|e| format!("{nfpm}: {e} (the config is written: {})", conf.display()))?;
        if !status.success() {
            return Err(format!("{nfpm} failed on {}", conf.display()));
        }
        println!("{}", target.display());
    }
    Ok(())
}
