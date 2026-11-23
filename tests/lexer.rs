//! The lexer as a whole: token order, coverage of the text, and the rules
//! that decide which matcher wins.

use tolkien::matchers::{self, LikeNumber, Word, DIGIT, LETTER, LIKE_NUMBER};
use tolkien::{Cursor, Error, Lexer, Matcher, Token, OTHER};

fn kinds_and_texts<'t>(lexer: &Lexer, text: &'t str) -> Vec<(&'static str, &'t str)> {
    lexer.tokens(text).map(|t| (t.kind, t.text)).collect()
}

/// Every byte of the text belongs to exactly one token, in order.
fn assert_covers(lexer: &Lexer, text: &str) {
    let mut pos = 0;
    for token in lexer.tokens(text) {
        assert_eq!(
            token.start, pos,
            "gap or overlap before {token:?} in {text:?}"
        );
        assert!(token.end > token.start, "empty token {token:?} in {text:?}");
        assert_eq!(token.text, &text[token.start..token.end]);
        pos = token.end;
    }
    assert_eq!(pos, text.len(), "tokens stop early in {text:?}");
}

/// A matcher built from a function, for tests about the lexer itself.
struct Probe {
    kind: &'static str,
    overrides: &'static [&'static str],
    eat: fn(&mut Cursor<'_>) -> bool,
}

impl Matcher for Probe {
    fn kind(&self) -> &'static str {
        self.kind
    }

    fn overrides(&self) -> &'static [&'static str] {
        self.overrides
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        (self.eat)(cur)
    }
}

fn probe(
    kind: &'static str,
    overrides: &'static [&'static str],
    eat: fn(&mut Cursor<'_>) -> bool,
) -> Box<dyn Matcher> {
    Box::new(Probe {
        kind,
        overrides,
        eat,
    })
}

fn letters(cur: &mut Cursor<'_>) -> bool {
    cur.advance_while(|c| c.is_ascii_alphabetic()) > 0
}

#[test]
fn default_lexer_names_the_parts_of_a_sentence() {
    let lexer = Lexer::default();
    let text = "Dana (dana@acme.com) paid $1,250.50 on Feb 17, 2009 at 8:05 PM, \
                see https://acme.com/inv?id=7 #paid @bob v2 100% 2022-01-06T08:42:27Z\n";
    let tokens: Vec<_> = kinds_and_texts(&lexer, text)
        .into_iter()
        .filter(|(kind, _)| *kind != "WHITE_SPACE")
        .collect();
    assert_eq!(
        tokens,
        [
            ("WORD", "Dana"),
            ("SYMBOL", "("),
            ("LIKE_EMAIL", "dana@acme.com"),
            ("SYMBOL", ")"),
            ("WORD", "paid"),
            ("CURRENCY", "$1,250.50"),
            ("WORD", "on"),
            ("DATE", "Feb 17, 2009"),
            ("WORD", "at"),
            ("TIME", "8:05 PM"),
            ("SYMBOL", ","),
            ("WORD", "see"),
            ("LIKE_URL", "https://acme.com/inv?id=7"),
            ("HASHTAG", "#paid"),
            ("MENTION", "@bob"),
            ("ALPHA_NUMERIC_SEQUENCE", "v2"),
            ("LIKE_NUMBER", "100%"),
            ("DATE_ISO_8601", "2022-01-06T08:42:27Z"),
            ("LINE_TERMINATION", "\n"),
        ]
    );
}

#[test]
fn tokens_cover_the_whole_text() {
    let lexer = Lexer::default();
    for text in [
        "",
        " ",
        "a",
        "plain words only",
        "tabs\tand\r\nlines\n\n",
        "שלום עולם, 17 בפברואר 2009",
        "日本語のテキスト。全角１２３",
        "emoji 👩‍👩‍👧 and 𝄞 clef",
        "\u{0}\u{1}\u{7f} controls",
        "a@b @ # $ % ^ & * ( ) : ; . , - _ + = ~ ` ' \" < > / \\ | ? !",
        "1 22 333 4444 55555 1.2.3 1/2/3 10:99 25:00 2022-13-45T99:99",
        "http:// https:// www. http://x www.x.y",
        "-word 'tis --help word- a-b_c -1 --1 ---1 .5 ..5",
        "$ $$ $5 5$ 5 USD USD 5 5usd 5 km",
        "Feb February Feb 1 Feb 1 2 1 Feb 1 Feb2",
    ] {
        assert_covers(&lexer, text);
    }
}

