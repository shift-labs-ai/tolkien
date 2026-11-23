//! Matchers written as a user of the crate writes them, with its public
//! interface only, and added to the built-in ones with [`Lexer::with`].
//!
//! They are the kind a detector of sensitive data needs. Each is written
//! twice: by hand, which shows what a matcher of your own has to get right,
//! and as a [`Pattern`], which says the same in data.

use tolkien::chars::is_letter_or_digit;
use tolkien::pattern::{Class, Pattern, Step};
use tolkien::{matchers, Cursor, Lexer, Matcher, Trie};

#[path = "support/reference.rs"]
mod reference;

/// 13 to 19 digits in at most eight groups joined by single spaces or
/// dashes: `4111 1111 1111 1111`.
///
/// It says where it can start, so the lexer asks it only at a digit. A
/// separator is taken only when a digit follows it, so the token does not
/// end with one. The number of groups is bounded: without that it reads a
/// whole column of numbers, fails, and reads it again from the next one.
struct CardNumber;

impl Matcher for CardNumber {
    fn kind(&self) -> &'static str {
        "CARD_NUMBER"
    }

    fn can_start(&self, first: char) -> bool {
        first.is_ascii_digit()
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        let mut digits = cur.advance_while(|c| c.is_ascii_digit());
        if digits == 0 {
            return false;
        }
        for _ in 0..7 {
            if !cur.followed_by(|c| c == ' ' || c == '-', |c| c.is_ascii_digit()) {
                break;
            }
            cur.advance();
            digits += cur.advance_while(|c| c.is_ascii_digit());
        }
        (13..=19).contains(&digits) && !cur.peek().is_some_and(is_letter_or_digit)
    }
}

/// Four numbers from 0 to 255 joined by full stops: `192.168.1.20`.
///
/// It reads the value of what it passed with [`Cursor::since`], and refuses
/// a run that goes on, as `1.2.3.4.5` does.
struct Ipv4;

impl Matcher for Ipv4 {
    fn kind(&self) -> &'static str {
        "IPV4"
    }

    fn can_start(&self, first: char) -> bool {
        first.is_ascii_digit()
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        for part in 0..4 {
            if part > 0 && !cur.advance_if(|c| c == '.') {
                return false;
            }
            let start = cur.pos();
            let digits = cur.advance_while(|c| c.is_ascii_digit());
            let value = cur.since(start).parse::<u16>();
            if !(1..=3).contains(&digits) || value.map_or(true, |value| value > 255) {
                return false;
            }
        }
        !cur.followed_by(|c| c == '.', |c| c.is_ascii_digit())
            && !cur.peek().is_some_and(is_letter_or_digit)
    }
}

/// Six pairs of hex digits joined by the same colon or dash:
/// `1d:d8:97:09:54:39`.
///
/// Its tokens start with a letter or a digit, and so do those of many
/// built-in matchers. It stands before them, so it wins.
struct MacAddress;

impl Matcher for MacAddress {
    fn kind(&self) -> &'static str {
        "MAC_ADDRESS"
    }

    fn can_start(&self, first: char) -> bool {
        first.is_ascii_hexdigit()
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        let pair = |cur: &mut Cursor<'_>| cur.advance_exactly(2, |c| c.is_ascii_hexdigit());
        if !pair(cur) {
            return false;
        }
        let Some(separator) = cur.peek().filter(|c| matches!(c, ':' | '-')) else {
            return false;
        };
        (0..5).all(|_| cur.advance_if(|c| c == separator) && pair(cur))
            && !cur.peek().is_some_and(is_letter_or_digit)
    }
}

/// A word from a list, in the case it is written in or in a common one.
///
/// A matcher may hold data. This one does not say where it can start, so it
/// is asked at every character, and the lexer is slower for it.
struct Keyword(Trie);

impl Matcher for Keyword {
    fn kind(&self) -> &'static str {
        "KEYWORD"
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        self.0.advance_longest(cur) && !cur.peek().is_some_and(is_letter_or_digit)
    }
}

