//! Dates as people write them.

use std::sync::LazyLock;

use super::BASE;
use crate::chars::{is, is_letter};
use crate::cursor::Cursor;
use crate::matcher::Matcher;
use crate::trie::Trie;
use crate::unicode_tables as tables;

/// Kind of [`Date`] tokens.
pub const DATE: &str = "DATE";

#[rustfmt::skip]
const MONTH_NAMES: &[&str] = &[
    "Jan", "January", "Feb", "February", "Mar", "March", "Apr", "April", "May", "Jun", "June",
    "Jul", "July", "Aug", "August", "Sep", "September", "Oct", "October", "Nov", "November",
    "Dec", "December",
];

static MONTHS: LazyLock<Trie> = LazyLock::new(|| Trie::from_words_in_common_cases(MONTH_NAMES));

/// A date with a day, a month and a year, in one of two forms.
///
/// Numbers only, with the same separator twice: a dash, a full stop, a slash
/// or an underscore. The year has four digits and stands first, or has two
/// or four and stands last. The month is 1 to 12 and the day 1 to 31, in
/// either order when the year is last. A space separates the parts only
/// when the year is first:
///
/// ```text
/// 02/17/2009   17.02.09   2009-2-17   2009 02 17   02_17_2009
/// ```
///
/// A version number or an IP address is not a date: `1.2.3` ends in one
/// digit, `2.9.165` in three, and `19.6.20.40` goes on after the third
/// number.
///
/// With an English month name, full or abbreviated, in any order and with or
/// without separators, which here include the comma and the space. A day may
/// carry its ordinal ending, and no letter may follow the last number:
///
/// ```text
/// February 17, 2009   17th Feb, 2009   2009, Feb 17   17Feb2009   Feb172009
/// ```
///
/// Digits are ASCII. In the second form only the shape is checked:
/// `99 Feb 9999` passes.
pub struct Date;

/// The classes of the marks that separate the numbers of a date: dashes,
/// full stops, underscores and slashes.
const NUMERIC_SEPARATORS: u32 =
    tables::DASH | tables::FULL_STOP | tables::UNDERSCORE | tables::SLASH;

fn is_numeric_separator(c: char) -> bool {
    c == ' ' || is(c, NUMERIC_SEPARATORS)
}

fn is_separator(c: char) -> bool {
    c == ' ' || is(c, NUMERIC_SEPARATORS | tables::COMMA)
}

fn is_ascii_digit(c: char) -> bool {
    c.is_ascii_digit()
}

/// A run of ASCII digits: how many there are, and their value. The value
/// stops growing at `u32::MAX`.
fn number(cur: &mut Cursor<'_>) -> (usize, u32) {
    let (mut count, mut value) = (0, 0u32);
    while let Some(digit) = cur.peek().and_then(|c| c.to_digit(10)) {
        count += 1;
        value = value.saturating_mul(10).saturating_add(digit);
        cur.advance();
    }
    (count, value)
}

/// The ordinal ending of a day (`st`, `nd`, `rd`, `th`), if one stands at
/// the cursor and no letter follows it.
fn ordinal_ending(cur: &mut Cursor<'_>) {
    let mut ahead = cur.clone();
    let mut letter = || ahead.advance().map(|c| c.to_ascii_lowercase());
    let ending = (letter(), letter());
    let known = [('s', 't'), ('n', 'd'), ('r', 'd'), ('t', 'h')]
        .iter()
        .any(|&(a, b)| ending == (Some(a), Some(b)));
    if known && !ahead.peek().is_some_and(is_letter) {
        cur.set_pos(ahead.pos());
    }
}

/// The number that ends a date with a month name: digits that no letter
/// follows, so that `3 May 5pm` is not a date.
fn last_number(cur: &mut Cursor<'_>) -> bool {
    cur.advance_while(is_ascii_digit) > 0 && !cur.peek().is_some_and(is_letter)
}

