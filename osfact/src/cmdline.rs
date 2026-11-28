// SPDX-License-Identifier: MIT OR Apache-2.0
//! The kernel command line of the one image: the base the declaration's `[boot] cmdline` says, then the pieces
//! the parts declare (`[[install.cmdline_piece]]`: an owner and a piece), in the order declared. A setting is a
//! word `key` or `key=value`; two owners that set one key to different values are refused naming both, the same
//! setting from two owners is written once, and a line over the boot format's room is refused naming the piece
//! that crosses it.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The owner the base line goes by in a refusal and in the log.
pub const BASE_OWNER: &str = "[boot] cmdline";

/// One piece of the command line, and the part that declares it.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Piece {
    /// The part that needs the piece (a name the lab gives its part, e.g. display)
    pub owner: String,
    /// The settings, as the kernel takes them: words separated by spaces (iommu.passthrough=1)
    pub piece: String,
}

/// One word of the line: its key, the whole word, and the owner that set it.
struct Setting<'a> {
    key: &'a str,
    word: &'a str,
    owner: &'a str,
}

fn key_of(word: &str) -> &str {
    word.split_once('=').map_or(word, |(k, _)| k)
}

/// The base and the pieces as one line no longer than `room` bytes.
pub fn compose(base: &str, pieces: &[Piece], room: usize) -> Result<String, String> {
    let mut line = base.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut set: Vec<Setting> = base
        .split_whitespace()
        .map(|word| Setting {
            key: key_of(word),
            word,
            owner: BASE_OWNER,
        })
        .collect();
    for p in pieces {
        let words: Vec<&str> = p.piece.split_whitespace().collect();
        if p.owner.trim().is_empty() || words.is_empty() {
            return Err(format!(
                "a command line piece needs an owner and a piece (owner {:?}, piece {:?})",
                p.owner, p.piece
            ));
        }
        for word in words {
            match set.iter().find(|s| s.key == key_of(word)) {
                Some(s) if s.word == word => continue,
                Some(s) => {
                    return Err(format!(
                        "the command line key {:?} is set to {} by {} and to {} by {}",
                        s.key,
                        &s.word[s.key.len()..],
                        s.owner,
                        &word[s.key.len()..],
                        p.owner
                    ));
                }
                None => {}
            }
            let longer = line.len() + usize::from(!line.is_empty()) + word.len();
            if longer > room {
                return Err(format!(
                    "the command line is {longer} bytes with {word:?} of {}; the header holds {room}",
                    p.owner
                ));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line += word;
            set.push(Setting {
                key: key_of(word),
                word,
                owner: &p.owner,
            });
        }
    }
    Ok(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(owner: &str, piece: &str) -> Piece {
        Piece {
            owner: owner.into(),
            piece: piece.into(),
        }
    }

    #[test]
    fn a_bare_word_and_a_key_are_told_apart() {
        let line = compose("quiet a=1", &[piece("x", "b"), piece("y", "a=1")], 100).unwrap();
        assert_eq!(line, "quiet a=1 b");
    }

    #[test]
    fn a_bare_word_against_a_valued_key_clashes() {
        let e = compose("a=1", &[piece("x", "a")], 100).unwrap_err();
        assert!(e.contains("\"a\"") && e.contains("x"), "{e}");
    }

    #[test]
    fn an_empty_piece_is_refused() {
        assert!(compose("a", &[piece("x", "  ")], 100).is_err());
        assert!(compose("a", &[piece(" ", "b")], 100).is_err());
    }
}
