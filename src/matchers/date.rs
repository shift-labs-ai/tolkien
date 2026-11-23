//! Dates as people write them.

use std::sync::LazyLock;

use super::BASE;
use crate::chars::{
    is_colon, is_comma, is_dash, is_digit, is_full_stop, is_slash, is_underscore, is_whitespace,
};
use crate::cursor::Cursor;
use crate::matcher::Matcher;
use crate::trie::Trie;

/// Kind of [`Date`] tokens.
pub const DATE: &str = "DATE";

const MONTH_NAMES: &[&str] = &[
    "Jan",
    "January",
    "Feb",
    "February",
    "Mar",
    "March",
    "Apr",
    "April",
    "May",
    "Jun",
    "June",
    "Jul",
    "July",
    "Aug",
    "August",
    "Sep",
    "September",
    "Oct",
    "October",
    "Nov",
    "November",
    "Dec",
    "December",
];

/// The month names as written (`Feb`), in lower case and in upper case.
static MONTHS: LazyLock<Trie> = LazyLock::new(|| {
    let mut trie = Trie::new();
    for name in MONTH_NAMES {
        trie.insert(name);
        trie.insert(&name.to_lowercase());
        trie.insert(&name.to_uppercase());
    }
    trie
});

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
/// without separators. This form must end at white space, a full stop, a
/// comma, a colon or the end of the text:
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

fn is_termination(c: char) -> bool {
    is_whitespace(c) || is_full_stop(c) || is_comma(c) || is_colon(c)
}

fn at_end_or_termination(cur: &Cursor<'_>) -> bool {
    cur.peek().is_none_or(is_termination)
}

/// A run of digits. `None` when it is longer than `limit`.
fn number(cur: &mut Cursor<'_>, limit: usize) -> Option<usize> {
    let digits = cur.advance_while(is_digit);
    (digits <= limit).then_some(digits)
}

/// A month name and the full stop of an abbreviation, if there is one.
fn month(cur: &mut Cursor<'_>) -> bool {
    if MONTHS.advance_longest(cur).is_none() {
        return false;
    }
    cur.advance_if(|c| c == '.');
    true
}

fn numeric_date(cur: &mut Cursor<'_>) -> bool {
    // The length of the first number says where the year is.
    let (middle, last) = match number(cur, 4) {
        Some(4) => (1..=2, 1..=2),
        Some(1 | 2) => (1..=2, 1..=4),
        _ => return false,
    };
    if !cur.advance_if(is_separator) {
        return false;
    }
    if !number(cur, 2).is_some_and(|digits| middle.contains(&digits)) {
        return false;
    }
    if !cur.advance_if(is_separator) {
        return false;
    }
    number(cur, *last.end()).is_some_and(|digits| last.contains(&digits))
}

fn month_name_date(cur: &mut Cursor<'_>) -> bool {
    let Some(first) = cur.peek() else {
        return false;
    };
    if MONTHS.can_start(first) {
        // <month> <number> <number>, or <month><six digits>
        if !month(cur) {
            return false;
        }
        cur.advance_while(is_separator);
        let first_number = cur.advance_while(is_digit);
        if first_number == 0 {
            return false;
        }
        if first_number == 6 && at_end_or_termination(cur) {
            return true;
        }
        cur.advance_while(is_separator);
        cur.advance_while(is_digit) > 0 && at_end_or_termination(cur)
    } else if is_digit(first) {
        // <number> <month> <number>
        cur.advance_while(is_digit);
        cur.advance_while(is_separator);
        if !month(cur) {
            return false;
        }
        cur.advance_while(is_separator);
        cur.advance_while(is_digit) > 0 && at_end_or_termination(cur)
    } else {
        false
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
            "",
        ] {
            assert_eq!(Date.match_prefix(text), None, "{text:?}");
        }
    }

    #[test]
    fn a_month_name_must_be_whole() {
        // In fastlex any prefix of a month name counted as the month.
        for text in ["Fe 17, 2009", "F 17, 2009", "17 Fe 2009", "Febr 17, 2009"] {
            assert_eq!(Date.match_prefix(text), None, "{text:?}");
        }
    }
}
