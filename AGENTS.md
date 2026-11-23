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

`tools/verify.sh` runs rustfmt, clippy with `-D warnings`, the tests, ten
seconds of the fuzzer on this repository's own text, the
`wasm32-unknown-unknown` build of the library and the stale check of the
generated tables. CI runs that script and nothing else, so a check is added
there.

## Architecture

- `src/cursor.rs`: `Cursor`, a byte position in a `&str` that matchers move
- `src/matcher.rs`: the `Matcher` trait (`kind`, `overrides`, `matches`)
- `src/lexer.rs`: `Lexer` (orders the matchers, runs them, also on several
  threads), `Token`, `Error`
- `src/shortcuts.rs`: what the default lexer answers for plain words, white
  space and punctuation without asking a matcher
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
- `tools/downloads.py`: downloading and hash pinning, for the two scripts
  that fetch

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
- **Leaving out trailing punctuation.** A token does not end with the
  punctuation that joins its parts: a number with a full stop, a word with a
  dash, an address with a dot. A matcher whose run can end that way steps
  back over it with `Cursor::back_while`.
- **Not the start of something longer.** A token of a fixed shape is not
  taken when its run plainly goes on: a date before the same separator and
  a digit, a time before a colon and more. `Cursor::followed_by` asks.
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
- **A matcher reads forward only.** A cursor shows nothing before the place
  it was created at, so a token depends only on the text from its start on.
  Lexing can begin at any token boundary, which is what `tokens_parallel`
  relies on. The fuzzer's `no-left-context` and `parallel` checks hold the
  lexer to that.
- **Speed never changes a token.** Three layers make the lexer fast: one
  generated lookup table for all character classes (`chars.rs`),
  `Matcher::can_start` so that a matcher is asked only where it can start,
  and `shortcuts.rs` for the default lexer. The last two restate what the
  matchers do. `can_start` must be true for the first character of anything
  `matches` accepts, and a shortcut must answer what the matchers, asked in
  order, would. Both are compared with asking every matcher at every
  position: `shortcuts_and_hints_change_no_token` and the random-text test
  in `tests/lexer.rs`, and the fuzzer's `reference` check. **A new matcher,
  or a matcher that takes a new first character or a new kind of text, has
  to be checked against every rule in `shortcuts.rs`**; each rule names the
  matchers it has ruled out.
- **Measure before and after.** `cargo run --release --example throughput`
  on the corpora, best of many rounds: a short file measured few times is
  noise. A repeated text flatters the result, because the processor learns
  it; use real text.
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
6. Give it a `can_start`, and read the rules of `src/shortcuts.rs`: each
   one that covers a character the new matcher can start with must rule it
   out or step aside. Add texts on that edge to
   `shortcuts_and_hints_change_no_token`.
7. Add a row to the matcher table in the README.
