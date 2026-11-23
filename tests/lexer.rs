//! The lexer as a whole: token order, coverage of the text, and the rules
//! that decide which matcher wins.

use tolkien::matchers::{self, LikeNumber, Word, DIGIT, LETTER, LIKE_NUMBER};
use tolkien::{Cursor, Error, Lexer, Matcher, Token, OTHER};

#[path = "support/reference.rs"]
mod reference;

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
        assert_eq!(spans(&lexer, &text), reference(&text), "{text:?}");
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

/// What the default lexer must give for `text`: the built-in matchers, each
/// asked at every position, in the default order.
fn reference(text: &str) -> Vec<(&'static str, usize, usize)> {
    reference::tokens(&matchers::all(), text)
}

fn spans(lexer: &Lexer, text: &str) -> Vec<(&'static str, usize, usize)> {
    lexer
        .tokens(text)
        .map(|t| (t.kind, t.start, t.end))
        .collect()
}

/// The default lexer skips matchers that cannot start with a character, and
/// knows plain words, numbers, white space and punctuation without asking
/// every matcher. Neither may change a token. Each text here sits on the
/// edge of a rule.
#[test]
fn shortcuts_and_hints_change_no_token() {
    let fast = Lexer::default();
    let hinted = Lexer::new(matchers::all()).unwrap();
    for text in [
        // A word, and what may follow it.
        "word",
        "word ",
        "word.",
        "word,",
        "word:",
        "word?",
        "word!",
        "word)",
        "word;",
        "word\"",
        "word-",
        "word-word",
        "word'",
        "don't",
        "word_",
        "word1",
        "word\n",
        "word\t",
        "wordé",
        "word\u{a0}x",
        "word。",
        "a",
        "ab",
        "abc",
        "abcdefgh",
        "abcdefghi.",
        "ABCDEFGHIJKLMNOPQ)",
        // A month name is the start of a date.
        "May",
        "May 5, 2009",
        "may 5, 2009",
        "MAY 5, 2009",
        "mAy 5, 2009",
        "Mayor 5, 2009",
        "Feb. 17, 2009",
        "March",
        "Mar",
        "Ma",
        "Marc 5 2009",
        "december 1 2009",
        "Sept 5 2009",
        // An email address and a URL start with letters too.
        "a.b@c.de",
        "a+b@c.de",
        "a@b.c",
        "ab.cd",
        "ab.cd@",
        "ab+",
        "name@",
        "x.y.z@example.com.",
        "http://x",
        "https://x.",
        "http:",
        "http:/x",
        "www.x",
        "www.",
        "www",
        "httpx://x",
        "h",
        "w.",
        // White space and line breaks.
        " ",
        "  ",
        " \t ",
        " \n ",
        "\n",
        "\n\n",
        "\r\n",
        "\n ",
        "\n\u{85}",
        " \u{a0}",
        "\t\u{2028}",
        "\u{b}\u{c}",
        " x",
        "\nx",
        // Marks, and the marks a number, a hashtag, a mention or an amount starts with.
        "(",
        ")",
        "\"",
        ";",
        "/",
        "\\",
        "'",
        "*",
        "&",
        "^",
        "`",
        "{",
        "\u{0}",
        "\u{7f}",
        ".",
        ",",
        "-",
        "_",
        "+",
        "%",
        "=",
        "<",
        ">",
        "|",
        "~",
        ". ",
        ".x",
        ".5",
        "..5",
        "...5",
        ",5",
        "-.5",
        "--5",
        "-x",
        "-(",
        "-#",
        "-$",
        "-.",
        ".-",
        "-٣",
        "-½",
        "#",
        "#tag",
        "#1",
        "# ",
        "@",
        "@me",
        "@ ",
        "$",
        "$5",
        "$ ",
        "$$",
        "€5",
        "%5",
        "5%",
        // A number, and what may follow it.
        "5",
        "5 ",
        "5\t",
        "5)",
        "5;",
        "5'",
        "5\"",
        "5/",
        "5\\",
        "5.",
        "5..",
        "5.,x",
        "5, 6",
        "1 1 1",
        "3.14",
        "1,000",
        "1,000.50",
        "1,000,",
        "5%",
        "5%%",
        "100%.",
        "5+5",
        "5=5",
        "5#",
        "5~",
        "5<",
        "5½",
        "5٣",
        "5\u{a0}x",
        // An ISO 8601 date starts with four digits and a dash.
        "2022-01-06",
        "2022-01-06T08:42",
        "2022-1",
        "2022-x",
        "20222-01-06",
        "202-01-06",
        // A date of numbers has its separator twice.
        "17.02.2009",
        "1.2.3",
        "19.6.20.40",
        "2009 02 17",
        "200 02 17",
        "20 02 17",
        "2009 02",
        "17/02/2009",
        "17\\02\\2009",
        "17-02-2009",
        "17_02_2009",
        "17/2",
        "17/022/2009",
        "17/02-2009",
        "06-184755-866851",
        "41.948,-53.5323",
        // A date with a month name goes on after one number.
        "5 May 2009",
        "17th Feb 2009",
        "17th of",
        "5th",
        "2nd Feb, 2009",
        "17Feb2009",
        "17 - Feb 2009",
        "17, Feb 2009",
        "17. Feb 2009",
        "2009, Feb 17",
        "17\u{2013}Feb 2009",
        "17 \u{2013} Feb 2009",
        "3 May 5pm",
        "17 Mayor",
        "17.5 Feb 2009",
        "17xxFeb 2009",
        "173rd Feb 2009",
        "5 mayo",
        // A time is an hour and a colon or a marker.
        "10:30",
        "10:30:15 PM",
        "4 PM",
        "4PM",
        "4 pm.",
        "4 p.m.",
        "12am",
        "13 PM",
        "4 Pmx",
        "4 apples",
        "4 a",
        "10:305",
        "07:01:75:c5",
        "25:00",
        "123 PM",
        "4:5",
        "4.5 PM",
        // An email address may start with a digit.
        "123@example.com",
        "1.2@example.com",
        "1+2@example.com",
        "1-2@example.com",
        "1_2@x.yz",
        "12abc@x.yz",
        "1,2@x.yz",
        "5@",
        "5@x",
        "5+x",
        // An amount of money is a number and a name.
        "5 USD",
        "5USD",
        "5\nUSD",
        "5 K",
        "5 Centsless",
        "94 won't",
        "5.4 Dollars",
        "5,000 cents",
        "5. USD",
        "5 $",
        "5$",
        "5.\u{783}",
        "5 kr",
        "5 Kr",
        "5 \u{433}\u{440}\u{43d}",
        "5\u{a0}USD",
        "5 the",
        "5 Thebe",
        // Digits start a run of letters and digits.
        "3L7gwx",
        "5-a",
        "5_a",
        "5-",
        "5_",
        "5-5",
        "5--a",
        "0.001b",
        "158centimeters",
        "5é",
        "5-é",
        "8bab:1afd",
        // Letters, and the digits and marks that join them.
        "word-1",
        "word1 word",
        "may-5-2009",
        "Feb17",
        "Feb-17-2009",
        "a1@x.yz",
        "a-b@x.yz",
        "a_b@x.yz",
        "a'b@x.yz",
        "http-x",
        "www1.x",
        "before11 and95",
        // Outside ASCII.
        "é",
        "été",
        "été-",
        "été-là",
        "été1",
        "été_x",
        "naïve)",
        "e\u{301}te\u{301}",
        "été\u{301}",
        "été’s",
        "été\u{2013}x",
        "été٣",
        "abcédef ghi",
        "é.",
        "éa1",
        "café\u{a0}x",
        "é\u{3002}",
        "Mayé 5, 2009",
        "wwwé.x",
        "abé@x.yz",
        " é",
        "\né",
        " \u{3000}x",
        "हिन्दी भाषा",
        "ไทย ภาษา",
        "naïve.",
        "שלום עולם",
        "日本語のテキスト。",
        "Привет, мир!",
        "“quoted”",
        "‘a’",
        "—",
        "–5",
        "１２３",
        "٣٥",
        "½",
        "€",
        "₪20",
        "\u{301}",
        "a\u{301}b",
        "👍",
        "👩\u{200d}💻",
        "ʼ",
        "x’s",
        "’tis",
    ] {
        let expected = reference(text);
        assert_eq!(
            spans(&fast, text),
            expected,
            "the default lexer on {text:?}"
        );
        assert_eq!(
            spans(&hinted, text),
            expected,
            "the hinted lexer on {text:?}"
        );
    }
}

