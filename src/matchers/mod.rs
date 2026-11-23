//! The built-in matchers.
//!
//! Each matcher is a unit struct that implements [`Matcher`], next to a
//! constant with the kind it produces. [`all`] returns every one of them in
//! the order [`Lexer::default`](crate::Lexer::default) uses.

mod alpha_numeric;
mod base;
mod currency;
mod currency_data;
mod date;
mod email;
mod iso8601;
mod number;
mod tags;
mod time;
mod url;
mod word;

pub use alpha_numeric::{AlphaNumericSequence, ALPHA_NUMERIC_SEQUENCE};
pub use base::{
    Digit, Letter, LineTermination, Symbol, WhiteSpace, DIGIT, LETTER, LINE_TERMINATION, SYMBOL,
    WHITE_SPACE,
};
pub use currency::{Currency, CURRENCY};
pub(crate) use date::starts_with_month;
pub use date::{Date, DATE};
pub(crate) use email::local_part;
pub use email::{LikeEmail, LIKE_EMAIL};
pub use iso8601::{Iso8601, DATE_ISO_8601};
pub use number::{LikeNumber, LIKE_NUMBER};
pub use tags::{Hashtag, Mention, HASHTAG, MENTION};
pub use time::{Time, TIME};
pub(crate) use url::has_prefix as has_url_prefix;
pub use url::{LikeUrl, LIKE_URL};
pub use word::{Word, WORD};

use crate::matcher::Matcher;

/// The kinds of the four [`base`] matchers. Every other built-in matcher
/// overrides them.
pub const BASE: &[&str] = &[DIGIT, LETTER, SYMBOL, WHITE_SPACE];

/// [`BASE`] and [`LIKE_NUMBER`]: what a matcher overrides when its tokens
/// start like a number does.
pub(crate) const BASE_AND_NUMBER: &[&str] = &[DIGIT, LETTER, SYMBOL, WHITE_SPACE, LIKE_NUMBER];

/// The four matchers that together claim every character: runs of digits,
/// runs of letters, runs of white space, and single symbols.
pub fn base() -> Vec<Box<dyn Matcher>> {
    vec![
        Box::new(Digit),
        Box::new(Letter),
        Box::new(Symbol),
        Box::new(WhiteSpace),
    ]
}

/// Every built-in matcher, most specific first.
///
/// The order matters where two matchers accept the same text and neither
/// overrides the other: the earlier one wins. A URL is tried before a word,
/// for example, because `www` on its own is also a word.
pub fn all() -> Vec<Box<dyn Matcher>> {
    let mut matchers: Vec<Box<dyn Matcher>> = vec![
        Box::new(Iso8601),
        Box::new(Date),
        Box::new(Time),
        Box::new(LikeEmail),
        Box::new(LikeUrl),
        Box::new(Currency),
        Box::new(Hashtag),
        Box::new(Mention),
        Box::new(Word),
        Box::new(AlphaNumericSequence),
        Box::new(LikeNumber),
        Box::new(LineTermination),
    ];
    matchers.extend(base());
    matchers
}
