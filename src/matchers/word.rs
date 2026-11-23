//! Words.

use super::BASE;
use crate::chars::{is_apostrophe, is_dash, is_letter, is_word_termination};
use crate::cursor::Cursor;
use crate::matcher::Matcher;

/// Kind of [`Word`] tokens.
pub const WORD: &str = "WORD";

/// A letter, then letters, dashes and at most one apostrophe, up to white
/// space, sentence punctuation or the end of the text: `word`,
/// `first-second`, `don't`.
///
/// Anything else in the run means it is not a word, so `word123` and
/// `some_word` do not match at all. A word ends with a letter: a dash or an
/// apostrophe at the end of the run is left out, so `dogs'` gives `dogs`.
pub struct Word;

impl Matcher for Word {
    fn kind(&self) -> &'static str {
        WORD
    }

    fn overrides(&self) -> &'static [&'static str] {
        BASE
    }

    fn can_start(&self, first: char) -> bool {
        is_letter(first)
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        if !cur.advance_if(is_letter) {
            return false;
        }
        let mut apostrophe = false;
        let terminated = loop {
            cur.advance_while(|c| is_letter(c) || is_dash(c));
            match cur.peek() {
                Some(c) if is_apostrophe(c) && !apostrophe => {
                    apostrophe = true;
                    cur.advance();
                }
                // A second apostrophe lands here too and is not a termination.
                Some(c) => break is_word_termination(c),
                None => break true,
            }
        };
        cur.back_while(|c| is_dash(c) || is_apostrophe(c));
        terminated
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
        assert_eq!(Word.match_prefix("word- next"), Some("word"));
        assert_eq!(Word.match_prefix("dogs' toys"), Some("dogs"));
        assert_eq!(Word.match_prefix("well-'"), Some("well"));
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
            "-word",
            "'tis",
            "",
        ] {
            assert_eq!(Word.match_prefix(text), None, "{text:?}");
        }
    }
}
