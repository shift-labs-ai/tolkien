//! Words.

use super::BASE;
use crate::chars::{is_apostrophe, is_dash, is_letter, is_word_termination};
use crate::cursor::Cursor;
use crate::matcher::Matcher;

/// Kind of [`Word`] tokens.
pub const WORD: &str = "WORD";

/// Letters, with dashes and at most one apostrophe inside, up to white space,
/// sentence punctuation or the end of the text: `word`, `first-second`,
/// `don't`.
///
/// Anything else in the run means it is not a word, so `word123` and
/// `some_word` do not match at all.
pub struct Word;

impl Matcher for Word {
    fn kind(&self) -> &'static str {
        WORD
    }

    fn overrides(&self) -> &'static [&'static str] {
        BASE
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        let mut letters = 0;
        let mut apostrophe = false;
        while let Some(c) = cur.peek() {
            if is_letter(c) {
                letters += 1;
            } else if is_apostrophe(c) {
                if apostrophe {
                    return false;
                }
                apostrophe = true;
            } else if is_dash(c) {
                // Dashes join the parts of a word and do not count.
            } else if is_word_termination(c) {
                break;
            } else {
                return false;
            }
            cur.advance();
        }
        letters > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_words() {
        assert_eq!(Word.match_prefix("word"), Some("word"));
        assert_eq!(Word.match_prefix("first second"), Some("first"));
        assert_eq!(Word.match_prefix("first-second"), Some("first-second"));
        assert_eq!(Word.match_prefix("first-second!!??!"), Some("first-second"));
        assert_eq!(Word.match_prefix("someword!"), Some("someword"));
        assert_eq!(Word.match_prefix("somewor'd"), Some("somewor'd"));
        assert_eq!(Word.match_prefix("don’t stop"), Some("don’t"));
        assert_eq!(Word.match_prefix("naïve, café"), Some("naïve"));
    }

    #[test]
    fn rejects_runs_that_are_not_only_a_word() {
        for text in [
            "123",
            "word-123",
            "word_123",
            "word123",
            "123word",
            "123word123",
            "some_word",
            "?someword",
            "it's'",
            "-",
            "",
        ] {
            assert_eq!(Word.match_prefix(text), None, "{text:?}");
        }
    }
}
