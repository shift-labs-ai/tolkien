//! Measures how fast the default lexer reads a file: on one thread, counting
//! the tokens as they come, and on every core, collecting them.
//!
//!     cargo run --release --example throughput -- FILE [ROUNDS] [catalog]
//!
//! With `catalog`, every matcher of the catalog is added to the lexer.

use std::time::Instant;

use tolkien::{catalog, Lexer};

/// The shortest of `rounds` runs of `run`, in seconds, and its result.
fn best(rounds: u32, run: impl Fn() -> usize) -> (f64, usize) {
    let mut best = (f64::MAX, 0);
    for _ in 0..rounds {
        let start = Instant::now();
        let tokens = run();
        best = (best.0.min(start.elapsed().as_secs_f64()), tokens);
    }
    best
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: throughput FILE [ROUNDS] [catalog]");
        std::process::exit(2);
    };
    let rounds: u32 = args.next().and_then(|n| n.parse().ok()).unwrap_or(5).max(1);
    let text = std::fs::read_to_string(&path).expect("a readable UTF-8 file");
    let lexer = match args.next().as_deref() {
        Some("catalog") => Lexer::with(catalog::all()).expect("the catalog forms a valid lexer"),
        _ => Lexer::default(),
    };
    let megabytes = text.len() as f64 / 1e6;

    let (seconds, tokens) = best(rounds, || lexer.tokens(&text).count());
    println!(
        "{megabytes:.1} MB, {tokens} tokens, best of {rounds}: {:.0} ms, {:.0} MB/s, {:.1} M tokens/s",
        seconds * 1e3,
        megabytes / seconds,
        tokens as f64 / seconds / 1e6,
    );
    let (seconds, _) = best(rounds, || lexer.tokens_parallel(&text).len());
    println!(
        "on every core, collected: {:.0} ms, {:.0} MB/s",
        seconds * 1e3,
        megabytes / seconds
    );
}
