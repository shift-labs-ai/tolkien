//! Checks the lexer on real documents and on mutations of them.
//!
//!     cargo run --release --example fuzz -- [--seconds N] [--seed N] FILE...
//!
//! Each FILE holds documents separated by NUL characters, as
//! `tools/corpora.py` writes them. Every document is checked as it is.
//! Then the start of every piece the mutations use is repeated into a long
//! run, which is timed. Then, for `--seconds` (default 30), documents are
//! picked at random, cut to a window, mutated and checked again.
//!
//! Three lexers are checked: the default one and the default one with the
//! whole catalog added, on every text, and the default one with one group
//! of the catalog on every mutated text, a different group from text to
//! text. Between them they run every built-in matcher, every matcher of
//! the catalog, patterns and matchers written by hand, and a lexer whose
//! shortcuts are in use for some first characters and not for others.
//!
//! A check is a property that must hold for any text:
//!
//! - `coverage`: the tokens cover the text with no gap, overlap or empty token.
//! - `reference`: the tokens are those of asking every matcher at every
//!   position, in order. The lexer skips matchers that cannot start with a
//!   character and knows the commonest tokens outright; neither may change
//!   an answer.
//! - `kind`: no `OTHER`, `LETTER` or `DIGIT` token appears.
//! - `parallel`: lexed in pieces on several threads, the text gives the
//!   same tokens. One text in sixteen is checked: threads are slow to start.
//! - `shape`: a token of a built-in kind has the form its kind documents.
//!   The rules are written here a second time, on purpose, so that they do
//!   not share a bug with the matchers. The kinds of the catalog are not
//!   written twice: the examples of their entries say what they are.
//! - `alone`: a token's text, given to its own matcher with nothing after
//!   it, is that whole token.
//! - `no-left-context`: lexing from the start of a token gives that token
//!   again, whatever came before it.
//! - `time`: lexing takes time in proportion to the length of the text.
//!
//! A text that breaks a check is cut down to a small one that still breaks it
//! and printed as a Rust string literal. The exit status is 1 when any check
//! failed. Run it in a release build: the `time` budget assumes one.

use std::collections::BTreeMap;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use tolkien::catalog::{self, Group};
use tolkien::chars::*;
use tolkien::matchers::*;
use tolkien::{Lexer, Matcher, Token, OTHER};

#[path = "../tests/support/reference.rs"]
mod reference;

/// A broken check: which one, on which kind of token, and what was seen.
struct Finding {
    check: &'static str,
    kind: &'static str,
    detail: String,
}

/// A lexer, and the matchers it has to agree with.
struct Checker {
    /// What the lexer is, for a report.
    name: &'static str,
    lexer: Lexer,
    /// The lexer's matchers in the order they are tried.
    matchers: Vec<Box<dyn Matcher>>,
}

impl Checker {
    /// The default lexer with the matchers `added` gives tried first.
    /// `added` is called twice: a matcher cannot be copied.
    fn new(name: &'static str, added: impl Fn() -> Vec<Box<dyn Matcher>>) -> Self {
        let mut matchers = added();
        matchers.extend(all());
        Self {
            name,
            lexer: Lexer::with(added()).expect("the matchers form a lexer"),
            matchers,
        }
    }

    /// True when a matcher of the catalog produces `kind`.
    fn is_of_the_catalog(kind: &str) -> bool {
        catalog::entries().iter().any(|entry| entry.kind == kind)
    }

