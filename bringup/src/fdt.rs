//! A flattened device tree, read only as far as the checks need: whether a blob is one, and the u32 cells of a
//! property of the root node (qcom,msm-id).

const MAGIC: [u8; 4] = [0xd0, 0x0d, 0xfe, 0xed];

fn be(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .map(|x| u32::from_be_bytes([x[0], x[1], x[2], x[3]]))
}

/// The last device tree appended to `blob`: where it starts and its total size, the one whose own size
/// ends exactly where the blob does (a kernel followed by its dtb).
pub fn appended(blob: &[u8]) -> Option<(usize, usize)> {
    let mut found = None;
    let mut from = 0;
    while let Some(i) = blob[from..].windows(4).position(|w| w == MAGIC) {
        let at = from + i;
        if let Some(total) = be(blob, at + 4).map(|t| t as usize) {
            if total >= 40 && at + total == blob.len() && root_property(&blob[at..], "").is_some() {
                found = Some((at, total));
            }
        }
        from = at + 1;
    }
    found
}

/// The big-endian u32 cells of the root node's property `name`; `name` empty only asks whether the tree walks.
pub fn root_property(dtb: &[u8], name: &str) -> Option<Vec<u32>> {
    if dtb.get(..4)? != MAGIC {
        return None;
    }
    let off_struct = be(dtb, 8)? as usize;
    let off_strings = be(dtb, 12)? as usize;
    let mut at = off_struct;
    let mut depth = 0u32;
    loop {
        let token = be(dtb, at)?;
        at += 4;
        match token {
            1 => {
                // BEGIN_NODE: a NUL-terminated name, padded to 4
                let end = dtb.get(at..)?.iter().position(|&c| c == 0)?;
                at += (end + 1 + 3) & !3;
                depth += 1;
            }
            2 => depth = depth.checked_sub(1)?,
            3 => {
                let len = be(dtb, at)? as usize;
                let nameoff = be(dtb, at + 4)? as usize;
                let data = dtb.get(at + 8..at + 8 + len)?;
                at += 8 + ((len + 3) & !3);
                if depth == 1 && !name.is_empty() {
                    let s = dtb.get(off_strings + nameoff..)?;
                    let n = &s[..s.iter().position(|&c| c == 0)?];
                    if n == name.as_bytes() && len.is_multiple_of(4) {
                        return Some(
                            data.chunks(4)
                                .map(|c| u32::from_be_bytes([c[0], c[1], c[2], c[3]]))
                                .collect(),
                        );
                    }
                }
            }
            4 => {}
            9 => return if name.is_empty() { Some(vec![]) } else { None },
            _ => return None,
        }
    }
}
