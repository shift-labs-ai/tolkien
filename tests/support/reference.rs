//! The plain way to lex, for comparing the lexer with.
//!
//! `tests/lexer.rs` and `examples/fuzz.rs` both include this file, so that
//! they hold the lexer to the same answer.

use tolkien::{Matcher, OTHER};

/// The kind, start and end of every token of `text`, found by asking every
/// matcher at every position, in the order given. A character that no
/// matcher takes is an `OTHER` token.
///
/// This is what a [`tolkien::Lexer`] over the same matchers must give,
/// however it gets there.
pub fn tokens(matchers: &[Box<dyn Matcher>], text: &str) -> Vec<(&'static str, usize, usize)> {
    let mut tokens = Vec::new();
    let mut pos = 0;
    while let Some(first) = text[pos..].chars().next() {
        let (kind, len) = matchers
            .iter()
            .find_map(|matcher| Some((matcher.kind(), matcher.match_prefix(&text[pos..])?.len())))
            .unwrap_or((OTHER, first.len_utf8()));
        tokens.push((kind, pos, pos + len));
        pos += len;
    }
    tokens
}
