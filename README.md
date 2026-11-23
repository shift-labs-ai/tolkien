# tolkien

A lexer for natural text. One pass over a string gives a stream of tokens,
each a slice of the text with a kind: a word, a number, a date, a time, an
email address, a URL, an amount of money.

```rust
use tolkien::Lexer;

let lexer = Lexer::default();
for token in lexer.tokens("Dana paid $1,250.50 on Feb 17, 2009 at 8:05 PM") {
    println!("{:<12} {:>2}..{:<2} {:?}", token.kind, token.start, token.end, token.text);
}
```

```text
WORD          0..4  "Dana"
WHITE_SPACE   4..5  " "
WORD          5..9  "paid"
WHITE_SPACE   9..10 " "
CURRENCY     10..19 "$1,250.50"
WHITE_SPACE  19..20 " "
WORD         20..22 "on"
WHITE_SPACE  22..23 " "
DATE         23..35 "Feb 17, 2009"
WHITE_SPACE  35..36 " "
WORD         36..38 "at"
WHITE_SPACE  38..39 " "
TIME         39..46 "8:05 PM"
```

tolkien is a Rust rewrite of [fastlex](https://github.com/Michaelliv/fastlex),
a Kotlin library. It has no dependencies and builds for `wasm32`.

## How it works

- **Matchers.** Each kind of token has a matcher: a small hand-written
  recogniser that reads characters from a cursor. There are no regular
  expressions.
- **First match wins.** At each position the lexer tries its matchers in
  order, takes the first that matches, and continues after that token.
- **Order.** Matchers are tried in the order they are given. A matcher may
  list kinds it overrides: it is then tried before them, wherever it was
  given, and no other matcher changes place.
- **Full coverage.** Every byte of the text belongs to exactly one token. A
  character no matcher claims becomes a one-character `OTHER` token.
- **Linear time.** No built-in matcher reads far ahead and then gives up, so
  the time to lex a text grows with its length, whatever the text holds.
- **Forward only.** A token depends only on the text from its start on, so
  a text can be lexed from any token boundary, and in pieces.

Offsets are byte offsets into the UTF-8 text.

## Built-in matchers

`Lexer::default()` uses all of them, in this order:

| Kind | Matches | Examples |
|---|---|---|
| `DATE_ISO_8601` | A full ISO 8601 date and the complete part of a time after it | `2022-01-06`, `2022-01-06T08:42:27.5+02:00` |
| `DATE` | Day, month and year, in numbers or with an English month name | `02/17/2009`, `17th Feb, 2009`, `Feb172009` |
| `TIME` | Hours and minutes, optional seconds, optional AM/PM; or an hour and AM/PM | `7:20`, `04:01:30`, `8:05 PM`, `4 PM` |
| `LIKE_EMAIL` | `local@domain.tld` in ASCII | `dana@mail.example.co.uk` |
| `LIKE_URL` | `http://`, `https://` or `www.` and what follows | `https://t.co/2Ecdop6zl1` |
| `CURRENCY` | Currency symbols with an amount, or an amount with a currency name or code | `$5`, `€1,000.50`, `5 USD`, `5.4 Dollars` |
| `HASHTAG` | `#` and letters, digits, underscores | `#hash_tag` |
| `MENTION` | `@` and letters, digits, underscores | `@dana` |
| `WORD` | A letter, then letters, dashes and at most one apostrophe | `word`, `first-second`, `don't` |
| `ALPHA_NUMERIC_SEQUENCE` | Letters mixed with digits, dashes, underscores | `armV7`, `some_word` |
| `LIKE_NUMBER` | Number characters and the punctuation written with them | `-0.5`, `1,000`, `100%`, `6.0.1` |
| `LINE_TERMINATION` | A run of line breaks | |
| `DIGIT` | A run of digits | |
| `LETTER` | A run of letters | |
| `SYMBOL` | One character that is not a letter, a digit or white space | `(` |
| `WHITE_SPACE` | A run of white space | |

With the default lexer `DIGIT` and `LETTER` never appear: `LIKE_NUMBER`,
`WORD` and `ALPHA_NUMERIC_SEQUENCE` are tried first and take every run they
would.

The matchers check shape and little more: a numeric date needs a month
from 1 to 12, but `31/02/2009` is a `DATE`.

Known weak spots:

- A version number that looks like a date is a `DATE`: `4.1.10`.
- A package reference is a `LIKE_EMAIL`: `liteparse@2.12.0`.
- Some currency abbreviations and names are also other words: `5 K`,
  `21 para`.
- An ISO date takes a two-digit number after a space as its hour:
  `2022-01-06 12 apples`.
- A possessive is one word: `Abby's`.
- The end of a MAC address is a `TIME`: `09:54:39` in `1d:d8:97:09:54:39`.
  A matcher reads forward only, so it cannot see what stands before it.

No matcher exists yet for phone numbers, IP and MAC addresses, dates
without a year (`30 January`, `March 2006`) or amounts with a magnitude
(`$5k`). [docs/benchmarks.md](docs/benchmarks.md) measures what that costs.

## Testing against real text

Two tools check the lexer against text nobody here wrote. Both read corpora
that `tools/corpora.py` downloads into `corpus/`, which is not committed.

```bash
python3 tools/corpora.py                                          # fetch and prepare
cargo run --release --example fuzz -- corpus/docs/*.txt           # properties
cargo build --release && python3 tools/evaluate.py                # labelled spans
```

- **`examples/fuzz.rs`** lexes every document, then mutates documents at
  random, and checks properties that must hold for any text: the tokens
  cover it, they are the tokens of asking every matcher at every position,
  they are the same when the text is lexed in pieces on several threads,
  every token has the form its kind documents, a token on its own is the
  same token, a token does not depend on what stands before it, and the
  time is in proportion to the length. A failing text is cut down and
  printed.
- **`tools/evaluate.py`** compares the tokens with the spans people labelled
  in two datasets: is an email address, a date or a card number one token,
  and of the expected kind? `docs/benchmarks.md` holds the results.

## Your own matcher

A matcher is a struct with a kind and a function that reads from a cursor.
`Lexer::with` adds matchers to the built-in ones and tries them first:

```rust
use tolkien::{Cursor, Lexer, Matcher};

/// Four numbers from 0 to 255 joined by full stops: `192.168.1.20`.
struct Ipv4;

impl Matcher for Ipv4 {
    fn kind(&self) -> &'static str {
        "IPV4"
    }

    // Where a token can start. Without it the matcher is asked at every
    // token of the text.
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
        // Not the start of something longer.
        !cur.followed_by(|c| c == '.', |c| c.is_ascii_digit())
            && !cur.peek().is_some_and(char::is_alphanumeric)
    }
}

// A matcher is tested on its own with `match_prefix`.
assert_eq!(Ipv4.match_prefix("10.0.0.255, and"), Some("10.0.0.255"));
assert_eq!(Ipv4.match_prefix("10.0.0.256"), None);

let lexer = Lexer::with([Box::new(Ipv4) as Box<dyn Matcher>]).unwrap();
let tokens: Vec<_> = lexer.tokens("from 192.168.1.20").map(|t| (t.kind, t.text)).collect();
assert_eq!(tokens[2], ("IPV4", "192.168.1.20"));
```

What to know:

- **Order is the order you give.** The matchers passed to `Lexer::with`
  are tried before the built-in ones, first to last, so they win where
  both match. Nothing has to be declared. `overrides` is for a matcher
  that is given later than one it must beat: it moves up to just before
  it, and no other matcher changes place.
- **`can_start` is a rule, not a hint.** The matcher is asked only where it
  answers `true`, by the lexer and by `match_prefix` alike. It is also
  what keeps the lexer fast: with five added matchers that say where they
  start, the texts of the table below lex at 300 to 370 MB/s; with one
  more that can start anywhere, at 140 to 210 MB/s.
- **The cursor reads forward.** `peek`, `advance_if`, `advance_while`,
  `advance_exactly` and `advance_str` move it; `followed_by` looks two
  characters ahead without moving; `back_while` steps back over
  punctuation at the end; `since` gives the text passed; `set_pos` returns
  to a saved place. It shows nothing before the token.
- **Fail early.** A matcher that reads a long run and then fails is asked
  again one character later. Bound what it reads before it can fail.
- **`Trie`** finds the longest of many words at the cursor, and
  `tolkien::chars` has the character classes the built-in matchers use.

[tests/custom.rs](tests/custom.rs) has four matchers written this way: a
card number, an IP address, a MAC address and a list of keywords.

`Lexer::new` takes any list of matchers and nothing else: no built-in
matcher is added, and each matcher is asked for every token it can start.

## Command line

```bash
cargo run -- "Mail dana@acme.com by 2026-11-23"   # kind, start, end, text
cargo run -- --skip-space < notes.txt
```

## Character sets

No character table is written by hand. `tools/gen_unicode.py` builds them
from the Unicode Character Database (Unicode 18.0.0, files pinned by hash)
and writes two files:

- `src/unicode_tables.rs`, the tables the code uses.
- `docs/charsets.md`, every member of every named set, to be reviewed.

Each table is a rule: a general category (letters, digits, currency
symbols), a property (`White_Space`, `Dash`), a line-break class, or a
pattern over character names (`FULL STOP`, `COMMA`, `QUESTION MARK`).

## Speed

With every built-in matcher, on an Apple M5 laptop with ten cores:

| Text | Size | One thread | Every core, collected |
|---|---:|---:|---:|
| Court cases (TAB) | 0.7 MB | 605 MB/s | 880 MB/s |
| Sentences with personal data (ai4privacy, English) | 7.6 MB | 480 MB/s | 1,200 MB/s |
| Documents converted to markdown | 0.6 MB | 530 MB/s | 500 MB/s |
| Wikipedia in 24 languages and scripts | 2.9 MB | 485 MB/s | 1,300 MB/s |

That is 115 to 225 million tokens a second on one thread. The figures are
the best of 40 runs and move by a few percent between runs. On every core,
the two small files gain little or nothing, because starting the threads
is a large part of a millisecond. The slowest inputs measured are long
runs of very short tokens the shortcuts do not cover, such as digits of
other scripts, at about 110 MB/s on one thread.

```bash
cargo run --release --example throughput -- FILE
```

Three things make it fast, and none of them changes a token:

- **One table for every character class.** A character is looked up once
  and tested against any number of classes. The table is generated.
- **A matcher is asked only where it can start.** `Matcher::can_start` says
  which first characters a matcher takes; the lexer lists, for each ASCII
  character, the matchers worth asking. A space asks one matcher, not
  sixteen.
- **The default lexer knows the commonest tokens.** A plain word, a
  number, a run of white space and a punctuation mark are most of a text,
  and `src/shortcuts.rs` reads them straight from the bytes, eight at a
  time for a word. Where the bytes leave a chance to one matcher or two, as
  a month name does to a date, it asks those and no other. It restates what
  the matchers would answer together, so the tests and the fuzzer compare
  it with asking every matcher at every position.

`Lexer::tokens_parallel` lexes a long text on every core: the text is cut
into pieces, each thread lexes one, and the pieces are joined where their
tokens meet. A cut may fall inside a token; the join does not.

## Differences from fastlex

The design is the same. These behaviours changed:

| Area | fastlex | tolkien |
|---|---|---|
| No matcher matches | The parser loops forever | The character becomes an `OTHER` token |
| A match of length zero | The parser loops forever | Ignored |
| Order of matchers | Not defined between unrelated matchers: it fell out of how the sort walked them | The order given; an overriding matcher moves up to just before the first it overrides |
| Offsets | UTF-16 code units | UTF-8 bytes |
| Characters above U+FFFF | Seen as two surrogates | Seen as one character |
| Word lists (months, currencies) | Any prefix of a word counted as the word | The whole word must match |
| `LIKE_URL` | Matched any text, because of the prefix bug | Needs `http://`, `https://` or `www.`; allows `%`; leaves out sentence punctuation and an unopened closing bracket at the end |
| `LINE_TERMINATION` | Included the letters `v` and `f` and the backslash | Line-break classes BK, CR, LF, NL |
| Character sets | Written by hand | Generated from the Unicode data; see the notes below |
| `LIKE_EMAIL` | Kept a full stop after the address; any length before the `@`, which could start with `.` or `-` | Leaves the full stop out; at most 64 characters before the `@`, starting with a letter or a digit |
| `TIME` | Accepted hours 50 to 99 and any digits as seconds; read `10:30 Am` in `10:30 Amsterdam`; needed minutes | Hours 0 to 24, seconds 00 to 59; the marker must end where its word ends; `4 PM` is a time; not a time before another digit or before a colon and more (`07:01:75:c5`) |
| `DATE_ISO_8601` | Took the start of `2022-01-061` | No digit may follow the day |
| `TIME`, `DATE`, `DATE_ISO_8601` digits | Any Unicode digit | ASCII digits |
| `DATE` in numbers | Any three numbers of the right lengths with any two separators: `1.2.3`, `1/04 9`, `3,4,15`, `99/99/9999` | The same separator twice (never a comma, a space only after a leading year); month 1 to 12, day 1 to 31; a closing year of two or four digits; not the start of a longer run |
| `DATE` with a month name | Had to end at white space, a full stop, a comma or a colon, so `(Feb 17, 2009)` was not a date; no ordinals | May end anywhere but before a letter; `17th Feb 2009` is a date |
| `WORD` | Could start or end with a dash or an apostrophe: `--help`, `-word`, `dogs'` | Starts and ends with a letter |
| `ALPHA_NUMERIC_SEQUENCE` | Could start with a dash or an underscore, and end with a dash | Starts with a letter or a digit; does not end with a dash |
| `LIKE_NUMBER` | Any amount of punctuation before the first digit; kept the punctuation after the last one (`1.`, `100%,`) | At most two punctuation characters before it; ends with a number character or a percent sign |
| `CURRENCY` amounts | Could start with any run of full stops and commas; one trailing full stop or comma dropped after a symbol | Start with a digit or one full stop; all trailing full stops and commas dropped |
| `CURRENCY` names | Codes and abbreviations matched in any case (`5 km`, `5 all`); a name could be followed by a digit or an apostrophe (`94 won't`) | Names in their common cases, codes and abbreviations only as written; no letter, digit or apostrophe after the name |
| Currency list | Included the region names Abkhazia and Artsakh | Without them |
| Time to lex | Quadratic on a long run of dashes, dots or underscores | Linear |
| `PHONE_NUMBER` | A stub that never matched | Not ported |
| Matcher options | Character sets passed to the constructor | None yet: the matchers are fixed |

Generating the character sets changed their members:

- The white space that ends a word or a date is the Unicode `White_Space`
  property. It gains the no-break space and the other Unicode spaces, and
  loses U+2422, U+2423 and U+FEFF.
- Dashes are the Unicode `Dash` property. The set gains the hyphens of other
  scripts and the two-em and three-em dashes, and loses `~` and the four
  overline characters.
- The apostrophe set gains `’` (U+2019) and the Armenian apostrophe, and
  loses `»`.
- Question marks and exclamation marks both gain the interrobang `‽` and the
  double marks `⁈` and `⁉`.
- Commas lose the middle dot, colons lose `≔` and `≕`, and number signs lose
  the letters ℂ ℊ ℕ ℚ ℝ.
- Full stops, commas, colons, question marks and exclamation marks gain the
  forms of other scripts, such as `。` and `،`.

Not ported: the helpers in fastlex's `common` module that the parser does
not use (lossy counters, hashing, the benchmark helper).
