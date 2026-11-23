#!/bin/sh
# Every check a change must pass. CI runs this file and nothing else.
set -eu
cd "$(dirname "$0")/../rust"

cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
# Property checks on this repository's own text and on mutations of it. The
# corpora of tools/corpora.py give a deeper run; they are not in the repository.
cargo run --quiet --release --example fuzz -- --seconds 10 ../README.md ../AGENTS.md ../docs/charsets.md src/*.rs src/matchers/*.rs
# The library must keep building for the browser.
cargo build --lib --target wasm32-unknown-unknown
# Compares the committed tables with what the rules produce. Downloads the
# pinned Unicode data files on first use.
python3 ../tools/gen_unicode.py --check