fn keywords() -> Trie {
    Trie::from_words_in_common_cases(["confidential", "secret"])
}

/// The four matchers above, each said as a pattern.
fn patterns() -> Vec<Pattern> {
    let digit = || Class::range('0', '9');
    let hex = || digit() | Class::range('a', 'f') | Class::range('A', 'F');
    let letter_or_digit = || Class::letter() | Class::digit();
    let part = || Step::number(3, 255);
    let dot = || Step::text(".");
    vec![
        Pattern::new(
            "CARD_NUMBER",
            [
                Step::run(digit()),
                Step::repeat(7, [Step::one(Class::chars(" -")), Step::run(digit())]),
                Step::not(letter_or_digit()),
            ],
        )
        .with_count(digit(), 13, 19),
        Pattern::new(
            "IPV4",
            [
                part(),
                dot(),
                part(),
                dot(),
                part(),
                dot(),
                part(),
                Step::not_before([dot(), Step::one(digit())]),
                Step::not(letter_or_digit()),
            ],
        ),
        Pattern::new(
            "MAC_ADDRESS",
            [
                Step::exactly(2, hex()),
                Step::separator(Class::chars(":-")),
                Step::exactly(2, hex()),
                Step::same_separator(),
                Step::exactly(2, hex()),
                Step::same_separator(),
                Step::exactly(2, hex()),
                Step::same_separator(),
                Step::exactly(2, hex()),
                Step::same_separator(),
                Step::exactly(2, hex()),
                Step::not(letter_or_digit()),
            ],
        ),
        Pattern::new(
            "KEYWORD",
            [Step::words(keywords()), Step::not(letter_or_digit())],
        ),
    ]
}

fn added() -> Vec<Box<dyn Matcher>> {
    vec![
        Box::new(CardNumber),
        Box::new(Ipv4),
        Box::new(MacAddress),
        Box::new(Keyword(keywords())),
    ]
}

fn kinds_and_texts<'t>(lexer: &Lexer, text: &'t str) -> Vec<(&'static str, &'t str)> {
    lexer
        .tokens(text)
        .filter(|token| token.kind != matchers::WHITE_SPACE)
        .map(|token| (token.kind, token.text))
        .collect()
}

#[test]
fn a_matcher_is_tested_alone_with_match_prefix() {
    assert_eq!(
        CardNumber.match_prefix("4111-1111-1111-1111."),
        Some("4111-1111-1111-1111")
    );
    assert_eq!(CardNumber.match_prefix("4111 1111 1111"), None);
    assert_eq!(CardNumber.match_prefix("4111 1111 1111 1111x"), None);
    assert_eq!(Ipv4.match_prefix("10.0.0.255, and"), Some("10.0.0.255"));
    assert_eq!(Ipv4.match_prefix("10.0.0.256"), None);
    assert_eq!(Ipv4.match_prefix("1.2.3.4.5"), None);
    assert_eq!(
        MacAddress.match_prefix("1d-d8-97-09-54-39 "),
        Some("1d-d8-97-09-54-39")
    );
    assert_eq!(MacAddress.match_prefix("1d:d8-97:09:54:39"), None);
}

#[test]
fn added_matchers_are_tried_before_the_built_in_ones() {
    let lexer = Lexer::with(added()).unwrap();
    let text = "Card 4111 1111 1111 1111 from 192.168.1.20 (1d:d8:97:09:54:39). \
                Secret: paid $5 on 17.02.2009 at 10:30, see v1.2";
    assert_eq!(
        kinds_and_texts(&lexer, text),
        [
            ("WORD", "Card"),
            ("CARD_NUMBER", "4111 1111 1111 1111"),
            ("WORD", "from"),
            ("IPV4", "192.168.1.20"),
            ("SYMBOL", "("),
            ("MAC_ADDRESS", "1d:d8:97:09:54:39"),
            ("SYMBOL", ")"),
            ("SYMBOL", "."),
            ("KEYWORD", "Secret"),
            ("SYMBOL", ":"),
            ("WORD", "paid"),
            ("CURRENCY", "$5"),
            ("WORD", "on"),
            ("DATE", "17.02.2009"),
            ("WORD", "at"),
            ("TIME", "10:30"),
            ("SYMBOL", ","),
            ("WORD", "see"),
            ("ALPHA_NUMERIC_SEQUENCE", "v1"),
            ("LIKE_NUMBER", ".2"),
        ]
    );
    let order: Vec<_> = lexer.kinds().collect();
    assert_eq!(
        order[..4],
        ["CARD_NUMBER", "IPV4", "MAC_ADDRESS", "KEYWORD"]
    );
}

