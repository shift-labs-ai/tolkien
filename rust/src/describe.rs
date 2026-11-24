//! What the Node and the browser bindings share: a lexer built from
//! matchers described as JSON, and its tokens as arrays.
//!
//! A binding does not call back into JavaScript for a matcher: a call for
//! every token would cost far more than the token. A matcher comes over
//! once, as the description of a [`Pattern`], and runs here.
//!
//! Tokens go back as two arrays and not as objects, of which a text has
//! one for every few bytes. The tokens of a text cover it, so each is
//! known by its kind and its end. Offsets are in UTF-16 code units, which
//! is how JavaScript indexes a string.

use std::collections::BTreeSet;
use std::sync::{Mutex, PoisonError};

use serde::Deserialize;

use crate::pattern::{Class, Pattern, Step};
use crate::{catalog, Lexer, Matcher, Trie, OTHER};

/// A matcher: one of the catalog by its kind, or a kind, the steps of
/// its pattern and a count over the whole token.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MatcherSpec {
    catalog: Option<String>,
    kind: Option<String>,
    steps: Option<Vec<StepSpec>>,
    count: Option<CountSpec>,
}

/// [`Pattern::with_count`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CountSpec {
    of: Vec<ClassPart>,
    min: usize,
    max: usize,
}

/// A part of a [`Class`]. A class is a list of them.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
enum ClassPart {
    Named(String),
    Chars(String),
    Range(char, char),
}

/// A [`Step`], named as its constructor is. A run with no `max` takes as
/// many characters as there are.
#[derive(Deserialize)]
#[serde(tag = "step", rename_all = "camelCase", deny_unknown_fields)]
enum StepSpec {
    Run {
        class: Vec<ClassPart>,
        min: usize,
        max: Option<usize>,
    },
    Text {
        text: String,
    },
    #[serde(rename_all = "camelCase")]
    Words {
        words: Vec<String>,
        common_cases: bool,
    },
    #[serde(rename_all = "camelCase")]
    Number {
        max_digits: usize,
        max_value: u64,
    },
    Separator {
        class: Vec<ClassPart>,
    },
    SameSeparator,
    Not {
        class: Vec<ClassPart>,
    },
    NotBefore {
        steps: Vec<StepSpec>,
    },
    Optional {
        steps: Vec<StepSpec>,
    },
    Repeat {
        max: usize,
        steps: Vec<StepSpec>,
    },
    Either {
        choices: Vec<Vec<StepSpec>>,
    },
    TrimEnd {
        class: Vec<ClassPart>,
    },
}

fn class(parts: Vec<ClassPart>) -> Result<Class, String> {
    parts.into_iter().try_fold(Class::default(), |class, part| {
        let part = match part {
            ClassPart::Named(name) => Class::named(&name)
                .ok_or_else(|| format!("no set of characters is named {name:?}"))?,
            ClassPart::Chars(chars) => Class::chars(&chars),
            ClassPart::Range(first, last) => Class::range(first, last),
        };
        Ok(class | part)
    })
}

fn steps(specs: Vec<StepSpec>) -> Result<Vec<Step>, String> {
    specs.into_iter().map(step).collect()
}

fn step(spec: StepSpec) -> Result<Step, String> {
    Ok(match spec {
        StepSpec::Run {
            class: parts,
            min,
            max,
        } => Step::between(min, max.unwrap_or(usize::MAX), class(parts)?),
        StepSpec::Text { text } => Step::text(&text),
        StepSpec::Words {
            words,
            common_cases,
        } => Step::words(if common_cases {
            Trie::from_words_in_common_cases(words)
        } else {
            Trie::from_words(words)
        }),
        StepSpec::Number {
            max_digits,
            max_value,
        } => Step::number(max_digits, max_value),
        StepSpec::Separator { class: parts } => Step::separator(class(parts)?),
        StepSpec::SameSeparator => Step::same_separator(),
        StepSpec::Not { class: parts } => Step::not(class(parts)?),
        StepSpec::NotBefore { steps: specs } => Step::not_before(steps(specs)?),
        StepSpec::Optional { steps: specs } => Step::optional(steps(specs)?),
        StepSpec::Repeat { max, steps: specs } => Step::repeat(max, steps(specs)?),
        StepSpec::Either { choices } => Step::either(
            choices
                .into_iter()
                .map(steps)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        StepSpec::TrimEnd { class: parts } => Step::trim_end(class(parts)?),
    })
}

/// The name `kind` for as long as the program runs, as the kind of a
/// matcher has to be. A name is kept once, however many lexers use it.
fn kept(kind: String) -> &'static str {
    static KEPT: Mutex<BTreeSet<&'static str>> = Mutex::new(BTreeSet::new());
    let mut kept = KEPT.lock().unwrap_or_else(PoisonError::into_inner);
    match kept.get(kind.as_str()) {
        Some(known) => known,
        None => {
            let kind: &'static str = Box::leak(kind.into_boxed_str());
            kept.insert(kind);
            kind
        }
    }
}

