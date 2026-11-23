//! Character classes.
//!
//! Every table behind these functions is generated from the Unicode
//! Character Database by `tools/gen_unicode.py`; `docs/charsets.md` lists the
//! members of each named set.

use std::cmp::Ordering;

use crate::unicode_tables as tables;

pub use crate::unicode_tables::UNICODE_VERSION;

fn in_ranges(table: &[(u32, u32)], c: char) -> bool {
    let cp = c as u32;
    table
        .binary_search_by(|&(first, last)| {
            if last < cp {
                Ordering::Less
            } else if first > cp {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        })
        .is_ok()
}

fn in_set(set: &[char], c: char) -> bool {
    set.binary_search(&c).is_ok()
}

/// A letter: general category L.
#[inline]
pub fn is_letter(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_alphabetic()
    } else {
        in_ranges(tables::LETTER, c)
    }
}

/// A decimal digit of any script: general category Nd.
#[inline]
pub fn is_digit(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_digit()
    } else {
        in_ranges(tables::DECIMAL_DIGIT, c)
    }
}

/// A letter or a decimal digit.
#[inline]
pub fn is_letter_or_digit(c: char) -> bool {
    is_letter(c) || is_digit(c)
}

/// Any number character: general category N (digits, letter numbers such as
/// Roman numerals, and other numbers such as fractions).
#[inline]
pub fn is_number(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_digit()
    } else {
        in_ranges(tables::NUMBER, c)
    }
}

/// White space: the Unicode `White_Space` property.
#[inline]
pub fn is_whitespace(c: char) -> bool {
    if c.is_ascii() {
        matches!(c, ' ' | '\t'..='\r')
    } else {
        in_ranges(tables::WHITE_SPACE, c)
    }
}

/// Anything that is not a letter, a decimal digit or white space.
#[inline]
pub fn is_symbol(c: char) -> bool {
    !is_letter_or_digit(c) && !is_whitespace(c)
}

/// General category Sm, such as `+`, `~` and `=`.
pub fn is_math_symbol(c: char) -> bool {
    in_ranges(tables::MATH_SYMBOL, c)
}

/// General category Pd, such as `-` and the en dash.
pub fn is_dash_punctuation(c: char) -> bool {
    in_ranges(tables::DASH_PUNCTUATION, c)
}

/// General category Pc, such as `_`.
pub fn is_connector_punctuation(c: char) -> bool {
    in_ranges(tables::CONNECTOR_PUNCTUATION, c)
}

/// General category Sc, such as `$` and the euro sign.
pub fn is_currency_symbol(c: char) -> bool {
    in_ranges(tables::CURRENCY_SYMBOL, c)
}

/// A full stop of any script or width.
pub fn is_full_stop(c: char) -> bool {
    in_set(tables::FULL_STOP, c)
}

/// A comma of any script or width.
pub fn is_comma(c: char) -> bool {
    in_set(tables::COMMA, c)
}

/// A colon of any script or width.
pub fn is_colon(c: char) -> bool {
    in_set(tables::COLON, c)
}

/// A question mark.
pub fn is_question_mark(c: char) -> bool {
    in_set(tables::QUESTION_MARK, c)
}

/// An exclamation mark.
pub fn is_exclamation_mark(c: char) -> bool {
    in_set(tables::EXCLAMATION_MARK, c)
}

/// A punctuation apostrophe that can stand inside a word, the typographic
/// one included. The modifier-letter apostrophes, such as U+02BC, are
/// [letters](is_letter).
pub fn is_apostrophe(c: char) -> bool {
    in_set(tables::APOSTROPHE, c)
}

/// A hyphen, dash or minus sign: the Unicode `Dash` property.
pub fn is_dash(c: char) -> bool {
    in_set(tables::DASH, c)
}

/// An underscore.
pub fn is_underscore(c: char) -> bool {
    in_set(tables::UNDERSCORE, c)
}

/// A slash or a backslash.
pub fn is_slash(c: char) -> bool {
    in_set(tables::SLASH, c)
}

/// A percent sign.
pub fn is_percent_sign(c: char) -> bool {
    in_set(tables::PERCENT_SIGN, c)
}

/// A number sign (`#`).
pub fn is_number_sign(c: char) -> bool {
    in_set(tables::NUMBER_SIGN, c)
}

/// A character that forces a line break.
pub fn is_line_termination(c: char) -> bool {
    in_set(tables::LINE_TERMINATION, c)
}

