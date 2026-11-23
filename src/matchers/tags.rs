//! Hashtags and mentions.

use super::BASE_AND_NUMBER;
use crate::chars::is_letter_or_digit;
use crate::cursor::Cursor;
use crate::matcher::Matcher;

/// Kind of [`Hashtag`] tokens.
pub const HASHTAG: &str = "HASHTAG";
/// Kind of [`Mention`] tokens.
pub const MENTION: &str = "MENTION";

/// `sigil`, then letters, digits and underscores with at least one letter or
/// digit.
fn tag(cur: &mut Cursor<'_>, sigil: char) -> bool {
    if !cur.advance_if(|c| c == sigil) {
        return false;
    }
    let (_, letters_or_digits) = cur.advance_while2(|c| c == '_', is_letter_or_digit);
    letters_or_digits > 0
}

/// `#hashtag`.
pub struct Hashtag;

impl Matcher for Hashtag {
    fn kind(&self) -> &'static str {
        HASHTAG
    }

    fn overrides(&self) -> &'static [&'static str] {
        BASE_AND_NUMBER
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        tag(cur, '#')
    }
}

/// `@mention`.
pub struct Mention;

impl Matcher for Mention {
    fn kind(&self) -> &'static str {
        MENTION
    }

    fn overrides(&self) -> &'static [&'static str] {
        BASE_AND_NUMBER
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        tag(cur, '@')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashtag() {
        assert_eq!(Hashtag.match_prefix("#hashtag"), Some("#hashtag"));
        assert_eq!(Hashtag.match_prefix("#hash_tag"), Some("#hash_tag"));
        assert_eq!(Hashtag.match_prefix("@mention"), None);
        assert_eq!(Hashtag.match_prefix("#Hashtag#Hashtag"), Some("#Hashtag"));
        assert_eq!(Hashtag.match_prefix("#Hashtag #Hashtag"), Some("#Hashtag"));
        assert_eq!(Hashtag.match_prefix("#Hashtag!!"), Some("#Hashtag"));
        assert_eq!(Hashtag.match_prefix("#Hashtag123!!"), Some("#Hashtag123"));
        assert_eq!(Hashtag.match_prefix("#Hash...tag123!!"), Some("#Hash"));
        assert_eq!(Hashtag.match_prefix("#"), None);
        assert_eq!(Hashtag.match_prefix("#_"), None);
        assert_eq!(Hashtag.match_prefix("# tag"), None);
    }

    #[test]
    fn mention() {
        assert_eq!(Mention.match_prefix("@mention"), Some("@mention"));
        assert_eq!(Mention.match_prefix("@men_tion"), Some("@men_tion"));
        assert_eq!(Mention.match_prefix("#hashtag"), None);
        assert_eq!(Mention.match_prefix("@mention@mention"), Some("@mention"));
        assert_eq!(Mention.match_prefix("@mention @mention"), Some("@mention"));
        assert_eq!(Mention.match_prefix("@mention!!"), Some("@mention"));
        assert_eq!(Mention.match_prefix("@mention123!!"), Some("@mention123"));
        assert_eq!(Mention.match_prefix("@ment...ion123!!"), Some("@ment"));
        assert_eq!(Mention.match_prefix("@"), None);
    }
}
