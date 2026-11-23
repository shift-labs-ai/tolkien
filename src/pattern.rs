//! Matchers described as data.
//!
//! A [`Pattern`] is a [`Matcher`] built from a list of [`Step`]s, each of
//! which is one of the few things a hand-written matcher does with a
//! [`Cursor`]: take a character of a class, take a run of them, take a
//! fixed text, look at what comes next, step back. It needs no code, so it
//! can be written in a configuration file or in another language and still
//! run as fast as the matchers here.
//!
//! ```
//! use tolkien::pattern::{Class, Pattern, Step};
//! use tolkien::{Lexer, Matcher};
//!
//! // `AKIA` and sixteen capital letters or digits.
//! let key = Pattern::new(
//!     "ACCESS_KEY",
//!     [
//!         Step::text("AKIA"),
//!         Step::exactly(16, Class::range('A', 'Z') | Class::range('0', '9')),
//!         Step::not(Class::letter() | Class::digit()),
//!     ],
//! );
//! assert_eq!(key.match_prefix("AKIAIOSFODNN7EXAMPLE."), Some("AKIAIOSFODNN7EXAMPLE"));
//! assert_eq!(key.match_prefix("AKIAIOSFODNN7EXAMPLES"), None);
//!
//! let lexer = Lexer::with([Box::new(key) as Box<dyn Matcher>]).unwrap();
//! let kinds: Vec<_> = lexer.tokens("key AKIAIOSFODNN7EXAMPLE").map(|t| t.kind).collect();
//! assert_eq!(kinds, ["WORD", "WHITE_SPACE", "ACCESS_KEY"]);
//! ```
//!
//! A pattern says where it can start by itself: [`Matcher::can_start`] is
//! worked out from its first steps.
//!
//! What a pattern cannot say is a rule over the whole token, such as a
//! checksum. Lex with the pattern, and check the tokens of its kind
//! afterwards.

use std::ops::BitOr;

use crate::chars::classes;
use crate::cursor::Cursor;
use crate::matcher::Matcher;
use crate::trie::Trie;
use crate::unicode_tables as tables;

/// A set of characters.
///
/// A class is made of the named sets of [`chars`](crate::chars), of single
/// characters and of ranges, joined with `|`:
///
/// ```
/// use tolkien::pattern::Class;
///
/// let class = Class::letter() | Class::chars("_-") | Class::range('0', '9');
/// assert!(class.contains('é') && class.contains('_') && class.contains('7'));
/// assert!(!class.contains(' '));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Class {
    /// The named sets in the class, as a mask of the bits in the tables.
    named: u32,
    /// The ranges in the class, each from its first character to its last.
    ranges: Vec<(char, char)>,
    /// The ASCII characters in the class, by either of the two: bit `c`
    /// stands for the character `c`.
    ascii: u128,
}

impl Class {
    fn of_named(named: u32) -> Self {
        Self {
            named,
            ..Self::default()
        }
        .with_ascii()
    }

    /// Fills in the ASCII characters from the named sets and the ranges.
    fn with_ascii(mut self) -> Self {
        self.ascii = 0;
        for byte in 0..128u8 {
            if self.in_sets(byte as char) {
                self.ascii |= 1 << byte;
            }
        }
        self
    }

    fn in_sets(&self, c: char) -> bool {
        classes(c) & self.named != 0
            || self
                .ranges
                .iter()
                .any(|&(first, last)| (first..=last).contains(&c))
    }

    /// Each character of `chars`.
    pub fn chars(chars: &str) -> Self {
        Self {
            ranges: chars.chars().map(|c| (c, c)).collect(),
            ..Self::default()
        }
        .with_ascii()
    }

    /// The characters from `first` to `last`, both included.
    pub fn range(first: char, last: char) -> Self {
        Self {
            ranges: vec![(first, last)],
            ..Self::default()
        }
        .with_ascii()
    }

