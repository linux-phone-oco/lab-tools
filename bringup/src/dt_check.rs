// SPDX-License-Identifier: MIT OR Apache-2.0
//! The kernel's devicetree checks (`dt_binding_check`, `CHECK_DTBS=y`) in a throwaway container, so the host
//! needs no Python: dtschema lives only in the image. The image is named by digest, never a tag, so a later
//! release of dtschema cannot change a result unseen. The source is mounted read-only and the output goes to
//! a scratch folder, handed back to the user afterwards. Measured in docs research
//! upstream-forks/2026-11-19/dt-checks-in-a-container.md.

use std::path::Path;
use std::process::Command;

/// What the container runs: run make (the tools are in the image, dt-check/Dockerfile) with the arguments docker passes after the script
/// (so nothing the caller names is spliced into a shell string), give the output back to the user.
const SCRIPT: &str = "make \"$@\"; \
rc=$?; chown -R \"$HOST_UID:$HOST_GID\" /out; exit $rc";

/// `name@sha256:<64 hex>`: an image by digest.
pub fn check_digest(image: &str) -> Result<(), String> {
    match image.split_once("@sha256:") {
        Some((name, hex))
            if !name.is_empty()
                && hex.len() == 64
                && hex.bytes().all(|b| b.is_ascii_hexdigit()) =>
        {
            Ok(())
        }
        _ => Err(format!(
            "--image {image}: name the image by digest (NAME@sha256:64 hex digits), never a tag"
        )),
    }
}

fn id(flag: &str) -> Result<String, String> {
    let o = Command::new("id")
        .arg(flag)
        .output()
        .map_err(|e| format!("cannot run id: {e}"))?;
    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
    if !o.status.success() || s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("id {flag} gave no number"));
    }
    Ok(s)
}

/// A path that docker reads as `-v SRC:DEST:ro` cannot hold `:` or `,`.
fn plain_path(what: &str, p: &Path) -> Result<String, String> {
    let s = p
        .to_str()
        .ok_or_else(|| format!("{what} {} is not UTF-8", p.display()))?;
    if s.contains([':', ',']) {
        return Err(format!(
            "{what} {s}: a path with : or , cannot be mounted read-only safely"
        ));
    }
    Ok(s.to_string())
}

/// A make word that is a plain target, such as qcom/x.dtb: no option, no assignment.
pub fn check_target(t: &str) -> Result<(), String> {
    if t.is_empty() || t.starts_with('-') || t.contains(['=', ' ']) {
        return Err(format!(
            "{t:?}: a dtb to build is a plain make target (no -option, no NAME=value)"
        ));
    }
    Ok(())
}

/// The docker arguments for one check. `make_args` are the arguments to make, after O=/out.
pub fn docker_args(
    image: &str,
    src: &Path,
    out: &Path,
    uid: &str,
    gid: &str,
    make_args: &[String],
) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "run".into(),
        "--rm".into(),
        "-e".into(),
        format!("HOST_UID={uid}"),
        "-e".into(),
        format!("HOST_GID={gid}"),
        "-v".into(),
        format!("{}:/src:ro", src.display()),
        "-v".into(),
        format!("{}:/out", out.display()),
        "-w".into(),
        "/src".into(),
        image.into(),
        "sh".into(),
        "-c".into(),
        SCRIPT.into(),
        "sh".into(),
        "O=/out".into(),
    ];
    a.extend(make_args.iter().cloned());
    a
}
/// The lines of the checker's output that are violations: dtschema names a file and what is wrong with it
/// (`arch/arm64/boot/dts/qcom/x.dtb: pmic: ... Unevaluated properties are not allowed`). make exits 0 on
/// these (CHECK_DTBS and the binding examples report them as warnings), so the verdict comes from them.
/// yamllint's parsable line: `FILE.yaml:LINE:COL: [error] ...`.
fn yamllint(l: &str) -> bool {
    l.split_once(".yaml:")
        .is_some_and(|(_, r)| r.starts_with(|c: char| c.is_ascii_digit()))
}

pub fn violations(output: &str) -> Vec<&str> {
    output
        .lines()
        .filter(|l| l.contains(".dtb: ") || l.contains(".yaml: ") || yamllint(l))
        .collect()
}

/// Run the check; 0 when clean, 1 when the schema's output names a violation. The container's own output
/// is passed through. make ending non-zero with no violation named is an error (the setup, not the check).
pub fn run(
    docker: &str,
    image: &str,
    src: &Path,
    out: &Path,
    make_args: &[String],
) -> Result<i32, String> {
    check_digest(image)?;
    if !src.is_dir() {
        return Err(format!(
            "{}: the kernel source folder is not there",
            src.display()
        ));
    }
    std::fs::create_dir_all(out).map_err(|e| format!("cannot make {}: {e}", out.display()))?;
    let src = src
        .canonicalize()
        .map_err(|e| format!("{}: {e}", src.display()))?;
    let out = out
        .canonicalize()
        .map_err(|e| format!("{}: {e}", out.display()))?;
    plain_path("--src", &src)?;
    plain_path("--out", &out)?;
    // a fresh --out has no .config, and the dtbs checks stop without one: make defconfig first
    let mut make_args = make_args.to_vec();
    if make_args.iter().any(|a| a == "CHECK_DTBS=y") && !out.join(".config").exists() {
        let at = make_args
            .iter()
            .position(|a| !a.contains('='))
            .unwrap_or(make_args.len());
        make_args.insert(at, "defconfig".into());
    }
    // make skips the check of a dtb already built (the check runs inside its build command): build it again
    let arch = make_args
        .iter()
        .find_map(|a| a.strip_prefix("ARCH="))
        .unwrap_or("arm64")
        .to_string();
    for t in make_args.iter().filter(|a| a.ends_with(".dtb")) {
        let _ = std::fs::remove_file(out.join(format!("arch/{arch}/boot/dts/{t}")));
    }
    let args = docker_args(image, &src, &out, &id("-u")?, &id("-g")?, &make_args);
    let o = Command::new(docker)
        .args(&args)
        .output()
        .map_err(|e| format!("cannot run {docker}: {e}"))?;
    let (so, se) = (
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr),
    );
    print!("{so}");
    eprint!("{se}");
    let said = make_args.join(" ");
    let found = format!("{so}\n{se}");
    let bad = violations(&found);
    if !bad.is_empty() {
        println!(
            "dt-check: failed ({said}): {} violation(s), named above",
            bad.len()
        );
        return Ok(1);
    }
    match o.status.code() {
        Some(0) => {
            println!("dt-check: clean ({said})");
            Ok(0)
        }
        other => Err(format!(
            "{docker} ended with {other:?} and no violation was named: the setup failed, not a check failure (the lines above say why; is the image's toolset complete?)"
        )),
    }
}
