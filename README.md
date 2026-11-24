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
a Kotlin library. The crate has no dependencies. It is also an npm package
for [Node and for browsers](#from-javascript).

## How it works

- **Matchers.** Each kind of token has a matcher: a small recogniser that
  reads characters from a cursor, written by hand or described as a
  pattern. There are no regular expressions.
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

Phone numbers, IP and MAC addresses and much else are in
[the catalog](#the-catalog). No matcher exists yet for dates without a year
(`30 January`, `March 2006`) or amounts with a magnitude (`$5k`).
[docs/benchmarks.md](docs/benchmarks.md) measures what that costs.

## The catalog

Thirty-eight more matchers, for what some texts hold and some users look
for: network addresses, secrets, payment details, personal numbers,
technical identifiers and the smaller forms of written text. None is in
the default lexer. Add all of them, a group, or one by its kind:

```rust
use tolkien::catalog::{self, Group};
use tolkien::Lexer;

let everything = Lexer::with(catalog::all()).unwrap();
let secrets = Lexer::with(catalog::group(Group::Secrets)).unwrap();

let found: Vec<_> = everything
    .tokens("Ping 192.168.1.20 (1d:d8:97:09:54:39), call +1 (555) 010-9999")
    .filter(|token| catalog::entries().iter().any(|entry| entry.kind == token.kind))
    .map(|token| (token.kind, token.text))
    .collect();
assert_eq!(
    found,
    [
        ("IPV4", "192.168.1.20"),
        ("MAC_ADDRESS", "1d:d8:97:09:54:39"),
        ("PHONE_NUMBER", "+1 (555) 010-9999"),
    ]
);
```

```ts
import { catalog, Lexer } from "@shiftlabs/tolkien";

const lexer = new Lexer(catalog.all());              // or catalog.group("secrets", "network")
const keys = lexer.tokens(text).ofKind("AWS_ACCESS_KEY_ID", "GITHUB_TOKEN");
```

On the command line, `tolkien --catalog` adds them all.

| Kind | Group | Matches | Example |
|---|---|---|---|
| `CIDR` | network | An IPv4 address, a slash and a prefix length up to 32 | `10.0.0.0/8` |
| `IPV4` | network | Four numbers from 0 to 255 joined by full stops | `192.168.1.20` |
| `IPV6` | network | Eight groups of hex digits joined by colons, or fewer around one `::`; the last two may be an IPv4 address | `cc87:f687:8bdb:8a6c:ffac:0910:1bde:3a63` |
| `MAC_ADDRESS` | network | Six pairs of hex digits joined by the same colon or dash, or three groups of four joined by full stops with a letter among them | `1d:d8:97:09:54:39` |
| `URI` | network | A scheme other than `http` and what follows: `ssh://`, `ftp://`, `s3://`, `postgres://`, `mailto:`, `tel:` and others; it ends as a `LIKE_URL` does | `ssh://git@host.example/repo.git` |
| `AWS_ACCESS_KEY_ID` | secrets | `AKIA` or `ASIA` and sixteen characters of base32: capital letters and the digits 2 to 7 | `AKIAIOSFODNN7EXAMPLE` |
| `GITHUB_TOKEN` | secrets | `ghp_`, `gho_`, `ghu_`, `ghs_` or `ghr_` and 36 or more letters and digits, or `github_pat_` and 82 or more with underscores | `ghp_abcdefghijklmnopqrstuvwxyzABCDEFGHIJ` |
| `SLACK_TOKEN` | secrets | `xoxb-`, `xoxp-`, `xapp-`, `xwfp-`, an older `xox` prefix or a rotating `xoxe` one, and ten or more letters, digits and dashes | `xoxb-123456789012-abcdefABCDEF` |
| `STRIPE_KEY` | secrets | `sk_`, `pk_` or `rk_`, then `live_` or `test_`, and sixteen or more letters and digits | `sk_live_abcdefghijklmnop1234` |
| `GOOGLE_API_KEY` | secrets | `AIza` and 35 letters, digits, dashes and underscores | `AIzaSyA-abcdefghijklmnopqrstuvwxyz01234` |
| `NPM_TOKEN` | secrets | `npm_` and 36 letters and digits | `npm_abcdefghijklmnopqrstuvwxyz0123456789` |
| `JWT` | secrets | Three parts of base64url letters joined by full stops, the first two starting with `eyJ`; the third, the signature, may be empty | `eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2lnbmF0dXJl-_x` |
| `PEM_HEADER` | secrets | `-----BEGIN`, a label of up to eight words in capitals and digits with one space between them, `-----` | `-----BEGIN PRIVATE KEY-----` |
| `ETHEREUM_ADDRESS` | finance | `0x` and forty hex digits | `0xe1dcd20433acf4060193a3df869fc93a5d656d7f` |
| `CARD_NUMBER` | finance | 13 to 19 digits, in up to five groups joined by single spaces or dashes, the first of four digits or more | `4111111111111111` |
| `IBAN` | finance | Two capital letters, two digits, and 11 to 30 capital letters and digits, together or in groups of up to four | `GB29NWBK60161331926819` |
| `BITCOIN_ADDRESS` | finance | `1` or `3` and 25 to 34 base58 characters, or `bc1` and 11 to 71 characters of the bech32 alphabet, in one case | `1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa` |
| `CASHTAG` | finance | `$` and one to six capital letters, then a full stop or an underscore and one or two more, or not | `$AAPL` |
| `IMEI` | personal | Digits in groups of 2, 6, 6 and 1 joined by dashes, or with a last group of 2 for the number with a software version | `39-650908-828872-7` |
| `US_SSN` | personal | Digits in groups of 3, 2 and 4 joined by the same dash or space, without the numbers that are never assigned: a first group of 000, 666 or 900 to 999, a group of zeros | `543-21-6046` |
| `COORDINATES` | personal | A latitude up to 90 and a longitude up to 180 in decimal degrees with a comma between them: in square brackets, or with four or more decimals each | `[39.6601,-36.8189]` |
| `PHONE_NUMBER` | personal | 7 to 15 digits after a `+`, after an area code in brackets, or in groups joined by dashes or full stops | `+1 (555) 010-9999` |
| `UUID` | technical | Hex digits in groups of 8, 4, 4, 4 and 12 joined by dashes | `123e4567-e89b-12d3-a456-426614174000` |
| `HASH` | technical | 32, 40, 56, 64, 96 or 128 hex digits: an MD5, SHA-1 or SHA-2 digest | `d41d8cd98f00b204e9800998ecf8427e` |
| `HEX_NUMBER` | technical | `0x` and hex digits | `0xFF` |
| `VERSION` | technical | `v`, numbers joined by full stops, and a pre-release or build after a dash or a plus | `v1.2.3` |
| `SCIENTIFIC_NUMBER` | technical | A number, `e` or `E`, and an exponent | `6.02e23` |
| `HEX_COLOR` | technical | `#` and 3, 4, 6 or 8 hex digits; without a letter among them, 6 only. A hashtag of those letters is one too: `#decade` | `#fff` |
| `FILE_PATH` | technical | A path that starts with `~/` or `../`, or with `./` and holds a second slash or a full stop | `~/Projects/tolkien/README.md` |
| `WINDOWS_PATH` | technical | A drive letter, `:\` and a path of names without spaces | `C:\Users\dana\notes.txt` |
| `UTC_OFFSET` | technical | `UTC` or `GMT`, then `+` and hours up to 14 or `-` and hours up to 12, and `:00`, `:30` or `:45` or not | `UTC+2` |
| `ISBN` | technical | The number of a book: 13 digits that start with `978` or `979`, together or in five groups, or 10 in four groups, of which the last may be an `X` | `978-3-931911-18-8` |
| `ORDINAL` | text | Digits and `st`, `nd`, `rd` or `th`, unless a month name follows | `1st` |
| `FRACTION` | text | Two numbers of up to three digits, neither starting with a zero, with a slash between them | `3/4` |
| `DURATION` | text | A number and a unit of time, with a space between them or not: `90 minutes`, `5ms`, `2 hrs` | `90 minutes` |
| `MEASUREMENT` | text | A number and a unit of length, mass, volume, speed, temperature, data, power or frequency | `5 km` |
| `ABBREVIATION` | text | Two to six single letters, each with a full stop after it | `e.g.` |
| `EMOTICON` | text | A face or a heart made of marks: `:-)`, `;)`, `:D`, `<3`, `^_^` | `:-)` |

What to know:

- **They are tried before the built-in matchers**, in the order of the
  table within a group and with the more specific first where two could
  take the same text: an IMEI and an ISBN before a card number, a social
  security number before a telephone number.
- **They check shape.** A `CARD_NUMBER` has the digits of a card number,
  and so has any number of 16 digits; whether its checksum holds is a check
  of your own on the token.
- **A few shapes are shared.** `#decade` is a `HEX_COLOR`, `$HOME` a
  `CASHTAG`, `2E18` a `SCIENTIFIC_NUMBER`, and a date without a year such
  as `1/20` a `FRACTION`. Take the kinds you need.
