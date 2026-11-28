// SPDX-License-Identifier: MIT OR Apache-2.0
//! The boot format: how a device's kernel and ramdisk become the image its bootloader takes, and how that image
//! is counted back. The device says it in its declaration's `[boot]` table (the header facts, in the words of
//! a device-info file) and names the format in `format`; a format is one adapter behind [`BootFormat`], looked
//! up by [`format`]. The first adapter is android-bootimg ([`crate::android_bootimg`]).

use crate::android_bootimg::AndroidBootimg;
use serde::Deserialize;

/// The header facts of a device's boot image: the `[boot]` table of its declaration. Keys that are the lab's
/// (hooks, gadget, waits) are not osfact's and are ignored here.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Boot {
    /// the boot format's name (an adapter of [`format`])
    #[serde(default = "default_format")]
    pub format: String,
    /// 0 is the only header version packed and checked so far
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
}

fn default_format() -> String {
    AndroidBootimg::NAME.to_string()
}

#[derive(Deserialize)]
struct Declaration {
    boot: Option<Boot>,
}

impl Boot {
    /// The `[boot]` table of a declaration's text.
    pub fn from_declaration(text: &str) -> Result<Boot, String> {
        let d: Declaration = toml::from_str(text).map_err(|e| format!("the declaration: {e}"))?;
        d.boot
            .ok_or_else(|| "the declaration has no [boot] table".to_string())
    }

    /// The os words as the header holds them: version << 11 | patch level (0 when none is asked).
    pub fn os_word(&self) -> Result<u32, String> {
        os_word(self.os_version.as_deref(), self.os_patch_level.as_deref())
    }
}

/// The os words as the header holds them for `A.B.C` and `YYYY-MM` (0 for each that is not asked).
pub fn os_word(version: Option<&str>, patch_level: Option<&str>) -> Result<u32, String> {
    let v = version.map(parse_os_version).transpose()?.unwrap_or(0);
    let p = patch_level.map(parse_patch_level).transpose()?.unwrap_or(0);
    Ok((v << 11) | p)
}

/// The os words as `A.B.C` and `YYYY-MM` (none when the word is 0).
pub fn os_words(word: u32) -> (Option<String>, Option<String>) {
    let (v, p) = (word >> 11, word & 0x7ff);
    let version = (v != 0).then(|| format!("{}.{}.{}", v >> 14, (v >> 7) & 127, v & 127));
    let patch = (p != 0).then(|| format!("{}-{:02}", 2000 + (p >> 4), p & 15));
    (version, patch)
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

/// One counted fact: `ok`, a miss (`FAIL`), or a thing the declaration did not ask about (`not measured`).
#[derive(Debug, PartialEq)]
pub enum Verdict {
    Ok(String),
    Fail(String),
    NotMeasured(String),
}

#[derive(Debug, PartialEq)]
pub struct Finding {
    pub name: &'static str,
    pub verdict: Verdict,
}

impl Finding {
    pub fn line(&self) -> String {
        match &self.verdict {
            Verdict::Ok(d) => format!("ok   {}: {d}", self.name),
            Verdict::Fail(d) => format!("FAIL {}: {d}", self.name),
            Verdict::NotMeasured(d) => format!("--   {}: not measured: {d}", self.name),
        }
    }
}

/// A boot format: what a bootloader's image is made of and counted by.
pub trait BootFormat {
    /// The image for these header facts, the kernel (a device tree appended when the device wants one) and the ramdisk.
    fn pack(&self, boot: &Boot, kernel: &[u8], ramdisk: &[u8]) -> Result<Vec<u8>, String>;
    /// What an image must be for the device whose declaration says `boot`, counted fact by fact.
    fn check(&self, image: &[u8], boot: &Boot) -> Vec<Finding>;
}

/// The adapters there are, by the name a declaration gives.
pub const FORMATS: &[&str] = &[AndroidBootimg::NAME];

/// The adapter named `name`.
pub fn format(name: &str) -> Result<Box<dyn BootFormat>, String> {
    match name {
        AndroidBootimg::NAME => Ok(Box::new(AndroidBootimg)),
        other => Err(format!(
            "boot format {other:?} is not one osfact has; it has {}",
            FORMATS.join(", ")
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_words_round_trip() {
        let w = os_word(Some("8.1.0"), Some("2020-01")).unwrap();
        assert_eq!(os_words(w), (Some("8.1.0".into()), Some("2020-01".into())));
    }
}

#[cfg(test)]
mod os_word_refusals {
    use super::*;

    #[test]
    fn what_is_not_a_version_or_a_patch_level_is_refused_in_words() {
        for (v, p, said) in [
            (Some("8.x"), None, "os_version `8.x` is not A.B.C"),
            (Some("8.1.200"), None, "each part below 128"),
            (Some("1.2.3.4"), None, "os_version `1.2.3.4`"),
            (None, Some("2020"), "os_patch_level `2020` is not YYYY-MM"),
            (None, Some("1999-05"), "os_patch_level `1999-05`"),
            (None, Some("2020-13"), "os_patch_level `2020-13`"),
        ] {
            let e = os_word(v, p).unwrap_err();
            assert!(e.contains(said), "{e}");
        }
        assert_eq!(os_word(None, None), Ok(0));
    }
}
