// SPDX-License-Identifier: MIT OR Apache-2.0
//! The kernel's devicetree checks (`dt_binding_check`, `CHECK_DTBS=y`) in a throwaway container, so the host
//! needs no Python: dtschema lives only in the image. The image is named by digest, never a tag, so a later
//! release of dtschema cannot change a result unseen. The source is mounted read-only and the output goes to
//! a scratch folder, handed back to the user afterwards. Measured in docs research
//! upstream-forks/2026-11-19/dt-checks-in-a-container.md.

use std::path::Path;
use std::process::Command;

/// What the container runs: install the tools, run make with the arguments docker passes after the script
/// (so nothing the caller names is spliced into a shell string), give the output back to the user.
const SCRIPT: &str =
    "apk add -q make gcc musl-dev flex bison yamllint py3-dt-schema dtc && make \"$@\"; \
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

/// Run the check; 0 when clean, 1 when the checker (or the container) reports a failure. The container's
/// own output is passed through, so a schema error is named in it.
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
    let args = docker_args(image, &src, &out, &id("-u")?, &id("-g")?, make_args);
    let st = Command::new(docker)
        .args(&args)
        .status()
        .map_err(|e| format!("cannot run {docker}: {e}"))?;
    let said = make_args.join(" ");
    match st.code() {
        Some(0) => {
            println!("dt-check: clean ({said})");
            Ok(0)
        }
        Some(1) | Some(2) => {
            println!("dt-check: failed ({said}); the checker's lines are above");
            Ok(1)
        }
        Some(70) => Err("the container could not install its tools (apk add)".into()),
        other => Err(format!(
            "{docker} ended with {other:?} before a verdict (not a check failure)"
        )),
    }
}