    /// The first broken check of `text`, if there is one. `rng` picks the
    /// tokens that the sampled checks look at.
    fn check(&self, text: &str, rng: &mut Rng) -> Option<Finding> {
        let started = Instant::now();
        let tokens: Vec<Token<'_>> = self.lexer.tokens(text).collect();
        let elapsed = started.elapsed();

        let mut pos = 0;
        for token in &tokens {
            if token.start != pos || token.end <= token.start {
                let detail = format!("token {}..{} after position {pos}", token.start, token.end);
                return Some(Finding {
                    check: "coverage",
                    kind: token.kind,
                    detail,
                });
            }
            pos = token.end;
        }
        if pos != text.len() {
            let detail = format!("tokens end at {pos} of {}", text.len());
            return Some(Finding {
                check: "coverage",
                kind: "",
                detail,
            });
        }

        // Both cover the text, so they differ in a token if they differ at all.
        let expected = reference::tokens(&self.matchers, text);
        let differs = |(ours, theirs): &(&Token<'_>, &(&str, usize, usize))| {
            (ours.kind, ours.start, ours.end) != **theirs
        };
        if let Some((ours, &(kind, start, end))) = tokens.iter().zip(&expected).find(differs) {
            let detail = format!(
                "{} {:?} where the matchers give {kind} {:?}",
                ours.kind,
                ours.text,
                &text[start..end]
            );
            return Some(Finding {
                check: "reference",
                kind: ours.kind,
                detail,
            });
        }

        if rng.below(16) == 0 {
            let pieces = 2 + rng.below(7);
            let parallel = self.lexer.tokens_parallel_on(text, pieces);
            if let Some((ours, theirs)) = tokens.iter().zip(&parallel).find(|(a, b)| a != b) {
                let detail = format!(
                    "in {pieces} pieces {} {:?}, whole {} {:?}",
                    theirs.kind, theirs.text, ours.kind, ours.text
                );
                return Some(Finding {
                    check: "parallel",
                    kind: ours.kind,
                    detail,
                });
            }
            if parallel.len() != tokens.len() {
                let detail = format!(
                    "{} tokens in {pieces} pieces, {} whole",
                    parallel.len(),
                    tokens.len()
                );
                return Some(Finding {
                    check: "parallel",
                    kind: "",
                    detail,
                });
            }
        }

        for token in &tokens {
            if [OTHER, LETTER, DIGIT].contains(&token.kind) {
                let detail = format!("{:?}", token.text);
                return Some(Finding {
                    check: "kind",
                    kind: token.kind,
                    detail,
                });
            }
            if !Self::is_of_the_catalog(token.kind) {
                if let Err(why) = shape(token, &text[token.end..]) {
                    let detail = format!("{:?}: {why}", token.text);
                    return Some(Finding {
                        check: "shape",
                        kind: token.kind,
                        detail,
                    });
                }
            }
            let matcher = self
                .matchers
                .iter()
                .find(|m| m.kind() == token.kind)
                .expect("a token has the kind of one of the lexer's matchers");
            if matcher.match_prefix(token.text) != Some(token.text) {
                let detail = format!(
                    "{:?} gives {:?}",
                    token.text,
                    matcher.match_prefix(token.text)
                );
                return Some(Finding {
                    check: "alone",
                    kind: token.kind,
                    detail,
                });
            }
        }

        for _ in 0..tokens.len().min(8) {
            let token = tokens[rng.below(tokens.len())];
            let again = self.lexer.tokens(&text[token.start..]).next();
            let same = again.is_some_and(|t| t.kind == token.kind && t.text == token.text);
            if !same {
                let detail = format!(
                    "{:?} becomes {:?}",
                    token.text,
                    again.map(|t| (t.kind, t.text))
                );
                return Some(Finding {
                    check: "no-left-context",
                    kind: token.kind,
                    detail,
                });
            }
        }

        self.too_slow(text, elapsed)
    }

    /// The `time` check: a finding when lexing `text` took `elapsed` and
    /// that is more than its length allows.
    fn too_slow(&self, text: &str, elapsed: Duration) -> Option<Finding> {
        // A release build lexes its slowest input at well over 10 MB/s, which
        // is 0.1 microseconds a byte. Twenty times that, and a floor for noise.
        let budget = Duration::from_millis(20) + Duration::from_nanos(2_000) * text.len() as u32;
        if elapsed <= budget {
            return None;
        }
        // Measure again before blaming the lexer for a busy machine.
        let (count, again) = self.timed(text);
        (again > budget).then(|| Finding {
            check: "time",
            kind: "",
            detail: format!("{} bytes, {count} tokens, {again:?}", text.len()),
        })
    }

    /// How many tokens `text` has, and how long lexing it took.
    fn timed(&self, text: &str) -> (usize, Duration) {
        let started = Instant::now();
        let count = self.lexer.tokens(text).count();
        (count, started.elapsed())
    }
}