/// The built-in matchers keep their order whatever is added, and wherever:
/// a matcher given after them moves up to the first one it overrides.
#[test]
fn adding_a_matcher_does_not_reorder_the_others() {
    struct Overriding(&'static str, &'static [&'static str]);
    impl Matcher for Overriding {
        fn kind(&self) -> &'static str {
            self.0
        }
        fn overrides(&self) -> &'static [&'static str] {
            self.1
        }
        fn matches(&self, _: &mut Cursor<'_>) -> bool {
            false
        }
    }
    let mut list = matchers::all();
    list.push(Box::new(Overriding(
        "A",
        &[matchers::DATE, matchers::LIKE_NUMBER],
    )));
    list.push(Box::new(Overriding(
        "B",
        &[matchers::WORD, matchers::DATE_ISO_8601],
    )));
    list.push(Box::new(Overriding("C", &[])));
    let lexer = Lexer::new(list).unwrap();
    let order: Vec<_> = lexer.kinds().collect();
    let built_in: Vec<_> = matchers::all().iter().map(|m| m.kind()).collect();
    let kept: Vec<_> = order
        .iter()
        .copied()
        .filter(|kind| built_in.contains(kind))
        .collect();
    assert_eq!(kept, built_in);
    assert_eq!(order[..4], ["B", "DATE_ISO_8601", "A", "DATE"]);
    assert_eq!(order.last(), Some(&"C"));
}

/// `Lexer::with` is faster than asking every matcher, and gives the same
/// tokens: here on texts at the edge of what the added matchers start with,
/// and on random text.
#[test]
fn a_lexer_with_added_matchers_gives_the_tokens_of_asking_every_matcher() {
    let lexer = Lexer::with(added()).unwrap();
    let mut all = added();
    all.extend(matchers::all());
    let check = |lexer: &Lexer, all: &[Box<dyn Matcher>], text: &str| {
        let tokens: Vec<_> = lexer
            .tokens(text)
            .map(|token| (token.kind, token.start, token.end))
            .collect();
        assert_eq!(tokens, reference::tokens(all, text), "{text:?}");
    };
    const PIECES: &[&str] = &[
        "4111",
        "1111",
        "1",
        "9",
        "255",
        "256",
        "10",
        ".",
        ":",
        "-",
        " ",
        " ",
        ",",
        "1d",
        "d8",
        "ff",
        "a",
        "Z",
        "secret",
        "Secret",
        "SECRET",
        "secrets",
        "confidential",
        "é",
        "\u{a0}",
        "日本",
        "Feb",
        "2009",
        "17",
        "$",
        "@",
        "x@y.zz",
        "www.",
        "\n",
        "(",
        "#",
    ];
    for piece in PIECES {
        check(&lexer, &all, piece);
    }
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) as usize
    };
    for _ in 0..4000 {
        let text: String = (0..next() % 16)
            .map(|_| PIECES[next() % PIECES.len()])
            .collect();
        check(&lexer, &all, &text);
    }
    // A matcher that can start anywhere, alone: no shortcut is left.
    let keyword = || -> Vec<Box<dyn Matcher>> {
        vec![Box::new(Keyword(Trie::from_words(["été", "a b", " x"])))]
    };
    let lexer = Lexer::with(keyword()).unwrap();
    let mut all = keyword();
    all.extend(matchers::all());
    for text in [
        "été",
        "un été chaud",
        "a b",
        "a  b",
        "word x",
        "é x",
        "日本 x",
    ] {
        check(&lexer, &all, text);
    }
}

