//! reference-capture-diff: `lpoco-bringup reference capture` saves a normalised snapshot of what a stock OS
//! does (device tree, debugfs, interrupts, drivers) taken over a transport (adb, ssh); `reference diff PART`
//! names every difference between two snapshots for one part. Walleye's UFS case (research
//! walleye-first-fastboot-boot/2026-11-18-ufs-android-way, live-ufs-props against ours-ufs-props) is the
//! fixture, and its known differences are named.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lpoco-ref-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn fixture(side: &str) -> String {
    format!(
        "{}/tests/fixtures/reference/{side}",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lpoco-bringup"))
        .arg("reference")
        .args(args)
        .output()
        .unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn diff_ufs() -> (i32, String) {
    let o = run(&[
        "diff",
        "ufs",
        "--live",
        &fixture("live"),
        "--ours",
        &fixture("ours"),
    ]);
    (o.status.code().unwrap_or(-1), text(&o.stdout))
}

#[test]
fn the_ufs_difference_walleye_found_by_hand_is_named() {
    let (code, out) = diff_ufs();
    assert_eq!(code, 1, "differences are exit 1: {out}");
    // a clock only the vendor driver has
    assert!(
        out.contains("differs: devicetree ufshc clock-names"),
        "{out}"
    );
    assert!(out.contains("core_clk_ice"), "{out}");
    // a property only Android sets, and one only ours sets
    assert!(
        out.contains("only live: devicetree ufshc ufs-qcom-crypto"),
        "{out}"
    );
    assert!(
        out.contains("only ours: devicetree ufshc power-domains"),
        "{out}"
    );
    // the interrupt trigger type, both values said
    let irq = out
        .lines()
        .find(|l| l.contains("ufshc interrupts"))
        .expect(&out);
    assert!(
        irq.contains("<0x0 0x109 0x0>") && irq.contains("<0x0 0x109 0x4>"),
        "{irq}"
    );
    // the phy node too
    assert!(
        out.contains("differs: devicetree ufsphy compatible"),
        "{out}"
    );
}

#[test]
fn a_property_that_matches_is_not_named() {
    let (_, out) = diff_ufs();
    assert!(!out.contains("ufshc lanes-per-direction"), "{out}");
    assert!(!out.contains("ufshc reg "), "{out}");
}

#[test]
fn text_snapshots_are_filtered_to_the_part_and_compared_by_line() {
    let (_, out) = diff_ufs();
    // the supply of the part that differs in voltage, the others (vreg_s3a) left out
    assert!(out.contains("vreg_l26a_ufs_vccq"), "{out}");
    assert!(out.contains("1200mV") && out.contains("1000mV"), "{out}");
    assert!(!out.contains("vreg_s3a"), "{out}");
    assert!(
        !out.contains("vreg_l20a_ufs_vcc "),
        "a matching line is not named: {out}"
    );
}

#[test]
fn a_snapshot_one_side_lacks_is_not_known_yet_not_missing() {
    let (_, out) = diff_ufs();
    assert!(
        out.contains("gpio: not measured yet on the ours side"),
        "{out}"
    );
}

#[test]
fn two_equal_snapshots_pass() {
    let live = fixture("live");
    let o = run(&["diff", "ufs", "--live", &live, "--ours", &live]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stdout));
    assert!(
        text(&o.stdout).contains("no difference"),
        "{}",
        text(&o.stdout)
    );
}

#[test]
fn a_part_no_line_names_is_not_known_yet_not_equal() {
    let o = run(&[
        "diff",
        "touch",
        "--live",
        &fixture("live"),
        "--ours",
        &fixture("ours"),
    ]);
    assert_eq!(o.status.code(), Some(2));
    let s = text(&o.stdout) + &text(&o.stderr);
    assert!(s.contains("touch"), "{s}");
    assert!(s.contains("not known yet"), "{s}");
}

#[test]
fn match_words_widen_the_part() {
    let o = run(&[
        "diff",
        "storage",
        "--match",
        "ufs,vreg_l20a",
        "--live",
        &fixture("live"),
        "--ours",
        &fixture("ours"),
    ]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        text(&o.stdout).contains("ufshc interrupts"),
        "{}",
        text(&o.stdout)
    );
}

