//! Shortcuts for the default lexer.
//!
//! Most tokens of most texts are plain words, numbers, spaces, line breaks
//! and punctuation marks. For those, [`token`] reads the bytes once and
//! answers what the built-in matchers, tried in their default order, would
//! answer. Where the bytes leave a chance to one matcher or two, it asks
//! those and no other. For anything else it answers `None`, and the lexer
//! asks the matchers.
//!
//! Every rule here restates what several matchers do together, so each one
//! says which matchers it has ruled out. A rule may always ask a matcher or
//! answer `None`: it must never answer something the matchers would not.
//! `tests/lexer.rs` and `examples/fuzz.rs` compare the two on every text
//! they lex. A change to a built-in matcher, or a new one, has to be checked
//! against these rules.

use crate::chars::{ascii_classes, classes, NUMBER_PUNCTUATION, WORD_TERMINATION};
use crate::cursor::Cursor;
use crate::matcher::{ask, Matcher};
use crate::matchers::{
    has_url_prefix, is_email_local_byte, local_part, month_may_follow, name_after_amount,
    starts_with_month, AlphaNumericSequence, Date, Iso8601, LikeEmail, LikeNumber, LikeUrl, Time,
    Word, ALPHA_NUMERIC_SEQUENCE, CURRENCY, LIKE_NUMBER, LINE_TERMINATION, SYMBOL, WHITE_SPACE,
    WORD,
};
use crate::unicode_tables as tables;

/// What a byte is to the shortcuts.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Byte {
    /// An ASCII letter.
    Letter,
    /// A space or a tab: white space that is not a line break.
    Space,
    /// A line break.
    Break,
    /// A mark that ends a word: `. , : ? !`.
    Termination,
    /// A mark that goes on with a word or with a run of letters and digits:
    /// a dash, an underscore, an apostrophe.
    Joiner,
    /// Any other mark, and a control character.
    Mark,
    /// An ASCII digit.
    Digit,
    /// A byte of a character outside ASCII.
    High,
}

/// The classes of the marks that go on with a word or with a run of letters
/// and digits.
const JOINERS: u32 = tables::DASH | tables::UNDERSCORE | tables::APOSTROPHE;

const BYTES: [Byte; 256] = {
    let mut bytes = [Byte::High; 256];
    let mut byte = 0;
    while byte < 128 {
        let classes = ascii_classes(byte as u8);
        bytes[byte] = if classes & tables::LETTER != 0 {
            Byte::Letter
        } else if classes & tables::DECIMAL_DIGIT != 0 {
            Byte::Digit
        } else if classes & tables::LINE_TERMINATION != 0 {
            Byte::Break
        } else if classes & tables::WHITE_SPACE != 0 {
            Byte::Space
        } else if classes & WORD_TERMINATION != 0 {
            Byte::Termination
        } else if classes & JOINERS != 0 {
            Byte::Joiner
        } else {
            Byte::Mark
        };
        byte += 1;
    }
    bytes
};

/// True for the ASCII marks that only `Symbol` starts with: not a mark a
/// number starts with (`. , - _ + % #` and the like, of which `#` starts a
/// hashtag too), not a currency sign, and not the `@` of a mention.
const ONLY_A_SYMBOL: [bool; 128] = {
    let mut only = [false; 128];
    let mut byte = 0;
    while byte < 128 {
        let classes = ascii_classes(byte as u8);
        let alone = tables::LETTER
            | tables::DECIMAL_DIGIT
            | tables::WHITE_SPACE
            | tables::CURRENCY_SYMBOL
            | NUMBER_PUNCTUATION;
        only[byte] = classes & alone == 0 && byte != b'@' as usize;
        byte += 1;
    }
    only
};

/// How many bytes from the start of `bytes` are of a kind `wanted` accepts.
#[inline]
fn run(bytes: &[u8], wanted: impl Fn(Byte) -> bool) -> usize {
    bytes
        .iter()
        .take_while(|&&byte| wanted(BYTES[byte as usize]))
        .count()
}

