//! Character classes.
//!
//! Every table behind these functions is generated from the Unicode
//! Character Database by `tools/gen_unicode.py`; `docs/charsets.md` lists the
//! members of each named set.

use crate::unicode_tables as tables;

pub use crate::unicode_tables::UNICODE_VERSION;

/// The classes of the code point `cp`, as a mask of the bits in the tables.
const fn lookup(cp: u32) -> u32 {
    let block = tables::BLOCK_OF[(cp >> 8) as usize] as usize;
    tables::CLASSES[tables::BLOCKS[block][(cp & 0xFF) as usize] as usize]
}

/// The classes of the ASCII characters, in one table: most text is ASCII.
const ASCII: [u32; 128] = {
    let mut classes = [0; 128];
    let mut cp = 0;
    while cp < 128 {
        classes[cp] = lookup(cp as u32);
        cp += 1;
    }
    classes
};

/// The classes of an ASCII character.
pub(crate) const fn ascii_classes(byte: u8) -> u32 {
    ASCII[byte as usize]
}

/// The classes of the punctuation written with numbers: full stops, commas,
/// percent and number signs, math symbols, dashes and connectors.
pub(crate) const NUMBER_PUNCTUATION: u32 = tables::FULL_STOP
    | tables::COMMA
    | tables::PERCENT_SIGN
    | tables::NUMBER_SIGN
    | tables::MATH_SYMBOL
    | tables::DASH_PUNCTUATION
    | tables::CONNECTOR_PUNCTUATION;

/// The classes of what may follow a word: white space and sentence
/// punctuation.
pub(crate) const WORD_TERMINATION: u32 = tables::WHITE_SPACE
    | tables::FULL_STOP
    | tables::COMMA
    | tables::COLON
    | tables::QUESTION_MARK
    | tables::EXCLAMATION_MARK;

/// The classes of `c`, as a mask of the bits in the tables.
#[inline]
pub(crate) fn classes(c: char) -> u32 {
    if c.is_ascii() {
        ASCII[c as usize]
    } else {
        lookup(c as u32)
    }
}

/// True when `c` is in at least one of the classes of `mask`.
#[inline]
pub(crate) fn is(c: char, mask: u32) -> bool {
    classes(c) & mask != 0
}

/// A letter: general category L.
#[inline]
pub fn is_letter(c: char) -> bool {
    is(c, tables::LETTER)
}

/// A decimal digit of any script: general category Nd.
#[inline]
pub fn is_digit(c: char) -> bool {
    is(c, tables::DECIMAL_DIGIT)
}

/// A letter or a decimal digit.
#[inline]
pub fn is_letter_or_digit(c: char) -> bool {
    is(c, tables::LETTER | tables::DECIMAL_DIGIT)
}

/// Any number character: general category N (digits, letter numbers such as
/// Roman numerals, and other numbers such as fractions).
#[inline]
pub fn is_number(c: char) -> bool {
    is(c, tables::NUMBER)
}

/// White space: the Unicode `White_Space` property.
#[inline]
pub fn is_whitespace(c: char) -> bool {
    is(c, tables::WHITE_SPACE)
}

/// Anything that is not a letter, a decimal digit or white space.
#[inline]
pub fn is_symbol(c: char) -> bool {
    !is(
        c,
        tables::LETTER | tables::DECIMAL_DIGIT | tables::WHITE_SPACE,
    )
}

/// General category Sm, such as `+`, `~` and `=`.
#[inline]
pub fn is_math_symbol(c: char) -> bool {
    is(c, tables::MATH_SYMBOL)
}

/// General category Pd, such as `-` and the en dash.
#[inline]
pub fn is_dash_punctuation(c: char) -> bool {
    is(c, tables::DASH_PUNCTUATION)
}

/// General category Pc, such as `_`.
#[inline]
pub fn is_connector_punctuation(c: char) -> bool {
    is(c, tables::CONNECTOR_PUNCTUATION)
}

