//! The repository's mark and the health service's description say lpoco, not the old names
//! (proof: lpoco-names-sweep, marks slice). Files that name the old programs on purpose
//! (aliases, the packaging `replaces`) are not read here.

fn read(rel: &str) -> String {
    let path = format!("{}/../{}", env!("CARGO_MANIFEST_DIR"), rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn value(file: &str, key: &str) -> String {
    read(file)
        .lines()
        .find_map(|l| {
            l.strip_prefix(key)
                .map(|v| v.trim_start_matches([' ', '=']).to_string())
        })
        .unwrap_or_else(|| panic!("{key} not in {file}"))
}

fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '-')
        .map(str::to_lowercase)
        .collect()
}

fn old_name_in(text: &str) -> Option<String> {
    words(text)
        .into_iter()
        .find(|w| ["devlab", "tone", "devlab-health"].contains(&w.as_str()))
}

#[test]
fn the_mark_names_lpoco() {
    for key in ["name", "title"] {
        let v = value("oco.toml", &format!("{key} "));
        assert_eq!(old_name_in(&v), None, "oco.toml {key}: {v}");
    }
}

#[test]
fn the_health_service_describes_itself_with_lpoco() {
    let v = value("health/suite/lpoco-health.service", "Description");
    assert_eq!(old_name_in(&v), None, "Description: {v}");
}
