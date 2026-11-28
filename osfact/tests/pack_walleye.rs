// SPDX-License-Identifier: MIT OR Apache-2.0
//! The proof of item osfact-pack-alpine-walleye: `osfact pack now` joins the three stages (the root file system,
//! the boot image, install.toml) for one device declaration and one distro profile, prints the sha256 of each image
//! and the one install command.
//!
//! Where each expected value comes from (a check names its source):
//! the declaration and its [boot] facts: tests/fixtures/google-walleye/device.toml (the lab's device.toml at 6ab243c5;
//!   `arch` in [device] is the one key added, proposed by item osfact-device-declaration);
//! the boot image's size, magic and AVB footer: that file's `partition_size` (33554432), the Android boot image magic
//!   `ANDROID!` and avbtool's footer magic `AVBf` 64 bytes before the end (src/avb.rs, add_hash_footer);
//! the parts: tests/fixtures/boot/ (kernel, ramdisk, appended.dtb), the files lpoco-bringup's tests used;
//! the plan's way back: tests/fixtures/plans/walleye-ram.toml (fastboot reboot, wait = [boot] back_wait = 240).
//! The first tests run anywhere. The last builds for real and needs the network, fakeroot, mke2fs, debugfs and sha256sum:
//!     cargo test -p osfact --test pack_walleye -- --ignored

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn scratch(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("osfact-{name}-{}", std::process::id()))
}

/// A parts folder as `pack now --parts` reads it: kernel, ramdisk and the declaration's [boot] dtb path.
fn parts(dir: &Path) -> PathBuf {
    let p = dir.join("parts");
    fs::create_dir_all(p.join("qcom")).expect("parts dir");
    fs::copy(fixtures().join("boot/kernel"), p.join("kernel")).expect("kernel");
    fs::copy(fixtures().join("boot/ramdisk"), p.join("ramdisk")).expect("ramdisk");
    fs::copy(
        fixtures().join("boot/appended.dtb"),
        p.join("qcom/msm8998-google-walleye.dtb"),
    )
    .expect("dtb");
    p
}

fn pack(device: &Path, parts: &Path, out: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_osfact"))
        .args(["pack", "now", "--device"])
        .arg(device)
        .arg("--distro")
        .arg(fixtures().join("distros/alpine.toml"))
        .arg("--parts")
        .arg(parts)
        .arg("--out")
        .arg(out)
        .args(extra)
        .output()
        .expect("osfact runs")
}

fn device() -> PathBuf {
    fixtures().join("google-walleye/device.toml")
}

fn sha256(file: &Path) -> String {
    let out = Command::new("sha256sum")
        .arg(file)
        .output()
        .expect("sha256sum runs");
    assert!(out.status.success(), "sha256sum {}", file.display());
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .expect("a sum")
        .to_string()
}

#[test]
fn a_dry_run_lists_the_three_stages_for_the_devices_arch_and_writes_nothing() {
    let dir = scratch("pack-dry");
    let out = dir.join("out");
    let run = pack(&device(), &dir.join("parts"), &out, &["--dry-run"]);
    let said = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success(),
        "pack now --dry-run failed: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    for want in [
        // stage 1: the root file system, for the arch the declaration's [device] says
        "--arch aarch64",
        " add alpine-base openssh e2fsprogs",
        "check: the two builds are the same bytes",
        // stage 2: the boot image, in the format the declaration's [boot] names
        "boot image: android-bootimg",
        "boot.img",
        // stage 3: the install plan and the one command
        "install.toml",
        "install: osfact install now --plan ",
    ] {
        assert!(said.contains(want), "the dry run lacks `{want}`:\n{said}");
    }
    assert!(!out.exists(), "a dry run made {}", out.display());
}

