//! Shortcuts for the default lexer.
//!
//! Most tokens of most texts are plain words, spaces, line breaks and
//! punctuation marks. For those, [`token`] reads the bytes once and answers
//! what the built-in matchers, tried in their default order, would answer.
//! For anything else it answers `None`, and the lexer asks the matchers.
//!
//! Every rule here restates what several matchers do together, so each one
//! says which matchers it has ruled out. A rule may always answer `None`: it
//! must never answer something the matchers would not. `tests/lexer.rs` and
//! `examples/fuzz.rs` compare the two on every text they lex. A change to a
//! built-in matcher, or a new one, has to be checked against these rules.

use crate::chars::{ascii_classes, classes, NUMBER_PUNCTUATION};
use crate::cursor::Cursor;
use crate::matcher::Matcher;
use crate::matchers::{
    has_url_prefix, local_part, starts_with_month, AlphaNumericSequence, Word,
    ALPHA_NUMERIC_SEQUENCE, LINE_TERMINATION, SYMBOL, WHITE_SPACE, WORD,
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

const BYTES: [Byte; 256] = {
    let mut bytes = [Byte::High; 256];
    let mut byte = 0;
    while byte < 128 {
        let classes = ascii_classes(byte as u8);
        let joiner = tables::DASH | tables::UNDERSCORE | tables::APOSTROPHE;
        let termination = tables::FULL_STOP
            | tables::COMMA
            | tables::COLON
            | tables::QUESTION_MARK
            | tables::EXCLAMATION_MARK;
        bytes[byte] = if classes & tables::LETTER != 0 {
            Byte::Letter
        } else if classes & tables::DECIMAL_DIGIT != 0 {
            Byte::Digit
        } else if classes & tables::LINE_TERMINATION != 0 {
            Byte::Break
        } else if classes & tables::WHITE_SPACE != 0 {
            Byte::Space
        } else if classes & termination != 0 {
            Byte::Termination
        } else if classes & joiner != 0 {
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

/// The kind and the end of the token that starts at byte offset `start` of
/// `text`, when a shortcut applies. `start` is before the end of the text.
#[inline]
pub(crate) fn token(text: &str, start: usize) -> Option<(&'static str, usize)> {
    let bytes = &text.as_bytes()[start..];
    let kind_at = |at: usize| bytes.get(at).map(|&byte| BYTES[byte as usize]);
    // A byte outside ASCII after a run may belong to a character that goes
    // on with the run, such as a no-break space or a letter with an accent.
    let ascii_or_end = |at: usize| kind_at(at) != Some(Byte::High);
    let first = bytes[0];
    match BYTES[first as usize] {
        Byte::Letter => {
            let len = letters(bytes);
            // Only `Date`, `LikeEmail`, `LikeUrl`, `Word`,
            // `AlphaNumericSequence` and `Letter` start with a letter, and
            // they are tried in that order.
            let next = kind_at(len);
            // A digit or a joiner goes on with the run, and a character
            // outside ASCII may: the matchers decide.
            if matches!(next, Some(Byte::Digit | Byte::Joiner | Byte::High)) {
                return None;
            }
            // A date starts with a month name; a URL with a prefix that ends
            // in `:` or `.`; an email address goes on to an `@`, through `.`
            // or `+` if the run does not reach it.
            let goes_on_to_an_at = matches!(bytes.get(len), Some(b'.' | b'+' | b'@'));
            if starts_with_month(bytes, len)
                || (next == Some(Byte::Termination) && has_url_prefix(bytes))
                || (goes_on_to_an_at && local_part(bytes).is_some())
            {
                return None;
            }
            // A word ends at white space, at a mark that ends a sentence or
            // a clause, and at the end. Before any other mark the run is not
            // a word, and is the whole of an alphanumeric sequence.
            let ended = matches!(
                next,
                None | Some(Byte::Space | Byte::Break | Byte::Termination)
            );
            Some((
                if ended { WORD } else { ALPHA_NUMERIC_SEQUENCE },
                start + len,
            ))
        }
        // Only `WhiteSpace` starts with a space or a tab, and it takes the
        // line breaks that follow as well.
        Byte::Space => {
            let len = run(bytes, |byte| matches!(byte, Byte::Space | Byte::Break));
            ascii_or_end(len).then_some((WHITE_SPACE, start + len))
        }
        // `LineTermination` is tried before `WhiteSpace`.
        Byte::Break => {
            let len = run(bytes, |byte| byte == Byte::Break);
            ascii_or_end(len).then_some((LINE_TERMINATION, start + len))
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
        // Seven matchers start with a digit, and which of them takes it
        // depends on much of what follows: they are asked.
        Byte::Digit => None,
        Byte::High => {
            let c = text[start..]
                .chars()
                .next()
                .expect("the text has not ended");
            let classes = classes(c);
            if classes == 0 {
                // In no class at all, such as a curly quote: every matcher
                // but `Symbol` starts with a character of some class.
                Some((SYMBOL, start + c.len_utf8()))
            } else if classes & tables::LETTER != 0 {
                // Only `Word`, `AlphaNumericSequence` and `Letter` start
                // with a letter outside ASCII, and the second always
                // matches there.
                let mut cur = Cursor::at(text, start);
                if Word.matches(&mut cur) {
                    return Some((WORD, cur.pos()));
                }
                cur.set_pos(start);
                AlphaNumericSequence
                    .matches(&mut cur)
                    .then(|| (ALPHA_NUMERIC_SEQUENCE, cur.pos()))
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(token("12 word.", 0), None);
    }
}