/// Cut into any number of pieces, a text gives the tokens it gives whole.
/// With as many pieces as bytes, every place in the text is a cut, also the
/// middle of every token and of every character.
#[test]
fn parallel_lexing_gives_the_same_tokens() {
    let lexer = Lexer::default();
    for text in [
        "",
        "a",
        "Dana (dana@acme.com) paid $1,250.50 on Feb 17, 2009 at 8:05 PM, see https://acme.com/inv?id=7.",
        "2022-01-06T08:42:27.549090+02:00 5 USD 17 February, 2009 10:30:15 pm x@y.zz www.example.com/a_(b)",
        "one   long    run     of      spaces\n\n\n\nand line breaks \t\t\t and-hyphens--and___underscores",
        "שלום עולם, 17 בפברואר 2009. 日本語のテキスト。 “quoted” — naïve café 👩\u{200d}💻 ½ ٣٥",
    ] {
        let whole: Vec<Token<'_>> = lexer.tokens(text).collect();
        for threads in 0..=text.len() + 1 {
            assert_eq!(
                lexer.tokens_parallel_on(text, threads),
                whole,
                "{threads} threads on {text:?}"
            );
        }
        assert_eq!(lexer.tokens_parallel(text), whole);
    }
    let long =
        "A sentence with 3 numbers, 1 date (Feb 17, 2009) and an address: a@b.cd. ".repeat(20_000);
    let whole: Vec<Token<'_>> = lexer.tokens(&long).collect();
    assert_eq!(lexer.tokens_parallel(&long), whole);
    assert_eq!(lexer.tokens_parallel_on(&long, 7), whole);
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
        "www.", "Feb ", "5 U", "1:", "a@", "é", "—", "。", "1-", "1_", "1/", "1 -", "1,", "1a",
        "1@", "é-", "é_", "é1", "é’", "a é", "1 P",
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
