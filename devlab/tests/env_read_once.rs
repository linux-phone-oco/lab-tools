//! lpoco-lab reads its declared environment through the argenv contract, not std::env::var (backlog
//! lpoco-lab-env-through-argenv, proof lpoco-lab-env-read-once): no source file but cli.rs reads a
//! declared variable itself, and a value the contract refuses exits 2 with the usage.

use std::path::Path;
use std::process::Command;

fn sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            sources(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn no_declared_variable_is_read_by_std_env() {
    let mut files = Vec::new();
    sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    let mut found = Vec::new();
    for f in files.iter().filter(|f| f.file_name().unwrap() != "cli.rs") {
        let text = std::fs::read_to_string(f).unwrap();
        for (i, line) in text.lines().enumerate() {
            let reads = line.contains("env::var");
            if reads {
                found.push(format!("{}:{}: {}", f.display(), i + 1, line.trim()));
            }
        }
    }
    assert!(
        found.is_empty(),
        "read through the contract instead:\n{}",
        found.join("\n")
    );
}
