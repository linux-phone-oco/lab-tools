// SPDX-License-Identifier: MIT OR Apache-2.0
//! image-recipe-kernel-from-int-tag: `lpoco-bringup image ensure NAME` for a `[kernel_builds.NAME]` builds the kernel
//! file from its tag's commit in Nix's sandbox as a fixed-output derivation (decision
//! 2026-11-23-image-recipes-on-nix-fixed-output-paths). The tree here is a stand-in with the kernel's interface
//! (make defconfig / olddefconfig / a target, scripts/kconfig/merge_config.sh -m -O): it writes into its output
//! what the build was given, so the test sees the fixed clock, user, host, counter, flags, config fragment and
//! patch arrive. What the real tree gives is measured once on the lab (research image-recipes-how-it-stands,
//! the kernel-build measurement), not here. Where nix is absent the test says "not measured".
//! The expected bytes are computed here from the declared values, never read back from the build.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

// 2026-11-23 08:03:37 UTC: the tagger time of int/walleye/ufs-off/v7.3/1 (git cat-file -p of the tag), and the text
// the kernel build's clock must show for it (`date -u -d @1795421017` in the C locale, read once by hand)
const EPOCH: u64 = 1795421017;
const CLOCK: &str = "Mon Nov 23 08:03:37 UTC 2026";

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "lpoco-bringup-ensure-kernel-{}-{name}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn run(dir: &Path, tool: &str, args: &[&str]) -> String {
    let o = Command::new(tool)
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(o.status.success(), "{tool} {args:?}: {}", text(&o.stderr));
    text(&o.stdout).trim().to_string()
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A git repo shaped like the kernel's interface, tagged int/test/list/v1/1; returns its commit.
fn tree(dir: &Path) -> String {
    fs::create_dir_all(dir.join("scripts/kconfig")).unwrap();
    fs::write(
        dir.join("Makefile"),
        "defconfig:\n\tmkdir -p $(O)\n\techo CONFIG_BASE=y > $(O)/.config\n\
         olddefconfig:\n\techo CONFIG_OLD=y >> $(O)/.config\n\
         out.bin:\n\tmkdir -p $(O)/arch\n\t{ echo \"clock=$(KBUILD_BUILD_TIMESTAMP)\"; echo \"user=$(KBUILD_BUILD_USER) host=$(KBUILD_BUILD_HOST) count=$(KBUILD_BUILD_VERSION)\"; echo \"dtc=$(DTC_FLAGS)\"; cat $(O)/.config; cat $(CURDIR)/extra.txt; } > $(O)/arch/out.bin\n",
    )
    .unwrap();
    // the kernel's merge_config.sh -m -O DIR BASE FRAGMENT...: the fragments' lines appended to BASE
    fs::write(
        dir.join("scripts/kconfig/merge_config.sh"),
        "#!/bin/sh\nshift 3\nbase=$1; shift\ncat \"$@\" >> \"$base\"\n",
    )
    .unwrap();
    run(dir, "chmod", &["+x", "scripts/kconfig/merge_config.sh"]);
    fs::write(dir.join("extra.txt"), "tree\n").unwrap();
    let g = |a: &[&str]| run(dir, "git", a);
    g(&["init", "-q", "-b", "main"]);
    g(&["add", "."]);
    g(&[
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "commit",
        "-q",
        "-m",
        "tree",
    ]);
    g(&[
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "tag",
        "-a",
        "-m",
        "t",
        "int/test/list/v1/1",
    ]);
    g(&["rev-parse", "int/test/list/v1/1^{commit}"])
}

fn manifest(dir: &Path, repo: &Path, rev: &str, out_sha: &str, cfg: &str, patch: &str) -> PathBuf {
    let t = format!(
        "[kernel_builds.test]\ntag = \"int/test/list/v1/1\"\nrev = \"{rev}\"\nrepo = \"file://{}\"\nepoch = {EPOCH}\n\
         configs = [\"{cfg}\"]\npatches = [\"{patch}\"]\ntargets = [\"out.bin\"]\ndtc_flags = \"-@\"\noutput = \"arch/out.bin\"\nsha256 = \"{out_sha}\"\n",
        repo.display()
    );
    let base = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/device.toml"),
    )
    .unwrap();
    let p = dir.join("device.toml");
    fs::write(&p, format!("{base}\n{t}")).unwrap();
    p
}

fn ensure(device: &Path, seed: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lpoco-bringup"))
        .args(["image", "ensure", "test", "--device"])
        .arg(device)
        .arg("--seed")
        .arg(seed)
        .output()
        .unwrap()
}

