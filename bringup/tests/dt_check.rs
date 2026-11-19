// SPDX-License-Identifier: MIT OR Apache-2.0
//! lpoco-bringup-dt-check: `dt-check binding|dtbs` runs the kernel's devicetree checks in a container with
//! nothing Python on the host (docs research upstream-forks/2026-11-19/dt-checks-in-a-container.md). The
//! docker program is a stand-in (tests/fixtures/dt-check/fake-docker.sh) that records its arguments, so the
//! tests count what the verb asks docker for, and the negative test counts that a schema error ends it non-zero
//! and is named.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const DIGEST: &str =
    "alpine@sha256:020dfcbaaf4cc1078bf2d9c7ba31a8466e334061dcd2f248001d68f79e52c000";

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "lpoco-bringup-dtcheck-{}-{name}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn fake() -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/dt-check/fake-docker.sh")
        .to_str()
        .unwrap()
        .to_string()
}

fn run(args: &[&str], log: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lpoco-bringup"))
        .arg("dt-check")
        .args(args)
        .env("LPOCO_DOCKER", fake())
        .env("FAKE_DOCKER_LOG", log)
        .output()
        .unwrap()
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

#[test]
fn binding_runs_the_image_by_digest_with_the_source_read_only_and_output_in_scratch() {
    let (src, scratch) = (tmp("src1"), tmp("out1"));
    let log = tmp("log1").join("args");
    let o = run(
        &[
            "binding",
            src.to_str().unwrap(),
            "--image",
            DIGEST,
            "--out",
            scratch.to_str().unwrap(),
            "--schema-files",
            "watchdog/qcom,pm8916-wdt.yaml",
        ],
        &log,
    );
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    let args = fs::read_to_string(&log).unwrap();
    let args: Vec<&str> = args.lines().collect();
    assert!(args.contains(&DIGEST), "{args:?}");
    assert!(
        args.contains(&format!("{}:/src:ro", src.display()).as_str()),
        "{args:?}"
    );
    assert!(
        args.contains(&format!("{}:/out", scratch.display()).as_str()),
        "{args:?}"
    );
    for want in [
        "dt_binding_check",
        "DT_SCHEMA_FILES=watchdog/qcom,pm8916-wdt.yaml",
        "O=/out",
    ] {
        assert!(args.contains(&want), "{want} in {args:?}");
    }
}

#[test]
fn dtbs_runs_check_dtbs_on_the_named_dtbs() {
    let (src, scratch) = (tmp("src2"), tmp("out2"));
    let log = tmp("log2").join("args");
    let o = run(
        &[
            "dtbs",
            src.to_str().unwrap(),
            "--image",
            DIGEST,
            "--out",
            scratch.to_str().unwrap(),
            "--dtbs",
            "qcom/msm8998-oneplus-cheeseburger.dtb,qcom/sm7125-xiaomi-joyeuse.dtb",
        ],
        &log,
    );
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    let args = fs::read_to_string(&log).unwrap();
    let args: Vec<&str> = args.lines().collect();
    for want in [
        "CHECK_DTBS=y",
        "qcom/msm8998-oneplus-cheeseburger.dtb",
        "qcom/sm7125-xiaomi-joyeuse.dtb",
    ] {
        assert!(args.contains(&want), "{want} in {args:?}");
    }
}

#[test]
fn an_image_named_by_a_tag_is_refused_before_docker_runs() {
    let (src, scratch) = (tmp("src3"), tmp("out3"));
    let log = tmp("log3").join("args");
    let o = run(
        &[
            "binding",
            src.to_str().unwrap(),
            "--image",
            "alpine:edge",
            "--out",
            scratch.to_str().unwrap(),
        ],
        &log,
    );
    assert_ne!(o.status.code(), Some(0));
    assert!(text(&o).contains("digest"), "{}", text(&o));
    assert!(!log.exists(), "docker must not run for a tag");
}

#[test]
fn a_checker_failure_passes_its_lines_through_and_ends_1() {
    let (src, scratch) = (tmp("src4"), tmp("out4"));
    fs::write(src.join("BOGUS"), "").unwrap();
    let log = tmp("log4").join("args");
    let o = run(
        &[
            "dtbs",
            src.to_str().unwrap(),
            "--image",
            DIGEST,
            "--out",
            scratch.to_str().unwrap(),
            "--dtbs",
            "qcom/x.dtb",
        ],
        &log,
    );
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    assert!(text(&o).contains("bogus-prop"), "{}", text(&o));
}

fn refused(extra: &[&str], tag: &str) -> String {
    let (src, scratch) = (tmp(&format!("s-{tag}")), tmp(&format!("o-{tag}")));
    let log = tmp(&format!("l-{tag}")).join("args");
    let mut a = vec![
        "dtbs",
        src.to_str().unwrap(),
        "--image",
        DIGEST,
        "--out",
        scratch.to_str().unwrap(),
    ];
    a.extend(extra);
    let o = run(&a, &log);
    assert_eq!(o.status.code(), Some(2), "{}", text(&o));
    assert!(!log.exists(), "docker must not run");
    text(&o)
}

#[test]
fn make_options_and_empty_lists_are_refused_before_docker_runs() {
    assert!(refused(&["--dtbs", "O=/src"], "a").contains("plain make target"));
    assert!(refused(&["--dtbs", "-C/"], "b").contains("plain make target"));
    assert!(refused(&["--dtbs", ","], "c").contains("plain make target"));
    assert!(
        refused(&["--dtbs", "x.dtb", "--schema-files", "y.yaml"], "d")
            .contains("--schema-files goes with")
    );
    assert!(refused(&["--dtbs", "x.dtb", "--arch", "ARCH=x"], "e").contains("plain make target"));
}

#[test]
fn docker_ending_before_a_verdict_is_an_error_not_a_failed_check() {
    let (src, scratch) = (tmp("src6"), tmp("out6"));
    let log = tmp("log6").join("args");
    let o = Command::new(env!("CARGO_BIN_EXE_lpoco-bringup"))
        .args([
            "dt-check",
            "binding",
            src.to_str().unwrap(),
            "--image",
            DIGEST,
            "--out",
            scratch.to_str().unwrap(),
        ])
        .env("LPOCO_DOCKER", fake())
        .env("FAKE_DOCKER_LOG", &log)
        .env("FAKE_DOCKER_EXIT", "125")
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2), "{}", text(&o));
    assert!(!text(&o).contains("checker"), "{}", text(&o));
}