fn matcher(spec: MatcherSpec) -> Result<Box<dyn Matcher>, String> {
    match spec {
        MatcherSpec {
            catalog: Some(kind),
            kind: None,
            steps: None,
            count: None,
        } => catalog::named(&kind).ok_or_else(|| format!("the catalog has no matcher {kind:?}")),
        MatcherSpec {
            catalog: None,
            kind: Some(kind),
            steps: Some(specs),
            count,
        } => {
            let pattern = Pattern::new(kept(kind), steps(specs)?);
            Ok(Box::new(match count {
                Some(count) => pattern.with_count(class(count.of)?, count.min, count.max),
                None => pattern,
            }))
        }
        _ => Err("a matcher is `catalog`, or `kind` and `steps`".to_string()),
    }
}

/// The catalog as a JSON array: for each matcher its kind, its group and
/// what it matches, in the order they are to be tried.
pub(crate) fn catalog_json() -> String {
    let entries = catalog::entries().iter().map(|entry| {
        serde_json::json!({
            "kind": entry.kind,
            "group": entry.group.name(),
            "summary": entry.summary,
        })
    });
    serde_json::Value::Array(entries.collect()).to_string()
}

/// A lexer and the kinds of its tokens, each known by its place.
pub(crate) struct Described {
    lexer: Lexer,
    /// The kinds of the lexer's matchers in the order they are tried, and
    /// [`OTHER`] after them.
    kinds: Vec<&'static str>,
    /// Where each kind stands in `kinds`, found by the address of its
    /// name: a kind is a constant, and comparing names for every token of
    /// a text would cost more than lexing it. A slot holds an address and
    /// a place, and an empty one the address zero.
    places: Vec<(usize, u16)>,
}

/// The tokens of a text: for each, the place of its kind in
/// [`Described::kinds`] and its end in UTF-16 code units. A token starts
/// where the one before it ends.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct TokenArrays {
    pub(crate) kinds: Vec<u16>,
    pub(crate) ends: Vec<u32>,
}

