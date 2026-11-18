//! The check soc-sleeps reads this boot's controlled cycle (lpoco-health suspend cycle), and no other boot's.

use std::fs;
use std::process::Command;

const CHECK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/suite/checks/base/90-sleep.sh");
const LIB: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/suite/lib.sh");

fn run(name: &str, cycle: Option<&str>) -> String {
    let d = std::env::temp_dir().join(format!("soc-sleeps-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    fs::write(d.join("boot_id"), "boot-aaa\n").unwrap();
    if let Some(c) = cycle {
        fs::write(d.join("cycle.json"), c).unwrap();
    }
    let o = Command::new("sh")
        .arg(CHECK)
        .env("LPOCO_HEALTH_LIB", LIB)
        .env("LPOCO_HEALTH_CYCLE", d.join("cycle.json"))
        .env("BOOT_ID_FILE", d.join("boot_id"))
        .output()
        .unwrap();
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn a_cycle_where_the_soc_slept_passes() {
    let o = run(
        "pass",
        Some(
            r#"{"asleep_s":15,"boot":"boot-aaa","seconds":15,"soc_slept":true,"suspended":true,"verb":"cycle"}"#,
        ),
    );
    assert!(o.starts_with("pass soc-sleeps deep-sleep "), "{o}");
    assert!(o.contains("one controlled cycle of 15 s"), "{o}");
}

#[test]
fn a_cycle_that_suspended_without_the_soc_sleeping_fails() {
    let o = run(
        "fail",
        Some(
            r#"{"asleep_s":15,"boot":"boot-aaa","seconds":15,"soc_slept":false,"suspended":true,"verb":"cycle"}"#,
        ),
    );
    assert!(o.starts_with("fail soc-sleeps deep-sleep "), "{o}");
}

#[test]
fn a_cycle_of_another_boot_or_none_is_not_read() {
    for c in [
        None,
        Some(r#"{"boot":"boot-zzz","soc_slept":true,"suspended":true}"#),
    ] {
        let o = run("other", c);
        assert!(!o.contains("controlled cycle"), "{o}");
        assert!(o.starts_with("skip soc-sleeps deep-sleep "), "{o}");
    }
}