    /// The set with the name `name`, if there is one. The names are those
    /// of the functions here: `letter`, `digit`, `number`, `whitespace`,
    /// `line_break`, `dash`, `underscore`, `apostrophe`, `full_stop`,
    /// `comma`, `colon`, `slash` and `currency_symbol`.
    pub fn named(name: &str) -> Option<Self> {
        let named = match name {
            "letter" => tables::LETTER,
            "digit" => tables::DECIMAL_DIGIT,
            "number" => tables::NUMBER,
            "whitespace" => tables::WHITE_SPACE,
            "line_break" => tables::LINE_TERMINATION,
            "dash" => tables::DASH,
            "underscore" => tables::UNDERSCORE,
            "apostrophe" => tables::APOSTROPHE,
            "full_stop" => tables::FULL_STOP,
            "comma" => tables::COMMA,
            "colon" => tables::COLON,
            "slash" => tables::SLASH,
            "currency_symbol" => tables::CURRENCY_SYMBOL,
            _ => return None,
        };
        Some(Self::of_named(named))
    }

    /// Letters of any script: [`chars::is_letter`](crate::chars::is_letter).
    pub fn letter() -> Self {
        Self::of_named(tables::LETTER)
    }

    /// Decimal digits of any script:
    /// [`chars::is_digit`](crate::chars::is_digit). The ASCII digits alone
    /// are `Class::range('0', '9')`.
    pub fn digit() -> Self {
        Self::of_named(tables::DECIMAL_DIGIT)
    }

    /// White space: [`chars::is_whitespace`](crate::chars::is_whitespace).
    pub fn whitespace() -> Self {
        Self::of_named(tables::WHITE_SPACE)
    }

    /// True when `c` is in the class.
    #[inline]
    pub fn contains(&self, c: char) -> bool {
        if c.is_ascii() {
            self.ascii & 1 << c as u32 != 0
        } else {
            self.in_sets(c)
        }
    }
}

impl BitOr for Class {
    type Output = Class;

    /// The characters of either class.
    fn bitor(mut self, other: Class) -> Class {
        self.named |= other.named;
        self.ranges.extend(other.ranges);
        self.ascii |= other.ascii;
        self
    }
}

/// One thing a [`Pattern`] does at the cursor. A step succeeds or fails,
/// and a pattern matches when all its steps succeed in turn.
#[derive(Debug)]
pub struct Step(Kind);

#[derive(Debug)]
enum Kind {
    Run {
        class: Class,
        min: usize,
        max: usize,
    },
    Text(String),
    Words(Box<Trie>),
    Number {
        max_digits: usize,
        max_value: u64,
    },
    Separator(Class),
    SameSeparator,
    Not(Class),
    NotBefore(Vec<Step>),
    Optional(Vec<Step>),
    Repeat(Vec<Step>),
    Either(Vec<Vec<Step>>),
    TrimEnd(Class),
}

impl Step {
    /// One character of `class`.
    pub fn one(class: Class) -> Self {
        Self::exactly(1, class)
    }

    /// One character of `class` or more, as many as there are.
    pub fn run(class: Class) -> Self {
        Self::between(1, usize::MAX, class)
    }

    /// Exactly `count` characters of `class`. More of them may follow: say
    /// [`Step::not`] after it to refuse that.
    pub fn exactly(count: usize, class: Class) -> Self {
        Self::between(count, count, class)
    }

    /// From `min` to `max` characters of `class`, as many as there are.
    pub fn between(min: usize, max: usize, class: Class) -> Self {
        Self(Kind::Run { class, min, max })
    }

    /// The text `text`, as it is written.
    pub fn text(text: &str) -> Self {
        Self(Kind::Text(text.to_string()))
    }

    /// The longest word of `words` that stands at the cursor.
    pub fn words(words: Trie) -> Self {
        Self(Kind::Words(Box::new(words)))
    }

    /// A number of one to `max_digits` ASCII digits that is at most
    /// `max_value`, such as a part of an IP address.
    pub fn number(max_digits: usize, max_value: u64) -> Self {
        Self(Kind::Number {
            max_digits,
            max_value,
        })
    }