- **A form is what its source says, and no more.** Where a vendor
  publishes no length, none is exact here, and a vendor may change a
  format without notice.
- **Dates, times and amounts stay with the built-in matchers**: `17th Feb
  2009` is a `DATE` and not an `ORDINAL`.
- **Each costs where it can start.** A matcher that starts with a text
  nothing else starts with, as the secrets do, costs next to nothing. One
  that starts with any letter or any hex digit is asked at every word. With
  all thirty-eight the texts of the [speed table](#speed) lex at 95 to
  160 MB/s on one thread, and 265 to 640 MB/s on every core.

[docs/catalog.md](docs/catalog.md) gives the standard or the vendor page
each form is taken from, and where a matcher departs from it.
[docs/benchmarks.md](docs/benchmarks.md) has what they find in labelled
text.

## Testing against real text

Two tools check the lexer against text nobody here wrote. Both read corpora
that `tools/corpora.py` downloads into `corpus/`, which is not committed.

```bash
python3 tools/corpora.py                                          # fetch and prepare
cd rust
cargo run --release --example fuzz -- ../corpus/docs/*.txt        # properties
cargo build --release && python3 ../tools/evaluate.py             # labelled spans
```

- **`rust/examples/fuzz.rs`** lexes every document, then mutates documents at
  random, and checks properties that must hold for any text: the tokens
  cover it, they are the tokens of asking every matcher at every position,
  they are the same when the text is lexed in pieces on several threads,
  every token of a built-in kind has the form its kind documents, a token
  on its own is the same token, a token does not depend on what stands
  before it, and the time is in proportion to the length. It checks three
  lexers: the default one and the default one with the whole catalog on
  every text, and the default one with one group of it on every mutated
  text. The start of every
  example of the catalog is also repeated into a long run and timed, which
  is where a matcher that reads without a bound shows. A failing text is
  cut down and printed.
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

Most matchers need no code. A `Pattern` is a matcher described as a list
of steps, each one thing the cursor does:

```rust
use tolkien::pattern::{Class, Pattern, Step};
use tolkien::{Lexer, Matcher};

let digit = || Class::range('0', '9');
let hex = || digit() | Class::range('a', 'f') | Class::range('A', 'F');
let letter_or_digit = || Class::letter() | Class::digit();

// 13 to 19 digits in at most eight groups joined by a space or a dash.
let card = Pattern::new(
    "CARD_NUMBER",
    [
        Step::run(digit()),
        Step::repeat(7, [Step::one(Class::chars(" -")), Step::run(digit())]),
        Step::not(letter_or_digit()),
    ],
)
.with_count(digit(), 13, 19);

// Six pairs of hex digits joined by the same colon or dash.
let pair = || Step::exactly(2, hex());
let again = || Step::same_separator();
let mac = Pattern::new(
    "MAC_ADDRESS",
    [
        pair(),
        Step::separator(Class::chars(":-")),
        pair(), again(), pair(), again(), pair(), again(), pair(), again(), pair(),
        Step::not(letter_or_digit()),
    ],
);

assert_eq!(card.match_prefix("4111 1111-1111 1111."), Some("4111 1111-1111 1111"));
assert_eq!(mac.match_prefix("1d:d8:97:09:54:39"), Some("1d:d8:97:09:54:39"));
assert_eq!(mac.match_prefix("1d:d8-97:09:54:39"), None);

let lexer = Lexer::with([Box::new(card) as Box<dyn Matcher>, Box::new(mac)]).unwrap();
```

| Step | What it does |
|---|---|
| `one`, `run`, `exactly`, `between` | Characters of a class: one, as many as there are, a fixed number, a range |
| `text` | A fixed text |
| `words` | The longest word of a `Trie` |
| `number` | ASCII digits with a largest value, such as a part of an IP address |
| `separator`, `same_separator` | A character of a class, and that same character again |
| `not`, `not_before` | What must not come next: a class, or a list of steps |
| `optional`, `repeat`, `either` | Steps that may be absent, that go round a bounded number of times, or that are a choice |
| `trim_end` | Step back over a class at the end |
| `Pattern::with_count` | How many characters of a class the whole token holds |

A pattern fails fast by itself. From its steps it works out three things
that are cheap to test before any step runs: the characters a token can
start with, the characters its first steps fix in place, and, up to the
length of the shortest token, that each character is one the pattern can
take at all. A card number is refused at `2009 was` by its sixth
character.

So a pattern costs what the same matcher written by hand costs: on the
texts of the table below, each of five patterns alone was within 3% of
its matcher by hand, and the five together within 6 to 9%. The exception
is text a pattern has to read through before it fails, such as a column
of numbers for the card number: there the steps run, and they cost about
twice the code written by hand.

Bound what a pattern reads. `repeat` takes the largest number of rounds
for that reason: a card number of any number of groups reads a whole
column of numbers, fails, and reads it again from the next number.

A pattern is data, which is what lets a matcher be written in another
language or in a file and still run here. It cannot check a rule over the
whole token, such as a checksum: lex with the pattern, then check the
tokens of its kind.

What to know when writing a matcher by hand:

- **Order is the order you give.** The matchers passed to `Lexer::with`
  are tried before the built-in ones, first to last, so they win where
  both match. Nothing has to be declared. `overrides` is for a matcher
  that is given later than one it must beat: it moves up to just before
  it, and no other matcher changes place.
- **`can_start` is a rule, not a hint.** The matcher is asked only where it
  answers `true`, by the lexer and by `match_prefix` alike. It is also
  what keeps the lexer fast: an added matcher costs where it can start, and
  everywhere else the lexer is the default one. A matcher that can start
  with any letter is asked at every word.
- **The cursor reads forward.** `peek`, `advance_if`, `advance_while`,
  `advance_exactly` and `advance_str` move it; `followed_by` looks two
  characters ahead without moving; `back_while` steps back over
  punctuation at the end; `since` gives the text passed; `set_pos` returns
  to a saved place. It shows nothing before the token.
- **Fail early.** A matcher that reads a long run and then fails is asked
  again one character later. Bound what it reads before it can fail.
- **`Trie`** finds the longest of many words at the cursor, and
  `tolkien::chars` has the character classes the built-in matchers use.

[rust/tests/custom.rs](rust/tests/custom.rs) has four matchers written both ways,
by hand and as patterns, and checks that the two agree: a card number, an
IP address, a MAC address and a list of keywords.

`Lexer::new` takes any list of matchers and nothing else: no built-in
matcher is added, and each matcher is asked for every token it can start.

## From JavaScript

The same lexer runs in Node, as a native addon, and in a browser, as
WebAssembly. Both packages have one API.

```bash
npm install @shiftlabs/tolkien        # Node 20.19 or later; macOS, Linux and Windows
npm install @shiftlabs/tolkien-wasm   # browsers
```

```ts
import { Lexer } from "@shiftlabs/tolkien";

const lexer = new Lexer();
for (const token of lexer.tokens("Dana paid $1,250.50 on Feb 17, 2009")) {
  console.log(token.kind, token.start, token.end, token.text);
}
```

`start` and `end` are indexes into the string, so
`text.slice(token.start, token.end)` is the token.

A matcher of your own is described with the same steps as a
[pattern](#your-own-matcher) in Rust, and runs in Rust: nothing of yours is
called while a text is lexed.

```ts
import {
  Lexer, matcher, run, one, repeat, not, chars, range, letter, digit,
} from "@shiftlabs/tolkien";

const asciiDigit = range("0", "9");

// 13 to 19 digits in at most eight groups joined by a space or a dash.
const cardNumber = matcher(
  "CARD_NUMBER",
  [
    run(asciiDigit),
    repeat(7, [one(chars(" -")), run(asciiDigit)]),
    not([letter, digit]),
  ],
  { count: { of: asciiDigit, min: 13, max: 19 } },
);

const lexer = new Lexer([cardNumber]);
const cards = lexer.tokens(text).ofKind("CARD_NUMBER");
```

A rule over the whole token, such as a checksum, is a check of your own on
the tokens that come back: `cards.filter((card) => luhn(card.text))`.

The tokens of a text are kept as two arrays of numbers. Going through them
with `for...of` makes an object for each; on a long text,
`tokens.kind(i)`, `tokens.start(i)`, `tokens.end(i)` and `tokens.text(i)`
read one without making anything.

Through the addon the texts of the [speed table](#speed) lex at 220 to
510 MB/s, tokens included. A long text is lexed on every core.

### Browser (WebAssembly)

```ts
import init, { Lexer } from "@shiftlabs/tolkien-wasm";

await init(); // loads the engine, once
const tokens = new Lexer().tokens("Mail dana@acme.com by 2026-11-23");
```

`init` fetches `engine_bg.wasm` from beside the module; pass a URL, a
`Response` or the bytes when a bundler or a server puts it elsewhere. A text
is lexed on the calling thread, so lex a long one in a Web Worker.

## Command line

```bash
cd rust
cargo run -- "Mail dana@acme.com by 2026-11-23"   # kind, start, end, text
cargo run -- --skip-space < notes.txt
cargo run -- --catalog "ssh 192.168.1.20"         # with every matcher of the catalog
```

## Character sets

No character table is written by hand. `tools/gen_unicode.py` builds them
from the Unicode Character Database (Unicode 18.0.0, files pinned by hash)
and writes two files:

- `rust/src/unicode_tables.rs`, the tables the code uses.
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
cd rust && cargo run --release --example throughput -- FILE
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
  and `rust/src/shortcuts.rs` reads them straight from the bytes, eight at a
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
| `PHONE_NUMBER` | A stub that never matched | In [the catalog](#the-catalog), with rules of its own |
| Matcher options | Character sets passed to the constructor | None: the built-in matchers are fixed, and a `Pattern` is the way to a matcher with other sets |

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