/// How many ASCII letters `bytes` starts with.
///
/// Words are most of a text, and the length of a word is what a processor
/// cannot guess: a loop that tests one byte at a time guesses wrong once a
/// word. This tests eight bytes at a time, without a branch per byte.
#[inline]
fn letters(bytes: &[u8]) -> usize {
    /// The same byte eight times.
    const fn each(byte: u8) -> u64 {
        byte as u64 * 0x0101_0101_0101_0101
    }
    let mut len = 0;
    while let Some(chunk) = bytes[len..].first_chunk::<8>() {
        let chunk = u64::from_le_bytes(*chunk);
        // Upper case becomes lower case, and the top bit of every byte is
        // cleared so that the sums below do not carry into the next byte.
        let folded = (chunk | each(0x20)) & each(0x7F);
        // The top bit of a byte of the first sum is set from `a` up, and of
        // the second from the byte after `z` up.
        let from_a = folded + each(0x80 - b'a');
        let after_z = folded + each(0x80 - (b'z' + 1));
        // A letter is in between, and its own top bit is clear.
        let letter = from_a & !after_z & !chunk & each(0x80);
        let other = letter ^ each(0x80);
        if other != 0 {
            return len + (other.trailing_zeros() / 8) as usize;
        }
        len += 8;
    }
    len + bytes[len..]
        .iter()
        .take_while(|byte| byte.is_ascii_alphabetic())
        .count()
}

/// The character at byte offset `at` of `text`, where a byte outside ASCII
/// stands, and its classes.
#[inline]
fn high_at(text: &str, at: usize) -> (char, u32) {
    let c = text[at..].chars().next().expect("the text has not ended");
    (c, classes(c))
}

/// How many bytes of letters stand at byte offset `from` of `text`, where
/// no ASCII letter stands: letters outside ASCII, and ASCII ones among them.
/// It is kept out of the path of a plain word, which is slower with it
/// inside.
#[inline(never)]
fn letters_of_any_script(text: &str, from: usize) -> usize {
    let mut rest = text[from..].chars();
    loop {
        let before = rest.as_str();
        match rest.next() {
            Some(c) if c.is_ascii_alphabetic() => {
                rest = before[letters(before.as_bytes())..].chars();
            }
            Some(c) if !c.is_ascii() && classes(c) & tables::LETTER != 0 => {}
            _ => return text.len() - from - before.len(),
        }
    }
}

/// What stands after a run of letters.
enum After {
    /// White space, a mark that ends a sentence or a clause, or the end of
    /// the text: the run is a word.
    WordEnd,
    /// A digit, a dash, an apostrophe or an underscore: the run may go on.
    /// A letter is counted here too, though the run has taken them all.
    Joiner,
    /// Any other mark: the run is not a word, and is the whole of an
    /// alphanumeric sequence.
    Mark,
}

#[inline]
fn after_letters(text: &str, at: usize) -> After {
    let Some(&byte) = text.as_bytes().get(at) else {
        return After::WordEnd;
    };
    match BYTES[byte as usize] {
        Byte::Space | Byte::Break | Byte::Termination => After::WordEnd,
        Byte::Letter | Byte::Digit | Byte::Joiner => After::Joiner,
        Byte::Mark => After::Mark,
        Byte::High => {
            let (_, classes) = high_at(text, at);
            if classes & (tables::LETTER | tables::DECIMAL_DIGIT | JOINERS) != 0 {
                After::Joiner
            } else if classes & WORD_TERMINATION != 0 {
                After::WordEnd
            } else {
                After::Mark
            }
        }
    }
}

/// The token at byte offset `start` of `text`, which starts with `len`
/// bytes of letters, when only `Word`, `AlphaNumericSequence` and `Letter`
/// are left to take it. The second always matches at a letter.
#[inline]
fn word(text: &str, start: usize, len: usize) -> Option<(&'static str, usize)> {
    let mut end = start + len;
    if text
        .as_bytes()
        .get(end)
        .is_some_and(|byte| !byte.is_ascii())
    {
        end += letters_of_any_script(text, end);
    }
    match after_letters(text, end) {
        After::WordEnd => Some((WORD, end)),
        After::Mark => Some((ALPHA_NUMERIC_SEQUENCE, end)),
        // How a run goes on through its joiners is for the two to say.
        After::Joiner => {
            ask(&Word, text, start).or_else(|| ask(&AlphaNumericSequence, text, start))
        }
    }
}

/// True for a byte that goes on to the `@` of an email address after the
/// start of a local part, or is the `@`.
fn goes_on_to_an_at(byte: u8) -> bool {
    byte == b'@' || is_email_local_byte(byte)
}