fn is_date_separator(c: char) -> bool {
    c == ' ' || is_dash(c) || is_full_stop(c) || is_comma(c) || is_underscore(c) || is_slash(c)
}

fn is_url_char(c: char) -> bool {
    is_letter_or_digit(c) || "!$&'()*+,;=:/?#[]@-._~%".contains(c)
}

fn require(condition: bool, why: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(why.to_string())
    }
}

/// Splits `text` at the first character `pred` rejects.
fn split_while(text: &str, pred: impl Fn(char) -> bool) -> (&str, &str) {
    let end = text.find(|c| !pred(c)).unwrap_or(text.len());
    text.split_at(end)
}

/// Checks that `token` has the form its kind documents. `rest` is the text
/// after the token.
fn shape(token: &Token<'_>, rest: &str) -> Result<(), String> {
    let text = token.text;
    let first = text.chars().next().expect("a token is not empty");
    let last = text.chars().next_back().expect("a token is not empty");
    let next = rest.chars().next();
    let count = |pred: fn(char) -> bool| text.chars().filter(|&c| pred(c)).count();

    match token.kind {
        WORD => {
            let joins = |c| is_dash(c) || is_apostrophe(c);
            require(is_letter(first), "does not start with a letter")?;
            require(is_letter(last), "does not end with a letter")?;
            require(
                text.chars().all(|c| is_letter(c) || joins(c)),
                "holds a character a word cannot",
            )?;
            require(count(is_apostrophe) <= 1, "holds two apostrophes")?;
            let (_, after) = split_while(rest, joins);
            require(
                after.chars().next().is_none_or(is_word_termination),
                "is not followed by a termination",
            )
        }
        ALPHA_NUMERIC_SEQUENCE => {
            let inside = |c| is_letter_or_digit(c) || is_dash(c) || is_underscore(c);
            require(
                is_letter_or_digit(first),
                "does not start with a letter or a digit",
            )?;
            require(!is_dash(last), "ends with a dash")?;
            require(text.chars().all(inside), "holds a character the run cannot")?;
            require(count(is_letter) > 0, "holds no letter")?;
            let (_, after) = split_while(rest, is_dash);
            require(
                !after.starts_with(inside),
                "stops before the end of its run",
            )
        }
        LIKE_NUMBER => {
            let inside = |c| is_number(c) || is_number_punctuation(c);
            require(
                text.chars().all(inside),
                "holds a character a number cannot",
            )?;
            let lead = text.chars().take_while(|&c| !is_number(c)).count();
            require(lead < text.chars().count(), "holds no number character")?;
            require(
                lead <= 2,
                "has more than two characters before its first number",
            )?;
            require(
                is_number(last) || is_percent_sign(last),
                "ends with punctuation",
            )?;
            let (_, after) = split_while(rest, is_number_punctuation);
            require(
                !after.starts_with(is_number),
                "stops before the end of its run",
            )
        }
        LIKE_EMAIL => {
            require(text.is_ascii(), "is not ASCII")?;
            let (local, domain) = text.split_once('@').ok_or("has no @")?;
            let local_char = |c: char| c.is_ascii_alphanumeric() || "_-.+".contains(c);
            require(
                local.chars().all(local_char),
                "has a bad character before the @",
            )?;
            require(
                first.is_ascii_alphanumeric(),
                "does not start with a letter or a digit",
            )?;
            require(
                local.len() <= 64,
                "has more than 64 characters before the @",
            )?;
            let (host, tld) = domain.split_once('.').ok_or("has no dot after the @")?;
            let host_char = |c: char| c.is_ascii_alphanumeric() || c == '-';
            require(
                host.chars().all(host_char),
                "has a bad character in its domain",
            )?;
            require(
                host.chars().any(|c| c.is_ascii_alphanumeric()),
                "has an empty domain",
            )?;
            require(
                tld.chars().all(|c| host_char(c) || c == '.'),
                "has a bad character at its end",
            )?;
            require(last.is_ascii_alphanumeric(), "ends with punctuation")
        }
        LIKE_URL => {
            let after = ["https://", "http://", "www."]
                .iter()
                .find_map(|prefix| text.strip_prefix(prefix))
                .ok_or("has no known prefix")?;
            require(!after.is_empty(), "has nothing after its prefix")?;
            require(
                after.chars().all(is_url_char),
                "holds a character a URL cannot",
            )?;
            require(!".,;:!?'".contains(last), "ends with sentence punctuation")?;
            let unopened = [(')', '('), (']', '[')].iter().any(|&(close, open)| {
                last == close && after.matches(close).count() > after.matches(open).count()
            });
            require(!unopened, "ends with a bracket that nothing opens")
        }
        TIME => time_shape(text, rest),
        DATE_ISO_8601 => {
            let bytes = text.as_bytes();
            let digits = |range: std::ops::Range<usize>| {
                bytes
                    .get(range)
                    .is_some_and(|part| part.iter().all(u8::is_ascii_digit))
            };
            let dash = |at: usize| bytes.get(at) == Some(&b'-');
            let date = digits(0..4) && dash(4) && digits(5..7) && dash(7) && digits(8..10);
            require(date, "does not start with YYYY-MM-DD")?;
            let time_char = |c: char| c.is_ascii_digit() || "T :.+-Z".contains(c);
            require(
                text[10..].chars().all(time_char),
                "holds a character a time cannot",
            )?;
            require(
                last.is_ascii_digit() || last == 'Z',
                "ends in the middle of a part",
            )?;
            let day_goes_on = text.len() == 10 && next.is_some_and(|c| c.is_ascii_digit());
            require(!day_goes_on, "has a digit after its day")
        }
        DATE => {
            require(last.is_ascii_digit(), "does not end with an ASCII digit")?;
            let words: Vec<&str> = text
                .split(|c| !is_letter(c))
                .filter(|w| !w.is_empty())
                .collect();
            if words.is_empty() {
                numeric_date_shape(text, rest)
            } else {
                let inside = |c: char| is_letter(c) || c.is_ascii_digit() || is_date_separator(c);
                require(text.chars().all(inside), "holds a character a date cannot")?;
                require(
                    is_letter(first) || first.is_ascii_digit(),
                    "starts with a separator",
                )?;
                let is_ordinal =
                    |w: &str| ["st", "nd", "rd", "th"].contains(&w.to_lowercase().as_str());
                let (ordinals, names): (Vec<&str>, Vec<&str>) =
                    words.iter().partition(|w| is_ordinal(w));
                require(
                    names.len() == 1 && is_month(names[0]),
                    "does not hold exactly one month name",
                )?;
                require(ordinals.len() <= 1, "holds two ordinal endings")?;
                require(!next.is_some_and(is_letter), "is followed by a letter")
            }
        }
        CURRENCY => {
            let (symbols, amount) = split_while(text, is_currency_symbol);
            if symbols.is_empty() {
                require(
                    is_digit(first) || is_full_stop(first),
                    "starts with neither a symbol nor an amount",
                )?;
                require(count(is_digit) > 0, "holds no digit")?;
                let glued = |c| is_letter_or_digit(c) || is_apostrophe(c);
                require(
                    !next.is_some_and(glued),
                    "is followed by a letter, a digit or an apostrophe",
                )
            } else if amount.is_empty() {
                Ok(())
            } else {
                let inside = |c| is_digit(c) || is_full_stop(c) || is_comma(c);
                require(
                    amount.chars().all(inside),
                    "holds a character an amount cannot",
                )?;
                require(is_digit(last), "ends with punctuation")
            }
        }
        HASHTAG | MENTION => {
            let sigil = if token.kind == HASHTAG { '#' } else { '@' };
            let name = text
                .strip_prefix(sigil)
                .ok_or("does not start with its sign")?;
            let inside = |c| is_letter_or_digit(c) || c == '_';
            require(name.chars().all(inside), "holds a character a name cannot")?;
            require(
                name.chars().any(is_letter_or_digit),
                "holds no letter or digit",
            )?;
            require(!next.is_some_and(inside), "stops before the end of its run")
        }
        SYMBOL => {
            require(text.chars().count() == 1, "is more than one character")?;
            require(is_symbol(first), "is not a symbol")
        }
        WHITE_SPACE => {
            require(
                text.chars().all(is_whitespace),
                "holds a character that is not white space",
            )?;
            require(!is_line_termination(first), "starts with a line break")?;
            require(
                !next.is_some_and(is_whitespace),
                "stops before the end of its run",
            )
        }
        LINE_TERMINATION => {
            require(
                text.chars().all(is_line_termination),
                "holds a character that is not a line break",
            )?;
            require(
                !next.is_some_and(is_line_termination),
                "stops before the end of its run",
            )
        }
        other => Err(format!("is of the unknown kind {other}")),
    }
}

