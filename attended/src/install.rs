// SPDX-License-Identifier: MIT OR Apache-2.0
//! The install steps lifted from walleye's proven write (google-walleye-lab tools/walleye-tries/src/install.rs): which
//! slot the phone runs from, whether a mount is read-only, and one `dd` of a staged image to a partition of the slot the
//! phone is NOT on, read back by sha256. They run as root on the phone through devlab (adb), take their values as
//! arguments (a procedure reads them from its device.toml) and write only what they are told is allowed.

use crate::{Kind, Step};

/// The block size of the write and of its read-back: an image is a whole number of these.
const BLOCK: u64 = 4096;

/// The slot the phone runs from, from `getprop ro.boot.slot_suffix` (`_a` or `_b`): the letter. Anything else is a doubt.
pub(crate) fn parse_slot_suffix(text: &str) -> Result<&'static str, String> {
    match text.trim() {
        "_a" => Ok("a"),
        "_b" => Ok("b"),
        other => Err(format!(
            "the phone's slot suffix is {other:?}, neither _a nor _b"
        )),
    }
}

/// The options of the mount at `point` in a `/proc/mounts` text; the failure names the point when nothing is mounted there.
pub(crate) fn mount_options<'a>(mounts: &'a str, point: &str) -> Result<&'a str, String> {
    mounts
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>())
        .find(|f| f.len() >= 4 && f[1] == point)
        .map(|f| f[3])
        .ok_or_else(|| format!("nothing is mounted at {point}"))
}

/// The mount at `point` is read-only: its options hold `ro` as a whole option (`rw` and `errors=remount-ro` are not it).
pub(crate) fn mount_readonly(mounts: &str, point: &str) -> Result<(), String> {
    let options = mount_options(mounts, point)?;
    if options.split(',').any(|o| o == "ro") {
        Ok(())
    } else {
        Err(format!("{point} is not read-only: mounted {options}"))
    }
}

/// The write is allowed: the partition is in the allow-list and is not one of the slot the phone runs from (`active`,
/// `a` or `b`). Both refusals name the partition.
pub(crate) fn write_allowed(allowed: &[&str], partition: &str, active: &str) -> Result<(), String> {
    if !allowed.contains(&partition) {
        return Err(format!(
            "{partition} is not in the allow-list ({}): nothing is written",
            allowed.join(", ")
        ));
    }
    let other = if active == "a" { "b" } else { "a" };
    if !partition.ends_with(&format!("_{other}")) {
        return Err(format!(
            "{partition} is not of slot {other}, the slot the phone is not on (it runs from {active}): nothing is written"
        ));
    }
    Ok(())
}

/// A word that is safe in the phone's shell script: letters, digits and `_ - . /`, no `..`, not empty.
fn plain(what: &str, value: &str) -> Result<(), String> {
    let ok = !value.is_empty()
        && !value.starts_with('-')
        && !value.contains("..")
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-./".contains(c));
    if ok {
        Ok(())
    } else {
        Err(format!("{what} {value:?} is not a plain name or path"))
    }
}

/// A sha256 as `sha256sum` prints it: 64 hex digits, lowercase.
fn sha256_word(value: &str) -> Result<String, String> {
    let v = value.trim().to_lowercase();
    if v.len() == 64 && v.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(v)
    } else {
        Err(format!("{value:?} is not a sha256 (64 hex digits)"))
    }
}

/// The root script of the write: refuses a staged image that is not whole blocks or not the declared sha256, before it
/// writes; writes with one `dd`; prints the sha256 of what the partition now holds over the image's length.
fn write_script(by_name: &str, partition: &str, staged: &str, sha256: &str) -> String {
    format!(
        "set -e\n\
         size=$(wc -c < {staged})\n\
         [ \"$size\" -gt 0 ] && [ $((size % {BLOCK})) -eq 0 ] || {{ echo \"the staged image is $size bytes, not whole {BLOCK}-byte blocks\" >&2; exit 1; }}\n\
         [ \"$(sha256sum {staged} | cut -d' ' -f1)\" = {sha256} ] || {{ echo \"the staged image is not the declared sha256\" >&2; exit 1; }}\n\
         [ -e '{by_name}/{partition}' ] || {{ echo \"no partition node {partition}\" >&2; exit 1; }}\n\
         blocks=$((size / {BLOCK}))\n\
         dd if={staged} of={by_name}/{partition} bs={BLOCK} count=$blocks conv=fsync,notrunc 2>/dev/null\n\
         blockdev --flushbufs {by_name}/{partition} 2>/dev/null || true\n\
         dd if={by_name}/{partition} bs={BLOCK} count=$blocks 2>/dev/null | sha256sum | cut -d' ' -f1\n"
    )
}

