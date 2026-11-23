//! Web addresses.

use super::BASE;
use crate::chars::is_letter_or_digit;
use crate::cursor::Cursor;
use crate::matcher::Matcher;

/// Kind of [`LikeUrl`] tokens.
pub const LIKE_URL: &str = "LIKE_URL";

const PREFIXES: &[&str] = &["http://", "https://", "www."];

/// `http://`, `https://` or `www.` followed by at least one URL character.
///
/// URL characters are letters, digits and the punctuation RFC 3986 allows:
/// the reserved and unreserved characters and `%`. The run is not checked any
/// further. Sentence punctuation at its very end (`. , ; : ! ? '`) is left
/// out, so an address that ends a sentence does not take the full stop. So
/// is a closing bracket at the end that no bracket inside the address opens:
/// `(see https://example.com)` gives `https://example.com`, and
/// `https://example.com/a_(b)` keeps its bracket.
pub struct LikeUrl;

fn is_sentence_punctuation(c: char) -> bool {
    matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | '\'')
}

fn is_url_symbol(c: char) -> bool {
    matches!(
        c,
        // reserved
        '!' | '$' | '&' | '\'' | '(' | ')' | '*' | '+' | ',' | ';' | '=' | ':' | '/' | '?' | '#'
            | '[' | ']' | '@'
            // unreserved
            | '-' | '.' | '_' | '~'
            // percent-encoding
            | '%'
    )
}

impl Matcher for LikeUrl {
    fn kind(&self) -> &'static str {
        LIKE_URL
    }

    fn overrides(&self) -> &'static [&'static str] {
        BASE
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        if !PREFIXES.iter().any(|prefix| cur.advance_str(prefix)) {
            return false;
        }
        let after_prefix = cur.pos();
        cur.advance_while2(is_letter_or_digit, is_url_symbol);
        let run = cur.since(after_prefix);
        let mut unopened = [(')', '('), (']', '[')].map(|(close, open)| {
            (
                close,
                run.matches(close)
                    .count()
                    .saturating_sub(run.matches(open).count()),
            )
        });
        loop {
            cur.back_while(is_sentence_punctuation);
            let Some(slot) = unopened
                .iter_mut()
                .find(|(close, n)| *n > 0 && cur.prev() == Some(*close))
            else {
                break;
            };
            slot.1 -= 1;
            cur.back();
        }
        cur.pos() > after_prefix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_addresses() {
        for (text, expected) in [
            ("https://t.co/2Ecdop6zl1", "https://t.co/2Ecdop6zl1"),
            (
                "http://example.com/a?b=c&d=e#f rest",
                "http://example.com/a?b=c&d=e#f",
            ),
            ("https://example.com/a%20b", "https://example.com/a%20b"),
            ("www.example.com", "www.example.com"),
            (
                "https://he.wikipedia.org/wiki/שלום \"",
                "https://he.wikipedia.org/wiki/שלום",
            ),
            ("www.example.com.", "www.example.com"),
            ("https://example.com/a, and", "https://example.com/a"),
            ("https://example.com/a/?!", "https://example.com/a/"),
            ("https://example.com/a_(b)", "https://example.com/a_(b)"),
            ("https://example.com) by", "https://example.com"),
            ("https://example.com).", "https://example.com"),
            ("https://example.com/a_(b)))", "https://example.com/a_(b)"),
            ("https://example.com/x]", "https://example.com/x"),
            ("https://example.com/[x]", "https://example.com/[x]"),
            ("https://example.com/a)b", "https://example.com/a)b"),
        ] {
            assert_eq!(LikeUrl.match_prefix(text), Some(expected), "{text:?}");
        }
    }

    #[test]
    fn needs_a_known_prefix_and_something_after_it() {
        for text in [
            "hello",
            "http",
            "http:/x",
            "ftp://x",
            "https://",
            "https://.",
            "https://)",
            "http://:",
            "www.",
            "www..",
            "ww.x",
            ".",
            " ",
            "",
        ] {
            assert_eq!(LikeUrl.match_prefix(text), None, "{text:?}");
        }
    }
}