/// Three numbers with the same separator twice: year, month, day, or day
/// and month in either order and then a year of two or four digits.
fn numeric_date_shape(text: &str, rest: &str) -> Result<(), String> {
    let (a, tail) = split_while(text, |c| c.is_ascii_digit());
    let separator = tail.chars().next().ok_or("is one number")?;
    let (b, tail) = split_while(&tail[separator.len_utf8()..], |c| c.is_ascii_digit());
    let c = tail
        .strip_prefix(separator)
        .ok_or("does not repeat its separator")?;
    require(
        c.chars().all(|c| c.is_ascii_digit()),
        "holds more than three numbers",
    )?;
    require(
        is_date_separator(separator) && !is_comma(separator),
        "has a separator a numeric date cannot",
    )?;
    let value = |part: &str| part.parse::<u32>().unwrap_or(u32::MAX);
    let (x, y, z) = (value(a), value(b), value(c));
    let day = |n| (1..=31).contains(&n);
    let short = |part: &str| (1..=2).contains(&part.len());
    if a.len() == 4 {
        require(
            short(b) && short(c) && (1..=12).contains(&y) && day(z),
            "is not year, month, day",
        )?;
    } else {
        require(
            separator != ' ',
            "is separated by spaces but does not start with its year",
        )?;
        require(
            short(a) && short(b) && [2, 4].contains(&c.len()),
            "has a part of the wrong length",
        )?;
        require(
            day(x) && day(y) && x.min(y) <= 12,
            "has no month from 1 to 12 or no day from 1 to 31",
        )?;
    }
    let mut after = rest.chars();
    let goes_on =
        after.next() == Some(separator) && after.next().is_some_and(|c| c.is_ascii_digit());
    require(!goes_on, "is the start of a longer run of numbers")
}

