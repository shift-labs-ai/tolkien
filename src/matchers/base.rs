//! The simplest matchers, defined by one character class each.

use super::BASE;
use crate::chars::{is_digit, is_letter, is_line_termination, is_symbol, is_whitespace};
use crate::cursor::Cursor;
use crate::matcher::Matcher;

/// Kind of [`Digit`] tokens.
pub const DIGIT: &str = "DIGIT";
/// Kind of [`Letter`] tokens.
pub const LETTER: &str = "LETTER";
/// Kind of [`Symbol`] tokens.
pub const SYMBOL: &str = "SYMBOL";
/// Kind of [`WhiteSpace`] tokens.
pub const WHITE_SPACE: &str = "WHITE_SPACE";
/// Kind of [`LineTermination`] tokens.
pub const LINE_TERMINATION: &str = "LINE_TERMINATION";

/// A run of decimal digits.
pub struct Digit;

impl Matcher for Digit {
    fn kind(&self) -> &'static str {
        DIGIT
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        cur.advance_while(is_digit) > 0
    }
}

/// A run of letters.
pub struct Letter;

impl Matcher for Letter {
    fn kind(&self) -> &'static str {
        LETTER
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        cur.advance_while(is_letter) > 0
    }
}

/// One character that is not a letter, a digit or white space.
pub struct Symbol;

impl Matcher for Symbol {
    fn kind(&self) -> &'static str {
        SYMBOL
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        cur.advance_if(is_symbol)
    }
}

/// A run of white space.
pub struct WhiteSpace;

impl Matcher for WhiteSpace {
    fn kind(&self) -> &'static str {
        WHITE_SPACE
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        cur.advance_while(is_whitespace) > 0
    }
}

/// A run of line breaks.
pub struct LineTermination;

impl Matcher for LineTermination {
    fn kind(&self) -> &'static str {
        LINE_TERMINATION
    }

    fn overrides(&self) -> &'static [&'static str] {
        BASE
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        cur.advance_while(is_line_termination) > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASCII_SYMBOLS: &str = "!\"#$%&'()*+,-./:;<=>?[\\]^_`{|}~";

    fn each_char(chars: impl IntoIterator<Item = char>, mut check: impl FnMut(&str)) {
        for c in chars {
            check(&c.to_string());
        }
    }

    #[test]
    fn digit() {
        each_char(('a'..='z').chain('A'..='Z'), |s| {
            assert_eq!(Digit.match_prefix(s), None)
        });
        each_char('0'..='9', |s| assert_eq!(Digit.match_prefix(s), Some(s)));
        each_char(ASCII_SYMBOLS.chars(), |s| {
            assert_eq!(Digit.match_prefix(s), None)
        });
        assert_eq!(Digit.match_prefix("1 "), Some("1"));
        assert_eq!(Digit.match_prefix("123a"), Some("123"));
        assert_eq!(Digit.match_prefix(" 1"), None);
    }

    #[test]
    fn letter() {
        each_char(('a'..='z').chain('A'..='Z'), |s| {
            assert_eq!(Letter.match_prefix(s), Some(s))
        });
        each_char('0'..='9', |s| assert_eq!(Letter.match_prefix(s), None));
        each_char(ASCII_SYMBOLS.chars(), |s| {
            assert_eq!(Letter.match_prefix(s), None)
        });
        assert_eq!(Letter.match_prefix("a "), Some("a"));
        assert_eq!(Letter.match_prefix("שלום!"), Some("שלום"));
        assert_eq!(Letter.match_prefix(" a"), None);
    }

    #[test]
    fn symbol() {
        each_char(('a'..='z').chain('A'..='Z'), |s| {
            assert_eq!(Symbol.match_prefix(s), None)
        });
        each_char('0'..='9', |s| assert_eq!(Symbol.match_prefix(s), None));
        each_char(ASCII_SYMBOLS.chars(), |s| {
            assert_eq!(Symbol.match_prefix(s), Some(s))
        });
        assert_eq!(Symbol.match_prefix("? "), Some("?"));
        assert_eq!(Symbol.match_prefix("??"), Some("?"), "one symbol per token");
        assert_eq!(Symbol.match_prefix(" ?"), None);
    }

    #[test]
    fn white_space() {
        assert_eq!(WhiteSpace.match_prefix(" \t\n a"), Some(" \t\n "));
        assert_eq!(WhiteSpace.match_prefix("\u{00A0}a"), Some("\u{00A0}"));
        assert_eq!(WhiteSpace.match_prefix("a "), None);
    }

    #[test]
    fn line_termination() {
        for text in [" ", "  ", "  .", "  a", "  \n", "\t"] {
            assert_eq!(LineTermination.match_prefix(text), None, "{text:?}");
        }
        assert_eq!(LineTermination.match_prefix("\n  "), Some("\n"));
        assert_eq!(LineTermination.match_prefix("\n  \t\n"), Some("\n"));
        assert_eq!(LineTermination.match_prefix("\n\n\n  \t\n"), Some("\n\n\n"));
        assert_eq!(LineTermination.match_prefix("\r\nx"), Some("\r\n"));
        assert_eq!(
            LineTermination.match_prefix("\u{b}\u{c}x"),
            Some("\u{b}\u{c}")
        );
        // The letters v and f are not the controls \v and \f.
        assert_eq!(LineTermination.match_prefix("v"), None);
        assert_eq!(LineTermination.match_prefix("f"), None);
    }
}
