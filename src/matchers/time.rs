//! Times of day.

use super::BASE_AND_NUMBER;
use crate::chars::is_letter;
use crate::cursor::Cursor;
use crate::matcher::Matcher;

/// Kind of [`Time`] tokens.
pub const TIME: &str = "TIME";

/// A time of day: `H:MM` or `HH:MM`, then optional `:SS`, then an optional
/// AM/PM marker: `7:20`, `04:01:30`, `8:05 PM`, `04:01am`, `4:01 a.m.`
///
/// The hour is 0 to 24, and the minutes and seconds are 00 to 59. Digits are
/// ASCII.
pub struct Time;

/// Two digits from 00 to 59.
fn sixty(cur: &mut Cursor<'_>) -> bool {
    cur.advance_if(|c| matches!(c, '0'..='5')) && cur.advance_if(|c| c.is_ascii_digit())
}

impl Matcher for Time {
    fn kind(&self) -> &'static str {
        TIME
    }

    fn overrides(&self) -> &'static [&'static str] {
        BASE_AND_NUMBER
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        // H or HH
        let Some(first) = cur.peek().filter(|c| c.is_ascii_digit()) else {
            return false;
        };
        cur.advance();
        let second = cur.peek().filter(|c| c.is_ascii_digit());
        if second.is_some() {
            cur.advance();
        }
        match (first, second) {
            ('2', Some('5'..='9')) | ('3'..='9', Some(_)) => return false,
            _ => {}
        }
        // :MM
        if !(cur.advance_if(|c| c == ':') && sixty(cur)) {
            return false;
        }
        // :SS, or a colon that belongs to what follows
        let after_minutes = cur.pos();
        if !(cur.advance_if(|c| c == ':') && sixty(cur)) {
            cur.set_pos(after_minutes);
        }
        // AM, PM, a.m., p.m., with or without a space before
        let after_time = cur.pos();
        cur.advance_if(|c| c == ' ');
        let marker = cur.advance_if(|c| matches!(c, 'a' | 'A' | 'p' | 'P')) && {
            let dotted = cur.advance_if(|c| c == '.');
            let complete = cur.advance_if(|c| matches!(c, 'm' | 'M'));
            if complete && dotted {
                cur.advance_if(|c| c == '.');
            }
            complete
        };
        // "10:30 Amsterdam" has no marker: one must end where its word ends.
        if !marker || cur.peek().is_some_and(is_letter) {
            cur.set_pos(after_time);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_times() {
        for (text, expected) in [
            ("1:26", "1:26"),
            ("7:20:", "7:20"),
            ("7:20", "7:20"),
            ("8:05 PM-8:55 PM", "8:05 PM"),
            ("2:30-5:30", "2:30"),
            ("5:20)", "5:20"),
            ("04:01", "04:01"),
            ("04:01 A-M", "04:01"),
            ("04:01 P/M", "04:01"),
            ("04:01 AM", "04:01 AM"),
            ("04:01AM", "04:01AM"),
            ("04:01am", "04:01am"),
            ("04:01 a.m.", "04:01 a.m."),
            ("04:01 p.m.", "04:01 p.m."),
            ("04:01 p.m", "04:01 p.m"),
            ("23:59:59 UTC", "23:59:59"),
            ("23:59:60", "23:59"),
            ("10:30:5", "10:30"),
            ("10:30:15pm", "10:30:15pm"),
            ("7:20: PM", "7:20"),
            ("24:00", "24:00"),
        ] {
            assert_eq!(Time.match_prefix(text), Some(expected), "{text:?}");
        }
    }

    #[test]
    fn rejects_text_that_is_not_a_time() {
        for text in [
            "3", "2", "1:1-3", "1:5", "25:00", "99:10", "12:60", ":30", "a:30", "",
        ] {
            assert_eq!(Time.match_prefix(text), None, "{text:?}");
        }
    }

    #[test]
    fn keeps_only_a_whole_am_pm_marker() {
        assert_eq!(Time.match_prefix("10:30 Amsterdam"), Some("10:30"));
        assert_eq!(Time.match_prefix("10:30 pm."), Some("10:30 pm"));
        assert_eq!(Time.match_prefix("10:30 "), Some("10:30"));
        assert_eq!(Time.match_prefix("10:30 x"), Some("10:30"));
    }
}
