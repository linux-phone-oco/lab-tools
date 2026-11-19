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
