//! Amounts of money.

use std::sync::LazyLock;

use super::currency_data::{NAMES, SYMBOLS};
use super::BASE_AND_NUMBER;
use crate::chars::{
    is_apostrophe, is_comma, is_currency_symbol, is_digit, is_full_stop, is_letter_or_digit,
    is_whitespace,
};
use crate::cursor::Cursor;
use crate::matcher::Matcher;
use crate::trie::Trie;

/// Kind of [`Currency`] tokens.
pub const CURRENCY: &str = "CURRENCY";

/// The names in their common cases (`Dollars`, `dollars`, `DOLLARS`) and the
/// codes and symbols only as they are written (`USD`, `kr`): in lower case a
/// code is too often a word, as `ALL`, `TRY` and `PEN` are.
static CURRENCIES: LazyLock<Trie> = LazyLock::new(|| {
    let mut trie = Trie::from_words_in_common_cases(NAMES);
    for symbol in SYMBOLS {
        trie.insert(symbol);
    }
    trie
});

/// An amount of money, in one of two forms.
///
/// Currency symbols first, then an optional amount: `$5`, `€1,000.50`, `$$$`.
/// The symbols are the characters of Unicode category Sc.
///
/// An amount first, then an optional white space character and a currency
/// name, code or abbreviation: `5 USD`, `5.4 Dollars`, `5,000 cents`. No
/// letter, digit or apostrophe may follow it, so `5 Centsless`, the start of
/// `3L7gwx` and `94 won't` do not match.
///
/// An amount is digits with full stops and commas between them, and may
/// start with a full stop: `.50`.
///
/// A name is matched in its
/// [common cases](Trie::from_words_in_common_cases), a code or abbreviation
/// only as it is written. The list is long, so some entries still collide
/// with other words: `5 K` (kina) and `21 para` (a subunit of the dinar)
/// match.
pub struct Currency;

fn is_amount_punctuation(c: char) -> bool {
    is_full_stop(c) || is_comma(c)
}

fn symbol_first(cur: &mut Cursor<'_>) -> bool {
    if cur.advance_while(is_currency_symbol) == 0 {
        return false;
    }
    let after_symbol = cur.pos();
    if !amount(cur) {
        cur.set_pos(after_symbol);
    }
    true
}

/// Digits with full stops and commas between them, after an optional full
/// stop. Punctuation after the last digit is left out: "$5." is "$5" and the
/// full stop of a sentence.
fn amount(cur: &mut Cursor<'_>) -> bool {
    cur.advance_if(is_full_stop);
    if !cur.advance_if(is_digit) {
        return false;
    }
    cur.advance_while2(is_amount_punctuation, is_digit);
    cur.back_while(is_amount_punctuation);
    true
}

/// What follows an amount that ends at the cursor: an optional white space
/// character and a currency name, code or abbreviation that no letter,
/// digit or apostrophe follows.
pub(crate) fn name_after_amount(cur: &mut Cursor<'_>) -> bool {
    cur.advance_if(is_whitespace);
    CURRENCIES.advance_longest(cur)
        && !cur
            .peek()
            .is_some_and(|c| is_letter_or_digit(c) || is_apostrophe(c))
}

fn amount_first(cur: &mut Cursor<'_>) -> bool {
    amount(cur) && name_after_amount(cur)
}

impl Matcher for Currency {
    fn kind(&self) -> &'static str {
        CURRENCY
    }

    fn overrides(&self) -> &'static [&'static str] {
        BASE_AND_NUMBER
    }

    fn can_start(&self, first: char) -> bool {
        is_currency_symbol(first) || is_digit(first) || is_full_stop(first)
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
        assert_eq!(Currency.match_prefix(".5 USD"), Some(".5 USD"));
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
            ("$5...", "$5"),
            ("$.5", "$.5"),
            ("$..5", "$"),
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
            "..5 USD",
            "5. USD",
            "5 Abkhazia",
            "5 usd",
            "3L7gwxiygty",
            "94 won't",
            "5 km",
            "5 ft",
            "5 all",
            "5 try",
            "2005 p. 805",
            "",
        ] {
            assert_eq!(Currency.match_prefix(text), None, "{text:?}");
        }
    }

    #[test]
    fn a_currency_name_must_be_whole() {
        for text in ["5 Dol", "5 Doll.", "5 U", "5 US "] {
            assert_eq!(Currency.match_prefix(text), None, "{text:?}");
        }
    }

    /// A known weak spot, pinned so that a change to it is a decision: some
    /// abbreviations and names are also other words.
    #[test]
    fn some_abbreviations_and_names_collide_with_other_words() {
        for (text, expected) in [("5 K", "5 K"), ("21 para. 6", "21 para"), ("5 kr", "5 kr")] {
            assert_eq!(Currency.match_prefix(text), Some(expected));
        }
    }
}