/// Random text from an alphabet chosen to trigger every matcher part way.
/// The generator is a fixed linear congruential one, so the run repeats.
#[test]
fn tokens_cover_random_text() {
    const ALPHABET: &[&str] = &[
        "a", "Z", "é", "ש", "0", "5", "9", " ", " ", "\n", "\t", ".", ",", ":", "-", "_", "/", "@",
        "#", "$", "€", "%", "+", "'", "’", "T", "Z", "m", "p", "A", "P", "Feb", "May", "USD",
        "www.", "http://", "https://", "2009", "17", "👍", "\u{00A0}", "。",
    ];
    let lexer = Lexer::default();
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) as usize
    };
    for _ in 0..3000 {
        let len = next() % 24;
        let text: String = (0..len)
            .map(|_| ALPHABET[next() % ALPHABET.len()])
            .collect();
        assert_covers(&lexer, &text);
        // WORD, ALPHA_NUMERIC_SEQUENCE and LIKE_NUMBER take every run that
        // LETTER and DIGIT would.
        assert!(
            lexer
                .tokens(&text)
                .all(|t| t.kind != LETTER && t.kind != DIGIT),
            "{text:?}"
        );
    }
}

#[test]
fn offsets_are_byte_offsets() {
    let lexer = Lexer::default();
    let tokens: Vec<Token<'_>> = lexer.tokens("é 1").collect();
    let spans: Vec<_> = tokens.iter().map(|t| (t.kind, t.start, t.end)).collect();
    assert_eq!(
        spans,
        [("WORD", 0, 2), ("WHITE_SPACE", 2, 3), ("LIKE_NUMBER", 3, 4)]
    );
}

#[test]
fn version_number_between_spaces_is_one_number() {
    let mut list: Vec<Box<dyn Matcher>> = vec![Box::new(LikeNumber)];
    list.extend(matchers::base());
    let lexer = Lexer::new(list).unwrap();
    let tokens: Vec<_> = lexer.tokens(" 8.4.25.771 armV7").collect();
    assert_eq!(tokens[1].kind, LIKE_NUMBER);
    assert_eq!((tokens[1].start, tokens[1].end), (1, 11));
    assert_eq!(tokens[1].text, "8.4.25.771");
}

#[test]
fn base_matchers_alone_claim_every_character() {
    let lexer = Lexer::new(matchers::base()).unwrap();
    assert_eq!(
        kinds_and_texts(&lexer, "abc 123 /.?"),
        [
            ("LETTER", "abc"),
            ("WHITE_SPACE", " "),
            ("DIGIT", "123"),
            ("WHITE_SPACE", " "),
            ("SYMBOL", "/"),
            ("SYMBOL", "."),
            ("SYMBOL", "?"),
        ]
    );
    assert!(lexer.tokens("ab é1 👍\n\u{0}").all(|t| t.kind != OTHER));
}

#[test]
fn empty_text_has_no_tokens() {
    let lexer = Lexer::default();
    assert_eq!(lexer.tokens("").count(), 0);
    let mut tokens = lexer.tokens("a");
    assert!(tokens.next().is_some());
    assert!(tokens.next().is_none());
    assert!(tokens.next().is_none());
}

#[test]
fn unclaimed_characters_become_other_tokens() {
    let lexer = Lexer::new([Box::new(Word) as Box<dyn Matcher>]).unwrap();
    assert_eq!(
        kinds_and_texts(&lexer, "hi 42 é!"),
        [
            ("WORD", "hi"),
            (OTHER, " "),
            (OTHER, "4"),
            (OTHER, "2"),
            (OTHER, " "),
            ("WORD", "é"),
            (OTHER, "!"),
        ]
    );
}

#[test]
fn a_match_that_consumes_nothing_is_ignored() {
    let lexer = Lexer::new([
        probe("EMPTY", &[], |_| true),
        probe("LETTERS", &[], letters),
    ])
    .unwrap();
    assert_eq!(
        kinds_and_texts(&lexer, "ab!"),
        [("LETTERS", "ab"), (OTHER, "!")]
    );
}

#[test]
fn a_failed_matcher_does_not_move_the_next_one() {
    fn eats_then_fails(cur: &mut Cursor<'_>) -> bool {
        cur.advance_while(|_| true);
        false
    }
    let lexer = Lexer::new([
        probe("GREEDY", &[], eats_then_fails),
        probe("LETTERS", &[], letters),
    ])
    .unwrap();
    assert_eq!(
        kinds_and_texts(&lexer, "ab cd"),
        [("LETTERS", "ab"), (OTHER, " "), ("LETTERS", "cd")]
    );
}

#[test]
fn the_first_matcher_given_wins_when_none_overrides() {
    fn one(cur: &mut Cursor<'_>) -> bool {
        cur.advance().is_some()
    }
    let lexer = Lexer::new([probe("LETTERS", &[], letters), probe("ONE", &[], one)]).unwrap();
    assert_eq!(lexer.kinds().collect::<Vec<_>>(), ["LETTERS", "ONE"]);
    assert_eq!(kinds_and_texts(&lexer, "ab"), [("LETTERS", "ab")]);

    let lexer = Lexer::new([probe("ONE", &[], one), probe("LETTERS", &[], letters)]).unwrap();
    assert_eq!(kinds_and_texts(&lexer, "ab"), [("ONE", "a"), ("ONE", "b")]);
}