/// Three bytes as one number, with upper case letters made lower case.
const fn folded(bytes: [u8; 3]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]) | 0x0020_2020
}

/// The slot, of sixteen, that `multiplier` sends the three letters `folded`
/// to.
const fn slot(folded: u32, multiplier: u32) -> usize {
    (folded.wrapping_mul(multiplier) >> 28) as usize
}

/// The first three letters of every month name, each in a slot of its own.
struct MonthStarts {
    /// The number that sends the twelve to twelve different slots.
    multiplier: u32,
    /// The letters in each slot, [`folded`], or zero in an empty one.
    slots: [u32; 16],
}

const MONTH_STARTS: MonthStarts = {
    let mut multiplier = 1;
    loop {
        let mut slots = [0; 16];
        let mut name = 0;
        while name < MONTH_NAMES.len() {
            let [a, b, c, ..] = *MONTH_NAMES[name].as_bytes() else {
                panic!("a month name has three letters or more");
            };
            let start = folded([a, b, c]);
            let slot = slot(start, multiplier);
            if slots[slot] != 0 && slots[slot] != start {
                break;
            }
            slots[slot] = start;
            name += 1;
        }
        if name == MONTH_NAMES.len() {
            break MonthStarts { multiplier, slots };
        }
        multiplier += 1;
    }
};

/// True when `text` starts with the first three letters of a month name, in
/// any mix of upper and lower case.
///
/// Every word of a text is asked about, and what letters a word starts with
/// is not something a processor can guess. So the three bytes are tested
/// together, with no branch between them, and exactly: words that only
/// resemble a month, such as `many` or `section`, are too common to be
/// looked at any further.
#[inline]
fn may_start_with_month(text: &[u8]) -> bool {
    text.first_chunk::<3>().is_some_and(|&start| {
        let start = folded(start);
        MONTH_STARTS.slots[slot(start, MONTH_STARTS.multiplier)] == start
    })
}

/// True when the first `len` bytes of `text`, a run of ASCII letters, are a
/// month name in any mix of upper and lower case. Stricter than this,
/// [`Date`] takes a name only in one of its common cases.
#[inline]
pub(crate) fn starts_with_month(text: &[u8], len: usize) -> bool {
    may_start_with_month(text)
        && MONTH_NAMES
            .iter()
            .any(|name| name.as_bytes().eq_ignore_ascii_case(&text[..len]))
}

/// True when a month name may follow a number that ends where `bytes`
/// starts: after the separators there, the first three letters of one
/// stand, or a character outside ASCII, which may be a separator too.
pub(crate) fn month_may_follow(bytes: &[u8]) -> bool {
    let separators = bytes
        .iter()
        .take_while(|&&byte| byte.is_ascii() && is_separator(byte as char))
        .count();
    let rest = &bytes[separators..];
    rest.first().is_some_and(|byte| !byte.is_ascii()) || may_start_with_month(rest)
}

/// A month name and the full stop of an abbreviation, if there is one.
fn month(cur: &mut Cursor<'_>) -> bool {
    may_start_with_month(cur.rest().as_bytes()) && MONTHS.advance_longest(cur) && {
        cur.advance_if(|c| c == '.');
        true
    }
}

fn numeric_date(cur: &mut Cursor<'_>) -> bool {
    let (first_len, first) = number(cur);
    // A year has four digits, and a day or a month one or two.
    if !matches!(first_len, 1 | 2 | 4) {
        return false;
    }
    let Some(separator) = cur.peek().filter(|&c| is_numeric_separator(c)) else {
        return false;
    };
    cur.advance();
    let (second_len, second) = number(cur);
    if !cur.advance_if(|c| c == separator) {
        return false;
    }
    let (third_len, third) = number(cur);

    let short = |len: usize| (1..=2).contains(&len);
    let day = |n: u32| (1..=31).contains(&n);
    let shaped = match first_len {
        // Year, month, day.
        4 => short(second_len) && short(third_len) && (1..=12).contains(&second) && day(third),
        // Day and month in either order, then the year.
        1 | 2 => {
            separator != ' '
                && short(second_len)
                && matches!(third_len, 2 | 4)
                && day(first)
                && day(second)
                && first.min(second) <= 12
        }
        _ => false,
    };
    // In "19.6.20.40" the run goes on: it is an address, not a date and more.
    shaped && !cur.followed_by(|c| c == separator, is_ascii_digit)
}

