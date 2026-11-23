//! Dates as people write them.

use std::sync::LazyLock;

use super::BASE;
use crate::chars::{is_comma, is_dash, is_digit, is_full_stop, is_letter, is_slash, is_underscore};
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
/// Numbers only, with one separator between the parts. The year has up to
/// four digits and stands first or last:
///
/// ```text
/// 02/17/2009   17.02.2009   2009-02-17   2009 2 17   02_17_2009
/// ```
///
/// With an English month name, full or abbreviated, in any order and with or
/// without separators. In this form no letter may follow the last number:
///
/// ```text
/// February 17, 2009   17 Feb, 2009   2009, Feb 17   17Feb2009   Feb172009
/// ```
///
/// Separators are dashes, full stops, commas, underscores, slashes and the
/// space. Only the shape is checked: `99/99/9999` passes, and so does a
/// version number such as `1.2.3`.
pub struct Date;

fn is_separator(c: char) -> bool {
    c == ' ' || is_dash(c) || is_full_stop(c) || is_comma(c) || is_underscore(c) || is_slash(c)
}

/// Between one and `limit` digits.
fn number(cur: &mut Cursor<'_>, limit: usize) -> bool {
    (1..=limit).contains(&cur.advance_while(is_digit))
}

/// The number that ends a date with a month name: digits that no letter
/// follows, so that `3 May 5pm` is not a date.
fn last_number(cur: &mut Cursor<'_>) -> bool {
    cur.advance_while(is_digit) > 0 && !cur.peek().is_some_and(is_letter)
}

/// A month name and the full stop of an abbreviation, if there is one.
fn month(cur: &mut Cursor<'_>) -> bool {
    MONTHS.advance_longest(cur) && {
        cur.advance_if(|c| c == '.');
        true
    }
}

fn numeric_date(cur: &mut Cursor<'_>) -> bool {
    // The length of the first number says where the year is: a year first
    // leaves two digits for the last number, a day or month first leaves four.
    let last = match cur.advance_while(is_digit) {
        4 => 2,
        1 | 2 => 4,
        _ => return false,
    };
    cur.advance_if(is_separator)
        && number(cur, 2)
        && cur.advance_if(is_separator)
        && number(cur, last)
}

fn month_name_date(cur: &mut Cursor<'_>) -> bool {
    if month(cur) {
        // <month> <number> <number>, or <month> <six digits>
        cur.advance_while(is_separator);
        match cur.advance_while(is_digit) {
            0 => false,
            6 if !cur.peek().is_some_and(is_letter) => true,
            _ => {
                cur.advance_while(is_separator);
                last_number(cur)
            }
        }
    } else {
        // <number> <month> <number>
        cur.advance_while(is_digit) > 0
            && {
                cur.advance_while(is_separator);
                month(cur)
            }
            && {
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
