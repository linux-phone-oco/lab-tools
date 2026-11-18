//! device.toml declared once: a misspelling is refused, the committed schema is the projection, the lab's
//! manifests validate.

use lpoco_manifest::{schema, Manifest};

const HEAD: &str = "[device]\ncodename = \"p\"\n[connection]\nuser = \"u\"\naddresses = [\"a\"]\n";

#[test]
fn a_misspelled_table_is_refused() {
    let e = Manifest::parse(&format!(
        "{HEAD}[[flash.overlays]]\nfrom = \"a\"\nto = \"root:/b\"\n"
    ))
    .unwrap_err();
    assert!(e.contains("overlays"), "the refusal names the word: {e}");
}

#[test]
fn a_misspelled_field_and_a_misspelled_top_table_are_refused() {
    assert!(Manifest::parse(&format!("{HEAD}[flash]\nsector_sise = 512\n")).is_err());
    assert!(Manifest::parse(&format!("{HEAD}[hosts]\nwatch = true\n")).is_err());
    assert!(Manifest::parse(&format!("{HEAD}[tools]\nlpoko = \"x\"\n")).is_err());
}

#[test]
fn schema_is_current() {
    let committed =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/device.schema.json"))
            .expect("device.schema.json is committed beside the crate");
    assert_eq!(
        committed,
        schema(),
        "the committed schema is not the projection: regenerate with `cargo run -p lpoco-manifest -- schema > manifest/device.schema.json`"
    );
}

/// The device manifests of the lab, named by LPOCO_LAB_MANIFESTS (paths, space separated); the fixtures
/// beside the crate always run.
#[test]
fn every_lab_manifest_validates() {
    let fixtures = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(fixtures)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    if let Ok(more) = std::env::var("LPOCO_LAB_MANIFESTS") {
        files.extend(more.split_whitespace().map(Into::into));
    }
    assert!(files.len() >= 3, "surya, walleye and template fixtures");
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        if let Err(e) = Manifest::parse(&text) {
            panic!("{}: {e}", f.display());
        }
    }
}