/// The slot the phone runs from reads as `_a` or `_b` (`getprop ro.boot.slot_suffix`).
pub fn read_slot() -> Step {
    Step::new(
        Kind::Check,
        "the phone says which slot it runs from",
        || {
            let said = crate::phone().root("getprop ro.boot.slot_suffix")?;
            parse_slot_suffix(&said).map(|_| ())
        },
    )
}

/// The mount at `point` is read-only (`/proc/mounts` as root).
pub fn mount_is_readonly(point: &str) -> Step {
    let point = point.to_string();
    Step::new(
        Kind::Check,
        &format!("{point} is mounted read-only"),
        move || mount_readonly(&crate::phone().root("cat /proc/mounts")?, &point),
    )
}

/// Write the staged image (a path on the phone) to `by_name/partition` with one `dd`, and read it back by sha256.
/// `allowed` is the allow-list the device declares: a partition outside it, or of the slot the phone runs from (read
/// now), is refused before anything is written, and so is a staged image that is not whole blocks or not `sha256`.
/// The step fails when the partition read back over the image's length does not hash to `sha256`.
pub fn adb_write_partition(
    allowed: &[&str],
    by_name: &str,
    partition: &str,
    staged: &str,
    sha256: &str,
) -> Step {
    let allowed: Vec<String> = allowed.iter().map(|s| s.to_string()).collect();
    let (by_name, partition, staged) = (
        by_name.to_string(),
        partition.to_string(),
        staged.to_string(),
    );
    let sha256 = sha256.to_string();
    Step::new(
        Kind::Do,
        &format!("write {staged} to {partition}, read back by sha256"),
        move || {
            plain("by-name folder", &by_name)?;
            plain("partition", &partition)?;
            plain("staged image", &staged)?;
            let sha256 = sha256_word(&sha256)?;
            let active = parse_slot_suffix(&crate::phone().root("getprop ro.boot.slot_suffix")?)?;
            let allowed: Vec<&str> = allowed.iter().map(String::as_str).collect();
            write_allowed(&allowed, &partition, active)?;
            let said =
                crate::phone().root(&write_script(&by_name, &partition, &staged, &sha256))?;
            let read_back = said.lines().last().unwrap_or("").trim();
            if read_back == sha256 {
                Ok(())
            } else {
                Err(format!(
                    "{partition} read back as {read_back:?}, not the image's {sha256}"
                ))
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // /proc/mounts as a phone printed it: tests/fixtures/proc-mounts-lower-ro.txt (see SOURCES.md there)
    const MOUNTS: &str = include_str!("../tests/fixtures/proc-mounts-lower-ro.txt");

    #[test]
    fn the_slot_suffix_a_phone_printed_is_slot_b() {
        let said = include_str!("../tests/fixtures/slot-suffix-on-b.txt");
        assert_eq!(parse_slot_suffix(said), Ok("b"));
        assert!(parse_slot_suffix("").is_err());
        assert!(parse_slot_suffix("b").is_err());
    }

    #[test]
    fn a_mount_is_read_only_by_the_whole_option_ro() {
        assert!(mount_readonly(MOUNTS, "/lower").is_ok());
        assert!(mount_readonly(MOUNTS, "/").unwrap_err().contains("rw,"));
        let remount = "/dev/x /data ext4 rw,errors=remount-ro 0 0\n";
        assert!(mount_readonly(remount, "/data").is_err());
    }

    #[test]
    fn a_write_needs_the_allow_list_and_the_other_slot() {
        let list = ["system_a", "system_b"];
        assert!(write_allowed(&list, "system_a", "b").is_ok());
        assert!(write_allowed(&list, "system_b", "b").is_err());
        assert!(write_allowed(&list, "boot_a", "b")
            .unwrap_err()
            .contains("boot_a"));
    }
}