/// The token at byte offset `start` of `text`, where an ASCII digit stands.
///
/// It starts with a number: digits with full stops and commas between
/// them, such as `2009`, `3.14` and `1,000`. Seven matchers start with a
/// digit. They are asked in their order, each only where the number and
/// what stands after it leave it a chance, and most numbers leave a chance
/// to none but the last.
fn number(text: &str, start: usize) -> Option<(&'static str, usize)> {
    let bytes = &text.as_bytes()[start..];
    let is_point = |byte: u8| matches!(byte, b'.' | b',');
    let digits_at = |from: usize| {
        let bytes = bytes.get(from..).unwrap_or_default();
        bytes
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count()
    };
    let digits = digits_at(0);
    let mut run = digits;
    while bytes.get(run).is_some_and(|&byte| is_point(byte)) {
        run += 1 + digits_at(run + 1);
    }
    // A number does not end with a full stop or a comma.
    let amount = run
        - bytes[..run]
            .iter()
            .rev()
            .take_while(|&&b| is_point(b))
            .count();
    let next = bytes.get(run).copied();
    // A character outside ASCII may be a digit, or a mark written with
    // numbers.
    if next.is_some_and(|byte| !byte.is_ascii()) {
        return None;
    }

    // `Iso8601`: four digits and a dash.
    let iso = digits == 4 && bytes.get(4) == Some(&b'-');
    // `Date`, of numbers: a number of one or two digits stands between two
    // separators that are the same. A full stop is in the run, and a space
    // separates only after a year.
    let stops = bytes[digits..amount].iter().filter(|&&b| b == b'.').count();
    let separator = next.is_some_and(|byte| match byte {
        b' ' => digits == 4,
        _ => matches!(byte, b'-' | b'_' | b'/' | b'\\'),
    });
    let twice = || {
        let second = digits_at(run + 1);
        (1..=2).contains(&second) && bytes.get(run + 1 + second).copied() == next
    };
    let numeric_date = stops >= 2 || (run == digits && separator && twice());
    // `Date`, with a month name: one number, the two letters of an ordinal
    // ending after a day, separators, and the name.
    let ordinal = digits <= 2 && next.is_some_and(|byte| byte.is_ascii_alphabetic());
    let month_date = amount == digits
        && (month_may_follow(&bytes[digits..])
            || (ordinal && bytes.get(digits + 2..).is_some_and(month_may_follow)));
    // `Time`: an hour, and then a colon, or a marker as in `4 PM`.
    let is_marker = |byte: Option<&u8>| matches!(byte, Some(b'a' | b'A' | b'p' | b'P'));
    let marker = is_marker(bytes.get(run)) || (next == Some(b' ') && is_marker(bytes.get(run + 1)));
    let time = run == digits && digits <= 2 && (next == Some(b':') || marker);
    // `LikeEmail`: the bytes of a local part, up to an `@`.
    let email = next.is_some_and(goes_on_to_an_at) && local_part(bytes).is_some();
    // `AlphaNumericSequence`: digits, and letters after them or after
    // dashes and underscores.
    let sequence =
        next.is_some_and(|byte| byte.is_ascii_alphabetic() || matches!(byte, b'-' | b'_'));

    let ask_if = |may: bool, matcher: &dyn Matcher| {
        if may {
            ask(matcher, text, start)
        } else {
            None
        }
    };
    ask_if(iso, &Iso8601)
        .or_else(|| ask_if(numeric_date || month_date, &Date))
        .or_else(|| ask_if(time, &Time))
        .or_else(|| ask_if(email, &LikeEmail))
        .or_else(|| {
            // `Currency`: the same number, and then a name.
            let mut cur = Cursor::at(text, start + amount);
            name_after_amount(&mut cur).then(|| (CURRENCY, cur.pos()))
        })
        .or_else(|| ask_if(sequence, &AlphaNumericSequence))
        .or_else(|| {
            // `LikeNumber`: the number, and more after a mark written with
            // numbers.
            let goes_on = next.is_some_and(|byte| ascii_classes(byte) & NUMBER_PUNCTUATION != 0);
            ask_if(goes_on, &LikeNumber).or(Some((LIKE_NUMBER, start + amount)))
        })
}