#[test]
fn a_declaration_that_says_no_arch_is_refused_naming_the_key() {
    let dir = scratch("pack-noarch");
    fs::create_dir_all(&dir).expect("dir");
    let file = dir.join("device.toml");
    let text = fs::read_to_string(device())
        .expect("fixture")
        .replace("arch = \"aarch64\"", "");
    fs::write(&file, text).expect("write");
    let run = pack(&file, &dir.join("parts"), &dir.join("out"), &["--dry-run"]);
    assert!(
        !run.status.success(),
        "a declaration without arch was packed"
    );
    let said = String::from_utf8_lossy(&run.stderr);
    assert!(
        said.contains("[device]") && said.contains("arch"),
        "the refusal does not name [device] arch: {said}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn an_unknown_device_key_is_refused_naming_it() {
    let dir = scratch("pack-unknown-device-key");
    fs::create_dir_all(&dir).expect("dir");
    let file = dir.join("device.toml");
    let text = fs::read_to_string(device())
        .expect("fixture")
        .replace("arch = \"aarch64\"", "arch = \"aarch64\"\nfrobnicate = 1");
    fs::write(&file, text).expect("write");
    let run = pack(&file, &dir.join("parts"), &dir.join("out"), &["--dry-run"]);
    assert!(
        !run.status.success(),
        "a declaration with an unknown [device] key was packed"
    );
    let said = String::from_utf8_lossy(&run.stderr);
    assert!(
        said.contains("frobnicate"),
        "the refusal does not name the key: {said}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "builds for real: needs the network, fakeroot, mke2fs, debugfs, gzip and sha256sum"]
fn the_walleye_declaration_packs_an_installable_alpine_image() {
    let dir = scratch("pack-real");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("dir");
    let parts = parts(&dir);
    let out = dir.join("out");
    let run = pack(&device(), &parts, &out, &[]);
    let said = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success(),
        "osfact pack now failed: {}\n{said}",
        String::from_utf8_lossy(&run.stderr)
    );

    // the root tree is the device's arch, though this host is not: busybox reads as an AArch64 ELF (e_machine 0xb7)
    let rootfs = out.join("rootfs.img");
    let busybox = Command::new("debugfs")
        .args(["-R", "cat /bin/busybox"])
        .arg(&rootfs)
        .output()
        .expect("debugfs runs");
    let elf = &busybox.stdout;
    assert_eq!(
        elf.get(..4),
        Some(&b"\x7fELF"[..]),
        "/bin/busybox is no ELF"
    );
    assert_eq!(
        elf.get(18..20),
        Some(&[0xb7, 0x00][..]),
        "busybox is not AArch64"
    );

    // the boot image: the device's format, the size of its partition, the AVB footer
    let boot = fs::read(out.join("boot.img")).expect("OUT/boot.img");
    assert_eq!(boot.get(..8), Some(&b"ANDROID!"[..]));
    assert_eq!(
        boot.len(),
        33554432,
        "boot.img is not [boot] partition_size long"
    );
    assert_eq!(
        boot.get(boot.len() - 64..boot.len() - 60),
        Some(&b"AVBf"[..]),
        "boot.img has no AVB footer"
    );

    // install.toml names the boot image and the compressed root image (the file that is pushed) with the sha256 the pack measured, boot, push and enter in order, and the command is osfact's own
    let plan = fs::read_to_string(out.join("install.toml")).expect("OUT/install.toml");
    for image in [&out.join("root.tar.gz"), &out.join("boot.img")] {
        let sum = sha256(image);
        assert!(
            plan.contains(&sum),
            "install.toml lacks {}'s sha256",
            image.display()
        );
        assert!(
            said.lines().any(|l| l.ends_with(&format!(" sha256 {sum}"))),
            "the pack did not print {}'s sha256:\n{said}",
            image.display()
        );
    }
    let listed_plan =
        osfact::install::read(&plan).expect("install.toml is one install::read accepts");
    let order: Vec<_> = listed_plan.install.step.iter().map(|s| s.action).collect();
    assert_eq!(
        order,
        [
            osfact::install::StepAction::BootFromRam,
            osfact::install::StepAction::Push,
            osfact::install::StepAction::Enter
        ]
    );
    let untar = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "gzip -dc {} | tar -t > /dev/null",
            out.join("root.tar.gz").display()
        ))
        .status()
        .expect("sh runs");
    assert!(untar.success(), "root.tar.gz is no gzip of a tar");
    let command = said
        .lines()
        .rev()
        .find_map(|l| l.strip_prefix("install: "))
        .unwrap_or_else(|| panic!("no 'install: COMMAND' line:\n{said}"));
    assert!(
        command.starts_with("osfact install now --plan ")
            && command.contains(&*out.to_string_lossy()),
        "not osfact's install command for this image: {command}"
    );
    // and that command's dry run lists a fastboot boot of the packed boot image, then the way back
    let dry = Command::new(env!("CARGO_BIN_EXE_osfact"))
        .args(["install", "now", "--dry-run", "--plan"])
        .arg(out.join("install.toml"))
        .output()
        .expect("osfact runs");
    let listed = String::from_utf8_lossy(&dry.stdout);
    assert!(
        dry.status.success(),
        "install dry run: {}",
        String::from_utf8_lossy(&dry.stderr)
    );
    assert!(
        listed.contains(" boot ") && listed.contains("boot.img"),
        "{listed}"
    );
    assert!(
        listed.contains("way back: in the shell") && listed.contains("wait 240"),
        "{listed}"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// The fixture declaration with one line removed, packed in a dry run: what the refusal says.
fn refused_without(line_start: &str, name: &str) -> String {
    let dir = scratch(name);
    fs::create_dir_all(&dir).expect("dir");
    let file = dir.join("device.toml");
    let text: String = fs::read_to_string(device())
        .expect("fixture")
        .lines()
        .filter(|l| !l.starts_with(line_start))
        .map(|l| format!("{l}\n"))
        .collect();
    fs::write(&file, text).expect("write");
    let run = pack(&file, &dir.join("parts"), &dir.join("out"), &["--dry-run"]);
    assert!(!run.status.success(), "{name}: the declaration was packed");
    let _ = fs::remove_dir_all(&dir);
    String::from_utf8_lossy(&run.stderr).into_owned()
}

#[test]
fn a_declaration_without_back_wait_is_refused_before_any_build_naming_file_and_key() {
    let said = refused_without("back_wait", "pack-noback");
    assert!(
        said.contains("device.toml") && said.contains("back_wait"),
        "{said}"
    );
}

#[test]
fn an_avb_footer_without_a_partition_size_is_refused_naming_file_and_key() {
    let said = refused_without("partition_size", "pack-nopart");
    assert!(
        said.contains("device.toml")
            && said.contains("avb_footer")
            && said.contains("partition_size"),
        "{said}"
    );
}

/// Item osfact-pack-writes-the-ram-root-plan. Where the expected values come from (a check names its source):
/// the plan that boots, pushes and enters is tests/fixtures/plans/walleye-root-in-ram.toml (its own header names its
/// sources: google-walleye-lab's build/initramfs-modem and procedures/steps/src/modem_boot.rs); the declaration's
/// [install.connection], [install.root] and [install.way_back] in tests/fixtures/google-walleye/device.toml carry
/// those same values. The pack made from the declaration must list the same commands, in the same order.
fn commands_of(text: &str) -> Vec<String> {
    let plan = osfact::install::read(text).expect("install::read accepts the plan");
    osfact::install::commands(&plan).expect("commands")
}

fn files_named(boot_sum: &str, root_sum: &str) -> osfact::pack::Packed {
    osfact::pack::Packed {
        rootfs: ("rootfs.img".into(), "2".repeat(64)),
        boot: ("boot.img".into(), boot_sum.into()),
        root: ("root.tar.gz".into(), root_sum.into()),
    }
}

#[test]
fn the_declaration_makes_the_plan_that_boots_pushes_and_enters_in_order() {
    let declaration = fs::read_to_string(device()).expect("fixture");
    let made =
        osfact::pack::plan_text(&declaration, &files_named(&"0".repeat(64), &"1".repeat(64)))
            .expect("the plan");
    let want = fs::read_to_string(fixtures().join("plans/walleye-root-in-ram.toml")).expect("plan");
    assert_eq!(
        commands_of(&made),
        commands_of(&want),
        "the pack's plan lists other commands than the walleye plan of walleye-root-in-ram.toml:\n{made}"
    );
    let plan = osfact::install::read(&made).expect("read");
    let order: Vec<_> = plan.install.step.iter().map(|s| s.action).collect();
    assert_eq!(
        order,
        [
            osfact::install::StepAction::BootFromRam,
            osfact::install::StepAction::Push,
            osfact::install::StepAction::Enter
        ]
    );
    let root = plan
        .install
        .image
        .iter()
        .find(|i| i.name == "root")
        .expect("root image");
    assert_eq!(
        root.sha256,
        "1".repeat(64),
        "the root image's sha256 is the compressed file's"
    );
    assert_eq!(root.compression, Some(osfact::install::Compression::Gzip));
}

/// The declaration without one table: its lines up to the next table header.
fn without_table(text: &str, header: &str) -> String {
    let mut dropping = false;
    let kept: Vec<&str> = text
        .lines()
        .filter(|l| {
            if l.starts_with('[') {
                dropping = l.trim() == header;
            }
            !dropping
        })
        .collect();
    kept.join("\n")
}

#[test]
fn a_declaration_with_a_connection_and_no_root_is_refused_naming_the_table() {
    let declaration = fs::read_to_string(device()).expect("fixture");
    let declaration = without_table(&declaration, "[install.root]");
    let said =
        osfact::pack::plan_text(&declaration, &files_named(&"0".repeat(64), &"1".repeat(64)))
            .expect_err("refused");
    assert!(said.contains("[install.root]"), "{said}");
}

#[test]
fn the_plan_is_the_walleye_plan_value_for_value() {
    // Source: plans/walleye-root-in-ram.toml; commands() leaves out waits, checks and ways, so the values are compared.
    let declaration = fs::read_to_string(device()).expect("fixture");
    let made =
        osfact::pack::plan_text(&declaration, &files_named(&"0".repeat(64), &"1".repeat(64)))
            .expect("the plan");
    let want = fs::read_to_string(fixtures().join("plans/walleye-root-in-ram.toml")).expect("plan");
    let (made, want): (toml::Value, toml::Value) = (
        toml::from_str(&made).expect("made"),
        toml::from_str(&want).expect("want"),
    );
    assert_eq!(made, want);
}

#[test]
fn a_declaration_without_the_install_tables_makes_the_boot_only_plan() {
    // Source: plans/walleye-ram.toml (boot from RAM, way back fastboot reboot, wait = [boot] back_wait = 240).
    let text = fs::read_to_string(device()).expect("fixture");
    let cut = text
        .find("\n# Proposed (item osfact-pack-writes")
        .expect("the install tables");
    let made =
        osfact::pack::plan_text(&text[..cut], &files_named(&"0".repeat(64), &"1".repeat(64)))
            .expect("the plan");
    let plan = osfact::install::read(&made).expect("read");
    assert!(plan.install.connection.is_none());
    assert_eq!(plan.install.step.len(), 1);
    assert_eq!(plan.install.way_back.wait, 240);
    assert_eq!(
        plan.install.way_back.via,
        osfact::install::Transport::Fastboot
    );
}

#[test]
fn a_root_declared_without_a_connection_is_refused_naming_both_tables() {
    let declaration = fs::read_to_string(device()).expect("fixture");
    let declaration = without_table(&declaration, "[install.connection]");
    let said =
        osfact::pack::plan_text(&declaration, &files_named(&"0".repeat(64), &"1".repeat(64)))
            .expect_err("refused");
    assert!(
        said.contains("[install.root]") && said.contains("[install.connection]"),
        "{said}"
    );
}

#[test]
fn the_binds_a_declaration_makes_reach_the_enter_step_in_order() {
    let declaration = fs::read_to_string(device()).expect("fixture");
    let check = "check = \"test -e /mm/sbin/init\"";
    assert!(
        declaration.contains(check),
        "the fixture's [install.root] moved"
    );
    let declaration = declaration.replace(
        check,
        &format!("{check}\nbinds = [\"proc\", \"sys\", \"dev\"]"),
    );
    let made =
        osfact::pack::plan_text(&declaration, &files_named(&"0".repeat(64), &"1".repeat(64)))
            .expect("the plan");
    let plan = osfact::install::read(&made).expect("read");
    let enter = plan.install.step.last().expect("a step");
    assert_eq!(
        enter.binds,
        [
            osfact::install::Bind::Proc,
            osfact::install::Bind::Sys,
            osfact::install::Bind::Dev
        ]
    );
}