/// A fake transport: a script that reads the capture script on its standard input and answers as a phone
/// would, so capture is proven without one.
#[test]
fn capture_over_a_transport_saves_the_normalised_snapshot() {
    let d = tmp("capture");
    let fake = d.join("fakedev");
    fs::write(
        &fake,
        "#!/bin/sh\ncat >/dev/null\n\
printf '@@dt ./soc/ufshc@1da4000/compatible\\n 71 63 6f 6d 2c 75 66 73 68 63 00\\n'\n\
printf '@@dt ./soc/ufshc@1da4000/interrupts\\n 00 00 00 00 00 00 01 09 00 00 00 04\\n'\n\
printf '@@dt ./soc/ufshc@1da4000/clocks\\n 00 00 00 12 00 00 00 03\\n'\n\
printf '@@dt ./soc/ufshc@1da4000/vcc-supply\\n 00 00 00 21\\n'\n\
printf '@@dt ./soc/ufshc@1da4000/ufs-qcom-crypto\\n'\n\
printf '@@dt ./soc/ufshc@1da4000/name\\n 75 66 73 68 63 00\\n'\n\
printf '@@dt ./soc/ufshc@1da4000/clock-names\\n 63 6f 72 65 5f 63 6c 6b 00 72 65 66 00\\n'\n\
printf '@@file gpio\\ngpiochip0: GPIOs 0-149\\n gpio-150 (ufs_reset) out hi\\n'\n",
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
    let out = d.join("snap");
    let o = run(&[
        "capture",
        "--via",
        fake.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}{}",
        text(&o.stdout),
        text(&o.stderr)
    );
    let dt = fs::read_to_string(out.join("devicetree.txt")).unwrap();
    assert_eq!(
        dt,
        "ufshc clock-names = \"core_clk\",\"ref\"\n\
ufshc clocks = (phandle or reference)\n\
ufshc compatible = \"qcom,ufshc\"\n\
ufshc interrupts = <0x0 0x109 0x4>\n\
ufshc ufs-qcom-crypto = (empty)\n\
ufshc vcc-supply = (phandle or reference)\n"
    );
    assert!(fs::read_to_string(out.join("gpio"))
        .unwrap()
        .contains("ufs_reset"));
    // what the phone did not give is said as not measured yet
    assert!(
        text(&o.stdout).contains("not measured yet: regulator_summary"),
        "{}",
        text(&o.stdout)
    );
}

#[test]
fn a_transport_that_fails_is_said_and_writes_nothing() {
    let d = tmp("fail");
    let out = d.join("snap");
    let o = run(&["capture", "--via", "false", "--out", out.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(2));
    assert!(!out.join("devicetree.txt").exists());
}

#[test]
fn a_missing_snapshot_folder_fails_in_words() {
    let o = run(&[
        "diff",
        "ufs",
        "--live",
        "/nonexistent-snap",
        "--ours",
        &fixture("ours"),
    ]);
    assert_eq!(o.status.code(), Some(2));
    assert!(
        text(&o.stderr).contains("no such snapshot folder"),
        "{}",
        text(&o.stderr)
    );
}

#[test]
fn a_side_without_a_device_tree_is_never_a_pass() {
    let d = tmp("nodt");
    fs::copy(
        format!("{}/regulator_summary", fixture("live")),
        d.join("regulator_summary"),
    )
    .unwrap();
    let o = run(&[
        "diff",
        "vcc",
        "--live",
        d.to_str().unwrap(),
        "--ours",
        d.to_str().unwrap(),
    ]);
    assert_eq!(o.status.code(), Some(2), "{}", text(&o.stdout));
    assert!(
        text(&o.stdout).contains("devicetree: not measured yet"),
        "{}",
        text(&o.stdout)
    );
}

#[test]
fn repeated_node_names_are_each_compared() {
    let d = tmp("dup");
    for (side, v) in [("a", "0x1"), ("b", "0x2")] {
        fs::create_dir_all(d.join(side)).unwrap();
        fs::write(
            d.join(side).join("devicetree.txt"),
            format!("regulator voltage = <0x5>\nregulator voltage = <{v}>\n"),
        )
        .unwrap();
    }
    let o = run(&[
        "diff",
        "regulator",
        "--live",
        d.join("a").to_str().unwrap(),
        "--ours",
        d.join("b").to_str().unwrap(),
    ]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o.stdout));
    assert!(
        text(&o.stdout).contains("<0x1>") && text(&o.stdout).contains("<0x2>"),
        "{}",
        text(&o.stdout)
    );
}

/// The real capture script, run by `sh` against a folder standing for /proc/device-tree: it must exit 0
/// (its last loop ends on a file that is not a link) and give sections the parser reads.
#[test]
fn the_real_capture_script_runs_to_a_zero_exit_and_is_read_back() {
    let d = tmp("script");
    let dt = d.join("dt/soc/ufshc@1da4000");
    fs::create_dir_all(&dt).unwrap();
    fs::write(dt.join("compatible"), b"qcom,ufshc\0").unwrap();
    let out = d.join("snap");
    let via = format!("env LPOCO_DT={} sh", d.join("dt").display());
    let o = run(&["capture", "--via", &via, "--out", out.to_str().unwrap()]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}{}",
        text(&o.stdout),
        text(&o.stderr)
    );
    assert_eq!(
        fs::read_to_string(out.join("devicetree.txt")).unwrap(),
        "ufshc compatible = \"qcom,ufshc\"\n"
    );
}

#[test]
fn a_recapture_does_not_leave_a_file_the_phone_did_not_give() {
    let d = tmp("stale");
    let out = d.join("snap");
    fs::create_dir_all(&out).unwrap();
    fs::write(out.join("clk_summary"), "old").unwrap();
    let fake = d.join("fakedev");
    fs::write(
        &fake,
        "#!/bin/sh\ncat >/dev/null\nprintf '@@dt ./a/b\\n 00 00 00 01\\n'\n",
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
    let o = run(&[
        "capture",
        "--via",
        fake.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(o.status.code(), Some(0));
    assert!(!out.join("clk_summary").exists());
}
