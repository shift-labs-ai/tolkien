//! Numbers as they are written in text.

use super::BASE;
use crate::chars::{
    is_comma, is_connector_punctuation, is_dash_punctuation, is_full_stop, is_math_symbol,
    is_number, is_number_sign, is_percent_sign,
};
use crate::cursor::Cursor;
use crate::matcher::Matcher;

/// Kind of [`LikeNumber`] tokens.
pub const LIKE_NUMBER: &str = "LIKE_NUMBER";

/// A run of number characters and the punctuation written with them, holding
/// at least one number character: `-0.5`, `1,000`, `10_000`, `100%`, `6.0.1`.
///
/// The punctuation is full stops, commas, percent and number signs, math
/// symbols, dashes and connectors. It is taken wherever it stands, so a
/// number at the end of a sentence keeps the full stop: `1.`
pub struct LikeNumber;

fn is_number_punctuation(c: char) -> bool {
    is_full_stop(c)
        || is_comma(c)
        || is_percent_sign(c)
        || is_number_sign(c)
        || is_math_symbol(c)
        || is_dash_punctuation(c)
        || is_connector_punctuation(c)
}

impl Matcher for LikeNumber {
    fn kind(&self) -> &'static str {
        LIKE_NUMBER
    }

    fn overrides(&self) -> &'static [&'static str] {
        BASE
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        let (numbers, _) = cur.advance_while2(is_number, is_number_punctuation);
        numbers > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_text_without_a_number() {
        for text in ["-", "a", "abc", "-a", "~a", "!!", "   ", ""] {
            assert_eq!(LikeNumber.match_prefix(text), None, "{text:?}");
        }
    }

    #[test]
    fn matches_numbers() {
        for (text, expected) in [
            ("6.0.1", "6.0.1"),
            ("--1", "--1"),
            ("1", "1"),
            ("-15", "-15"),
            ("-0.5", "-0.5"),
            ("+15", "+15"),
            ("~15", "~15"),
            ("100", "100"),
            ("10_000", "10_000"),
            ("10_000_0", "10_000_0"),
            ("1,000", "1,000"),
            ("10.5", "10.5"),
            ("0.1%", "0.1%"),
            ("0.1 ", "0.1"),
            ("100%", "100%"),
            ("100% ", "100%"),
            ("100%.", "100%."),
            ("100%,", "100%,"),
            ("1,000 ", "1,000"),
            ("1,000!!", "1,000"),
            ("1.", "1."),
            ("1.-", "1.-"),
            ("100%,!", "100%,"),
            ("½ cup", "½"),
            ("٣٥ ", "٣٥"),
        ] {
            assert_eq!(LikeNumber.match_prefix(text), Some(expected), "{text:?}");
        }
    }
}