/// Each pattern finds what the matcher written by hand finds, at every
/// place of random text made of the pieces both are built from. It may say
/// more exactly where it can start, as the keyword pattern does.
#[test]
fn a_pattern_says_what_the_matcher_written_by_hand_does() {
    const PIECES: &[&str] = &[
        "4111",
        "1111",
        "11",
        "1",
        "9",
        "255",
        "256",
        "0",
        ".",
        ":",
        "-",
        " ",
        "1d",
        "d8",
        "FF",
        "fg",
        "a",
        "Z",
        "secret",
        "Secret",
        "SECRET",
        "secrets",
        "confidential",
        "é",
        "٣",
        "x",
        "4111 1111 1111 111",
        "10.0.0",
        "1d:d8:97:09:54",
        "1d-d8-97-09-54",
    ];
    let by_hand = added();
    let patterns = patterns();
    let mut state: u64 = 0xD1B5_4A32_D192_ED03;
    let mut next = || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) as usize
    };
    let mut matched = [0; 4];
    for _ in 0..20_000 {
        let text: String = (0..next() % 14)
            .map(|_| PIECES[next() % PIECES.len()])
            .collect();
        for (at, _) in text.char_indices() {
            for (index, (hand, pattern)) in by_hand.iter().zip(&patterns).enumerate() {
                let rest = &text[at..];
                assert_eq!(hand.kind(), pattern.kind());
                assert_eq!(
                    pattern.match_prefix(rest),
                    hand.match_prefix(rest),
                    "{} at {rest:?}",
                    hand.kind()
                );
                matched[index] += usize::from(hand.match_prefix(rest).is_some());
            }
        }
    }
    // The random text holds tokens of every kind.
    assert!(matched.iter().all(|&count| count > 50), "{matched:?}");
}

/// A matcher that reads far and then fails is asked again at the next
/// token, and lexing takes quadratic time. Each text here is a run one of
/// the matchers reads into. In a debug build a run takes a few hundredths
/// of a second when lexing is linear, and many seconds when it is not.
#[test]
fn long_runs_are_lexed_in_linear_time_with_added_matchers() {
    const UNITS: &[&str] = &[
        "1 ", "12-", "1111 ", "1.", "10.0.", "1d:", "1d-", "ab:", "secret", "secre ", "a ",
    ];
    let by_hand = Lexer::with(added()).unwrap();
    let as_patterns = Lexer::with(
        patterns()
            .into_iter()
            .map(|pattern| Box::new(pattern) as Box<dyn Matcher>),
    )
    .unwrap();
    for unit in UNITS {
        let text = unit.repeat(20_000 / unit.len());
        for (name, lexer) in [("by hand", &by_hand), ("as patterns", &as_patterns)] {
            let started = std::time::Instant::now();
            let end = lexer.tokens(&text).map(|token| token.end).last();
            assert_eq!(end, Some(text.len()));
            let elapsed = started.elapsed();
            assert!(
                elapsed.as_secs() < 2,
                "a run of {unit:?} took {elapsed:?} {name}"
            );
        }
    }
}

/// A matcher added to the built-in ones cannot take the kind of one.
#[test]
fn an_added_matcher_needs_a_kind_of_its_own() {
    struct Word;
    impl Matcher for Word {
        fn kind(&self) -> &'static str {
            matchers::WORD
        }
        fn matches(&self, _: &mut Cursor<'_>) -> bool {
            false
        }
    }
    assert_eq!(
        Lexer::with([Box::new(Word) as Box<dyn Matcher>]).unwrap_err(),
        tolkien::Error::DuplicateKind(matchers::WORD)
    );
}
