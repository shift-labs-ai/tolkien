//! Dates as people write them.

use std::sync::LazyLock;

use super::BASE;
use crate::chars::{is_comma, is_dash, is_full_stop, is_letter, is_slash, is_underscore};
use crate::cursor::Cursor;
use crate::matcher::Matcher;
use crate::trie::Trie;

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

fn is_numeric_separator(c: char) -> bool {
    c == ' ' || is_dash(c) || is_full_stop(c) || is_underscore(c) || is_slash(c)
}

fn is_separator(c: char) -> bool {
    is_numeric_separator(c) || is_comma(c)
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

/// A month name and the full stop of an abbreviation, if there is one.
fn month(cur: &mut Cursor<'_>) -> bool {
    MONTHS.advance_longest(cur) && {
        cur.advance_if(|c| c == '.');
        true
    }
}

fn numeric_date(cur: &mut Cursor<'_>) -> bool {
    let (first_len, first) = number(cur);
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
    let mut ahead = cur.clone();
    let goes_on = ahead.advance_if(|c| c == separator) && ahead.peek().is_some_and(is_ascii_digit);
    shaped && !goes_on
}

fn month_name_date(cur: &mut Cursor<'_>) -> bool {
    if month(cur) {
        // <month> <day> <year>, or <month> <six digits>
        cur.advance_while(is_separator);
        match cur.advance_while(is_ascii_digit) {
            0 => false,
            6 if !cur.peek().is_some_and(is_letter) => true,
            day => {
                if day <= 2 {
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
