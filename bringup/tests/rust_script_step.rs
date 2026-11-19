// SPDX-License-Identifier: MIT OR Apache-2.0
//! A device repository's rust-script step can use lpoco-bringup as a library: examples/step.rs, run by
//! rust-script (its git dependency pointed at this very checkout so the test needs no network), packs the
//! same image the command does.

use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn a_rust_script_step_packs_through_the_library() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fx = root.join("tests/fixtures");
    let dir = std::env::temp_dir().join(format!("lpoco-bringup-rs-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let src = fs::read_to_string(root.join("examples/step.rs")).unwrap();
    let git = src
        .lines()
        .find(|l| l.starts_with("//! lpoco-bringup = { git"))
        .expect("the example depends on lpoco-bringup by git tag");
    assert!(git.contains("tag = \""), "{git}");
    let local = format!("//! lpoco-bringup = {{ path = \"{}\" }}", root.display());
    fs::write(dir.join("step.rs"), src.replace(git, &local)).unwrap();

    // a kernel with its device tree appended, as the manifest's msm_id check wants
    let mut k = fs::read(fx.join("kernel")).unwrap();
    k.extend(fs::read(fx.join("appended.dtb")).unwrap());
    let kernel = dir.join("kernel");
    fs::write(&kernel, k).unwrap();
    // a kernel with its device tree appended, as the manifest's msm_id check wants
    let mut k = fs::read(fx.join("kernel")).unwrap();
    k.extend(fs::read(fx.join("appended.dtb")).unwrap());
    let kernel = dir.join("kernel");
    fs::write(&kernel, k).unwrap();
    let (a, b) = (dir.join("a.img"), dir.join("b.img"));
    let o = Command::new("rust-script")
        .arg(dir.join("step.rs"))
        .args([
            fx.join("device.toml"),
            kernel.clone(),
            fx.join("ramdisk"),
            a.clone(),
        ])
        .output()
        .expect("rust-script is on the lab machine");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let o = Command::new(env!("CARGO_BIN_EXE_lpoco-bringup"))
        .args(["image", "pack", "--device"])
        .arg(fx.join("device.toml"))
        .arg("--kernel")
        .arg(&kernel)
        .arg("--ramdisk")
        .arg(fx.join("ramdisk"))
        .arg("--out")
        .arg(&b)
        .output()
        .unwrap();
    assert!(o.status.success());
    assert_eq!(fs::read(&a).unwrap(), fs::read(&b).unwrap());
}