fn month_name_date(cur: &mut Cursor<'_>) -> bool {
    if cur.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
        if !month(cur) {
            return false;
        }
        // <month> <day> <year>, or <month> <six digits>
        cur.advance_while(is_separator);
        match cur.advance_while(is_ascii_digit) {
            0 => false,
            6 if !cur.peek().is_some_and(is_letter) => true,
            digits => {
                if digits <= 2 {
                    ordinal_ending(cur);
                }
                cur.advance_while(is_separator);
                last_number(cur)
            }
        }
    } else {
        // <day or year> <month> <year or day>
        let first = cur.advance_while(is_ascii_digit);
        if first == 0 {
            return false;
        }
        if first <= 2 {
            ordinal_ending(cur);
        }
        cur.advance_while(is_separator);
        month(cur) && {
            cur.advance_while(is_separator);
            last_number(cur)
        }
    }
}

impl Matcher for Date {
    fn kind(&self) -> &'static str {
        DATE
    }

    fn overrides(&self) -> &'static [&'static str] {
        BASE
    }

    fn can_start(&self, first: char) -> bool {
        // The names are written with a capital.
        first.is_ascii_digit()
            || MONTH_NAMES
                .iter()
                .any(|name| name.starts_with(first.to_ascii_uppercase()))
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        let start = cur.pos();
        if numeric_date(cur) {
            return true;
        }
        cur.set_pos(start);
        month_name_date(cur)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_three_letters_tell_a_month_from_a_word_like_one() {
        for name in MONTH_NAMES {
            for name in [name.to_string(), name.to_lowercase(), name.to_uppercase()] {
                assert!(may_start_with_month(name.as_bytes()), "{name}");
            }
        }
        // Each has, place by place, a letter some month name has there.
        for word in [
            "many",
            "section",
            "not",
            "apply",
            "July"[..2].into(),
            "j\u{1}n",
            "JAN"[..2].into(),
        ] {
            assert!(!may_start_with_month(word.as_bytes()), "{word}");
        }
        // The same letters with another bit set are not letters.
        for start in [*b"\x0a\x01\x0e", *b"*!.", *b"ja\xee"] {
            assert!(!may_start_with_month(&start), "{start:?}");
        }
        assert!(may_start_with_month(b"decision"));
        assert!(!starts_with_month(b"decision", 8));
        assert!(starts_with_month(b"dEcEmBeR 1", 8));
    }

    #[test]
    fn matches_numeric_dates() {
        for (text, expected) in [
            ("02/17/2009", "02/17/2009"),
            ("2/17/2009", "2/17/2009"),
            ("02/17/2009 ", "02/17/2009"),
            ("02/17/2009!", "02/17/2009"),
            ("17/02/2009", "17/02/2009"),
            ("2009/02/17", "2009/02/17"),
            ("02_17_2009", "02_17_2009"),
            ("17_02_2009", "17_02_2009"),
            ("2009_02_17", "2009_02_17"),
            ("02.17.2009", "02.17.2009"),
            ("17.02.2009", "17.02.2009"),
            ("2009.02.17", "2009.02.17"),
            ("02-17-2009", "02-17-2009"),
            ("17-02-2009", "17-02-2009"),
            ("2009-02-17", "2009-02-17"),
            ("2009 02 17", "2009 02 17"),
            ("2009 2 17", "2009 2 17"),
            ("17–02–2009", "17–02–2009"),
            ("17.02.09", "17.02.09"),
            ("2009-2-17", "2009-2-17"),
            ("1/04/2023 9:33 AM", "1/04/2023"),
        ] {
            assert_eq!(Date.match_prefix(text), Some(expected), "{text:?}");
        }
    }

    #[test]
    fn rejects_incomplete_numeric_dates() {
        for text in [
            "02/17/",
            "2009-02!-17",
            "2009-02!",
            "2009-02-",
            "2009-02",
            "20091-02-17",
            "200-02-17",
            "2009-123-17",
            "1,000,000",
            "3.14",
            "10:30",
            // The two separators differ.
            "1/04 9",
            "7/16 15",
            "17/02-2009",
            // A space separates only when the year is first.
            "10 12 2014",
            // A comma never separates numbers.
            "3,4,15",
            // Version numbers and addresses.
            "1.2.3",
            "2.9.165",
            "10_7_2",
            "29.28.197.248",
            "19.6.20.40",
            // No month from 1 to 12, or no day from 1 to 31.
            "13/13/2009",
            "99/99/9999",
            "0/5/2009",
            "32/1/2009",
            "2009-13-01",
            "2009-01-32",
            // A year has two or four digits.
            "17/02/9",
            "17/02/200",
            // Other digits than ASCII ones.
            "٠٢/١٧/٢٠٠٩",
        ] {
            assert_eq!(Date.match_prefix(text), None, "{text:?}");
        }
    }

    #[test]
    fn matches_dates_with_a_month_name() {
        for (text, expected) in [
            ("February 17, 2009", "February 17, 2009"),
            ("17 February, 2009", "17 February, 2009"),
            ("2009, February 17", "2009, February 17"),
            ("Feb 17, 2009", "Feb 17, 2009"),
            ("2009, Feb 17", "2009, Feb 17"),
            ("Feb 17, 2014", "Feb 17, 2014"),
            ("17 Feb, 2014", "17 Feb, 2014"),
            ("2014, Feb 17", "2014, Feb 17"),
            ("2014_Feb_17", "2014_Feb_17"),
            ("2014-Feb-17", "2014-Feb-17"),
            ("Feb. 17, 2009", "Feb. 17, 2009"),
            ("FEBRUARY 17, 2009", "FEBRUARY 17, 2009"),
            ("february 17, 2009", "february 17, 2009"),
            ("Feb 17, 2009, at noon", "Feb 17, 2009"),
            ("Feb 17, 2009)", "Feb 17, 2009"),
            ("Feb 17, 2009!", "Feb 17, 2009"),
            ("17 Feb 2009; then", "17 Feb 2009"),
            ("17th February 2009", "17th February 2009"),
            ("1st May, 2009.", "1st May, 2009"),
            ("May 2ND, 2009", "May 2ND, 2009"),
            ("September 3rd 2009", "September 3rd 2009"),
            // without separators
            ("Feb172009", "Feb172009"),
            ("17Feb2009", "17Feb2009"),
            ("2009Feb17", "2009Feb17"),
        ] {
            assert_eq!(Date.match_prefix(text), Some(expected), "{text:?}");
        }
    }

    #[test]
    fn rejects_text_that_only_starts_like_a_date() {
        for text in [
            "Ebruary 17, 2009",
            "2014-Feb-abc",
            "February",
            "February, ",
            "February,",
            "February!",
            "February 17",
            "17 February",
            "May I",
            "march",
            "Feb 17, 2009th",
            "3 May 5pm",
            "Feb 123456x",
            "20th September",
            "17the Feb 2009",
            "2009th Feb 17",
            "Feb 17th",
            "",
        ] {
            assert_eq!(Date.match_prefix(text), None, "{text:?}");
        }
    }

    #[test]
    fn a_month_name_must_be_whole() {
        for text in ["Fe 17, 2009", "F 17, 2009", "17 Fe 2009", "Febr 17, 2009"] {
            assert_eq!(Date.match_prefix(text), None, "{text:?}");
        }
    }
}