    /// One character of `class`, which the pattern remembers:
    /// [`Step::same_separator`] takes that character again.
    pub fn separator(class: Class) -> Self {
        Self(Kind::Separator(class))
    }

    /// The character the last [`Step::separator`] took. It fails where
    /// none was taken.
    pub fn same_separator() -> Self {
        Self(Kind::SameSeparator)
    }

    /// Succeeds, without moving, when the next character is not in `class`
    /// or the text has ended. At the end of a pattern it says the token is
    /// not the start of something longer.
    pub fn not(class: Class) -> Self {
        Self(Kind::Not(class))
    }

    /// Succeeds, without moving, when `steps` do not succeed here: a token
    /// that is not before a full stop and a digit, say, where
    /// [`Step::not`] looks at one character only.
    pub fn not_before(steps: impl IntoIterator<Item = Step>) -> Self {
        Self(Kind::NotBefore(steps.into_iter().collect()))
    }

    /// `steps`, if they succeed; nothing otherwise. It always succeeds.
    pub fn optional(steps: impl IntoIterator<Item = Step>) -> Self {
        Self(Kind::Optional(steps.into_iter().collect()))
    }

    /// `steps` again and again, as long as they succeed and move on. It
    /// always succeeds, also with no round.
    pub fn repeat(steps: impl IntoIterator<Item = Step>) -> Self {
        Self(Kind::Repeat(steps.into_iter().collect()))
    }

    /// The first of `choices` whose steps succeed.
    pub fn either<C>(choices: impl IntoIterator<Item = C>) -> Self
    where
        C: IntoIterator<Item = Step>,
    {
        let choices = choices.into_iter();
        Self(Kind::Either(
            choices.map(|steps| steps.into_iter().collect()).collect(),
        ))
    }

    /// Steps back over the characters of `class` just before the cursor,
    /// so that the token does not end with them. It always succeeds.
    pub fn trim_end(class: Class) -> Self {
        Self(Kind::TrimEnd(class))
    }

    /// Does the step at the cursor. `separator` is the character the
    /// pattern remembers.
    fn matches(&self, cur: &mut Cursor<'_>, separator: &mut Option<char>) -> bool {
        match &self.0 {
            Kind::Run { class, min, max } => {
                let mut count = 0;
                while count < *max && cur.advance_if(|c| class.contains(c)) {
                    count += 1;
                }
                count >= *min
            }
            Kind::Text(text) => cur.advance_str(text),
            Kind::Words(words) => words.advance_longest(cur),
            Kind::Number {
                max_digits,
                max_value,
            } => {
                let (mut digits, mut value) = (0, 0u64);
                while digits < *max_digits {
                    let Some(digit) = cur.peek().and_then(|c| c.to_digit(10)) else {
                        break;
                    };
                    value = value.saturating_mul(10).saturating_add(digit.into());
                    digits += 1;
                    cur.advance();
                }
                digits > 0 && value <= *max_value
            }
            Kind::Separator(class) => {
                *separator = cur.peek().filter(|&c| class.contains(c));
                separator.is_some() && cur.advance().is_some()
            }
            Kind::SameSeparator => separator.is_some_and(|same| cur.advance_if(|c| c == same)),
            Kind::Not(class) => !cur.peek().is_some_and(|c| class.contains(c)),
            // Where the steps succeed, this one fails, and the steps it is
            // among put the cursor back.
            Kind::NotBefore(steps) => !all(steps, cur, separator),
            Kind::Optional(steps) => {
                all(steps, cur, separator);
                true
            }
            Kind::Repeat(steps) => {
                let mut before = cur.pos();
                while all(steps, cur, separator) && cur.pos() > before {
                    before = cur.pos();
                }
                true
            }
            Kind::Either(choices) => choices.iter().any(|steps| all(steps, cur, separator)),
            Kind::TrimEnd(class) => {
                cur.back_while(|c| class.contains(c));
                true
            }
        }
    }

