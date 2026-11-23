// SPDX-License-Identifier: MIT OR Apache-2.0
//! What a check's source may be, and whether it holds: a research row (`research:ID`), a doc (`.md`) or a file of
//! text a phone printed (under a `research` folder), read from the folder of the device's `device.toml`.

use std::io::Read;
use std::path::Path;

/// How much of a source is read: enough to tell text from nothing.
const HEAD: u64 = 64 * 1024;

/// The research row an id names: `research:ID`.
const ROW: &str = "research:";

/// The id `lpoco attended new` writes for the author to replace: a procedure that still has it names no row.
const PLACEHOLDER: &str = "ROW-ID";

/// Whether `source` is one a check may name, as far as this program can see: a row is taken as named (the base it
/// lives in is not this program's), a file must be there, hold text, and, unless it is a doc, be under a `research` folder.
pub(crate) fn judge(from: &Path, source: Option<&str>) -> Result<(), String> {
    let source = source.map(str::trim).unwrap_or_default();
    if source.is_empty() {
        return Err("it names no source (check_from(what, source, ..): a research row, a doc or captured text)".into());
    }
    if let Some(id) = source.strip_prefix(ROW) {
        return if id.is_empty() || id.contains(char::is_whitespace) || id == PLACEHOLDER {
            Err(format!("the source {source:?} names no research row id"))
        } else {
            Ok(())
        };
    }
    let path = Path::new(source);
    if path.is_absolute() || path.components().any(|c| c.as_os_str() == "..") {
        return Err(format!(
            "the source {source:?} is not a path inside the repository"
        ));
    }
    let file = from.join(path);
    let unreadable = |e: std::io::Error| {
        format!(
            "the source {source:?} cannot be read ({e}): {}",
            file.display()
        )
    };
    // a link may not carry the name out of the repository, nor point at a device or a pipe
    let (real, root) = (
        file.canonicalize().map_err(unreadable)?,
        from.canonicalize()
            .map_err(|e| format!("cannot read {}: {e}", from.display()))?,
    );
    if !real.starts_with(&root) || !real.is_file() {
        return Err(format!(
            "the source {source:?} is not a file inside the repository"
        ));
    }
    let mut head = Vec::new();
    std::fs::File::open(&real)
        .and_then(|f| f.take(HEAD).read_to_end(&mut head))
        .map_err(|e| format!("the source {source:?} cannot be read ({e})"))?;
    if head.iter().all(u8::is_ascii_whitespace) {
        return Err(format!("the source {source:?} has nothing in it"));
    }
    // a read cut inside a character is still text
    if std::str::from_utf8(&head).is_err_and(|e| e.error_len().is_some()) {
        return Err(format!("the source {source:?} is not text"));
    }
    let is_doc = path.extension().is_some_and(|e| e == "md");
    let in_research = path.components().count() > 1
        && path
            .components()
            .next()
            .is_some_and(|c| c.as_os_str() == "research");
    if !is_doc && !in_research {
        return Err(format!(
            "the source {source:?} is not under a research folder, so it is not text a phone printed (a doc is a .md file)"
        ));
    }
    Ok(())
}