/// The slot an address is looked for first, in a table of `slots` slots,
/// a power of two.
fn slot(address: usize, slots: usize) -> usize {
    // Names are a few bytes apart, so the low bits tell them apart.
    ((address as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32) as usize & (slots - 1)
}

impl Described {
    /// The lexer of the built-in matchers with the matchers that
    /// `matchers` describes tried before them: a JSON array.
    pub(crate) fn new(matchers: &str) -> Result<Self, String> {
        let specs: Vec<MatcherSpec> =
            serde_json::from_str(matchers).map_err(|error| format!("not a matcher: {error}"))?;
        let matchers = specs
            .into_iter()
            .map(matcher)
            .collect::<Result<Vec<_>, String>>()?;
        let lexer = Lexer::with(matchers).map_err(|error| error.to_string())?;
        let kinds: Vec<_> = lexer.kinds().chain([OTHER]).collect();
        if kinds.len() > usize::from(u16::MAX) {
            return Err(format!("{} kinds are too many", kinds.len()));
        }
        // At most a quarter full, so a kind is nearly always in its slot.
        let mut places = vec![(0, 0); (kinds.len() * 4).next_power_of_two()];
        for (place, kind) in kinds.iter().enumerate() {
            let address = kind.as_ptr() as usize;
            let mut at = slot(address, places.len());
            while places[at].0 != 0 && places[at].0 != address {
                at = (at + 1) & (places.len() - 1);
            }
            places[at] = (address, place as u16);
        }
        Ok(Self {
            lexer,
            kinds,
            places,
        })
    }

    /// The kinds a token can have. A token's kind is given as a place in
    /// this list.
    pub(crate) fn kinds(&self) -> Vec<String> {
        self.kinds.iter().map(|kind| kind.to_string()).collect()
    }

    /// The tokens of `text`, lexed on every core where threads can be
    /// started and the text is long enough to gain from them.
    pub(crate) fn tokens(&self, text: &str) -> TokenArrays {
        let tokens = self.lexer.tokens_parallel(text);
        let mut arrays = TokenArrays {
            kinds: Vec::with_capacity(tokens.len()),
            ends: Vec::with_capacity(tokens.len()),
        };
        let bytes = text.as_bytes();
        // Where the next character outside ASCII is. Up to there a byte is
        // a code unit, and no token has to be read again to count them.
        let mut outside_ascii = first_outside_ascii(bytes, 0);
        // How many code units fewer than bytes the text has so far.
        let mut shorter = 0;
        for token in tokens {
            if token.end > outside_ascii {
                shorter += token.text.len() - utf16_len(token.text);
                outside_ascii = first_outside_ascii(bytes, token.end);
            }
            arrays.kinds.push(self.place(token.kind));
            arrays.ends.push(
                u32::try_from(token.end - shorter)
                    .expect("a JavaScript string is shorter than 4 GB"),
            );
        }
        arrays
    }

    /// The place of `kind` in the list of kinds.
    #[inline]
    fn place(&self, kind: &'static str) -> u16 {
        let address = kind.as_ptr() as usize;
        let mut at = slot(address, self.places.len());
        loop {
            match self.places[at] {
                (found, place) if found == address => return place,
                // The same name at another address: a compiler may keep a
                // constant in more than one place.
                (0, _) => return self.place_by_name(kind),
                _ => at = (at + 1) & (self.places.len() - 1),
            }
        }
    }

    #[cold]
    fn place_by_name(&self, kind: &str) -> u16 {
        let place = self.kinds.iter().position(|&known| known == kind);
        place.expect("a token has the kind of one of the lexer's matchers") as u16
    }
}

/// The offset of the first byte from `from` on that is not ASCII, or the
/// length of `bytes`.
fn first_outside_ascii(bytes: &[u8], from: usize) -> usize {
    let rest = &bytes[from..];
    let (eights, tail) = rest.as_chunks::<8>();
    // Eight bytes at a time: the top bit of a byte outside ASCII is set.
    let ascii = eights
        .iter()
        .take_while(|eight| u64::from_ne_bytes(**eight) & 0x8080_8080_8080_8080 == 0)
        .count()
        * 8;
    let tail = if ascii < eights.len() * 8 {
        &rest[ascii..]
    } else {
        tail
    };
    from + ascii + tail.iter().take_while(|byte| byte.is_ascii()).count()
}

/// How many UTF-16 code units `text` is: one for each character, and one
/// more for each character of four bytes.
fn utf16_len(text: &str) -> usize {
    text.bytes()
        .map(|byte| usize::from(byte & 0xC0 != 0x80) + usize::from(byte >= 0xF0))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CARD: &str = r#"{
        "kind": "CARD_NUMBER",
        "steps": [
            {"step": "run", "class": [{"range": ["0", "9"]}], "min": 1},
            {"step": "repeat", "max": 7, "steps": [
                {"step": "run", "class": [{"chars": " -"}], "min": 1, "max": 1},
                {"step": "run", "class": [{"range": ["0", "9"]}], "min": 1}
            ]},
            {"step": "not", "class": [{"named": "letter"}, {"named": "digit"}]}
        ],
        "count": {"of": [{"range": ["0", "9"]}], "min": 13, "max": 19}
    }"#;

    fn spans(described: &Described, text: &str) -> Vec<(String, u32)> {
        let arrays = described.tokens(text);
        let kinds = described.kinds();
        let kind = |place: &u16| kinds[*place as usize].clone();
        arrays.kinds.iter().map(kind).zip(arrays.ends).collect()
    }

    #[test]
    fn with_no_matchers_it_is_the_default_lexer() {
        let described = Described::new("[]").unwrap();
        let text = "Mail dana@acme.com by 2026-11-23";
        let expected: Vec<_> = Lexer::default()
            .tokens(text)
            .map(|token| (token.kind.to_string(), token.end as u32))
            .collect();
        assert_eq!(spans(&described, text), expected);
        assert_eq!(described.kinds().last().map(String::as_str), Some(OTHER));
        assert_eq!(described.tokens(""), TokenArrays::default());
    }

    #[test]
    fn ends_are_in_utf16_code_units() {
        let described = Described::new("[]").unwrap();
        // One code unit for é and for 語, two for the emoji.
        let ends: Vec<_> = spans(&described, "é 👍 語x 1").into_iter().collect();
        assert_eq!(
            ends,
            [
                ("WORD".to_string(), 1),
                ("WHITE_SPACE".to_string(), 2),
                ("SYMBOL".to_string(), 4),
                ("WHITE_SPACE".to_string(), 5),
                ("WORD".to_string(), 7),
                ("WHITE_SPACE".to_string(), 8),
                ("LIKE_NUMBER".to_string(), 9),
            ]
        );
        let text = "naïve 👩\u{200d}💻 x";
        assert_eq!(
            spans(&described, text).last().unwrap().1 as usize,
            text.encode_utf16().count()
        );
    }

    /// Every end is the length in code units of the text up to it, with
    /// characters outside ASCII at every distance from each other.
    #[test]
    fn ends_count_code_units_wherever_ascii_stops() {
        let described = Described::new("[]").unwrap();
        for gap in 0..20 {
            let text = format!("{0}é{0} 👍{0}語 {0}", "ab ".repeat(gap));
            let lexed: Vec<_> = Lexer::default().tokens(&text).collect();
            let ends = described.tokens(&text).ends;
            assert_eq!(ends.len(), lexed.len());
            for (token, end) in lexed.iter().zip(ends) {
                let expected = text[..token.end].encode_utf16().count();
                assert_eq!(end as usize, expected, "{gap}: {token:?}");
            }
        }
        for (bytes, from, expected) in [
            (&b"abcdefghij"[..], 0, 10),
            (b"abcdefghij", 10, 10),
            ("abcdefghijé".as_bytes(), 1, 10),
            ("é".as_bytes(), 0, 0),
            ("abcdefgh12345678é9".as_bytes(), 0, 16),
            ("abcé".as_bytes(), 0, 3),
        ] {
            assert_eq!(first_outside_ascii(bytes, from), expected);
        }
    }

    /// A kind is found whether or not its name is at the known address.
    #[test]
    fn a_kind_has_its_place_at_any_address() {
        let described = Described::new(&format!("[{CARD}]")).unwrap();
        let kinds = described.kinds();
        for (place, kind) in described.kinds.iter().enumerate() {
            assert_eq!(described.place(kind) as usize, place);
            let elsewhere: &'static str = Box::leak(kind.to_string().into_boxed_str());
            assert_eq!(described.place(elsewhere) as usize, place, "{kind}");
        }
        assert_eq!(kinds.len(), described.kinds.len());
    }

    /// A described matcher is the pattern written in Rust, and is tried
    /// before the built-in ones.
    #[test]
    fn a_described_matcher_is_the_pattern_it_describes() {
        let described = Described::new(&format!("[{CARD}]")).unwrap();
        let digit = || Class::range('0', '9');
        let card = Pattern::new(
            "CARD_NUMBER",
            [
                Step::run(digit()),
                Step::repeat(7, [Step::one(Class::chars(" -")), Step::run(digit())]),
                Step::not(Class::letter() | Class::digit()),
            ],
        )
        .with_count(digit(), 13, 19);
        let lexer = Lexer::with([Box::new(card) as Box<dyn Matcher>]).unwrap();
        for text in [
            "card 4111 1111-1111 1111.",
            "4111 1111 1111",
            "2009 was 4111111111111111x",
            "",
        ] {
            let expected: Vec<_> = lexer
                .tokens(text)
                .map(|token| (token.kind.to_string(), token.end as u32))
                .collect();
            assert_eq!(spans(&described, text), expected, "{text:?}");
        }
        assert_eq!(described.kinds()[0], "CARD_NUMBER");
    }

    /// Every kind of step the engine has can be described, and under the
    /// name the engine gives it: the names a description may use, which
    /// serde lists when it refuses one, are those of
    /// [`DESCRIBED`](crate::pattern::DESCRIBED).
    #[test]
    fn every_step_of_the_engine_has_a_description() {
        let refused = serde_json::from_str::<StepSpec>(r#"{"step": "jump"}"#)
            .err()
            .expect("no step is named jump")
            .to_string();
        let (_, listed) = refused.split_once("expected one of").expect(&refused);
        let mut known: Vec<_> = listed.split('`').skip(1).step_by(2).collect();
        let mut described = crate::pattern::DESCRIBED.to_vec();
        known.sort_unstable();
        described.sort_unstable();
        assert_eq!(known, described);

        // Each description gives the step of its name.
        let specs: serde_json::Value = serde_json::from_str(EVERY_STEP).unwrap();
        let mut seen = Vec::new();
        for spec in specs[0]["steps"].as_array().unwrap() {
            let name = spec["step"].as_str().unwrap();
            let built = step(serde_json::from_value(spec.clone()).unwrap()).unwrap();
            assert_eq!(built.described_as(), name);
            seen.push(name);
        }
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen, described, "a step is missing from EVERY_STEP");
    }

    /// A matcher that uses every step.
    const EVERY_STEP: &str = r#"[{"kind": "EVERYTHING", "steps": [
            {"step": "text", "text": "id:"},
            {"step": "words", "words": ["a", "b"], "commonCases": true},
            {"step": "number", "maxDigits": 3, "maxValue": 255},
            {"step": "separator", "class": [{"chars": ":-"}]},
            {"step": "run", "class": [{"named": "letter"}], "min": 2, "max": 2},
            {"step": "sameSeparator"},
            {"step": "optional", "steps": [{"step": "text", "text": "?"}]},
            {"step": "either", "choices": [[{"step": "text", "text": "x"}], []]},
            {"step": "run", "class": [{"chars": "."}], "min": 0},
            {"step": "trimEnd", "class": [{"chars": "."}]},
            {"step": "notBefore", "steps": [{"step": "text", "text": "!"}]},
            {"step": "repeat", "max": 2, "steps": [{"step": "text", "text": "+"}]},
            {"step": "not", "class": [{"named": "letter"}]}
        ]}]"#;

    #[test]
    fn every_step_can_be_described() {
        let described = Described::new(EVERY_STEP).unwrap();
        assert_eq!(
            spans(&described, "id:A12-ab-x.. "),
            [
                ("EVERYTHING".to_string(), 11),
                ("SYMBOL".to_string(), 12),
                ("SYMBOL".to_string(), 13),
                ("WHITE_SPACE".to_string(), 14),
            ]
        );
        assert_eq!(spans(&described, "id:A12-ab-!")[0].0, "WORD");
    }

    #[test]
    fn a_matcher_of_the_catalog_is_named_by_its_kind() {
        let matchers = r#"[{"catalog": "MAC_ADDRESS"}, {"catalog": "IPV4"}]"#;
        let described = Described::new(matchers).unwrap();
        assert_eq!(described.kinds()[..2], ["MAC_ADDRESS", "IPV4"]);
        assert_eq!(
            spans(&described, "1d:d8:97:09:54:39 8.8.8.8"),
            [
                ("MAC_ADDRESS".to_string(), 17),
                ("WHITE_SPACE".to_string(), 18),
                ("IPV4".to_string(), 25),
            ]
        );
        let listed: serde_json::Value = serde_json::from_str(&catalog_json()).unwrap();
        let listed = listed.as_array().unwrap();
        assert_eq!(listed.len(), catalog::entries().len());
        assert_eq!(listed[0]["kind"], catalog::entries()[0].kind);
        assert_eq!(listed[0]["group"], "secrets");
        // Every kind the list names is one a lexer takes.
        let all: Vec<_> = listed
            .iter()
            .map(|entry| format!(r#"{{"catalog": {}}}"#, entry["kind"]))
            .collect();
        assert!(Described::new(&format!("[{}]", all.join(","))).is_ok());
    }

    /// The name of a kind is kept once.
    #[test]
    fn a_kind_is_kept_once() {
        let first = kept("KEPT_ONCE".to_string());
        assert!(std::ptr::eq(first, kept("KEPT_ONCE".to_string())));
    }

    #[test]
    fn a_wrong_description_says_what_is_wrong() {
        for (matchers, part) in [
            ("{}", "not a matcher"),
            (
                r#"[{"kind": "A"}]"#,
                "a matcher is `catalog`, or `kind` and `steps`",
            ),
            (
                r#"[{"catalog": "IPV4", "kind": "A"}]"#,
                "a matcher is `catalog`",
            ),
            (
                r#"[{"catalog": "NOPE"}]"#,
                "the catalog has no matcher \"NOPE\"",
            ),
            (
                r#"[{"catalog": "IPV4"}, {"catalog": "IPV4"}]"#,
                "two matchers produce the kind IPV4",
            ),
            (
                r#"[{"kind": "A", "steps": [{"step": "jump"}]}]"#,
                "unknown variant `jump`",
            ),
            (
                r#"[{"kind": "A", "steps": [{"step": "not", "class": [{"named": "vowel"}]}]}]"#,
                "no set of characters is named \"vowel\"",
            ),
            (
                r#"[{"kind": "A", "steps": [{"step": "repeat", "max": -1, "steps": []}]}]"#,
                "invalid value",
            ),
            (
                r#"[{"kind": "WORD", "steps": []}]"#,
                "two matchers produce the kind WORD",
            ),
            (
                r#"[{"kind": "A", "steps": [], "extra": 1}]"#,
                "unknown field `extra`",
            ),
        ] {
            let error = Described::new(matchers).err().expect(matchers);
            assert!(error.contains(part), "{error:?} for {matchers}");
        }
    }
}
