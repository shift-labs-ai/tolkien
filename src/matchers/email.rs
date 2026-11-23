//! Email addresses.

use super::BASE;
use crate::cursor::Cursor;
use crate::matcher::Matcher;

/// Kind of [`LikeEmail`] tokens.
pub const LIKE_EMAIL: &str = "LIKE_EMAIL";

/// Something shaped like an email address: `local@domain.tld`.
///
/// The rules are the simple ones spaCy uses, not the full grammar of RFC
/// 5322. The local part is ASCII letters, digits and `_ - . +`; the domain is
/// ASCII letters, digits and `-`; the part after its first dot is ASCII
/// letters, digits, `-` and `.`. Each part needs at least one letter or
/// digit. A dot or dash at the very end is left out, so an address that ends
/// a sentence does not take the full stop with it.
pub struct LikeEmail;

fn is_local_symbol(c: char) -> bool {
    matches!(c, '_' | '-' | '.' | '+')
}

fn is_tld_symbol(c: char) -> bool {
    matches!(c, '-' | '.')
}

fn is_ascii_letter_or_digit(c: char) -> bool {
    c.is_ascii_alphanumeric()
}

impl Matcher for LikeEmail {
    fn kind(&self) -> &'static str {
        LIKE_EMAIL
    }

    fn overrides(&self) -> &'static [&'static str] {
        BASE
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        let (_, local) = cur.advance_while2(is_local_symbol, is_ascii_letter_or_digit);
        if local == 0 || !cur.advance_if(|c| c == '@') {
            return false;
        }
        let (_, domain) = cur.advance_while2(|c| c == '-', is_ascii_letter_or_digit);
        if domain == 0 || !cur.advance_if(|c| c == '.') {
            return false;
        }
        let (_, tld) = cur.advance_while2(is_tld_symbol, is_ascii_letter_or_digit);
        if tld == 0 {
            return false;
        }
        while cur.prev().is_some_and(is_tld_symbol) {
            cur.back();
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_addresses() {
        for address in [
            "prettyandsimple@example.com",
            "PRETTYANDSIMPLE@EXAMPLE.COM",
            "very.common@example.com",
            "disposable.style.email.with+symbol@example.com",
            "other.email-with-dash@example.com",
            "other.email_with-dash@example.com",
            "other.email_with-dash@exa-mple.com",
            "x@example.com",
            "example-indeed@strange-example.com",
            "dana@mail.example.co.uk",
        ] {
            assert_eq!(LikeEmail.match_prefix(address), Some(address));
        }
        assert_eq!(
            LikeEmail.match_prefix("prettyandsimple@example.com "),
            Some("prettyandsimple@example.com")
        );
    }

    #[test]
    fn leaves_out_the_punctuation_after_an_address() {
        assert_eq!(
            LikeEmail.match_prefix("dana@acme.com."),
            Some("dana@acme.com")
        );
        assert_eq!(
            LikeEmail.match_prefix("dana@acme.com, and"),
            Some("dana@acme.com")
        );
        assert_eq!(
            LikeEmail.match_prefix("dana@acme.com..."),
            Some("dana@acme.com")
        );
    }

    #[test]
    fn rejects_text_that_is_not_an_address() {
        for text in [
            "\"prettyandsimple@example.com",
            "prettyandsimple.@example",
            "prettyandsimple@example",
            "prettyandsimple@example...",
            "prettyand",
            "prettyandsimple@examp_le.c",
            "prettyandsimple\"@example.com",
            "prettyand\"simple@example.com",
            "prettyandsimple.example.co_m",
            "@example.com",
            "",
        ] {
            assert_eq!(LikeEmail.match_prefix(text), None, "{text:?}");
        }
    }
}
