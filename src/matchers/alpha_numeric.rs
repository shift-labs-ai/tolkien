//! Identifiers and codes: letters mixed with digits.

use super::BASE;
use crate::chars::{is_dash, is_digit, is_letter, is_letter_or_digit, is_underscore};
use crate::cursor::Cursor;
use crate::matcher::Matcher;

/// Kind of [`AlphaNumericSequence`] tokens.
pub const ALPHA_NUMERIC_SEQUENCE: &str = "ALPHA_NUMERIC_SEQUENCE";

/// A run of letters, digits, dashes and underscores that starts with a
/// letter or a digit and holds at least one letter: `armV7`, `some_word`,
/// `B-52`.
///
/// Unlike a [`Word`](super::Word), the run simply ends at the first character
/// that does not belong to it.
pub struct AlphaNumericSequence;

impl Matcher for AlphaNumericSequence {
    fn kind(&self) -> &'static str {
        ALPHA_NUMERIC_SEQUENCE
    }

    fn overrides(&self) -> &'static [&'static str] {
        BASE
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        if !cur.peek().is_some_and(is_letter_or_digit) {
            return false;
        }
        let (letters, _) =
            cur.advance_while2(is_letter, |c| is_digit(c) || is_dash(c) || is_underscore(c));
        letters > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_runs_with_a_letter() {
        let m = AlphaNumericSequence;
        assert_eq!(m.match_prefix("armV7"), Some("armV7"));
        assert_eq!(m.match_prefix("word123 next"), Some("word123"));
        assert_eq!(m.match_prefix("123word"), Some("123word"));
        assert_eq!(m.match_prefix("some_word."), Some("some_word"));
        assert_eq!(m.match_prefix("B-52)"), Some("B-52"));
        assert_eq!(m.match_prefix("hello)"), Some("hello"));
        assert_eq!(m.match_prefix("v2.0"), Some("v2"));
    }

    #[test]
    fn needs_a_letter_and_starts_with_a_letter_or_a_digit() {
        let m = AlphaNumericSequence;
        for text in ["123", "10_000", "1-2", "-", "_", "-a", "_a", " a", "", "!a"] {
            assert_eq!(m.match_prefix(text), None, "{text:?}");
        }
    }
}
