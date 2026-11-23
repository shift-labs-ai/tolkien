# tolkien

A lexer for natural text: one pass turns a `&str` into typed tokens (words,
numbers, dates, times, emails, URLs, amounts of money). A Rust rewrite of the
Kotlin library `Michaelliv/fastlex`. Internal: `publish = false`.

## Commands

```bash
tools/verify.sh                                    # every check, before a commit
cargo test                                         # unit, integration and doc tests
python3 tools/gen_unicode.py                       # regenerate the character tables
cargo run -- "some text"                           # print tokens
cargo run --release --example throughput -- FILE   # measure speed
python3 tools/corpora.py                           # download the test corpora into corpus/
cargo run --release --example fuzz -- corpus/docs/*.txt   # property checks on real and mutated text
python3 tools/evaluate.py                          # score tokens against labelled spans
```

`tools/verify.sh` runs rustfmt, clippy with `-D warnings`, the tests, the
`wasm32-unknown-unknown` build of the library and the stale check of the
generated tables. CI runs that script and nothing else, so a check is added
there.

## Architecture

- `src/cursor.rs`: `Cursor`, a byte position in a `&str` that matchers move
- `src/matcher.rs`: the `Matcher` trait (`kind`, `overrides`, `matches`)
- `src/lexer.rs`: `Lexer` (orders the matchers, runs them), `Token`, `Error`
- `src/matchers/`: one file per built-in matcher; `mod.rs` holds `all()`,
  the default order
- `src/trie.rs`: `Trie`, longest-word lookup for month and currency names
- `src/chars.rs`: character class functions over the generated tables
- `src/unicode_tables.rs`: generated, never edited
- `src/matchers/currency_data.rs`: currency names and symbols from fastlex
- `tools/gen_unicode.py`: the rules that define every character table
- `docs/charsets.md`: generated listing of every named set, for review
- `src/main.rs`: the `tolkien` command
- `tests/lexer.rs`: tests of the lexer as a whole
- `tests/cli.rs`: tests of the command
- `tools/verify.sh`: every check, for CI and for a person
- `examples/fuzz.rs`: the fuzzer; its `shape` function states every kind's
  rules a second time, independently of the matchers
- `tools/corpora.py`, `tools/evaluate.py`: fetch the benchmark corpora and
  score the tokens against their labels; `docs/benchmarks.md` has the results

## Key patterns

- **No dependencies.** The crate has none, and the library builds for
  `wasm32-unknown-unknown`. A new dependency needs a reason and must keep
  that build.
- **No regular expressions.** A matcher reads characters from the cursor.
- **Matcher contract.** `matches` returns `true` with the cursor just past
  the token, or `false` with the cursor anywhere. The lexer gives each
  matcher a fresh cursor and ignores a match of length zero.
- **Linear time.** A matcher that reads a long run and then fails is asked
  again at the next character, which makes lexing that run quadratic. A
  matcher must fail within a bounded number of characters, or fail only on
  text that some matcher then takes as one token. The email matcher bounds
  its local part and the number matcher its leading punctuation for this
  reason. `long_runs_are_lexed_in_linear_time` in `tests/lexer.rs` guards it;
  add the new matcher's worst run there.
- **Leaving out trailing punctuation.** A matcher whose run can end in
  sentence punctuation steps back over it with `Cursor::back_while`.
- **Order.** A matcher is tried before the kinds in its `overrides`. Other
  matchers keep the order of `matchers::all()`, most specific first. Adding
  a matcher means choosing its place there.
- **Coverage.** The tokens of a text cover it with no gaps. `tests/lexer.rs`
  checks this on fixed and on random text; keep both passing.
- **Character sets are generated.** To change one, change its rule in
  `tools/gen_unicode.py`, run the script, and read the diff of
  `docs/charsets.md`. Never edit `src/unicode_tables.rs`.
- **Behaviour changes against fastlex** are listed in the README. Add to
  that table when a matcher is changed on purpose.
- **A matcher reads forward only.** It never looks at what stands before
  its start, so that lexing can begin at any token boundary. The fuzzer's
  `no-left-context` check holds it to that.
- **A change to a matcher is a change to three places:** the matcher, its
  rules in `shape` in `examples/fuzz.rs`, and the README. Then run the
  fuzzer and `tools/evaluate.py` on the corpora and compare the scores with
  `docs/benchmarks.md`: a fix for one label must not cost another.

## Adding a matcher

1. Create `src/matchers/<name>.rs` with a unit struct, its kind constant and
   the `Matcher` impl. Document what it accepts on the struct.
2. Add tests in the same file: what matches, what does not, and where a
   match ends inside longer text.
3. Export it from `src/matchers/mod.rs` and add it to `all()` in the right
   place.
4. In `tests/lexer.rs`, add a sample to
   `default_lexer_names_the_parts_of_a_sentence`, to the random-text alphabet
   and to the long runs.
5. Add the kind's rules to `shape` in `examples/fuzz.rs` and pieces that
   start or break it to the fuzzer's `DICTIONARY`.
6. Add a row to the matcher table in the README.