/// White space or sentence punctuation (`. , : ? !`): what may follow a word.
pub fn is_word_termination(c: char) -> bool {
    is_whitespace(c)
        || is_full_stop(c)
        || is_comma(c)
        || is_colon(c)
        || is_question_mark(c)
        || is_exclamation_mark(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lookups are binary searches, so an unsorted table would answer
    /// wrongly without failing.
    #[test]
    fn generated_tables_are_sorted_and_not_empty() {
        for (name, table) in tables::RANGE_TABLES {
            assert!(!table.is_empty(), "{name} is empty");
            assert!(
                table.iter().all(|(first, last)| first <= last),
                "{name} has a reversed range"
            );
            assert!(
                table.windows(2).all(|w| w[0].1 < w[1].0),
                "{name} is not sorted"
            );
        }
        for (name, set) in tables::NAMED_SETS {
            assert!(!set.is_empty(), "{name} is empty");
            assert!(set.windows(2).all(|w| w[0] < w[1]), "{name} is not sorted");
        }
    }

    /// The ASCII shortcuts must agree with the tables they skip.
    #[test]
    fn ascii_shortcuts_agree_with_the_tables() {
        for c in (0u8..0x80).map(char::from) {
            assert_eq!(is_letter(c), in_ranges(tables::LETTER, c), "letter {c:?}");
            assert_eq!(
                is_digit(c),
                in_ranges(tables::DECIMAL_DIGIT, c),
                "digit {c:?}"
            );
            assert_eq!(is_number(c), in_ranges(tables::NUMBER, c), "number {c:?}");
            assert_eq!(
                is_whitespace(c),
                in_ranges(tables::WHITE_SPACE, c),
                "whitespace {c:?}"
            );
        }
    }

    #[test]
    fn classifies_characters_outside_ascii() {
        assert!(is_letter('é') && is_letter('ש') && is_letter('漢'));
        assert!(!is_letter('1') && !is_letter('_') && !is_letter('€'));
        assert!(is_digit('٣') && !is_digit('½') && !is_digit('Ⅷ'));
        assert!(is_number('½') && is_number('Ⅷ') && is_number('٣'));
        assert!(
            is_whitespace('\u{00A0}') && is_whitespace('\u{3000}') && !is_whitespace('\u{200B}')
        );
        assert!(is_symbol('€') && is_symbol('!') && !is_symbol('a') && !is_symbol(' '));
        assert!(is_currency_symbol('$') && is_currency_symbol('€') && is_currency_symbol('₪'));
        assert!(is_math_symbol('+') && is_math_symbol('~') && !is_math_symbol('-'));
        assert!(is_dash_punctuation('-') && is_dash_punctuation('–'));
        assert!(is_connector_punctuation('_'));
    }

    #[test]
    fn named_sets_hold_the_ascii_character_and_its_variants() {
        assert!(is_full_stop('.') && is_full_stop('。') && is_full_stop('．'));
        assert!(is_comma(',') && is_comma('،') && is_comma('、'));
        assert!(is_colon(':') && is_colon('：') && !is_colon(';'));
        assert!(is_question_mark('?') && is_question_mark('¿') && !is_question_mark('!'));
        assert!(is_exclamation_mark('!') && is_exclamation_mark('¡') && !is_exclamation_mark('?'));
        for both in ['‽', '⁈', '⁉'] {
            assert!(
                is_question_mark(both) && is_exclamation_mark(both),
                "{both}"
            );
        }
        assert!(is_apostrophe('\'') && is_apostrophe('’') && !is_apostrophe('»'));
        assert!(!is_apostrophe('ʼ') && is_letter('ʼ'));
        assert!(is_dash('-') && is_dash('—') && is_dash('−') && !is_dash('~'));
        assert!(is_underscore('_') && is_underscore('＿'));
        assert!(is_slash('/') && is_slash('\\') && is_slash('⁄'));
        assert!(is_percent_sign('%') && is_number_sign('#'));
        assert!(is_line_termination('\n') && is_line_termination('\r'));
        assert!(is_line_termination('\u{2028}') && !is_line_termination(' '));
        assert!(!is_line_termination('v') && !is_line_termination('f'));
    }

    #[test]
    fn word_termination_is_space_or_sentence_punctuation() {
        for c in [' ', '\n', '.', ',', ':', '?', '!'] {
            assert!(is_word_termination(c), "{c:?}");
        }
        for c in ['a', '1', ')', ';', '-', '@'] {
            assert!(!is_word_termination(c), "{c:?}");
        }
    }
}
