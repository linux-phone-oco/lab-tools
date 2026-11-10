//! A device's manifest, `device.toml`, kept in its lab repository: what the tools need to know about
//! the phone. Secrets and what is learned at run time stay outside the repository.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const MANIFEST: &str = "device.toml";

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Device {
    pub device: Identity,
    pub connection: Connection,
    #[serde(default)]
    pub tests: Tests,
    #[serde(default)]
    pub provision: Provision,
}

/// What a freshly flashed phone gets.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Provision {
    /// The package list, relative to the manifest
    #[serde(default = "default_packages")]
    pub packages: String,
    /// Files to install - `{compatible}` is the phone's device-tree name
    #[serde(default)]
    pub install: Vec<Install>,
}

fn default_packages() -> String {
    "provision/packages.txt".into()
}

impl Default for Provision {
    fn default() -> Self {
        Provision {
            packages: default_packages(),
            install: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Install {
    pub from: String,
    pub to: String,
    #[serde(default = "default_mode")]
    pub mode: String,
    /// A command on the phone that must succeed for the file to go on (empty: always)
    #[serde(default)]
    pub when: String,
}

fn default_mode() -> String {
    "644".into()
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Tests {
    /// The checks script, relative to the manifest; run on the phone as root, one JSON line per check
    pub checks: String,
}

impl Default for Tests {
    fn default() -> Self {
        Tests {
            checks: "tests/automatic/checks.sh".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Identity {
    /// The postmarketOS-style codename, e.g. "xiaomi-surya"
    pub codename: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Connection {
    pub user: String,
    /// Addresses to try, in order (USB networking first, as a rule)
    pub addresses: Vec<String>,
}

impl Device {
    pub fn parse(text: &str) -> Result<Device> {
        let d: Device = toml::from_str(text).context("reading device.toml")?;
        if d.connection.addresses.is_empty() {
            bail!("device.toml: connection.addresses is empty");
        }
        Ok(d)
    }

    /// The manifest at `path`, or found by walking up from `start` as cargo finds Cargo.toml.
    pub fn load(path: Option<&Path>, start: &Path) -> Result<(Device, PathBuf)> {
        let file = match path {
            Some(p) => p.to_path_buf(),
            None => start
                .ancestors()
                .map(|d| d.join(MANIFEST))
                .find(|f| f.is_file())
                .with_context(|| {
                    format!(
                        "no {MANIFEST} here or above {} (or pass --device)",
                        start.display()
                    )
                })?,
        };
        let text = std::fs::read_to_string(&file)
            .with_context(|| format!("reading {}", file.display()))?;
        Ok((Device::parse(&text)?, file))
    }

    /// Private per-device configuration (the password): ~/.config/devlab/CODENAME/
    pub fn config_dir(&self) -> PathBuf {
        home().join(".config/devlab").join(&self.device.codename)
    }

    /// What is learned at run time (the phone's Wi-Fi address): ~/.local/state/devlab/CODENAME/
    pub fn state_dir(&self) -> PathBuf {
        home()
            .join(".local/state/devlab")
            .join(&self.device.codename)
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// The addresses to try: the manifest's, then a remembered one not among them.
pub fn candidates(manifest: &[String], remembered: Option<&str>) -> Vec<String> {
    let mut all = manifest.to_vec();
    if let Some(r) = remembered.map(str::trim).filter(|r| !r.is_empty()) {
        if !all.iter().any(|a| a == r) {
            all.push(r.to_string());
        }
    }
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    const SURYA: &str = "[device]\ncodename = \"xiaomi-surya\"\n[connection]\nuser = \"user\"\naddresses = [\"172.16.42.1\"]\n";

    #[test]
    fn a_manifest_reads() {
        let d = Device::parse(SURYA).unwrap();
        assert_eq!(d.device.codename, "xiaomi-surya");
        assert_eq!(d.connection.addresses, ["172.16.42.1"]);
    }

    #[test]
    fn a_manifest_without_addresses_is_refused() {
        assert!(Device::parse(
            "[device]\ncodename = \"x\"\n[connection]\nuser = \"u\"\naddresses = []\n"
        )
        .is_err());
    }

    #[test]
    fn the_remembered_address_comes_last_and_once() {
        let m = vec!["172.16.42.1".to_string()];
        assert_eq!(
            candidates(&m, Some("192.168.1.5\n")),
            ["172.16.42.1", "192.168.1.5"]
        );
        assert_eq!(
            candidates(&m, Some("172.16.42.1")),
            ["172.16.42.1"],
            "not twice"
        );
        assert_eq!(candidates(&m, None), ["172.16.42.1"]);
    }

    #[test]
    fn the_manifest_is_found_by_walking_up() {
        let root = std::env::temp_dir().join(format!("devlab-test-{}", std::process::id()));
        let deep = root.join("a/b");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(root.join(MANIFEST), SURYA).unwrap();
        let (d, f) = Device::load(None, &deep).unwrap();
        assert_eq!(d.device.codename, "xiaomi-surya");
        assert_eq!(f, root.join(MANIFEST));
        std::fs::remove_dir_all(root).unwrap();
    }
}
