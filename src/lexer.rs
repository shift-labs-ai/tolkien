//! The lexer: runs matchers over a text and yields tokens.

use std::fmt;
use std::iter::FusedIterator;

use crate::matcher::{match_at, Matcher};
use crate::matchers;
use crate::shortcuts;

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
    /// These matchers override each other in a circle: each one overrides
    /// the next and the last one overrides the first, so no order exists.
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
                    kinds.join(" > ")
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
    /// For every ASCII character, the matchers that can start with it, as
    /// indexes into `matchers` in the order they are tried. The list of
    /// character `c` is `candidates[starts[c]..starts[c + 1]]`.
    candidates: Vec<u16>,
    starts: [u32; 129],
    /// True for the default lexer, whose commonest tokens `shortcuts` knows
    /// without asking the matchers.
    shortcuts: bool,
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
        let matchers = order(matchers)?;
        let mut candidates = Vec::new();
        let mut starts = [0; 129];
        for ascii in 0..128u8 {
            starts[ascii as usize] = candidates.len() as u32;
            for (index, matcher) in matchers.iter().enumerate() {
                if matcher.can_start(ascii as char) {
                    candidates.push(index as u16);
                }
            }
        }
        starts[128] = candidates.len() as u32;
        Ok(Self {
            matchers,
            candidates,
            starts,
            shortcuts: false,
        })
    }

    /// The kind and the end of the token that starts at byte offset `start`
    /// of `text`, whose first character is `first`.
    #[inline]
    fn token_at(&self, text: &str, start: usize, first: char) -> (&'static str, usize) {
        let attempt = |matcher: &dyn Matcher| {
            let end = match_at(matcher, text, start)?;
            Some((matcher.kind(), end))
        };
        let found = if first.is_ascii() {
            let list =
                self.starts[first as usize] as usize..self.starts[first as usize + 1] as usize;
            self.candidates[list]
                .iter()
                .find_map(|&index| attempt(self.matchers[index as usize].as_ref()))
        } else {
            self.matchers
                .iter()
                .filter(|matcher| matcher.can_start(first))
                .find_map(|matcher| attempt(matcher.as_ref()))
        };
        found.unwrap_or((OTHER, start + first.len_utf8()))
    }

    /// The kinds of this lexer's matchers, in the order they are tried.
    pub fn kinds(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.matchers.iter().map(|matcher| matcher.kind())
    }

    /// The tokens of `text`, from start to end.
    pub fn tokens<'l, 't>(&'l self, text: &'t str) -> Tokens<'l, 't> {
        self.tokens_from(text, 0)
    }

    /// The tokens of `text` from byte offset `start` on.
    fn tokens_from<'l, 't>(&'l self, text: &'t str, start: usize) -> Tokens<'l, 't> {
        Tokens {
            lexer: self,
            text,
            pos: start,
        }
    }

    /// All the tokens of `text`, lexed on every core of the machine.
    ///
    /// The result is the same as `lexer.tokens(text).collect()`. It comes
    /// sooner for a long text; a short one is lexed on the calling thread.
    pub fn tokens_parallel<'t>(&self, text: &'t str) -> Vec<Token<'t>> {
        /// Below this many bytes for each thread, starting threads costs
        /// more than they save.
        const WORTH_A_THREAD: usize = 64 * 1024;
        let cores = std::thread::available_parallelism().map_or(1, usize::from);
        self.tokens_parallel_on(text, cores.min(text.len() / WORTH_A_THREAD))
    }

    /// All the tokens of `text`, lexed on `threads` threads.
    ///
    /// The text is cut into `threads` pieces of the same length. A cut
    /// rarely falls between two tokens, so each thread starts at its cut and
    /// lexes until it has passed the next one; the tokens of a piece are
    /// taken from the first one that starts where the tokens before it end.
    /// From such a place on they are right, because a token depends only on
    /// the text from its start on. The few tokens in between are lexed
    /// again.
    pub fn tokens_parallel_on<'t>(&self, text: &'t str, threads: usize) -> Vec<Token<'t>> {
        let piece = text.len().div_ceil(threads.max(1)).max(1);
        let mut cuts: Vec<usize> = (0..text.len())
            .step_by(piece)
            .map(|cut| {
                (cut..=text.len())
                    .find(|&at| text.is_char_boundary(at))
                    .unwrap_or(text.len())
            })
            .collect();
        cuts.dedup();
        // Threads cannot be started on wasm32.
        if cuts.len() < 2 || cfg!(target_arch = "wasm32") {
            return self.tokens(text).collect();
        }

        let lex_piece = |index: usize| -> Vec<Token<'t>> {
            let end = cuts.get(index + 1).copied().unwrap_or(text.len());
            let mut tokens = Vec::with_capacity((end - cuts[index]) / 3);
            for token in self.tokens_from(text, cuts[index]) {
                tokens.push(token);
                if token.end >= end {
                    break;
                }
            }
            tokens
        };
        let pieces: Vec<Vec<Token<'t>>> = std::thread::scope(|scope| {
            let threads: Vec<_> = (1..cuts.len())
                .map(|index| scope.spawn(move || lex_piece(index)))
                .collect();
            let first = lex_piece(0);
            std::iter::once(first)
                .chain(
                    threads
                        .into_iter()
                        .map(|thread| thread.join().expect("a matcher panicked")),
                )
                .collect()
        });

        let mut pieces = pieces.into_iter();
        let mut tokens = pieces.next().unwrap_or_default();
        tokens.reserve(text.len() / 3);
        for piece in pieces {
            let mut pos = tokens.last().map_or(0, |token| token.end);
            let piece_end = piece.last().map_or(0, |token| token.end);
            // Lex on from the last token until one of this piece starts
            // where the last one ends, or the piece is passed.
            while pos < piece_end {
                if let Ok(index) = piece.binary_search_by_key(&pos, |token| token.start) {
                    tokens.extend_from_slice(&piece[index..]);
                    break;
                }
                let token = self
                    .tokens_from(text, pos)
                    .next()
                    .expect("the text has not ended");
                tokens.push(token);
                pos = token.end;
            }
        }
        tokens
    }
}

