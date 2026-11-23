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

    /// Tries to match at the cursor. On a match, leaves the cursor just past
    /// the token and returns `true`. On no match, returns `false`; the cursor
    /// may be left anywhere.
    fn matches(&self, cur: &mut Cursor<'_>) -> bool;

    /// The token at the start of `text`, if `text` starts with one.
    fn match_prefix<'t>(&self, text: &'t str) -> Option<&'t str> {
        let mut cur = Cursor::new(text);
        if self.matches(&mut cur) && cur.pos() > 0 {
            Some(&text[..cur.pos()])
        } else {
            None
        }
    }
}
