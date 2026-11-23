# tolkien

A lexer for natural text: one pass turns a `&str` into typed tokens (words,
numbers, dates, times, emails, URLs, amounts of money). A Rust rewrite of the
Kotlin library `Michaelliv/fastlex`. Internal: `publish = false`.

## Commands

```bash
cargo test                                         # unit, integration and doc tests
cargo fmt --check                                  # formatting
cargo clippy --all-targets -- -D warnings          # lints
cargo build --lib --target wasm32-unknown-unknown  # the library must build for wasm32
python3 tools/gen_unicode.py                       # regenerate the character tables
python3 tools/gen_unicode.py --check               # fail if they are stale
cargo run -- "some text"                           # print tokens
cargo run --release --example throughput -- FILE   # measure speed
```

Run all of the first five before a commit. CI runs the same.

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

## Key patterns

- **No dependencies.** The crate has none, and the library builds for
  `wasm32-unknown-unknown`. A new dependency needs a reason and must keep
  that build.
- **No regular expressions.** A matcher reads characters from the cursor.
- **Matcher contract.** `matches` returns `true` with the cursor just past
  the token, or `false` with the cursor anywhere. The lexer gives each
  matcher a fresh cursor and ignores a match of length zero.
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

## Adding a matcher

1. Create `src/matchers/<name>.rs` with a unit struct, its kind constant and
   the `Matcher` impl. Document what it accepts on the struct.
2. Add tests in the same file: what matches, what does not, and where a
   match ends inside longer text.
3. Export it from `src/matchers/mod.rs` and add it to `all()` in the right
   place.
4. Add a sample to `default_lexer_names_the_parts_of_a_sentence` and to the
   random-text alphabet in `tests/lexer.rs`.
5. Add a row to the matcher table in the README.