    /// What the step says of a text that starts with `first`.
    fn start(&self, first: char) -> Start {
        let taken = |takes: bool| if takes { Start::Taken } else { Start::Refused };
        match &self.0 {
            Kind::Run { max: 0, .. } => Start::Passed,
            Kind::Run { class, min, .. } => match class.contains(first) {
                true => Start::Taken,
                false if *min == 0 => Start::Passed,
                false => Start::Refused,
            },
            Kind::Text(text) => match text.chars().next() {
                Some(c) => taken(c == first),
                None => Start::Passed,
            },
            Kind::Words(words) => taken(words.can_start(first)),
            Kind::Number { .. } => taken(first.is_ascii_digit()),
            Kind::Separator(class) => taken(class.contains(first)),
            Kind::SameSeparator => Start::Refused,
            Kind::Not(class) => match class.contains(first) {
                true => Start::Refused,
                false => Start::Passed,
            },
            Kind::Optional(steps) | Kind::Repeat(steps) => match start(steps, first) {
                Start::Taken => Start::Taken,
                _ => Start::Passed,
            },
            Kind::Either(choices) => {
                let starts = || choices.iter().map(|steps| start(steps, first));
                if starts().any(|start| start == Start::Taken) {
                    Start::Taken
                } else if starts().any(|start| start == Start::Passed) {
                    Start::Passed
                } else {
                    Start::Refused
                }
            }
            // What the steps refuse is not worked out: they may fail later.
            Kind::NotBefore(_) | Kind::TrimEnd(_) => Start::Passed,
        }
    }
}

/// What a step, or a list of them, says of the first character of a text.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Start {
    /// It may take the character.
    Taken,
    /// It fails at the character.
    Refused,
    /// It may succeed without taking the character: what follows says.
    Passed,
}

/// Does `steps` in turn. Where one fails, the cursor and the separator are
/// as they were before the first.
fn all(steps: &[Step], cur: &mut Cursor<'_>, separator: &mut Option<char>) -> bool {
    let before = (cur.pos(), *separator);
    let matched = steps.iter().all(|step| step.matches(cur, separator));
    if !matched {
        cur.set_pos(before.0);
        *separator = before.1;
    }
    matched
}

/// What `steps` say of a text that starts with `first`.
fn start(steps: &[Step], first: char) -> Start {
    steps
        .iter()
        .map(|step| step.start(first))
        .find(|&start| start != Start::Passed)
        .unwrap_or(Start::Passed)
}

/// A [`Matcher`] built from [`Step`]s. See the [module](self).
#[derive(Debug)]
pub struct Pattern {
    kind: &'static str,
    steps: Vec<Step>,
    /// A class, and how many characters of it the token may hold.
    count: Option<(Class, usize, usize)>,
    /// The ASCII characters a token can start with: bit `c` stands for the
    /// character `c`.
    ascii_starts: u128,
}

impl Pattern {
    /// A matcher of the kind `kind` that matches where `steps` succeed in
    /// turn, and takes what they passed.
    ///
    /// The name of the kind is kept for as long as the program runs, as the
    /// names of the built-in kinds are. Build a pattern once, not once for
    /// every text.
    pub fn new(kind: &str, steps: impl IntoIterator<Item = Step>) -> Self {
        let mut pattern = Self {
            kind: Box::leak(kind.to_string().into_boxed_str()),
            steps: steps.into_iter().collect(),
            count: None,
            ascii_starts: 0,
        };
        for byte in 0..128u8 {
            if start(&pattern.steps, byte as char) == Start::Taken {
                pattern.ascii_starts |= 1 << byte;
            }
        }
        pattern
    }

    /// The same pattern, which matches only when the token holds from
    /// `min` to `max` characters of `class`: a card number has 13 to 19
    /// digits, however they are grouped.
    pub fn with_count(mut self, class: Class, min: usize, max: usize) -> Self {
        self.count = Some((class, min, max));
        self
    }
}

