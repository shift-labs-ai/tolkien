# tolkien

A lexer for natural text: one pass turns a `&str` into typed tokens (words,
numbers, dates, times, emails, URLs, amounts of money). A Rust rewrite of the
Kotlin library `Michaelliv/fastlex`. The repository is public and the
packages are on npm; nothing is pushed or published without being asked
for.

## Commands

```bash
tools/verify.sh                                    # every check, before a commit
python3 tools/gen_unicode.py                       # regenerate the character tables
python3 tools/corpora.py                           # download the test corpora into corpus/
python3 tools/evaluate.py                          # score tokens against labelled spans

cd rust                                            # the crate; cargo runs here
cargo test                                         # unit, integration and doc tests
cargo run -- -t "some text"                        # print tokens
cargo run --release --example throughput -- FILE   # measure speed
cargo run --release --example fuzz -- ../corpus/docs/*.txt   # property checks on real and mutated text
```

`tools/verify.sh` runs rustfmt, clippy with `-D warnings` (the library with
the shared binding layer, then each binding for its target), the tests, ten
seconds of the fuzzer on this repository's own text, the
`wasm32-unknown-unknown` build of the library and the stale check of the
generated tables. Then it builds the addon and the wasm package, runs the
TypeScript checks and tests against them, and packs both packages. CI runs
that script and nothing else, so a check is added there.

```bash
bun install                 # once
npm run build:native        # the addon for this machine, tolkien.<platform>.node
npm run build:wasm          # npm/wasm: the engine and its JavaScript
bun test                    # the Node package, its command, and the wasm package against it
bun run dev -- -t "some text"   # the command, from source
```

## Architecture

- `rust/src/cursor.rs`: `Cursor`, a byte position in a `&str` that matchers move
- `rust/src/matcher.rs`: the `Matcher` trait (`kind`, `overrides`,
  `can_start`, `matches`)
- `rust/src/lexer.rs`: `Lexer` (orders the matchers, runs them, also on several
  threads; `new` for any matchers, `with` for the built-in ones and more),
  `Token`, `Error`
- `rust/src/shortcuts.rs`: what the default lexer answers for plain words,
  numbers, white space and punctuation without asking every matcher
- `rust/src/matchers/`: one file per built-in matcher; `mod.rs` holds `all()`,
  the default order
- `rust/src/pattern.rs`: `Pattern`, a matcher built from a list of `Step`s over
  `Class`es of characters: matchers as data, for users and for bindings
- `rust/src/catalog.rs`: the catalog, matchers that are offered and not in
  the default lexer; each is an `Entry` with its examples and its source
- `docs/catalog.md`: where each form of the catalog is taken from, and what
  could not be found
- `rust/src/trie.rs`: `Trie`, longest-word lookup for month and currency names
- `rust/src/chars.rs`: character class functions over the generated tables
- `rust/src/unicode_tables.rs`: generated, never edited
- `rust/src/matchers/currency_data.rs`: currency names and symbols from fastlex
- `tools/gen_unicode.py`: the rules that define every character table
- `docs/charsets.md`: generated listing of every named set, for review
- `rust/src/main.rs`: the `tolkien` command of the crate, for work on the
  lexer: the arguments of the npm one, and few of its options
- `rust/tests/lexer.rs`: tests of the lexer as a whole
- `rust/tests/support/reference.rs`: the plain way to lex, every matcher asked
  at every position; the tests and the fuzzer both include it and compare
  the lexer with it
- `rust/tests/custom.rs`: matchers written as a user writes them, by hand and
  as patterns, added with `Lexer::with` and compared with asking every
  matcher and with each other
- `rust/tests/cli.rs`: tests of the command
- `rust/src/describe.rs`: a lexer from matchers described as JSON, and its
  tokens as arrays; shared by the two bindings
- `rust/src/bindings.rs`, `rust/src/wasm.rs`: the Node addon (napi-rs) and
  the browser API (wasm-bindgen) over `describe.rs`
- `src/describe.ts`: the builders a JavaScript user describes a matcher
  with; `src/lexer.ts`: `Lexer` and `Tokens` over either engine;
  `src/index.ts` and `src/wasm.ts`: the entry of each package
- `src/main.ts`: the `tolkien` command of the npm package (commander),
  with a file for each command in `src/commands/` and what they print with
  in `src/utils/`; `src/main.test.ts` runs it
- `native.cjs`: loads the addon of the platform
- `npm/`: a package for each platform's addon, and `npm/wasm` for browsers
- `test/fixtures.ts`: matchers and texts both packages are tested with
- `.github/workflows/release.yml`: builds every addon and the wasm package
  from a `v*` tag, loads each addon where it was built
  (`tools/load-addon.cjs`) and publishes them; run by hand, it does all
  but publish
