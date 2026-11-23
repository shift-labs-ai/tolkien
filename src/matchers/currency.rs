//! Amounts of money.

use std::sync::LazyLock;

use super::currency_data::{NAMES, SYMBOLS};
use super::BASE_AND_NUMBER;
use crate::chars::{
    is_comma, is_currency_symbol, is_digit, is_full_stop, is_letter, is_whitespace,
};
use crate::cursor::Cursor;
use crate::matcher::Matcher;
use crate::trie::Trie;

/// Kind of [`Currency`] tokens.
pub const CURRENCY: &str = "CURRENCY";

/// Every name and symbol as written, in lower case, in upper case, and with
/// its first letter in upper case.
static CURRENCIES: LazyLock<Trie> = LazyLock::new(|| {
    let mut trie = Trie::new();
    for word in NAMES.iter().chain(SYMBOLS) {
        trie.insert(word);
        trie.insert(&word.to_lowercase());
        trie.insert(&word.to_uppercase());
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            let capitalised: String = first.to_uppercase().chain(chars).collect();
            trie.insert(&capitalised);
        }
    }
    trie
});

/// An amount of money, in one of two forms.
///
/// Currency symbols first, then an optional amount: `$5`, `€1,000.50`, `$$$`.
/// The symbols are the characters of Unicode category Sc.
///
/// An amount first, then an optional white space character and a currency
/// name, code or abbreviation that is not followed by a letter: `5 USD`,
/// `5.4 Dollars`, `5,000 cents`.
///
/// The list of names and abbreviations is long and is matched in any of
/// three cases, so short ones collide with other words: `5 km` (convertible
/// marks) and `5 ft` (forints) match.
pub struct Currency;

fn is_amount_punctuation(c: char) -> bool {
    is_full_stop(c) || is_comma(c)
}

fn symbol_first(cur: &mut Cursor<'_>) -> bool {
    if cur.advance_while(is_currency_symbol) == 0 {
        return false;
    }
    let after_symbol = cur.pos();
    let (_, digits) = cur.advance_while2(is_amount_punctuation, is_digit);
    if digits == 0 {
        cur.set_pos(after_symbol);
    } else if cur.prev().is_some_and(is_amount_punctuation) {
        // "$5." is "$5" and the full stop of a sentence.
        cur.back();
    }
    true
}

fn amount_first(cur: &mut Cursor<'_>) -> bool {
    let (_, digits) = cur.advance_while2(is_amount_punctuation, is_digit);
    if digits == 0 || cur.prev().is_some_and(is_amount_punctuation) {
        return false;
    }
    cur.advance_if(is_whitespace);
    CURRENCIES.advance_longest(cur).is_some() && !cur.peek().is_some_and(is_letter)
}

impl Matcher for Currency {
    fn kind(&self) -> &'static str {
        CURRENCY
    }

    fn overrides(&self) -> &'static [&'static str] {
        BASE_AND_NUMBER
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        let start = cur.pos();
        if symbol_first(cur) {
            return true;
        }
        cur.set_pos(start);
        amount_first(cur)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_an_amount_before_a_currency() {
        for text in [
            "5 Cents",
            "5 cents",
            "5 CENTS",
            "5 USD",
            "5 Dollars",
            "5.4 Dollars",
            "5,000 Dollars",
        ] {
            assert_eq!(Currency.match_prefix(text), Some(text));
        }
        assert_eq!(Currency.match_prefix("5USD."), Some("5USD"));
        assert_eq!(
            Currency.match_prefix("1,000.50 euros, please"),
            Some("1,000.50 euros")
        );
        assert_eq!(Currency.match_prefix("20 ₪"), Some("20 ₪"));
        assert_eq!(
            Currency.match_prefix("5 Convertible Mark"),
            Some("5 Convertible Mark")
        );
    }

    #[test]
    fn matches_a_symbol_before_an_amount() {
        for (text, expected) in [
            ("$$$", "$$$"),
            ("$5", "$5"),
            ("$5.", "$5"),
            ("€1,000.50 total", "€1,000.50"),
            ("$ 5", "$"),
            ("₪20,", "₪20"),
        ] {
            assert_eq!(Currency.match_prefix(text), Some(expected), "{text:?}");
        }
    }

    #[test]
    fn rejects_text_that_is_not_an_amount() {
        for text in [
            "5",
            "5 apples",
            "5.",
            "Dollars",
            "USD 5",
            "5 Centsless",
            "5 Dollarsign",
            "",
        ] {
            assert_eq!(Currency.match_prefix(text), None, "{text:?}");
        }
    }

    #[test]
    fn a_currency_name_must_be_whole() {
        // In fastlex any prefix of a name counted as the name.
        for text in ["5 Dol", "5 Doll.", "5 U", "5 US "] {
            assert_eq!(Currency.match_prefix(text), None, "{text:?}");
        }
    }

    /// Short abbreviations collide with units and words. This records the
    /// behaviour taken over from fastlex; it is not a goal.
    #[test]
    fn short_abbreviations_collide_with_other_words() {
        assert_eq!(Currency.match_prefix("5 km"), Some("5 km"));
        assert_eq!(Currency.match_prefix("5 ft"), Some("5 ft"));
    }
}
