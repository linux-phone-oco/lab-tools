//! The family's pin: a device manifest says which lpoco family version it follows, `[tools] lpoco = "X.Y"`
//! (decision 2026-11-17-bringup-in-lab-tools). A program of another version stops with one line.

use std::path::Path;

/// Does `version` ("0.1.7") satisfy `pin` ("0", "0.1" or "0.1.7")? A pin names a prefix of the version.
pub fn matches(pin: &str, version: &str) -> bool {
    let (p, v): (Vec<&str>, Vec<&str>) = (
        pin.trim().split('.').collect(),
        version.split('.').collect(),
    );
    p.len() <= 3 && p.len() <= v.len() && p.iter().zip(&v).all(|(a, b)| !a.is_empty() && a == b)
}

/// The pin a manifest's text asks for (none when it has no `[tools] lpoco`).
pub fn of_text(text: &str) -> Result<Option<String>, String> {
    let v: toml::Value = toml::from_str(text).map_err(|e| format!("manifest: {e}"))?;
    Ok(v.get("tools")
        .and_then(|t| t.get("lpoco"))
        .and_then(|p| p.as_str())
        .map(str::to_string))
}

/// Stop, in one line naming both versions, when the manifest at `path` pins another version than `version`.
/// A manifest that is not there asks nothing.
pub fn check(path: &Path, version: &str) -> Result<(), String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(());
    };
    let pin = of_text(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    match pin {
        Some(p) if !matches(&p, version) => Err(format!(
            "{} pins lpoco {p}, this is lpoco-bringup {version}",
            path.display()
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pin_is_a_prefix() {
        assert!(matches("0.1", "0.1.7"));
        assert!(matches("0", "0.1.7"));
        assert!(matches("0.1.7", "0.1.7"));
        assert!(!matches("0.2", "0.1.7"));
        assert!(!matches("0.1.8", "0.1.7"));
        assert!(!matches("", "0.1.7"));
    }
}