/// An English month name or its three-letter form, in lower case, in upper
/// case or capitalised.
fn is_month(word: &str) -> bool {
    const MONTHS: [&str; 12] = [
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
    ];
    let lower = word.to_ascii_lowercase();
    let known = word.is_ascii()
        && MONTHS
            .iter()
            .any(|month| lower == *month || lower == month[..3]);
    let capitalised =
        known && word[..1] == lower[..1].to_ascii_uppercase() && word[1..] == lower[1..];
    known && (word == lower || word == word.to_ascii_uppercase() || capitalised)
}

/// `H:MM` or `HH:MM`, then `:SS`, then a marker, the last two optional; or
/// an hour from 1 to 12 and a marker.
fn time_shape(text: &str, rest: &str) -> Result<(), String> {
    let (hour, tail) = split_while(text, |c| c.is_ascii_digit());
    require(
        (1..=2).contains(&hour.len()),
        "does not start with one or two digits",
    )?;
    let hour: u32 = hour.parse().map_err(|_| "has no hour")?;
    require(hour <= 24, "has an hour above 24")?;
    let sixty = |tail: &str| -> Option<usize> {
        let part = tail.strip_prefix(':')?.get(..2)?;
        (part.bytes().all(|b| b.is_ascii_digit()) && part < "60").then_some(3)
    };
    let minutes = sixty(tail);
    let tail = &tail[minutes.unwrap_or(0)..];
    let tail = &tail[minutes.and(sixty(tail)).unwrap_or(0)..];
    let marker = tail.strip_prefix(' ').unwrap_or(tail).to_ascii_lowercase();
    let markers = ["am", "pm", "a.m", "p.m", "a.m.", "p.m."];
    if minutes.is_none() {
        require(
            markers.contains(&marker.as_str()),
            "has neither minutes nor AM or PM",
        )?;
        return require(
            (1..=12).contains(&hour),
            "has an hour outside 1 to 12 before AM or PM",
        );
    }
    if marker.is_empty() {
        let mut after = rest.chars();
        let goes_on = match after.next() {
            Some(':') => after.next().is_some_and(is_letter_or_digit),
            next => next.is_some_and(|c| c.is_ascii_digit()),
        };
        require(!goes_on, "is the start of a longer run")
    } else {
        require(
            markers.contains(&marker.as_str()),
            "ends with something that is not AM or PM",
        )
    }
}