impl Default for Lexer {
    /// A lexer with every built-in matcher: [`matchers::all`].
    ///
    /// It gives the same tokens as `Lexer::new(matchers::all())` and is
    /// several times faster: it knows what the built-in matchers answer for
    /// plain words, white space and punctuation without asking each of them.
    fn default() -> Self {
        let lexer = Self::new(matchers::all()).expect("the built-in matchers form a valid lexer");
        Self {
            shortcuts: true,
            ..lexer
        }
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
            None => return Err(Error::OverrideCycle(cycle(waiting.iter().flatten()))),
        }
    }
    Ok(ordered)
}

/// One circle of overrides among `stuck`, a set of matchers in which every
/// one is overridden by another of the set.
fn cycle<'m>(stuck: impl Iterator<Item = &'m Box<dyn Matcher>>) -> Vec<&'static str> {
    let stuck: Vec<_> = stuck.collect();
    // Walking from any matcher to one that overrides it must come back to a
    // matcher already on the path, and the path from there on is a circle.
    let mut path = vec![stuck[0].kind()];
    loop {
        let last = path[path.len() - 1];
        let overrider = stuck
            .iter()
            .find(|m| m.overrides().contains(&last))
            .expect("every stuck matcher is overridden by another")
            .kind();
        if let Some(first) = path.iter().position(|kind| *kind == overrider) {
            // The walk went from overridden to overrider; reverse it so that
            // each matcher overrides the next.
            let mut circle = path.split_off(first);
            circle.reverse();
            return circle;
        }
        path.push(overrider);
    }
}

/// The tokens of one text. Created by [`Lexer::tokens`].
#[derive(Debug, Clone)]
pub struct Tokens<'l, 't> {
    lexer: &'l Lexer,
    text: &'t str,
    pos: usize,
}

impl<'t> Iterator for Tokens<'_, 't> {
    type Item = Token<'t>;

    #[inline]
    fn next(&mut self) -> Option<Token<'t>> {
        let start = self.pos;
        if start >= self.text.len() {
            return None;
        }
        let shortcut = if self.lexer.shortcuts {
            shortcuts::token(self.text, start)
        } else {
            None
        };
        let (kind, end) = shortcut.unwrap_or_else(|| {
            let first = self.text[start..]
                .chars()
                .next()
                .expect("the text has not ended");
            self.lexer.token_at(self.text, start, first)
        });
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
