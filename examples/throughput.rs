//! Measures how fast the default lexer reads a file.
//!
//!     cargo run --release --example throughput -- FILE [ROUNDS]

use std::time::Instant;

use tolkien::Lexer;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: throughput FILE [ROUNDS]");
        std::process::exit(2);
    };
    let rounds: u32 = args.next().and_then(|n| n.parse().ok()).unwrap_or(5).max(1);
    let text = std::fs::read_to_string(&path).expect("a readable UTF-8 file");
    let lexer = Lexer::default();

    let mut best = f64::MAX;
    let mut tokens = 0;
    for _ in 0..rounds {
        let start = Instant::now();
        tokens = lexer.tokens(&text).count();
        best = best.min(start.elapsed().as_secs_f64());
    }
    let megabytes = text.len() as f64 / 1e6;
    println!(
        "{megabytes:.1} MB, {tokens} tokens, best of {rounds}: {:.0} ms, {:.0} MB/s, {:.1} M tokens/s",
        best * 1e3,
        megabytes / best,
        tokens as f64 / best / 1e6,
    );
}
