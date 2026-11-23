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
/// Matchers are tried in the order they are given, with one exception: a
/// matcher that [overrides](Matcher::overrides) another is tried before it,
/// and moves up to just before it if it was given later. No matcher moves
/// down, so adding a matcher never changes the order of the others.
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
    /// How many matchers stand before the built-in ones, when those are
    /// all there in their order. `shortcuts` then knows the commonest
    /// tokens without asking the built-in matchers, wherever none of the
    /// matchers before them finds one.
    added: Option<usize>,
    /// For every byte a token can start with, whether `shortcuts` is asked
    /// at once: none of the matchers before the built-in ones can start
    /// with it.
    shortcut_at: [bool; 256],
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
                    candidates.push(u16::try_from(index).expect("fewer than 65,536 matchers"));
                }
            }
        }
        starts[128] = candidates.len() as u32;
        Ok(Self {
            matchers,
            candidates,
            starts,
            added: None,
            shortcut_at: [false; 256],
        })
    }

    /// A lexer with every built-in matcher, and `matchers` tried before
    /// them.
    ///
    /// This is the lexer to build on: it gives the tokens of
    /// `Lexer::new(matchers.chain(matchers::all()))` and keeps what makes
    /// [`Lexer::default`] fast. The matchers given are asked first, each
    /// where it [can start](Matcher::can_start), and the rest of the work
    /// is that of the default lexer. So the cost of a matcher is the cost
    /// of asking it: one that says where it can start costs little, and one
    /// that does not is asked at every token.
    ///
    /// ```
    /// use tolkien::{Cursor, Lexer, Matcher};
    ///
    /// /// `AKIA` and sixteen capital letters or digits.
    /// struct AccessKey;
    ///
    /// impl Matcher for AccessKey {
    ///     fn kind(&self) -> &'static str {
    ///         "ACCESS_KEY"
    ///     }
    ///
    ///     fn can_start(&self, first: char) -> bool {
    ///         first == 'A'
    ///     }
    ///
    ///     fn matches(&self, cur: &mut Cursor<'_>) -> bool {
    ///         cur.advance_str("AKIA")
    ///             && cur.advance_exactly(16, |c| c.is_ascii_uppercase() || c.is_ascii_digit())
    ///             && !cur.peek().is_some_and(char::is_alphanumeric)
    ///     }
    /// }
    ///
    /// let lexer = Lexer::with([Box::new(AccessKey) as Box<dyn Matcher>]).unwrap();
    /// let kinds: Vec<_> = lexer.tokens("key AKIAIOSFODNN7EXAMPLE").map(|t| t.kind).collect();
    /// assert_eq!(kinds, ["WORD", "WHITE_SPACE", "ACCESS_KEY"]);
    /// ```
    pub fn with(matchers: impl IntoIterator<Item = Box<dyn Matcher>>) -> Result<Self, Error> {
        let mut list: Vec<_> = matchers.into_iter().collect();
        let added = list.len();
        list.extend(matchers::all());
        let mut lexer = Self::new(list)?;
        // An added matcher may override a built-in one and so stay before
        // it; a built-in one overrides no kind an added one can have.
        let (first, built_in) = lexer.matchers.split_at(added);
        let in_order = built_in
            .iter()
            .map(|matcher| matcher.kind())
            .eq(matchers::all().iter().map(|matcher| matcher.kind()));
        if in_order {
            for byte in 0..=255u8 {
                // A byte outside ASCII starts many characters: with added
                // matchers, the character is asked about when it is read.
                lexer.shortcut_at[byte as usize] = if byte.is_ascii() {
                    !first.iter().any(|matcher| matcher.can_start(byte as char))
                } else {
                    added == 0
                };
            }
            lexer.added = Some(added);
        }
        Ok(lexer)
    }

    /// The matchers that can start with the ASCII character `first`, as
    /// indexes into `matchers` in the order they are tried.
    #[inline]
    fn candidates_at(&self, first: char) -> &[u16] {
        let list = self.starts[first as usize] as usize..self.starts[first as usize + 1] as usize;
        &self.candidates[list]
    }

    /// The kind and the end of the token that starts at byte offset `start`
    /// of `text`, whose first character is `first`: the first of the
    /// matchers from index `from` on that finds one.
    #[inline]
    fn token_at(
        &self,
        text: &str,
        start: usize,
        first: char,
        from: usize,
    ) -> (&'static str, usize) {
        let attempt = |matcher: &dyn Matcher| {
            let end = match_at(matcher, text, start)?;
            Some((matcher.kind(), end))
        };
        let found = if first.is_ascii() {
            self.candidates_at(first)
                .iter()
                .filter(|&&index| index as usize >= from)
                .find_map(|&index| attempt(self.matchers[index as usize].as_ref()))
        } else {
            self.matchers[from..]
                .iter()
                .filter(|matcher| matcher.can_start(first))
                .find_map(|matcher| attempt(matcher.as_ref()))
        };
        found.unwrap_or((OTHER, start + first.len_utf8()))
    }

    /// The kind and the end of the token that starts at byte offset `start`
    /// of `text`, where no shortcut answered at once. With matchers before
    /// the built-in ones, those are asked, then the shortcuts, then the
    /// built-in ones.
    ///
    /// It is marked cold to keep it out of the loop over the tokens, which
    /// is slower with it inside.
    #[cold]
    fn token_from_matchers(&self, text: &str, start: usize) -> (&'static str, usize) {
        let first = text[start..]
            .chars()
            .next()
            .expect("the text has not ended");
        let added = match self.added {
            // The shortcut was asked already, or none is in use.
            Some(0) | None => return self.token_at(text, start, first, 0),
            Some(added) => added,
        };
        let attempt = |matcher: &dyn Matcher| {
            let end = match_at(matcher, text, start)?;
            Some((matcher.kind(), end))
        };
        let found = if first.is_ascii() {
            // The matchers that can start here are listed in the order
            // they are tried, so the added ones stand first in the list.
            self.candidates_at(first)
                .iter()
                .take_while(|&&index| (index as usize) < added)
                .find_map(|&index| attempt(self.matchers[index as usize].as_ref()))
        } else {
            self.matchers[..added]
                .iter()
                .filter(|matcher| matcher.can_start(first))
                .find_map(|matcher| attempt(matcher.as_ref()))
        };
        found
            .or_else(|| shortcuts::token(text, start))
            .unwrap_or_else(|| self.token_at(text, start, first, added))
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

    /// All the tokens of `text`, lexed on `threads` threads: one is started
    /// for every piece but the first, however many are asked for.
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
    /// two to three times faster: it knows what the built-in matchers
    /// answer for plain words, numbers, white space and punctuation without
    /// asking each of them. [`Lexer::with`] adds matchers to it.
    fn default() -> Self {
        Self::with([]).expect("the built-in matchers form a valid lexer")
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
/// order they were given in: a matcher that overrides another moves up to
/// just before it, and no matcher moves down.
fn order(matchers: Vec<Box<dyn Matcher>>) -> Result<Vec<Box<dyn Matcher>>, Error> {
    /// Where a matcher is on its way into the order.
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        Waiting,
        /// Its overriders are being placed. Meeting it again is a circle.
        Placing,
        Placed,
    }

    /// Places matcher `index` after every matcher that overrides it.
    /// `path` holds the matchers being placed, each overridden by the next.
    fn place(
        index: usize,
        matchers: &[Box<dyn Matcher>],
        states: &mut [State],
        path: &mut Vec<usize>,
        placed: &mut Vec<usize>,
    ) -> Result<(), Error> {
        match states[index] {
            State::Placed => return Ok(()),
            State::Placing => {
                // The path from this matcher on is a circle. It runs from
                // overridden to overrider; reversed, each overrides the next.
                let first = path.iter().position(|&on_path| on_path == index);
                let circle = &path[first.expect("a matcher being placed is on the path")..];
                return Err(Error::OverrideCycle(
                    circle.iter().rev().map(|&i| matchers[i].kind()).collect(),
                ));
            }
            State::Waiting => {}
        }
        states[index] = State::Placing;
        path.push(index);
        let kind = matchers[index].kind();
        for overrider in 0..matchers.len() {
            if matchers[overrider].overrides().contains(&kind) {
                place(overrider, matchers, states, path, placed)?;
            }
        }
        path.pop();
        states[index] = State::Placed;
        placed.push(index);
        Ok(())
    }

    let mut states = vec![State::Waiting; matchers.len()];
    let mut placed = Vec::with_capacity(matchers.len());
    for index in 0..matchers.len() {
        place(index, &matchers, &mut states, &mut Vec::new(), &mut placed)?;
    }
    let mut waiting: Vec<Option<Box<dyn Matcher>>> = matchers.into_iter().map(Some).collect();
    Ok(placed
        .into_iter()
        .filter_map(|index| waiting[index].take())
        .collect())
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
        let first = self.text.as_bytes()[start];
        let shortcut = if self.lexer.shortcut_at[first as usize] {
            shortcuts::token(self.text, start)
        } else {
            None
        };
        let (kind, end) =
            shortcut.unwrap_or_else(|| self.lexer.token_from_matchers(self.text, start));
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
