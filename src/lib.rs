//! tolkien turns text into tokens.
//!
//! It is a lexer for natural text. One pass over a `&str` yields a stream of
//! [`Token`]s, each a slice of the text with a kind: a word, a number, a
//! date, a time, an email address, a URL, an amount of money, and so on.
//!
//! ```
//! use tolkien::Lexer;
//!
//! let lexer = Lexer::default();
//! for token in lexer.tokens("Call at 10:30 AM, or mail dana@acme.com") {
//!     println!("{:<14} {:?}", token.kind, token.text);
//! }
//! ```
//!
//! The design has three parts:
//!
//! - A [`Matcher`] recognises one kind of token. Each is a small hand-written
//!   recogniser that reads characters from a [`Cursor`]. There are no regular
//!   expressions.
//! - A [`Lexer`] holds an ordered list of matchers. At each position it takes
//!   the first one that matches. A matcher declares which kinds it
//!   [overrides](Matcher::overrides), and that decides the order.
//! - The tokens cover the whole text: every byte belongs to exactly one
//!   token.
//!
//! The built-in matchers are in [`matchers`]. Write your own by implementing
//! [`Matcher`].

#![deny(clippy::all)]
#![warn(missing_docs)]

pub mod chars;
mod cursor;
mod lexer;
mod matcher;
pub mod matchers;
mod shortcuts;
mod trie;
// Generated: its layout is the generator's.
#[rustfmt::skip]
mod unicode_tables;

pub use cursor::Cursor;
pub use lexer::{Error, Lexer, Token, Tokens, OTHER};
pub use matcher::Matcher;
pub use trie::Trie;

/// Compiles and runs the Rust examples in the README as tests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExamples;
