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
/// digit, and the local part starts with one and has at most 64 characters.
/// A dot or dash at the very end is left out, so an address that ends a
/// sentence does not take the full stop with it.
pub struct LikeEmail;

/// The longest local part RFC 5321 allows. The limit also bounds the work on
/// a long run of local-part characters that holds no `@`.
const MAX_LOCAL_PART: usize = 64;

/// A byte of the local part after its first: an ASCII letter or digit, or
/// one of `_ - . +`.
fn is_local_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'+')
}

/// The length of the local part at the start of `bytes`, when one stands
/// there and an `@` follows it.
pub(crate) fn local_part(bytes: &[u8]) -> Option<usize> {
    if !bytes.first().is_some_and(u8::is_ascii_alphanumeric) {
        return None;
    }
    let local = bytes
        .iter()
        .take(MAX_LOCAL_PART)
        .take_while(|&&byte| is_local_byte(byte))
        .count();
    (bytes.get(local) == Some(&b'@')).then_some(local)
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

    fn can_start(&self, first: char) -> bool {
        is_ascii_letter_or_digit(first)
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        // The local part is ASCII, so it is read as bytes: every word of a
        // text comes through here, and almost none is an address.
        let Some(local) = local_part(cur.rest().as_bytes()) else {
            return false;
        };
        cur.set_pos(cur.pos() + local + 1);
        let (_, domain) = cur.advance_while2(|c| c == '-', is_ascii_letter_or_digit);
        if domain == 0 || !cur.advance_if(|c| c == '.') {
            return false;
        }
        let (_, tld) = cur.advance_while2(is_tld_symbol, is_ascii_letter_or_digit);
        if tld == 0 {
            return false;
        }
        cur.back_while(is_tld_symbol);
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
    fn local_part_has_at_most_64_characters() {
        let longest = format!("{}@example.com", "a".repeat(64));
        assert_eq!(LikeEmail.match_prefix(&longest), Some(longest.as_str()));
        let too_long = format!("{}@example.com", "a".repeat(65));
        assert_eq!(LikeEmail.match_prefix(&too_long), None);
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
            ".dana@example.com",
            "_@example.com",
            "",
        ] {
            assert_eq!(LikeEmail.match_prefix(text), None, "{text:?}");
        }
    }
}