fn dtbs_run(src: &Path, scratch: &Path, log: &Path) -> Output {
    run(
        &[
            "dtbs",
            src.to_str().unwrap(),
            "--image",
            DIGEST,
            "--out",
            scratch.to_str().unwrap(),
            "--dtbs",
            "qcom/x.dtb",
        ],
        log,
    )
}

#[test]
fn a_schema_violation_printed_while_make_exits_0_is_a_failed_check_with_its_lines_named() {
    let (src, scratch) = (tmp("src7"), tmp("out7"));
    fs::write(src.join("WARN"), "").unwrap();
    let log = tmp("log7").join("args");
    let o = dtbs_run(&src, &scratch, &log);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    let t = text(&o);
    assert!(t.contains("Unevaluated properties are not allowed"), "{t}");
    assert!(!t.contains("dt-check: clean"), "{t}");
    assert!(t.contains("1 violation"), "{t}");
}

#[test]
fn a_node_that_is_enabled_is_checked_and_one_that_is_disabled_is_not_a_violation() {
    let (src, scratch) = (tmp("src8"), tmp("out8"));
    fs::write(src.join("pm.dts"), "pmic@0 { status = \"okay\"; };\n").unwrap();
    let o = dtbs_run(&src, &scratch, &tmp("log8").join("args"));
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    assert!(text(&o).contains("pm8998.dtb"), "{}", text(&o));
    fs::write(src.join("pm.dts"), "pmic@0 { status = \"disabled\"; };\n").unwrap();
    let o = dtbs_run(&src, &scratch, &tmp("log8b").join("args"));
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
}

#[test]
fn dtbs_on_a_fresh_out_runs_defconfig_first_and_on_a_configured_one_does_not() {
    let (src, scratch) = (tmp("src9"), tmp("out9"));
    let log = tmp("log9").join("args");
    assert_eq!(dtbs_run(&src, &scratch, &log).status.code(), Some(0));
    let args = fs::read_to_string(&log).unwrap();
    let args: Vec<&str> = args.lines().collect();
    let at = |w: &str| args.iter().position(|a| *a == w);
    let (d, t) = (at("defconfig"), at("qcom/x.dtb"));
    assert!(d.is_some() && d < t, "defconfig before the dtb in {args:?}");
    fs::write(scratch.join(".config"), "").unwrap();
    assert_eq!(dtbs_run(&src, &scratch, &log).status.code(), Some(0));
    let args = fs::read_to_string(&log).unwrap();
    assert!(!args.lines().any(|a| a == "defconfig"), "{args}");
}

#[test]
fn a_make_failure_without_a_violation_is_a_setup_error_never_a_failed_check() {
    let (src, scratch) = (tmp("src10"), tmp("out10"));
    fs::write(src.join("SETUP"), "").unwrap();
    let o = dtbs_run(&src, &scratch, &tmp("log10").join("args"));
    assert_eq!(o.status.code(), Some(2), "{}", text(&o));
    assert!(!text(&o).contains("checker failed"), "{}", text(&o));
    assert!(text(&o).contains("not a check failure"), "{}", text(&o));
}

#[test]
fn the_image_carries_its_tools_and_nothing_is_installed_at_run_time() {
    let (src, scratch) = (tmp("src11"), tmp("out11"));
    let log = tmp("log11").join("args");
    assert_eq!(dtbs_run(&src, &scratch, &log).status.code(), Some(0));
    let args = fs::read_to_string(&log).unwrap();
    assert!(!args.contains("apk add"), "{args}");
    let df = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("dt-check/Dockerfile"))
        .unwrap();
    for pkg in [
        "py3-dt-schema",
        "dtc",
        "make",
        "gcc",
        "flex",
        "bison",
        "yamllint",
    ] {
        assert!(df.contains(pkg), "{pkg} in the Dockerfile");
    }
    assert!(df.contains("@sha256:"), "the base is pinned by digest");
}

#[test]
fn a_dtb_already_built_in_out_is_removed_so_its_check_runs_again() {
    let (src, scratch) = (tmp("src12"), tmp("out12"));
    let d = scratch.join("arch/arm64/boot/dts/qcom");
    fs::create_dir_all(&d).unwrap();
    fs::write(d.join("x.dtb"), "old").unwrap();
    fs::write(d.join("keep.dtb"), "other").unwrap();
    let o = dtbs_run(&src, &scratch, &tmp("log12").join("args"));
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    assert!(!d.join("x.dtb").exists());
    assert!(d.join("keep.dtb").exists());
}

#[test]
fn a_yamllint_line_is_a_violation() {
    let l = "Documentation/devicetree/bindings/x.yaml:12:3: [error] syntax error";
    assert_eq!(lpoco_bringup::dt_check::violations(l), vec![l]);
}
