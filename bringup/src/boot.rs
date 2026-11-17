//! The `[boot]` table of a device's manifest (device.toml): the header facts of the phone's boot image,
//! in postmarketOS deviceinfo's words. Every other table of the manifest is devlab's and is not read here.

use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Boot {
    /// 0 is the only header version this tool packs and checks so far
    #[serde(default)]
    pub header_version: u32,
    #[serde(default)]
    pub base: u32,
    pub kernel_offset: u32,
    pub ramdisk_offset: u32,
    #[serde(default)]
    pub second_offset: u32,
    pub tags_offset: u32,
    pub pagesize: u32,
    /// "A.B.C"; none says the header holds no os words
    pub os_version: Option<String>,
    /// "YYYY-MM"
    pub os_patch_level: Option<String>,
    pub cmdline: String,
    /// the boot partition's size in bytes: an image must fit it
    pub partition_size: Option<u64>,
    /// the qcom,msm-id words the appended device tree must carry
    pub msm_id: Option<Vec<u32>>,
    // read by the later items (swap, initramfs, avb), declared now so a manifest says them once
    pub dtb: Option<String>,
    pub kernel_compression: Option<String>,
    pub ramdisk_compression: Option<String>,
    pub avb_footer: Option<bool>,
}

#[derive(Deserialize)]
struct Manifest {
    boot: Option<Boot>,
}

impl Boot {
    pub fn parse(text: &str) -> Result<Boot, String> {
        let m: Manifest = toml::from_str(text).map_err(|e| format!("manifest: {e}"))?;
        m.boot
            .ok_or_else(|| "manifest has no [boot] table".to_string())
    }

    pub fn read(path: &Path) -> Result<Boot, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        Boot::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The os words as the header holds them: version << 11 | patch level (0 when none is asked).
    pub fn os_word(&self) -> Result<u32, String> {
        let v = match &self.os_version {
            Some(s) => parse_os_version(s)?,
            None => 0,
        };
        let p = match &self.os_patch_level {
            Some(s) => parse_patch_level(s)?,
            None => 0,
        };
        Ok((v << 11) | p)
    }
}

fn parse_os_version(s: &str) -> Result<u32, String> {
    let parts: Vec<u32> = s
        .split('.')
        .map(|p| {
            p.parse::<u32>()
                .map_err(|_| format!("os_version `{s}` is not A.B.C"))
        })
        .collect::<Result<_, _>>()?;
    if parts.is_empty() || parts.len() > 3 || parts.iter().any(|&p| p > 127) {
        return Err(format!(
            "os_version `{s}` is not A.B.C (each part below 128)"
        ));
    }
    let g = |i: usize| parts.get(i).copied().unwrap_or(0);
    Ok((g(0) << 14) | (g(1) << 7) | g(2))
}

fn parse_patch_level(s: &str) -> Result<u32, String> {
    let bad = || format!("os_patch_level `{s}` is not YYYY-MM");
    let (y, m) = s.split_once('-').ok_or_else(bad)?;
    let (y, m): (u32, u32) = (y.parse().map_err(|_| bad())?, m.parse().map_err(|_| bad())?);
    if !(2000..=2127).contains(&y) || !(1..=12).contains(&m) {
        return Err(bad());
    }
    Ok(((y - 2000) << 4) | m)
}

/// The os words as `A.B.C` and `YYYY-MM` (none when the word is 0).
pub fn os_words(word: u32) -> (Option<String>, Option<String>) {
    let (v, p) = (word >> 11, word & 0x7ff);
    let version = (v != 0).then(|| format!("{}.{}.{}", v >> 14, (v >> 7) & 127, v & 127));
    let patch = (p != 0).then(|| format!("{}-{:02}", 2000 + (p >> 4), p & 15));
    (version, patch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_words_round_trip() {
        let b = Boot::parse(
            "[boot]\nkernel_offset=0x8000\nramdisk_offset=0x1000000\ntags_offset=0x100\npagesize=4096\ncmdline=\"x\"\nos_version=\"8.1.0\"\nos_patch_level=\"2020-01\"\n",
        )
        .unwrap();
        let w = b.os_word().unwrap();
        assert_eq!(os_words(w), (Some("8.1.0".into()), Some("2020-01".into())));
    }

    #[test]
    fn a_manifest_without_boot_is_refused_in_words() {
        assert_eq!(
            Boot::parse("[device]\ncodename=\"x\"\n").unwrap_err(),
            "manifest has no [boot] table"
        );
    }

    #[test]
    fn an_unknown_boot_field_is_refused() {
        assert!(Boot::parse("[boot]\nbogus = 1\n").is_err());
    }
}
