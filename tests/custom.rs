//! Matchers written as a user of the crate writes them, with its public
//! interface only, and added to the built-in ones with [`Lexer::with`].
//!
//! They are the kind a detector of sensitive data needs. Each shows one
//! thing a matcher of your own has to get right.

use tolkien::{matchers, Cursor, Lexer, Matcher, Trie};

#[path = "support/reference.rs"]
mod reference;

fn is_letter_or_digit(c: char) -> bool {
    c.is_alphanumeric()
}

/// 13 to 19 digits in groups joined by single spaces or dashes:
/// `4111 1111 1111 1111`.
///
/// It says where it can start, so the lexer asks it only at a digit. A
/// separator is taken only when a digit follows it, so the token does not
/// end with one.
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
        while cur.followed_by(|c| c == ' ' || c == '-', |c| c.is_ascii_digit()) {
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

fn added() -> Vec<Box<dyn Matcher>> {
    vec![
        Box::new(CardNumber),
        Box::new(Ipv4),
        Box::new(MacAddress),
        Box::new(Keyword(Trie::from_words_in_common_cases([
            "confidential",
            "secret",
        ]))),
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
