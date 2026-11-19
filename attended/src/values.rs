// SPDX-License-Identifier: MIT OR Apache-2.0
//! The values a procedure reads from its device's `device.toml` (decision 2026-11-19-procedure-values-from-the-named-device,
//! 2026-11-19-attended-keys-in-the-manifest): `dev("health.battery_floor_mv")` is a key of the table that owns the
//! fact, `proc("window")` a key of this procedure's own `[procedures.NAME]` (NAME is the procedure's name in lowercase
//! words joined by `-`). A key that is missing, or of another kind than asked, ends the run with exit 2 naming it.

use std::fmt;

/// One value read from `device.toml`, held with the key it came from so that a wrong use names it.
#[derive(Debug, Clone)]
pub struct Val {
    key: String,
    value: toml::Value,
}

impl Val {
    /// A whole number.
    pub fn int(&self) -> i64 {
        self.value
            .as_integer()
            .unwrap_or_else(|| self.wrong("a whole number"))
    }

    /// A number, whole or not.
    pub fn float(&self) -> f64 {
        self.value
            .as_float()
            .or_else(|| self.value.as_integer().map(|i| i as f64))
            .unwrap_or_else(|| self.wrong("a number"))
    }

    /// Words.
    pub fn text(&self) -> String {
        self.value
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| self.wrong("text"))
    }

    fn wrong(&self, wanted: &str) -> ! {
        crate::stop(&format!("{} in device.toml is not {wanted}", self.key))
    }
}

impl fmt::Display for Val {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.value {
            toml::Value::String(s) => write!(f, "{s}"),
            v => write!(f, "{v}"),
        }
    }
}

/// The value at a dotted path (`health.battery_floor_mv`) of the device's manifest, read as the file says it.
pub fn find(raw: &toml::Value, path: &str) -> Option<Val> {
    let mut at = raw;
    for part in path.split('.') {
        at = at.get(part)?;
    }
    Some(Val {
        key: path.to_string(),
        value: at.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dotted_path_finds_its_value_and_a_wrong_kind_is_not_taken() {
        let raw: toml::Value = toml::from_str("[health]\nfloor = 3700\nname = \"x\"\n").unwrap();
        let v = find(&raw, "health.floor").unwrap();
        assert_eq!((v.int(), v.float()), (3700, 3700.0));
        assert_eq!(find(&raw, "health.name").unwrap().text(), "x");
        assert!(find(&raw, "health.none").is_none());
        assert!(find(&raw, "health.floor.deeper").is_none());
    }
}