- `tools/verify.sh`: every check, for CI and for a person
- `rust/examples/fuzz.rs`: the fuzzer, over the default lexer, the whole
  catalog and each group of it; its `shape` function states every built-in
  kind's rules a second time, independently of the matchers, and its long
  runs are built from the examples of the catalog
- `tools/corpora.py`, `tools/evaluate.py`: fetch the benchmark corpora and
  score the tokens against their labels; `docs/benchmarks.md` has the results
- `tools/downloads.py`: downloading and hash pinning, for the two scripts
  that fetch

## Key patterns

- **No dependencies.** The library has none, and builds for
  `wasm32-unknown-unknown`. The ones in `rust/Cargo.toml` are optional and
  belong to the bindings: each is behind the feature of a binding, and the
  default build compiles none of them. A dependency of the library itself
  needs a reason and must keep the wasm build.
- **One layout for a Rust engine with npm packages**, the one markit has:
  the crate in `rust/` with the bindings as features (`napi`, `wasm`), the
  Node package at the root (`package.json`, `src/`, `native.cjs`), a
  package for each platform's addon and one for the browser in `npm/`.
  A platform is four things: a triple in `package.json`, a package in
  `npm/`, a case in `native.cjs` and a row in the release workflow.
  Versions are the same in `rust/Cargo.toml`, `package.json` and every
  `npm/*/package.json`; the release workflow refuses a tag that differs.
- **A library and a command, as markit and codemode are.** The npm
  package exports the library from `dist/index.js` and has the command
  as its `bin`, `dist/main.js`, also exported as `./cli`. `src/main.ts`
  declares options and commands and nothing else; a command is a function
  in `src/commands/` that it imports when the command runs, so `--help`
  does not load the engine. A command calls the library as a user does,
  and prints through `output()` in `src/utils/output.ts`: every command
  takes `--json` and `-q`. The command's dependencies (`commander`,
  `chalk`) are the package's only ones, and the library imports neither;
  the browser package is built from `src/wasm.ts` alone and has no
  command. Where the two commands take the same argument it means the
  same: change both.
- **A binding passes data, never a callback.** A call from Rust into
  JavaScript for every token would cost far more than the token. Matchers
  come over once as JSON and become `Pattern`s (`describe.rs`), and tokens
  go back as two arrays, kinds and ends, with ends in UTF-16 code units.
  What both bindings need is in `describe.rs`, behind the `describe`
  feature, which links anywhere and so has tests; `bindings.rs` and
  `wasm.rs` only hand it to napi and to wasm-bindgen.
- **A step has three homes**: `Step` in `pattern.rs`, `StepSpec` in
  `describe.rs` and a builder in `src/describe.ts`. Add to all three, to
  `EVERY_STEP` in `describe.rs` and to `every step has a builder the
  engine takes` in `src/index.test.ts`. Each pair is held together: a
  new `Step` does not compile under test until `Step::described_as`
  names it, `every_step_of_the_engine_has_a_description` compares those
  names with the ones `StepSpec` takes, and the TypeScript test compares
  the steps the builders give with the list the engine gives.
- **No regular expressions.** A matcher reads characters from the cursor.
- **Matcher contract.** `matches` returns `true` with the cursor just past
  the token, or `false` with the cursor anywhere. The lexer gives each
  matcher a fresh cursor and ignores a match of length zero.
- **Linear time.** A matcher that reads a long run and then fails is asked
  again at the next character, which makes lexing that run quadratic. A
  matcher must fail within a bounded number of characters, or fail only on
  text that some matcher then takes as one token. The email matcher bounds
  its local part and the number matcher its leading punctuation for this
  reason. `long_runs_are_lexed_in_linear_time` in `rust/tests/lexer.rs` guards it;
  add the new matcher's worst run there.
- **Leaving out trailing punctuation.** A token does not end with the
  punctuation that joins its parts: a number with a full stop, a word with a
  dash, an address with a dot. A matcher whose run can end that way steps
  back over it with `Cursor::back_while`.
- **Not the start of something longer.** A token of a fixed shape is not
  taken when its run plainly goes on: a date before the same separator and
  a digit, a time before a colon and more. `Cursor::followed_by` asks.
- **Order.** Matchers are tried in the order given; one that overrides
  another moves up to just before it, and no matcher moves down. The
  built-in ones keep the order of `matchers::all()`, most specific first:
  adding a built-in matcher means choosing its place there.
- **Built on from outside.** `Lexer::with` puts a user's matchers before
  the built-in ones and keeps the shortcuts: the added matchers are asked
  first, each where it `can_start`. `rust/tests/custom.rs` is written with the
  public interface only and is the worked example; keep it compiling when
  that interface changes, and add to it what a new building block makes
  possible.
- **A long list is gathered, never grown.** Tokens whose number is not
  known are gathered in parts of a fixed size and copied once into a
  vector of exactly their number (`Gathered` in `lexer.rs`). A vector
  that grows asks the system to make a large block larger, and macOS 26
  keeps the old block: 48 bytes a token left behind at every call.