impl Matcher for Pattern {
    fn kind(&self) -> &'static str {
        self.kind
    }

    fn can_start(&self, first: char) -> bool {
        if first.is_ascii() {
            self.ascii_starts & 1 << first as u32 != 0
        } else {
            start(&self.steps, first) == Start::Taken
        }
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        let from = cur.pos();
        all(&self.steps, cur, &mut None)
            && self.count.as_ref().is_none_or(|(class, min, max)| {
                let count = cur.since(from).chars().filter(|&c| class.contains(c));
                (*min..=*max).contains(&count.count())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ascii_digit() -> Class {
        Class::range('0', '9')
    }

    #[test]
    fn a_class_is_named_sets_characters_and_ranges() {
        let class = Class::letter() | Class::chars("_-。") | Class::range('0', '3');
        for c in ['a', 'é', '漢', '_', '-', '。', '0', '3'] {
            assert!(class.contains(c), "{c}");
        }
        for c in ['4', ' ', '.', '٣', '+'] {
            assert!(!class.contains(c), "{c}");
        }
        assert!(!Class::default().contains('a'));
        assert_eq!(Class::named("letter"), Some(Class::letter()));
        assert_eq!(Class::named("vowel"), None);
        assert!(Class::named("dash").unwrap().contains('—'));
    }

    #[test]
    fn runs_take_as_many_as_there_are_within_their_bounds() {
        let two_to_three = Pattern::new("N", [Step::between(2, 3, ascii_digit())]);
        assert_eq!(two_to_three.match_prefix("1"), None);
        assert_eq!(two_to_three.match_prefix("12x"), Some("12"));
        assert_eq!(two_to_three.match_prefix("12345"), Some("123"));
        let whole = Pattern::new("N", [Step::run(ascii_digit()), Step::not(ascii_digit())]);
        assert_eq!(whole.match_prefix("12345 "), Some("12345"));
        let exact = Pattern::new(
            "N",
            [Step::exactly(2, ascii_digit()), Step::not(ascii_digit())],
        );
        assert_eq!(exact.match_prefix("12"), Some("12"));
        assert_eq!(exact.match_prefix("123"), None);
    }

    #[test]
    fn optional_and_repeated_steps_leave_no_half_taken_text() {
        // Digits in groups: a dash is taken only with digits after it.
        let groups = Pattern::new(
            "GROUPS",
            [
                Step::run(ascii_digit()),
                Step::repeat([Step::one(Class::chars("-")), Step::run(ascii_digit())]),
                Step::optional([Step::text("!"), Step::text("!")]),
            ],
        );
        assert_eq!(groups.match_prefix("1-22-333-x"), Some("1-22-333"));
        assert_eq!(groups.match_prefix("1-!"), Some("1"));
        assert_eq!(groups.match_prefix("1!!"), Some("1!!"));
        assert_eq!(groups.match_prefix("-1"), None);
        // A round that takes nothing ends the repeat.
        let idle = Pattern::new(
            "IDLE",
            [Step::text("a"), Step::repeat([Step::optional([])])],
        );
        assert_eq!(idle.match_prefix("ab"), Some("a"));
    }

    #[test]
    fn a_number_has_a_value_and_a_separator_is_taken_again() {
        let part = || Step::number(3, 255);
        let dot = || Step::text(".");
        let address = Pattern::new("IP", [part(), dot(), part(), dot(), part(), dot(), part()]);
        assert_eq!(address.match_prefix("10.0.0.255,"), Some("10.0.0.255"));
        assert_eq!(address.match_prefix("10.0.0.256"), None);
        assert_eq!(address.match_prefix("10.0.0."), None);

        let date = Pattern::new(
            "D",
            [
                Step::number(2, 31),
                Step::separator(Class::chars("/-")),
                Step::number(2, 12),
                Step::same_separator(),
                Step::exactly(4, ascii_digit()),
            ],
        );
        assert_eq!(date.match_prefix("17/02/2009"), Some("17/02/2009"));
        assert_eq!(date.match_prefix("17-02-2009"), Some("17-02-2009"));
        assert_eq!(date.match_prefix("17/02-2009"), None);
        let alone = Pattern::new("S", [Step::text("a"), Step::same_separator()]);
        assert_eq!(alone.match_prefix("aa"), None);

        let last = Pattern::new(
            "LAST",
            [part(), Step::not_before([dot(), Step::one(ascii_digit())])],
        );
        assert_eq!(last.match_prefix("10.x"), Some("10"));
        assert_eq!(last.match_prefix("10."), Some("10"));
        assert_eq!(last.match_prefix("10.5"), None);
    }

    #[test]
    fn either_takes_the_first_choice_that_succeeds_and_words_the_longest() {
        let address = Pattern::new(
            "URL",
            [
                Step::either([[Step::text("https://")], [Step::text("http://")]]),
                Step::run(Class::letter() | Class::chars(".")),
                Step::trim_end(Class::chars(".")),
            ],
        );
        assert_eq!(address.match_prefix("http://a.b. "), Some("http://a.b"));
        assert_eq!(address.match_prefix("https://a"), Some("https://a"));
        assert_eq!(address.match_prefix("ftp://a"), None);
        let unit = Pattern::new(
            "UNIT",
            [
                Step::words(Trie::from_words(["m", "mm", "km"])),
                Step::not(Class::letter()),
            ],
        );
        assert_eq!(unit.match_prefix("mm."), Some("mm"));
        assert_eq!(unit.match_prefix("mmm"), None);
    }

    #[test]
    fn a_count_is_over_the_whole_token() {
        let card = Pattern::new(
            "CARD",
            [
                Step::run(ascii_digit()),
                Step::repeat([Step::one(Class::chars(" -")), Step::run(ascii_digit())]),
            ],
        )
        .with_count(ascii_digit(), 13, 19);
        assert_eq!(
            card.match_prefix("4111 1111-1111 1111."),
            Some("4111 1111-1111 1111")
        );
        assert_eq!(card.match_prefix("4111 1111 1111"), None);
    }

    /// The start a pattern works out is never too narrow, and for all of
    /// these but the one with `not_before` it is exact: the pattern can
    /// start with a character when, and only when, some text that starts
    /// with it matches.
    #[test]
    fn a_pattern_can_start_where_its_first_steps_can() {
        let starts = |pattern: &Pattern| -> String {
            "abx1-.# é"
                .chars()
                .filter(|&c| pattern.can_start(c))
                .collect()
        };
        let step = |text| Step::text(text);
        for (steps, expected) in [
            (vec![step("ab")], "a"),
            (vec![Step::run(Class::letter())], "abxé"),
            (vec![Step::optional([step("#")]), step("x")], "x#"),
            (
                vec![Step::repeat([step("-")]), Step::run(ascii_digit())],
                "1-",
            ),
            (
                vec![Step::between(0, 2, Class::chars(".")), step("a")],
                "a.",
            ),
            (
                vec![Step::not(Class::chars("a")), Step::run(Class::letter())],
                "bxé",
            ),
            (vec![Step::either([[step("a")], [step("1")]])], "a1"),
            (
                vec![Step::either([vec![step("a")], vec![]]), step("b")],
                "ab",
            ),
            (vec![Step::number(2, 99)], "1"),
            (vec![Step::separator(Class::chars("-.")), step("a")], "-."),
            (vec![Step::words(Trie::from_words(["é", "x1"]))], "xé"),
            (vec![Step::trim_end(Class::chars(".")), step("b")], "b"),
            (
                vec![Step::not_before([step("ab")]), Step::run(Class::letter())],
                "abxé",
            ),
            (vec![Step::same_separator()], ""),
            (vec![Step::optional([step("a")])], "a"),
            (vec![], ""),
        ] {
            assert_eq!(starts(&Pattern::new("P", steps)), expected);
        }
    }

    #[test]
    fn a_pattern_that_takes_nothing_gives_no_token() {
        let nothing = Pattern::new("NOTHING", [Step::optional([Step::text("a")])]);
        assert_eq!(nothing.match_prefix("b"), None);
        assert_eq!(nothing.match_prefix("a"), Some("a"));
    }
}
