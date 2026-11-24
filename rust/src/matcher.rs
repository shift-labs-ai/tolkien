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
    /// before each of them, wherever it was given.
    fn overrides(&self) -> &'static [&'static str] {
        &[]
    }

    /// Whether a token of this kind can start with `first`.
    /// [`matches`](Matcher::matches) is asked only where the answer is
    /// `true`, by a lexer and by [`match_prefix`](Matcher::match_prefix)
    /// alike, so the matcher finds no token anywhere else.
    ///
    /// The default, `true`, asks `matches` at every character. A precise
    /// answer makes lexing much faster: a lexer asks once for every ASCII
    /// character when it is built, and [`Lexer::with`](crate::Lexer::with)
    /// keeps its shortcuts wherever no added matcher can start.
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

    /// The token at the start of `text`, if `text` starts with one: what a
    /// lexer that holds only this matcher gives first. It is the way to
    /// test a matcher on its own.
    fn match_prefix<'t>(&self, text: &'t str) -> Option<&'t str> {
        let first = text.chars().next()?;
        if !self.can_start(first) {
            return None;
        }
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

/// The kind and the end of the token that starts at byte offset `start` of
/// `text`, if `matcher` finds one.
#[inline]
pub(crate) fn ask<M: Matcher + ?Sized>(
    matcher: &M,
    text: &str,
    start: usize,
) -> Option<(&'static str, usize)> {
    match_at(matcher, text, start).map(|end| (matcher.kind(), end))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One or more `x`, which says it starts with a capital only.
    struct Capital;

    impl Matcher for Capital {
        fn kind(&self) -> &'static str {
            "CAPITAL"
        }

        fn can_start(&self, first: char) -> bool {
            first == 'X'
        }

        fn matches(&self, cur: &mut Cursor<'_>) -> bool {
            cur.advance_while(|c| c.eq_ignore_ascii_case(&'x')) > 0
        }
    }

    #[test]
    fn a_matcher_finds_no_token_where_it_cannot_start() {
        assert_eq!(Capital.match_prefix("Xxx!"), Some("Xxx"));
        assert_eq!(Capital.match_prefix("xxx"), None);
        assert_eq!(Capital.match_prefix(""), None);
    }
}