/// General category Sc, such as `$` and the euro sign.
#[inline]
pub fn is_currency_symbol(c: char) -> bool {
    is(c, tables::CURRENCY_SYMBOL)
}

/// A full stop of any script or width.
#[inline]
pub fn is_full_stop(c: char) -> bool {
    is(c, tables::FULL_STOP)
}

/// A comma of any script or width.
#[inline]
pub fn is_comma(c: char) -> bool {
    is(c, tables::COMMA)
}

/// A colon of any script or width.
#[inline]
pub fn is_colon(c: char) -> bool {
    is(c, tables::COLON)
}

/// A question mark.
#[inline]
pub fn is_question_mark(c: char) -> bool {
    is(c, tables::QUESTION_MARK)
}

/// An exclamation mark.
#[inline]
pub fn is_exclamation_mark(c: char) -> bool {
    is(c, tables::EXCLAMATION_MARK)
}

/// A punctuation apostrophe that can stand inside a word, the typographic
/// one included. The modifier-letter apostrophes, such as U+02BC, are
/// [letters](is_letter).
#[inline]
pub fn is_apostrophe(c: char) -> bool {
    is(c, tables::APOSTROPHE)
}

/// A hyphen, dash or minus sign: the Unicode `Dash` property.
#[inline]
pub fn is_dash(c: char) -> bool {
    is(c, tables::DASH)
}

/// An underscore.
#[inline]
pub fn is_underscore(c: char) -> bool {
    is(c, tables::UNDERSCORE)
}

/// A slash or a backslash.
#[inline]
pub fn is_slash(c: char) -> bool {
    is(c, tables::SLASH)
}

/// A percent sign.
#[inline]
pub fn is_percent_sign(c: char) -> bool {
    is(c, tables::PERCENT_SIGN)
}

/// A number sign (`#`).
#[inline]
pub fn is_number_sign(c: char) -> bool {
    is(c, tables::NUMBER_SIGN)
}

/// Punctuation written with numbers: a full stop, a comma, a percent or
/// number sign, a math symbol, a dash or a connector.
#[inline]
pub fn is_number_punctuation(c: char) -> bool {
    is(c, NUMBER_PUNCTUATION)
}

/// A character that forces a line break.
#[inline]
pub fn is_line_termination(c: char) -> bool {
    is(c, tables::LINE_TERMINATION)
}

/// White space or sentence punctuation (`. , : ? !`): what may follow a word.
#[inline]
pub fn is_word_termination(c: char) -> bool {
    is(c, WORD_TERMINATION)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks one class of the lookup table against its members, given as
    /// sorted ranges, for every character.
    fn assert_class(name: &str, bit: u32, members: impl Iterator<Item = (u32, u32)>) {
        let mut members = members.peekable();
        assert!(members.peek().is_some(), "{name} is empty");
        for cp in 0..=0x10FFFF {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            // The members are sorted, so one walk over them serves every character.
            while members.next_if(|&(_, last)| last < cp).is_some() {}
            let expected = members.peek().is_some_and(|&(first, _)| first <= cp);
            if is(c, bit) != expected {
                panic!("{name} is wrong for {c:?}");
            }
        }
    }

    /// The lookup table against the ranges and lists it was built from, for
    /// every character and every class.
    #[test]
    fn lookup_table_agrees_with_its_source() {
        for (name, bit, ranges) in tables::source::RANGES {
            assert_class(name, *bit, ranges.iter().copied());
        }
        for (name, bit, list) in tables::source::LISTS {
            assert_class(name, *bit, list.iter().map(|&c| (c as u32, c as u32)));
        }
    }

    /// The ASCII shortcut against the full table.
    #[test]
    fn ascii_table_agrees_with_the_lookup_table() {
        for c in (0u8..0x80).map(char::from) {
            assert_eq!(classes(c), lookup(c as u32), "{c:?}");
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
