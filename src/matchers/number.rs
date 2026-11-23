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

/// A run of number characters and the punctuation written with them:
/// `-0.5`, `1,000`, `10_000`, `100%`, `6.0.1`.
///
/// The punctuation is full stops, commas, percent and number signs, math
/// symbols, dashes and connectors. At most two punctuation characters stand
/// before the first number character (`-.5`, `--1`). A number ends with a
/// number character or a percent sign, so one at the end of a sentence does
/// not take the full stop: `1.` gives `1`.
pub struct LikeNumber;

/// How many punctuation characters may stand before the first number
/// character. The limit bounds the work on a long run of punctuation.
const MAX_LEADING_PUNCTUATION: usize = 2;

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
        for _ in 0..MAX_LEADING_PUNCTUATION {
            if !cur.advance_if(is_number_punctuation) {
                break;
            }
        }
        if !cur.advance_if(is_number) {
            return false;
        }
        cur.advance_while2(is_number, is_number_punctuation);
        cur.back_while(|c| is_number_punctuation(c) && !is_percent_sign(c));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_text_without_a_number() {
        for text in ["-", "a", "abc", "-a", "~a", "!!", "   ", "", "---1", "...5"] {
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
            ("-.5", "-.5"),
            (".5", ".5"),
            ("#1", "#1"),
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
            ("100%.", "100%"),
            ("100%,", "100%"),
            ("1,000 ", "1,000"),
            ("1,000!!", "1,000"),
            ("1.", "1"),
            ("1.-", "1"),
            ("100%,!", "100%"),
            ("215.114.180.213. As", "215.114.180.213"),
            ("18+", "18"),
            ("5%-10%", "5%-10%"),
            ("½ cup", "½"),
            ("٣٥ ", "٣٥"),
        ] {
            assert_eq!(LikeNumber.match_prefix(text), Some(expected), "{text:?}");
        }
    }
}