#[test]
fn a_kernel_is_built_from_its_tag_with_the_clock_and_names_fixed_and_held_to_its_sha256() {
    if !Command::new("nix-store")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        eprintln!("not measured: no nix");
        return;
    }
    let dir = tmp("k");
    let repo = dir.join("linux");
    fs::create_dir_all(&repo).unwrap();
    let rev = tree(&repo);
    let seed = dir.join("seed");
    fs::create_dir_all(&seed).unwrap();
    // inputs by sha256: a config fragment, and a diff that changes extra.txt (patch -p1)
    let cfg = "CONFIG_FRAGMENT=y\n";
    let patch = "--- a/extra.txt\n+++ b/extra.txt\n@@ -1 +1 @@\n-tree\n+patched\n";
    fs::write(seed.join("walleye.config"), cfg).unwrap();
    fs::write(seed.join("dts.patch"), patch).unwrap();
    // the declared sha256: of what the declared values must produce (date -u -d @EPOCH is the clock's text)
    let clock = CLOCK;
    let want = format!(
        "clock={clock}\nuser=lpoco host=lab count=1\ndtc=-@\nCONFIG_BASE=y\n{cfg}CONFIG_OLD=y\npatched\n"
    );
    let want_sha = sha256(want.as_bytes());
    // a path an earlier run left in the store would be served, not built: the test starts without it
    let stored = run(
        &dir,
        "nix-store",
        &[
            "--print-fixed-path",
            "sha256",
            &want_sha,
            &format!("kernel-{}", &want_sha[..12]),
        ],
    );
    let _ = Command::new("nix-store")
        .args(["--delete", &stored])
        .output();
    let device = manifest(
        &dir,
        &repo,
        &rev,
        &want_sha,
        &sha256(cfg.as_bytes()),
        &sha256(patch.as_bytes()),
    );

    let o = ensure(&device, &seed);
    assert!(
        o.status.success(),
        "ensure (build): {}{}",
        text(&o.stdout),
        text(&o.stderr)
    );
    let path = text(&o.stdout).trim().to_string();
    assert_eq!(
        fs::read(&path).unwrap(),
        want.as_bytes(),
        "the kernel built is not what its declared inputs make"
    );
    assert!(
        text(&o.stderr).contains("built from its recipe"),
        "{}",
        text(&o.stderr)
    );

    // served by its sha256 the second time: nothing runs
    let o = ensure(&device, &seed);
    assert!(
        text(&o.stderr).contains("in the store") && !text(&o.stderr).contains("built"),
        "{}",
        text(&o.stderr)
    );

    // a declared sha256 the inputs do not give is refused by Nix
    let wrong = "0".repeat(64);
    let device = manifest(
        &dir,
        &repo,
        &rev,
        &wrong,
        &sha256(cfg.as_bytes()),
        &sha256(patch.as_bytes()),
    );
    let o = ensure(&device, &seed);
    assert!(
        !o.status.success(),
        "a kernel that is not the declared sha256 passed"
    );
    assert!(
        text(&o.stderr).contains("hash mismatch"),
        "refused for another reason than the hash: {}",
        text(&o.stderr)
    );
}

#[test]
fn a_recipes_kernel_that_nothing_holds_is_built_by_the_row_that_declares_its_sha256() {
    if !Command::new("nix-store")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        eprintln!("not measured: no nix");
        return;
    }
    let dir = tmp("r");
    let repo = dir.join("linux");
    fs::create_dir_all(&repo).unwrap();
    let rev = tree(&repo);
    let seed = dir.join("seed");
    fs::create_dir_all(&seed).unwrap();
    let (cfg, patch) = (
        "CONFIG_FRAGMENT=y\n",
        "--- a/extra.txt\n+++ b/extra.txt\n@@ -1 +1 @@\n-tree\n+patched\n",
    );
    fs::write(seed.join("walleye.config"), cfg).unwrap();
    fs::write(seed.join("dts.patch"), patch).unwrap();
    let want = format!(
        "clock={CLOCK}\nuser=lpoco host=lab count=1\ndtc=-@\nCONFIG_BASE=y\n{cfg}CONFIG_OLD=y\npatched\n"
    );
    let want_sha = sha256(want.as_bytes());
    let stored = run(
        &dir,
        "nix-store",
        &[
            "--print-fixed-path",
            "sha256",
            &want_sha,
            &format!("kernel-{}", &want_sha[..12]),
        ],
    );
    let _ = Command::new("nix-store")
        .args(["--delete", &stored])
        .output();
    // an image whose recipe names the kernel by sha256; its control is nowhere, so ensure fails at the control -
    // after it has made the kernel (the parts are found first), which is what is looked at
    let base = manifest(
        &dir,
        &repo,
        &rev,
        &want_sha,
        &sha256(cfg.as_bytes()),
        &sha256(patch.as_bytes()),
    );
    let t = format!(
        "{}\n[images.ctl]\npath = \"ctl.img\"\nsha256 = \"{}\"\n[images.out]\npath = \"out.img\"\nsha256 = \"{}\"\n[images.out.recipe]\ncontrol = \"ctl\"\nkernel = \"{want_sha}\"\n",
        fs::read_to_string(&base).unwrap(),
        "11".repeat(32),
        "22".repeat(32)
    );
    fs::write(&base, t).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_lpoco-bringup"))
        .args(["image", "ensure", "out", "--device"])
        .arg(&base)
        .arg("--seed")
        .arg(&seed)
        .output()
        .unwrap();
    let said = text(&o.stderr);
    assert!(!o.status.success(), "the missing control passed: {said}");
    assert!(
        said.contains("built from its recipe (tag int/test/list/v1/1)"),
        "{said}"
    );
    assert_eq!(fs::read(&stored).unwrap(), want.as_bytes());
}