/// The kind and the end of the token that starts at byte offset `start` of
/// `text`, when a shortcut applies. `start` is before the end of the text.
#[inline]
pub(crate) fn token(text: &str, start: usize) -> Option<(&'static str, usize)> {
    let bytes = &text.as_bytes()[start..];
    let kind_at = |at: usize| bytes.get(at).map(|&byte| BYTES[byte as usize]);
    // A character outside ASCII after a run of white space may be white
    // space as well, such as a no-break space, and go on with the run.
    let no_white_at = |at: usize| {
        const WHITE: u32 = tables::WHITE_SPACE | tables::LINE_TERMINATION;
        kind_at(at) != Some(Byte::High) || high_at(text, start + at).1 & WHITE == 0
    };
    let first = bytes[0];
    match BYTES[first as usize] {
        Byte::Letter => {
            let len = letters(bytes);
            // Only `Date`, `LikeEmail`, `LikeUrl`, `Word`,
            // `AlphaNumericSequence` and `Letter` start with a letter, and
            // they are tried in that order. A date starts with a month
            // name, and the name is the whole run; an email address goes on
            // to an `@`, through the bytes of a local part if the run does
            // not reach it; a URL starts with a prefix that ends in `:` or
            // `.`.
            if starts_with_month(bytes, len) {
                if let Some(date) = ask(&Date, text, start) {
                    return Some(date);
                }
            }
            // Most words stand before white space or end the text. Neither
            // an address nor a URL goes on there, and a word ends.
            if matches!(kind_at(len), None | Some(Byte::Space | Byte::Break)) {
                return Some((WORD, start + len));
            }
            let next = bytes.get(len).copied();
            if next.is_some_and(goes_on_to_an_at) && local_part(bytes).is_some() {
                if let Some(email) = ask(&LikeEmail, text, start) {
                    return Some(email);
                }
            }
            if matches!(next, Some(b':' | b'.')) && has_url_prefix(bytes) {
                if let Some(url) = ask(&LikeUrl, text, start) {
                    return Some(url);
                }
            }
            word(text, start, len)
        }
        // Only `WhiteSpace` starts with a space or a tab, and it takes the
        // line breaks that follow as well.
        Byte::Space => {
            let len = run(bytes, |byte| matches!(byte, Byte::Space | Byte::Break));
            no_white_at(len).then_some((WHITE_SPACE, start + len))
        }
        // `LineTermination` is tried before `WhiteSpace`.
        Byte::Break => {
            let len = run(bytes, |byte| byte == Byte::Break);
            no_white_at(len).then_some((LINE_TERMINATION, start + len))
        }
        Byte::Termination | Byte::Joiner | Byte::Mark => {
            if ONLY_A_SYMBOL[first as usize] {
                return Some((SYMBOL, start + 1));
            }
            // `LikeNumber` takes at most two marks and then needs a number
            // character, and `Currency` needs a digit after a full stop. A
            // letter, white space or a mark of another kind next rules both
            // out. A hashtag, a mention and a currency sign are left to
            // their matchers.
            let number_mark = ascii_classes(first) & NUMBER_PUNCTUATION != 0 && first != b'#';
            let rules_out_a_number = match bytes.get(1) {
                None => true,
                Some(&next) => {
                    matches!(
                        BYTES[next as usize],
                        Byte::Letter | Byte::Space | Byte::Break
                    ) || (next < 128 && ONLY_A_SYMBOL[next as usize])
                }
            };
            (number_mark && rules_out_a_number).then_some((SYMBOL, start + 1))
        }
        Byte::Digit => number(text, start),
        Byte::High => {
            let (c, classes) = high_at(text, start);
            if classes == 0 {
                // In no class at all, such as a curly quote: every matcher
                // but `Symbol` starts with a character of some class.
                Some((SYMBOL, start + c.len_utf8()))
            } else if classes & tables::LETTER != 0 {
                // Only `Word`, `AlphaNumericSequence` and `Letter` start
                // with a letter outside ASCII.
                word(text, start, 0)
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matchers::TIME;

    /// The eight-bytes-at-a-time count against the plain one, with every
    /// byte value at every place of a chunk and of the tail after it.
    #[test]
    fn letters_counts_like_a_loop_over_the_bytes() {
        let plain = |bytes: &[u8]| bytes.iter().take_while(|b| b.is_ascii_alphabetic()).count();
        for len in 0..20 {
            let mut bytes = vec![b'a'; len];
            assert_eq!(letters(&bytes), len, "{len} letters");
            for at in 0..len {
                for byte in 0..=255 {
                    bytes[at] = byte;
                    assert_eq!(
                        letters(&bytes),
                        plain(&bytes),
                        "{byte:#04x} at {at} of {len}"
                    );
                }
                bytes[at] = b'Z';
            }
        }
    }

    #[test]
    fn a_shortcut_starts_at_the_given_place() {
        assert_eq!(token("12 word.", 3), Some((WORD, 7)));
        assert_eq!(token("12 word.", 2), Some((WHITE_SPACE, 3)));
        assert_eq!(token("12 word.", 7), Some((SYMBOL, 8)));
        assert_eq!(token("12 word.", 0), Some((LIKE_NUMBER, 2)));
        assert_eq!(token("12:30 word.", 0), Some((TIME, 5)));
        assert_eq!(token("12\u{2013}30", 0), None);
    }
}
