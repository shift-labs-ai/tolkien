#!/bin/sh
# Every check a change must pass. CI runs this file and nothing else.
set -eu
cd "$(dirname "$0")/.."

cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
# The library has no dependencies and must keep building for the browser.
cargo build --lib --target wasm32-unknown-unknown
# Compares the committed tables with what the rules produce. Downloads the
# pinned Unicode data files on first use.
python3 tools/gen_unicode.py --check