/// xorshift64*: small, fast and the same on every run with the same seed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A number from 0 up to, but not including, `n`.
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

/// Pieces that start, end or break a token of a built-in kind.
#[rustfmt::skip]
const DICTIONARY: &[&str] = &[
    "@", "#", "$", "€", "£", "₪", "%", "+", "-", "--", "_", ".", "..", ",", ":", ";", "/", "\\",
    "'", "’", "\"", "(", ")", "[", "]", "!", "?", "~", "=", "*", "&", " ", "  ", "\t", "\n",
    "\r\n", "\u{b}", "\u{85}", "\u{a0}", "\u{2028}", "\u{3000}", "\u{200b}", "\u{200f}",
    "\u{feff}", "\u{301}", "0", "1", "5", "9", "12", "24", "25", "59", "60", "99", "2009",
    "20091", "123456", "٣", "½", "Ⅷ", "a", "Z", "é", "ß", "ש", "ا", "漢", "あ", "ʼ", "𝄞", "👍",
    "\u{1F469}\u{200D}\u{1F4BB}", "T", "Z", "am", "PM", "a.m.", "p.m", "Jan", "Feb", "May",
    "march", "DECEMBER", "Sept", "USD", "usd", "Dollars", "kr.", "km", "K", "all",
    "Convertible Mark", "http://", "https://", "www.", "ftp://", ".com", ".co.uk", "@example.com",
    "x@y.z", "2022-01-06", "2022-01-06T08:42:27.5+02:00", "02/17/2009", "17 Feb 2009", "10:30",
    "10:30:15 PM", "$5", "5 USD", "#tag", "@name", "1,000.50", "-.5", "6.0.1", "100%", "—", "–",
    "‐", "−", "。", "،", "？", "！", "＠", "＃", "＿", "．", "１２３", "：",
];

/// The pieces a mutation puts into a text: [`DICTIONARY`], and for the
/// catalog the texts its entries match and refuse, which are the nearest
/// things to a token of each kind.
fn pieces() -> Vec<&'static str> {
    let entries = catalog::entries().iter();
    let examples = entries.flat_map(|entry| {
        let matched = entry.matches.iter().map(|&(text, _)| text);
        matched.chain(entry.rejects.iter().copied())
    });
    DICTIONARY.iter().copied().chain(examples).collect()
}

/// The units of the long runs: the first characters of every piece, and
/// the same with a space after them. A matcher that reads without a bound
/// reads a run of such a unit whole, fails, and reads it again from the
/// next token, and the run is one a random mutation seldom builds.
fn run_units(pieces: &[&str]) -> Vec<String> {
    /// A unit longer than this starts no run a shorter one does not.
    const LONGEST: usize = 5;
    let mut units = std::collections::BTreeSet::new();
    for piece in pieces {
        for (end, c) in piece.char_indices().take(LONGEST) {
            let start = &piece[..end + c.len_utf8()];
            units.insert(start.to_string());
            units.insert(format!("{start} "));
        }
    }
    units.into_iter().collect()
}

/// The longest window of a document that is mutated. Short texts run more
/// mutations a second, and a failure in a short text is easier to read.
const WINDOW: usize = 400;

/// `text` cut to a window of at most `WINDOW` bytes at a random place.
fn window<'t>(text: &'t str, rng: &mut Rng) -> &'t str {
    if text.len() <= WINDOW {
        return text;
    }
    let mut start = rng.below(text.len() - WINDOW);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    let mut end = start + WINDOW;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[start..end]
}

