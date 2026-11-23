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
- **Order.** A matcher lists the kinds it overrides and is tried before them.
  Between matchers that do not override each other, the one given first is
  tried first.
- **Full coverage.** Every byte of the text belongs to exactly one token. A
  character no matcher claims becomes a one-character `OTHER` token.

Offsets are byte offsets into the UTF-8 text.

## Built-in matchers

`Lexer::default()` uses all of them, in this order:

| Kind | Matches | Examples |
|---|---|---|
| `DATE_ISO_8601` | A full ISO 8601 date and the complete part of a time after it | `2022-01-06`, `2022-01-06T08:42:27.5+02:00` |
| `DATE` | Day, month and year, in numbers or with an English month name | `02/17/2009`, `17 Feb, 2009`, `Feb172009` |
| `TIME` | Hours and minutes, optional seconds, optional AM/PM | `7:20`, `04:01:30`, `8:05 PM`, `4:01 a.m.` |
| `LIKE_EMAIL` | `local@domain.tld` in ASCII | `dana@mail.example.co.uk` |
| `LIKE_URL` | `http://`, `https://` or `www.` and what follows | `https://t.co/2Ecdop6zl1` |
| `CURRENCY` | Currency symbols with an amount, or an amount with a currency name or code | `$5`, `€1,000.50`, `5 USD`, `5.4 Dollars` |
| `HASHTAG` | `#` and letters, digits, underscores | `#hash_tag` |
| `MENTION` | `@` and letters, digits, underscores | `@dana` |
| `WORD` | Letters, with dashes and one apostrophe inside | `word`, `first-second`, `don't` |
| `ALPHA_NUMERIC_SEQUENCE` | Letters mixed with digits, dashes, underscores | `armV7`, `some_word` |
| `LIKE_NUMBER` | Number characters and the punctuation written with them | `-0.5`, `1,000`, `100%`, `6.0.1` |
| `LINE_TERMINATION` | A run of line breaks | |
| `DIGIT`, `LETTER`, `WHITE_SPACE` | A run of one character class | |
| `SYMBOL` | One character that is none of the above | `(` |

The matchers check shape, not meaning. `99/99/9999` is a `DATE`.

Known weak spots, all taken over from fastlex:

- A version number with three parts is a `DATE`: `1.2.3`.
- A package reference is a `LIKE_EMAIL`: `liteparse@2.12.0`.
- A URL takes a closing bracket that follows it: `(https://example.com)`.
- Short currency abbreviations collide with units: `5 km`, `5 ft`.
- A number keeps the punctuation after it: `1.` at the end of a sentence.

## Your own matcher

```rust
use tolkien::{matchers, Cursor, Lexer, Matcher};

/// `$NAME`, an environment variable reference.
struct EnvVar;

impl Matcher for EnvVar {
    fn kind(&self) -> &'static str {
        "ENV_VAR"
    }

    fn overrides(&self) -> &'static [&'static str] {
        &[matchers::CURRENCY]
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        cur.advance_if(|c| c == '$')
            && cur.advance_while(|c| c.is_ascii_uppercase() || c == '_') > 0
    }
}

let mut list = matchers::all();
list.push(Box::new(EnvVar));
let lexer = Lexer::new(list).unwrap();
```

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

About 57 MB/s and 20 million tokens per second with every built-in matcher,
on an Apple M5 laptop, over 1.2 MB of English technical prose:

```bash
cargo run --release --example throughput -- FILE
```

## Differences from fastlex

The design is the same. These behaviours changed:

| Area | fastlex | tolkien |
|---|---|---|
| No matcher matches | The parser loops forever | The character becomes an `OTHER` token |
| A match of length zero | The parser loops forever | Ignored |
| Order of unrelated matchers | Not defined: it fell out of how the sort walked the matchers | The one given first is tried first |
| Offsets | UTF-16 code units | UTF-8 bytes |
| Characters above U+FFFF | Seen as two surrogates | Seen as one character |
| Word lists (months, currencies) | Any prefix of a word counted as the word | The whole word must match |
| `LIKE_URL` | Matched any text, because of the prefix bug | Needs `http://`, `https://` or `www.`; allows `%`; leaves out sentence punctuation at the end |
| `LINE_TERMINATION` | Included the letters `v` and `f` and the backslash | Line-break classes BK, CR, LF, NL |
| Character sets | Written by hand | Generated from the Unicode data; see the notes below |
| `LIKE_EMAIL` | Kept a full stop after the address | Leaves it out |
| `TIME` | Accepted hours 50 to 99; read `10:30 Am` in `10:30 Amsterdam` | Hours 0 to 24; the marker must end where its word ends |
| `TIME`, `DATE_ISO_8601` digits | Any Unicode digit | ASCII digits |
| `PHONE_NUMBER` | A stub that never matched | Not ported |
| Matcher options | Character sets passed to the constructor | None yet: the matchers are fixed |

Generating the character sets changed their members:

- The white space that ends a word or a date is the Unicode `White_Space`
  property. It gains the no-break space and the other Unicode spaces, and
  loses U+2422, U+2423 and U+FEFF.
- Dashes are the Unicode `Dash` property. The set gains the hyphens of other
  scripts and the two-em and three-em dashes, and loses `~` and the four
  overline characters.
- The apostrophe set gains `’` (U+2019) and loses `»`.
- Commas lose the middle dot, colons lose `≔` and `≕`, and number signs lose
  the letters ℂ ℊ ℕ ℚ ℝ.
- Full stops, commas, colons, question marks and exclamation marks gain the
  forms of other scripts, such as `。` and `،`.

Not ported: the helpers in fastlex's `common` module that the parser does
not use (lossy counters, hashing, the benchmark helper).
