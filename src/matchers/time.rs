//! Times of day.

use super::BASE_AND_NUMBER;
use crate::chars::{is_letter, is_letter_or_digit};
use crate::cursor::Cursor;
use crate::matcher::Matcher;

/// Kind of [`Time`] tokens.
pub const TIME: &str = "TIME";

/// A time of day, in one of two forms: `H:MM` or `HH:MM`, then optional
/// `:SS`, then an optional AM/PM marker; or an hour from 1 to 12 and the
/// marker.
///
/// ```text
/// 7:20   04:01:30   8:05 PM   04:01am   4:01 a.m.   4 PM   12am
/// ```
///
/// The hour is 0 to 24, and the minutes and seconds are 00 to 59. A time
/// does not stand before another digit, or before a colon and more:
/// `07:01:75:c5` is part of an address. Digits are ASCII.
pub struct Time;

/// Two digits from 00 to 59.
fn sixty(cur: &mut Cursor<'_>) -> bool {
    cur.advance_if(|c| matches!(c, '0'..='5')) && cur.advance_if(|c| c.is_ascii_digit())
}

/// `AM`, `pm`, `a.m.` and the like, after an optional space. The marker
/// ends where its word ends: `10:30 Amsterdam` holds none.
fn marker(cur: &mut Cursor<'_>) -> bool {
    let before = cur.pos();
    cur.advance_if(|c| c == ' ');
    let complete = cur.advance_if(|c| matches!(c, 'a' | 'A' | 'p' | 'P')) && {
        let dotted = cur.advance_if(|c| c == '.');
        let complete = cur.advance_if(|c| matches!(c, 'm' | 'M'));
        if complete && dotted {
            cur.advance_if(|c| c == '.');
        }
        complete
    };
    if complete && !cur.peek().is_some_and(is_letter) {
        return true;
    }
    cur.set_pos(before);
    false
}

impl Matcher for Time {
    fn kind(&self) -> &'static str {
        TIME
    }

    fn overrides(&self) -> &'static [&'static str] {
        BASE_AND_NUMBER
    }

    fn can_start(&self, first: char) -> bool {
        first.is_ascii_digit()
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        // H or HH
        let digit = |cur: &mut Cursor<'_>| {
            let digit = cur.peek()?.to_digit(10)?;
            cur.advance();
            Some(digit)
        };
        let Some(first) = digit(cur) else {
            return false;
        };
        let hour = digit(cur).map_or(first, |second| first * 10 + second);
        if hour > 24 {
            return false;
        }
        // :MM
        let after_hour = cur.pos();
        if !(cur.advance_if(|c| c == ':') && sixty(cur)) {
            // An hour and a marker: "4 PM".
            cur.set_pos(after_hour);
            return (1..=12).contains(&hour) && marker(cur);
        }
        // :SS, or a colon that belongs to what follows
        let after_minutes = cur.pos();
        if !(cur.advance_if(|c| c == ':') && sixty(cur)) {
            cur.set_pos(after_minutes);
        }
        // In "07:01:75:c5" and "10:305" the run goes on: it is not a time.
        if cur.peek().is_some_and(|c| c.is_ascii_digit())
            || cur.followed_by(|c| c == ':', is_letter_or_digit)
        {
            return false;
        }
        // The marker is optional here.
        marker(cur);
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
            ("10:30:15pm", "10:30:15pm"),
            ("4 PM over", "4 PM"),
            ("4pm", "4pm"),
            ("12 a.m.", "12 a.m."),
            ("9 p.m", "9 p.m"),
            ("1 am.", "1 am"),
            ("7:20: PM", "7:20"),
            ("24:00", "24:00"),
        ] {
            assert_eq!(Time.match_prefix(text), Some(expected), "{text:?}");
        }
    }

    #[test]
    fn rejects_text_that_is_not_a_time() {
        for text in [
            "3",
            "2",
            "1:1-3",
            "1:5",
            "25:00",
            "99:10",
            "12:60",
            ":30",
            "a:30",
            "",
            // An hour needs minutes or a marker, and with a marker is 1 to 12.
            "4 P",
            "4 Paris",
            "0 am",
            "13 pm",
            "4  pm",
            // A colon and more follows: an address or a ratio.
            "07:01:75:c5:2f:f0",
            "10:30:5",
            "23:59:60",
            "1:18:x",
            // Another digit follows.
            "10:305",
            "10:30:155",
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