/// A random character boundary of `text`.
fn boundary(text: &str, rng: &mut Rng) -> usize {
    let mut at = rng.below(text.len() + 1);
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// A random range of `text` between character boundaries, at most `max` bytes.
fn range(text: &str, rng: &mut Rng, max: usize) -> (usize, usize) {
    let start = boundary(text, rng);
    let mut end = (start + rng.below(max + 1)).min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (start, end.max(start))
}

/// `text` after between one and four random edits. `other` is a second
/// document to take ranges from, and `pieces` what to put in.
fn mutate(text: &str, other: &str, pieces: &[&str], rng: &mut Rng) -> String {
    let mut out = text.to_string();
    for _ in 0..1 + rng.below(4) {
        let (start, end) = range(&out, rng, 24);
        match rng.below(9) {
            // Insert a piece that means something to a matcher.
            0..=2 => out.insert_str(start, rng.pick(pieces)),
            // Replace a range with such a piece.
            3 => out.replace_range(start..end, rng.pick(pieces)),
            // Delete a range.
            4 => out.replace_range(start..end, ""),
            // Repeat a range, a few times or many: long runs are where
            // lexing goes quadratic.
            5 => {
                let times = if rng.below(4) == 0 {
                    200 + rng.below(3000)
                } else {
                    2 + rng.below(6)
                };
                let piece = if start == end {
                    rng.pick(pieces).to_string()
                } else {
                    out[start..end].to_string()
                };
                out.insert_str(start, &piece.repeat(times));
            }
            // Bring in a range of another document.
            6 => {
                let (from, to) = range(other, rng, 40);
                out.insert_str(start, &other[from..to]);
            }
            // Swap the case of a range.
            7 => {
                let swapped: String = out[start..end]
                    .chars()
                    .flat_map(|c| {
                        if c.is_lowercase() {
                            c.to_uppercase().collect::<Vec<_>>()
                        } else {
                            c.to_lowercase().collect()
                        }
                    })
                    .collect();
                out.replace_range(start..end, &swapped);
            }
            // Cut the text short.
            _ => out.truncate(start),
        }
    }
    out
}

/// Cuts `text` down while it still breaks the same check on the same kind.
fn minimise(checker: &Checker, text: &str, finding: &Finding) -> String {
    let still_fails = |candidate: &str| {
        let mut rng = Rng(1);
        // The sampled check looks at eight tokens; ask a few times.
        (0..4).any(|_| {
            checker
                .check(candidate, &mut rng)
                .is_some_and(|f| f.check == finding.check && f.kind == finding.kind)
        })
    };
    let mut chars: Vec<char> = text.chars().collect();
    let mut chunk = chars.len().div_ceil(2);
    while chunk > 0 {
        let mut start = 0;
        while start < chars.len() {
            let end = (start + chunk).min(chars.len());
            let candidate: String = chars[..start].iter().chain(&chars[end..]).collect();
            if still_fails(&candidate) {
                chars.drain(start..end);
            } else {
                start = end;
            }
        }
        chunk /= 2;
    }
    chars.into_iter().collect()
}

struct Report {
    /// The first text that broke each check on each kind in each lexer,
    /// cut down.
    findings: BTreeMap<(&'static str, &'static str, &'static str), (String, String, usize)>,
}

impl Report {
    /// Checks `text` with `checker` and records what it finds.
    fn check(&mut self, checker: &Checker, text: &str, rng: &mut Rng) {
        if let Some(finding) = checker.check(text, rng) {
            self.record(checker, text, finding);
        }
    }

    fn record(&mut self, checker: &Checker, text: &str, finding: Finding) {
        let key = (checker.name, finding.check, finding.kind);
        if let Some(entry) = self.findings.get_mut(&key) {
            entry.2 += 1;
            return;
        }
        // A slow text is slow because it is long; cutting it down proves little.
        let small = if finding.check == "time" {
            text.chars().take(120).collect()
        } else {
            minimise(checker, text, &finding)
        };
        let mut rng = Rng(1);
        let detail = checker
            .check(&small, &mut rng)
            .map_or(finding.detail, |f| f.detail);
        eprintln!(
            "FAIL {} {} with {}: {detail}\n     text: {small:?}",
            finding.check, finding.kind, checker.name
        );
        self.findings.insert(key, (small, detail, 1));
    }
}

fn main() -> ExitCode {
    let mut seconds = 30;
    let mut seed = 0x9E37_79B9_7F4A_7C15_u64;
    let mut files = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut number = |name: &str| -> u64 {
            args.next()
                .and_then(|value| value.parse().ok())
                .unwrap_or_else(|| {
                    eprintln!("fuzz: {name} needs a number");
                    std::process::exit(2);
                })
        };
        match arg.as_str() {
            "--seconds" => seconds = number("--seconds"),
            "--seed" => seed = number("--seed").max(1),
            option if option.starts_with("--") => {
                eprintln!("fuzz: unknown option {option}");
                return ExitCode::from(2);
            }
            _ => files.push(arg),
        }
    }
    if files.is_empty() {
        eprintln!("usage: fuzz [--seconds N] [--seed N] FILE...");
        return ExitCode::from(2);
    }

    let contents: Vec<String> = files
        .iter()
        .map(|file| std::fs::read_to_string(file).unwrap_or_else(|error| panic!("{file}: {error}")))
        .collect();
    let docs: Vec<&str> = contents
        .iter()
        .flat_map(|content| content.split('\0'))
        .filter(|doc| !doc.is_empty())
        .collect();
    if docs.is_empty() {
        eprintln!("fuzz: the files hold no documents");
        return ExitCode::from(2);
    }

    let default = Checker::new("the default lexer", Vec::new);
    let whole_catalog = Checker::new("the whole catalog", catalog::all);
    let groups = [
        Checker::new("the network group", || catalog::group(Group::Network)),
        Checker::new("the secrets group", || catalog::group(Group::Secrets)),
        Checker::new("the finance group", || catalog::group(Group::Finance)),
        Checker::new("the personal group", || catalog::group(Group::Personal)),
        Checker::new("the technical group", || catalog::group(Group::Technical)),
        Checker::new("the text group", || catalog::group(Group::Text)),
    ];
    assert_eq!(groups.len(), Group::ALL.len(), "a group is not checked");
    let pieces = pieces();
    let mut rng = Rng(seed);
    let mut report = Report {
        findings: BTreeMap::new(),
    };

    let started = Instant::now();
    let mut bytes = 0;
    for doc in &docs {
        bytes += doc.len();
        report.check(&default, doc, &mut rng);
        report.check(&whole_catalog, doc, &mut rng);
    }
    println!(
        "documents: {} checked, {:.1} MB, {:.1} s",
        docs.len(),
        bytes as f64 / 1e6,
        started.elapsed().as_secs_f64()
    );

    // Long enough for a quadratic run to take many times its budget.
    const RUN: usize = 40_000;
    let started = Instant::now();
    let units = run_units(&pieces);
    for unit in &units {
        let text = unit.repeat(RUN / unit.len());
        for checker in [&default, &whole_catalog] {
            let (_, elapsed) = checker.timed(&text);
            if let Some(finding) = checker.too_slow(&text, elapsed) {
                report.record(checker, &text, finding);
            }
        }
    }
    println!(
        "long runs: {} checked, {:.1} s",
        units.len(),
        started.elapsed().as_secs_f64()
    );

    let started = Instant::now();
    let mut mutations = 0u64;
    while started.elapsed().as_secs() < seconds {
        for _ in 0..256 {
            let doc = window(docs[rng.below(docs.len())], &mut rng);
            let other = window(docs[rng.below(docs.len())], &mut rng);
            let text = mutate(doc, other, &pieces, &mut rng);
            mutations += 1;
            report.check(&default, &text, &mut rng);
            report.check(&whole_catalog, &text, &mut rng);
            let group = &groups[rng.below(groups.len())];
            report.check(group, &text, &mut rng);
        }
    }
    println!("mutations: {mutations} checked, {seconds} s, seed {seed}");

    if report.findings.is_empty() {
        println!("no check failed");
        return ExitCode::SUCCESS;
    }
    println!("\n{} checks failed:", report.findings.len());
    for ((lexer, check, kind), (text, detail, times)) in &report.findings {
        println!("- {check} {kind} with {lexer} ({times} times): {detail}\n  {text:?}");
    }
    ExitCode::FAILURE
}
