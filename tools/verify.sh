#!/bin/sh
# Every check a change must pass. CI runs this file and nothing else.
#
# It needs Rust with the wasm32-unknown-unknown target, Python 3, Node, Bun
# and the wasm-bindgen command of the version rust/Cargo.toml pins.
set -eu
cd "$(dirname "$0")/../rust"

cargo fmt --check
# The "describe" feature is what the two bindings share. It links anywhere,
# so it is linted and tested with everything else.
cargo clippy --all-targets --features describe -- -D warnings
cargo test --features describe
# Each binding links only where it runs, so it is linted and not tested here.
cargo clippy --lib --no-default-features --features napi -- -D warnings
cargo clippy --lib --target wasm32-unknown-unknown --no-default-features --features wasm -- -D warnings
# Property checks on this repository's own text and on mutations of it. The
# corpora of tools/corpora.py give a deeper run; they are not in the repository.
cargo run --quiet --release --example fuzz -- --seconds 10 ../README.md ../AGENTS.md ../docs/charsets.md src/*.rs src/matchers/*.rs
# The library alone has no dependencies and must keep building for the browser.
cargo build --lib --target wasm32-unknown-unknown
# Compares the committed tables with what the rules produce. Downloads the
# pinned Unicode data files on first use.
python3 ../tools/gen_unicode.py --check

# The Node package and the browser package, built from this source and
# tested against each other.
cd ..
bun install --frozen-lockfile
bun run check
npm run build:native
npm run build:wasm
bun test
npm run build
npm pack --dry-run >/dev/null
npm pack --dry-run ./npm/wasm >/dev/null
node -e "import('./dist/index.js').then(({ Lexer }) => { const token = new Lexer().tokens('built').at(0); if (token.kind !== 'WORD' || token.text !== 'built') process.exit(1) })"
