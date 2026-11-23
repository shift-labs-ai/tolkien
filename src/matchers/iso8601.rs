//! ISO 8601 dates and date-times.

use super::BASE;
use crate::cursor::Cursor;
use crate::matcher::Matcher;

/// Kind of [`Iso8601`] tokens.
pub const DATE_ISO_8601: &str = "DATE_ISO_8601";

/// A complete ISO 8601 date, `YYYY-MM-DD`, and as much of a time after it as
/// is well formed:
///
/// ```text
/// 2022-01-06
/// 2022-01-06T08
/// 2022-01-06T08:42
/// 2022-01-06T08:42:27
/// 2022-01-06T08:42:27.549090
/// 2022-01-06T08:42:27.549090Z
/// 2022-01-06T08:42:27.549090+02:00
/// ```
///
/// A space may stand in place of the `T`. The time zone is read only after
/// the seconds. The match stops before the first part that is incomplete, so
/// `2022-01-06T08:` gives `2022-01-06T08`. Only the shape is checked: month
/// 13 passes. Digits are ASCII.
pub struct Iso8601;

/// Exactly `n` digits.
fn digits(cur: &mut Cursor<'_>, n: usize) -> bool {
    (0..n).all(|_| cur.advance_if(|c| c.is_ascii_digit()))
}

fn one(cur: &mut Cursor<'_>, expected: char) -> bool {
    cur.advance_if(|c| c == expected)
}

impl Matcher for Iso8601 {
    fn kind(&self) -> &'static str {
        DATE_ISO_8601
    }

    fn overrides(&self) -> &'static [&'static str] {
        BASE
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        // YYYY-MM-DD
        if !(digits(cur, 4) && one(cur, '-') && digits(cur, 2) && one(cur, '-') && digits(cur, 2)) {
            return false;
        }
        // From here on the date has matched. Each further part is taken only
        // if it is complete; otherwise the match ends before it.
        let mut end = cur.pos();
        let complete = 'time: {
            // Thh
            if !(cur.advance_if(|c| c == 'T' || c == ' ') && digits(cur, 2)) {
                break 'time false;
            }
            end = cur.pos();
            // :mm
            if !(one(cur, ':') && digits(cur, 2)) {
                break 'time false;
            }
            end = cur.pos();
            // :ss
            if !(one(cur, ':') && digits(cur, 2)) {
                break 'time false;
            }
            end = cur.pos();
            // .s
            if one(cur, '.') {
                if cur.advance_while(|c| c.is_ascii_digit()) == 0 {
                    break 'time false;
                }
                end = cur.pos();
            }
            // Z or +hh or +hh:mm
            if one(cur, 'Z') {
                break 'time true;
            }
            if !(cur.advance_if(|c| c == '+' || c == '-') && digits(cur, 2)) {
                break 'time false;
            }
            end = cur.pos();
            one(cur, ':') && digits(cur, 2)
        };
        if !complete {
            cur.set_pos(end);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_dates_and_the_complete_part_of_a_time() {
        for (text, expected) in [
            ("2022-01-06", "2022-01-06"),
            ("2022-01-06 ", "2022-01-06"),
            ("2022-01-06T", "2022-01-06"),
            ("2022-01-06T ", "2022-01-06"),
            ("2022-01-06T08", "2022-01-06T08"),
            ("2022-01-06T08:", "2022-01-06T08"),
            ("2022-01-06T08:42", "2022-01-06T08:42"),
            ("2022-01-06T08:42:", "2022-01-06T08:42"),
            ("2022-01-06T08:42:27", "2022-01-06T08:42:27"),
            ("2022-01-06T08:42:27.", "2022-01-06T08:42:27"),
            ("2022-01-06T08:42:27.549090", "2022-01-06T08:42:27.549090"),
            ("2022-01-06 08:42:27.549090", "2022-01-06 08:42:27.549090"),
            ("2022-01-06 08:42:27.549090+", "2022-01-06 08:42:27.549090"),
            ("2022-01-06 08:42:27.549090-", "2022-01-06 08:42:27.549090"),
            (
                "2022-01-06 08:42:27.549090+02",
                "2022-01-06 08:42:27.549090+02",
            ),
            (
                "2022-01-06 08:42:27.549090+02:02",
                "2022-01-06 08:42:27.549090+02:02",
            ),
            (
                "2022-01-06 08:42:27.549090-02:02",
                "2022-01-06 08:42:27.549090-02:02",
            ),
            ("2022-01-06 08:42:27.549090-a", "2022-01-06 08:42:27.549090"),
            ("2022-01-06 08:42:27+ ", "2022-01-06 08:42:27"),
            ("2022-01-06 08:42:27+-", "2022-01-06 08:42:27"),
            ("2022-01-06T08:42:27Z.", "2022-01-06T08:42:27Z"),
            ("2022-01-06T08:42:27+02:0", "2022-01-06T08:42:27+02"),
            ("2022-01-06 is a date", "2022-01-06"),
        ] {
            assert_eq!(Iso8601.match_prefix(text), Some(expected), "{text:?}");
        }
    }

    #[test]
    fn rejects_incomplete_dates() {
        for text in [
            "2022",
            "2022-01",
            "2022-01-0",
            "2022/01/06",
            "22-01-06",
            "x2022-01-06",
            "",
        ] {
            assert_eq!(Iso8601.match_prefix(text), None, "{text:?}");
        }
    }
}