#[test]
fn an_overriding_matcher_wins_wherever_it_was_given() {
    fn one(cur: &mut Cursor<'_>) -> bool {
        cur.advance().is_some()
    }
    // LETTERS is given last but overrides ONE, so it is tried first.
    let lexer = Lexer::new([probe("ONE", &[], one), probe("LETTERS", &["ONE"], letters)]).unwrap();
    assert_eq!(lexer.kinds().collect::<Vec<_>>(), ["LETTERS", "ONE"]);
    assert_eq!(
        kinds_and_texts(&lexer, "ab1"),
        [("LETTERS", "ab"), ("ONE", "1")]
    );
}

#[test]
fn overrides_order_a_chain_and_ignore_unknown_kinds() {
    fn never(_: &mut Cursor<'_>) -> bool {
        false
    }
    let lexer = Lexer::new([
        probe("C", &[], never),
        probe("B", &["C", "NOT_REGISTERED"], never),
        probe("D", &[], never),
        probe("A", &["B"], never),
    ])
    .unwrap();
    assert_eq!(lexer.kinds().collect::<Vec<_>>(), ["D", "A", "B", "C"]);
}

#[test]
fn default_order_respects_every_override() {
    let lexer = Lexer::default();
    let order: Vec<_> = lexer.kinds().collect();
    assert_eq!(order.len(), matchers::all().len());
    for matcher in matchers::all() {
        let own = order.iter().position(|k| *k == matcher.kind()).unwrap();
        for overridden in matcher.overrides() {
            let other = order.iter().position(|k| k == overridden).unwrap();
            assert!(
                own < other,
                "{} must come before {overridden}",
                matcher.kind()
            );
        }
    }
    // The order given by matchers::all() already satisfies the overrides, so
    // the lexer keeps it as it is.
    assert_eq!(
        order,
        matchers::all().iter().map(|m| m.kind()).collect::<Vec<_>>()
    );
}

#[test]
fn rejects_matcher_sets_that_cannot_form_a_lexer() {
    fn never(_: &mut Cursor<'_>) -> bool {
        false
    }
    assert_eq!(Lexer::new(Vec::new()).unwrap_err(), Error::NoMatchers);
    assert_eq!(
        Lexer::new([probe("A", &[], never), probe("A", &[], never)]).unwrap_err(),
        Error::DuplicateKind("A")
    );
    assert_eq!(
        Lexer::new([
            probe("A", &["B"], never),
            probe("B", &["A"], never),
            probe("C", &[], never)
        ])
        .unwrap_err(),
        Error::OverrideCycle(vec!["B", "A"])
    );
    assert_eq!(
        Lexer::new([probe("A", &["A"], never)]).unwrap_err(),
        Error::OverrideCycle(vec!["A"])
    );
    assert_eq!(
        Error::OverrideCycle(vec!["B", "A"]).to_string(),
        "matchers override each other in a circle: B > A"
    );
}

#[test]
fn an_override_cycle_names_only_the_matchers_in_the_circle() {
    fn never(_: &mut Cursor<'_>) -> bool {
        false
    }
    // X is stuck behind the circle and Y leads into it; neither is part of it.
    let error = Lexer::new([
        probe("X", &[], never),
        probe("A", &["B", "X"], never),
        probe("B", &["C"], never),
        probe("C", &["A"], never),
        probe("Y", &["A"], never),
    ])
    .unwrap_err();
    assert_eq!(error, Error::OverrideCycle(vec!["B", "C", "A"]));
}

/// A matcher that reads a long run and then fails must not be asked again at
/// every character of that run, or lexing the run takes quadratic time. Each
/// text here is a run that some matcher reads far into. In a debug build one
/// of them takes a few hundredths of a second when lexing is linear and many
/// seconds when it is not.
#[test]
fn long_runs_are_lexed_in_linear_time() {
    const UNITS: &[&str] = &[
        "-", "_", ".", ",", "+", "#", "@", "'", "/", " ", "$", "a.", "a-", "1.", "1 ", "a'", "a_",
        "www.", "Feb ", "5 U", "1:", "a@", "é", "—", "。",
    ];
    let lexer = Lexer::default();
    for unit in UNITS {
        let text = unit.repeat(10_000 / unit.len());
        let started = std::time::Instant::now();
        assert_covers(&lexer, &text);
        let elapsed = started.elapsed();
        assert!(elapsed.as_secs() < 2, "a run of {unit:?} took {elapsed:?}");
    }
}