- **Fail fast, on what is cheap to test.** A matcher is asked far more
  often than it matches, so what it costs is what it costs to fail. Each
  layer refuses on a cheaper test than the next: `can_start` on one
  character, a `Trie` on two bytes, a `Pattern` on the places its first
  steps fix and on the characters its shortest token can hold
  (`first_places`), a shortcut on a few bytes before it asks a matcher. A
  new building block gets such a test, and the test may refuse only what
  the full match refuses: check it against the match without it, as the
  tests of `pattern.rs` do.
- **A pattern step is a cursor method.** `pattern.rs` adds no way of
  reading text: each `Step` does what a matcher written by hand does with
  one `Cursor` call or a short loop of them. A new step gets its rule for
  `can_start` in `Step::start`, which may say too much but never too
  little, and a line in `a_pattern_can_start_where_its_first_steps_can`.
- **Coverage.** The tokens of a text cover it with no gaps. `rust/tests/lexer.rs`
  checks this on fixed and on random text; keep both passing.
- **Character sets are generated.** To change one, change its rule in
  `tools/gen_unicode.py`, run the script, and read the diff of
  `docs/charsets.md`. Never edit `rust/src/unicode_tables.rs`.
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
  matchers do. `can_start` decides where a matcher is asked, so it must be
  true for the first character of every token the matcher is to find, and
  a shortcut must answer what the matchers, asked in order, would. Both
  are compared with asking every matcher at every position:
  `shortcuts_and_hints_change_no_token` and the random-text test in
  `rust/tests/lexer.rs`, and the fuzzer's `reference` check. **A new matcher,
  or a matcher that takes a new first character or a new kind of text, has
  to be checked against every rule in `shortcuts.rs`**; each rule names the
  matchers it has ruled out. Where a rule takes more than a few bytes to
  state, a shortcut asks the matcher itself and does not state it again.
- **Measure before and after.** `cargo run --release --example throughput`
  on the corpora, best of many rounds: a short file measured few times is
  noise. A repeated text flatters the result, because the processor learns
  it; use real text.
- **A change to a matcher is a change to three places:** the matcher, its
  rules in `shape` in `rust/examples/fuzz.rs`, and the README. Then run the
  fuzzer and `tools/evaluate.py` on the corpora and compare the scores with
  `docs/benchmarks.md`: a fix for one label must not cost another.

## Adding a matcher to the catalog

A built-in matcher is for what any text holds, and changes what every
user gets. Anything else goes in the catalog.

1. Read the standard or the vendor's page for the form, and write in the
   entry's `source` what it is and where the matcher departs from it. A
   form written from memory is wrong in ways that pass every test: the
   first version had the wrong alphabet for two kinds and missed forms of
   nine.
2. Add its kind constant and an `Entry` to `ENTRIES` in
   `rust/src/catalog.rs`, as a `Pattern` where the steps can say it and as
   a `Matcher` written by hand where they cannot. Place it before any
   entry that could take the same text.
3. Give the entry texts it matches and texts it must refuse. The refused
   ones are the point: the nearest things that are not one. The fuzzer
   takes its pieces and its long runs from these texts.
4. Bound what it reads: a `repeat` with its number of rounds, a `trim_end`
   that cannot give back everything a run took. Add its worst run to
   `long_runs_are_lexed_in_linear_time_with_the_whole_catalog`.
5. If a built-in matcher takes a longer token that starts the same way, as
   a date does for an ordinal, refuse that case and add it to
   `the_catalog_leaves_dates_times_and_amounts_alone`.
6. Add its row to the catalog table of the README and to the table of
   `docs/catalog.md`; `the_readme_lists_the_catalog` and
   `the_docs_give_the_source_of_every_matcher` print the tables they
   expect.
7. Look at what it finds in real text before trusting it:
   `python3 tools/evaluate.py --catalog`, and the tokens of its kind in
   `corpus/docs/*.txt`. Ten matchers of the first version were tightened
   after that look.

## Adding a matcher

1. Create `rust/src/matchers/<name>.rs` with a unit struct, its kind constant and
   the `Matcher` impl. Document what it accepts on the struct.
2. Add tests in the same file: what matches, what does not, and where a
   match ends inside longer text.
3. Export it from `rust/src/matchers/mod.rs` and add it to `all()` in the right
   place.
4. In `rust/tests/lexer.rs`, add a sample to
   `default_lexer_names_the_parts_of_a_sentence`, to the random-text alphabet
   and to the long runs.
5. Add the kind's rules to `shape` in `rust/examples/fuzz.rs` and pieces that
   start or break it to the fuzzer's `DICTIONARY`.
6. Give it a `can_start`, and read the rules of `rust/src/shortcuts.rs`: each
   one that covers a character the new matcher can start with must rule it
   out or step aside. Add texts on that edge to
   `shortcuts_and_hints_change_no_token`.
7. Add a row to the matcher table in the README.
