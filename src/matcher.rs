//! The interface every token kind implements.

use crate::cursor::Cursor;

/// Recognises one kind of token at a position in a text.
///
/// A matcher is a small hand-written recogniser: it reads characters from a
/// [`Cursor`], and when the text at the cursor is one of its tokens it leaves
/// the cursor just past that token and returns `true`.
///
/// ```
/// use tolkien::{Cursor, Lexer, Matcher};
///
/// /// `$NAME`, an environment variable reference.
/// struct EnvVar;
///
/// impl Matcher for EnvVar {
///     fn kind(&self) -> &'static str {
///         "ENV_VAR"
///     }
///
///     fn matches(&self, cur: &mut Cursor<'_>) -> bool {
///         cur.advance_if(|c| c == '$')
///             && cur.advance_while(|c| c.is_ascii_uppercase() || c == '_') > 0
///     }
/// }
///
/// let lexer = Lexer::new([Box::new(EnvVar) as Box<dyn Matcher>]).unwrap();
/// let kinds: Vec<_> = lexer.tokens("$HOME!").map(|t| (t.kind, t.text)).collect();
/// assert_eq!(kinds, [("ENV_VAR", "$HOME"), ("OTHER", "!")]);
/// ```
pub trait Matcher: Send + Sync {
    /// The name of the token kind this matcher produces, such as `"DATE"`.
    /// It is the matcher's identity: a lexer holds one matcher per kind.
    fn kind(&self) -> &'static str;

    /// Kinds this matcher takes precedence over. The lexer tries this matcher
    /// before each of them, wherever they were registered.
    fn overrides(&self) -> &'static [&'static str] {
        &[]
    }

    /// Whether a token of this kind can start with `first`. The lexer asks
    /// once for every ASCII character when it is built, and then calls
    /// [`matches`](Matcher::matches) only where the answer is `true`, so a
    /// precise answer makes lexing faster.
    ///
    /// The default, `true`, is always correct. An override must answer `true`
    /// for the first character of every text that `matches` accepts.
    fn can_start(&self, _first: char) -> bool {
        true
    }

    /// Tries to match at the cursor. On a match, leaves the cursor just past
    /// the token and returns `true`. On no match, returns `false`; the cursor
    /// may be left anywhere.
    ///
    /// The answer depends only on the text from the cursor on: the cursor
    /// shows nothing before it.
    fn matches(&self, cur: &mut Cursor<'_>) -> bool;

    /// The token at the start of `text`, if `text` starts with one.
    fn match_prefix<'t>(&self, text: &'t str) -> Option<&'t str> {
        match_at(self, text, 0).map(|end| &text[..end])
    }
}

/// The end of the token that starts at byte offset `start` of `text`, if
/// `matcher` finds one. A match that consumed nothing is not a token: it
/// would never move a lexer on.
///
/// This is a function and not a method of the trait so that the lexer's loop
/// makes one dynamic call per attempt, to [`Matcher::matches`].
#[inline]
pub(crate) fn match_at<M: Matcher + ?Sized>(
    matcher: &M,
    text: &str,
    start: usize,
) -> Option<usize> {
    let mut cur = Cursor::at(text, start);
    (matcher.matches(&mut cur) && cur.pos() > start).then(|| cur.pos())
}
