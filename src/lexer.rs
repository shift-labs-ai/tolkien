//! The lexer: runs matchers over a text and yields tokens.

use std::fmt;
use std::iter::FusedIterator;

use crate::cursor::Cursor;
use crate::matcher::Matcher;
use crate::matchers;

/// The kind of a character no matcher claimed.
pub const OTHER: &str = "OTHER";

/// A piece of the text and what it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token<'t> {
    /// The [`Matcher::kind`] that produced the token, or [`OTHER`].
    pub kind: &'static str,
    /// Byte offset of the first character.
    pub start: usize,
    /// Byte offset just past the last character.
    pub end: usize,
    /// The token's text: `&text[start..end]`.
    pub text: &'t str,
}

/// Why a set of matchers cannot form a lexer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// No matchers were given.
    NoMatchers,
    /// Two matchers produce the same kind.
    DuplicateKind(&'static str),
    /// These matchers override each other in a circle, so no order exists.
    OverrideCycle(Vec<&'static str>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoMatchers => write!(f, "a lexer needs at least one matcher"),
            Error::DuplicateKind(kind) => write!(f, "two matchers produce the kind {kind}"),
            Error::OverrideCycle(kinds) => {
                write!(
                    f,
                    "matchers override each other in a circle: {}",
                    kinds.join(", ")
                )
            }
        }
    }
}

impl std::error::Error for Error {}

/// Splits text into tokens with a fixed, ordered list of matchers.
///
/// At each position the lexer tries its matchers in order and takes the first
/// that matches, then continues after that token. A character that no matcher
/// claims becomes a one-character [`OTHER`] token, so the tokens always cover
/// the whole text with no gaps and no overlaps.
///
/// # Order
///
/// A matcher is tried before every matcher it [overrides](Matcher::overrides).
/// Between matchers that do not override each other, the one given first is
/// tried first.
///
/// ```
/// use tolkien::Lexer;
///
/// let lexer = Lexer::default();
/// let tokens: Vec<_> = lexer
///     .tokens("Mail dana@acme.com by 2026-11-23")
///     .filter(|t| t.kind != "WHITE_SPACE")
///     .map(|t| (t.kind, t.text))
///     .collect();
/// assert_eq!(
///     tokens,
///     [
///         ("WORD", "Mail"),
///         ("LIKE_EMAIL", "dana@acme.com"),
///         ("WORD", "by"),
///         ("DATE_ISO_8601", "2026-11-23"),
///     ]
/// );
/// ```
pub struct Lexer {
    matchers: Vec<Box<dyn Matcher>>,
}

impl Lexer {
    /// A lexer over `matchers`, ordered as described on [`Lexer`].
    pub fn new(matchers: impl IntoIterator<Item = Box<dyn Matcher>>) -> Result<Self, Error> {
        let matchers: Vec<_> = matchers.into_iter().collect();
        if matchers.is_empty() {
            return Err(Error::NoMatchers);
        }
        for (i, matcher) in matchers.iter().enumerate() {
            if matchers[..i]
                .iter()
                .any(|earlier| earlier.kind() == matcher.kind())
            {
                return Err(Error::DuplicateKind(matcher.kind()));
            }
        }
        Ok(Self {
            matchers: order(matchers)?,
        })
    }

    /// The kinds of this lexer's matchers, in the order they are tried.
    pub fn kinds(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.matchers.iter().map(|matcher| matcher.kind())
    }

    /// The tokens of `text`, from start to end.
    pub fn tokens<'l, 't>(&'l self, text: &'t str) -> Tokens<'l, 't> {
        Tokens {
            lexer: self,
            text,
            pos: 0,
        }
    }
}

impl Default for Lexer {
    /// A lexer with every built-in matcher: [`matchers::all`].
    fn default() -> Self {
        Self::new(matchers::all()).expect("the built-in matchers form a valid lexer")
    }
}

impl fmt::Debug for Lexer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Lexer")
            .field("kinds", &self.kinds().collect::<Vec<_>>())
            .finish()
    }
}

/// Puts every matcher before the ones it overrides, and otherwise keeps the
/// order they were given in.
fn order(matchers: Vec<Box<dyn Matcher>>) -> Result<Vec<Box<dyn Matcher>>, Error> {
    let mut waiting: Vec<Option<Box<dyn Matcher>>> = matchers.into_iter().map(Some).collect();
    let mut ordered = Vec::with_capacity(waiting.len());
    while ordered.len() < waiting.len() {
        // The first waiting matcher that no other waiting matcher overrides.
        let next = waiting.iter().position(|slot| {
            slot.as_ref().is_some_and(|candidate| {
                !waiting
                    .iter()
                    .flatten()
                    .any(|m| m.overrides().contains(&candidate.kind()))
            })
        });
        match next {
            Some(i) => ordered.extend(waiting[i].take()),
            None => {
                let kinds = waiting.iter().flatten().map(|m| m.kind()).collect();
                return Err(Error::OverrideCycle(kinds));
            }
        }
    }
    Ok(ordered)
}

/// The tokens of one text. Created by [`Lexer::tokens`].
pub struct Tokens<'l, 't> {
    lexer: &'l Lexer,
    text: &'t str,
    pos: usize,
}

impl<'t> Iterator for Tokens<'_, 't> {
    type Item = Token<'t>;

    fn next(&mut self) -> Option<Token<'t>> {
        let start = self.pos;
        let first = self.text[start..].chars().next()?;
        let (kind, end) = self
            .lexer
            .matchers
            .iter()
            .find_map(|matcher| {
                let mut cur = Cursor::at(self.text, start);
                // A match that consumed nothing would never move the lexer on.
                (matcher.matches(&mut cur) && cur.pos() > start)
                    .then(|| (matcher.kind(), cur.pos()))
            })
            .unwrap_or((OTHER, start + first.len_utf8()));
        self.pos = end;
        Some(Token {
            kind,
            start,
            end,
            text: &self.text[start..end],
        })
    }
}

impl FusedIterator for Tokens<'_, '_> {}
