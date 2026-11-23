//! A position in a text that matchers move forward.

/// A cursor over a `&str`. Positions are byte offsets and always sit on a
/// character boundary.
///
/// A matcher reads with [`peek`](Cursor::peek) and moves with the `advance*`
/// methods. It may leave the cursor anywhere when it reports no match: the
/// lexer gives every matcher its own cursor.
#[derive(Debug, Clone)]
pub struct Cursor<'t> {
    text: &'t str,
    pos: usize,
}

impl<'t> Cursor<'t> {
    /// A cursor at the start of `text`.
    pub fn new(text: &'t str) -> Self {
        Self { text, pos: 0 }
    }

    /// A cursor at byte offset `pos` of `text`.
    ///
    /// # Panics
    /// If `pos` is not on a character boundary of `text`.
    pub fn at(text: &'t str, pos: usize) -> Self {
        assert!(
            text.is_char_boundary(pos),
            "position {pos} is not a character boundary"
        );
        Self { text, pos }
    }

    /// The current byte offset.
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Moves to byte offset `pos`, usually one saved earlier from [`pos`](Cursor::pos).
    ///
    /// # Panics
    /// If `pos` is not on a character boundary of the text.
    pub fn set_pos(&mut self, pos: usize) {
        assert!(
            self.text.is_char_boundary(pos),
            "position {pos} is not a character boundary"
        );
        self.pos = pos;
    }

    /// True when no characters are left.
    pub fn is_end(&self) -> bool {
        self.pos >= self.text.len()
    }

    /// The text from the cursor to the end.
    pub fn rest(&self) -> &'t str {
        &self.text[self.pos..]
    }

    /// The character at the cursor, or `None` at the end.
    #[inline]
    pub fn peek(&self) -> Option<char> {
        let byte = *self.text.as_bytes().get(self.pos)?;
        if byte < 0x80 {
            Some(byte as char)
        } else {
            self.text[self.pos..].chars().next()
        }
    }

    /// The character just before the cursor, or `None` at the start.
    pub fn prev(&self) -> Option<char> {
        self.text[..self.pos].chars().next_back()
    }

    /// Moves past the character at the cursor and returns it.
    #[inline]
    pub fn advance(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    /// Moves back one character and returns it.
    pub fn back(&mut self) -> Option<char> {
        let c = self.prev()?;
        self.pos -= c.len_utf8();
        Some(c)
    }

    /// Moves past the character at the cursor if `pred` accepts it.
    #[inline]
    pub fn advance_if(&mut self, pred: impl FnOnce(char) -> bool) -> bool {
        match self.peek() {
            Some(c) if pred(c) => {
                self.pos += c.len_utf8();
                true
            }
            _ => false,
        }
    }

    /// Moves past `s` if the rest of the text starts with it.
    pub fn advance_str(&mut self, s: &str) -> bool {
        if self.rest().starts_with(s) {
            self.pos += s.len();
            true
        } else {
            false
        }
    }

    /// Moves while `pred` accepts the character at the cursor. Returns how
    /// many characters were passed.
    #[inline]
    pub fn advance_while(&mut self, pred: impl Fn(char) -> bool) -> usize {
        let mut count = 0;
        while self.advance_if(&pred) {
            count += 1;
        }
        count
    }

    /// Moves while either predicate accepts the character at the cursor, and
    /// counts how many characters each one accepted. `p1` is asked first.
    #[inline]
    pub fn advance_while2(
        &mut self,
        p1: impl Fn(char) -> bool,
        p2: impl Fn(char) -> bool,
    ) -> (usize, usize) {
        let (mut n1, mut n2) = (0, 0);
        while let Some(c) = self.peek() {
            if p1(c) {
                n1 += 1;
            } else if p2(c) {
                n2 += 1;
            } else {
                break;
            }
            self.pos += c.len_utf8();
        }
        (n1, n2)
    }

    /// [`advance_while2`](Cursor::advance_while2) with three predicates.
    #[inline]
    pub fn advance_while3(
        &mut self,
        p1: impl Fn(char) -> bool,
        p2: impl Fn(char) -> bool,
        p3: impl Fn(char) -> bool,
    ) -> (usize, usize, usize) {
        let (mut n1, mut n2, mut n3) = (0, 0, 0);
        while let Some(c) = self.peek() {
            if p1(c) {
                n1 += 1;
            } else if p2(c) {
                n2 += 1;
            } else if p3(c) {
                n3 += 1;
            } else {
                break;
            }
            self.pos += c.len_utf8();
        }
        (n1, n2, n3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_multibyte_text_on_character_boundaries() {
        let mut cur = Cursor::new("aé→𝄞");
        assert_eq!(cur.prev(), None);
        assert_eq!(cur.advance(), Some('a'));
        assert_eq!(cur.advance(), Some('é'));
        assert_eq!(cur.pos(), 3);
        assert_eq!(cur.peek(), Some('→'));
        assert_eq!(cur.prev(), Some('é'));
        assert_eq!(cur.advance(), Some('→'));
        assert_eq!(cur.advance(), Some('𝄞'));
        assert!(cur.is_end());
        assert_eq!(cur.peek(), None);
        assert_eq!(cur.advance(), None);
        assert_eq!(cur.back(), Some('𝄞'));
        assert_eq!(cur.rest(), "𝄞");
    }

    #[test]
    fn advance_while_counts_characters_not_bytes() {
        let mut cur = Cursor::new("ééé1");
        assert_eq!(cur.advance_while(|c| c == 'é'), 3);
        assert_eq!(cur.pos(), 6);
    }

    #[test]
    fn advance_while2_counts_each_predicate() {
        let mut cur = Cursor::new("a1b2-c");
        let counts = cur.advance_while2(|c| c.is_ascii_digit(), |c| c.is_ascii_alphabetic());
        assert_eq!(counts, (2, 2));
        assert_eq!(cur.rest(), "-c");
    }

    #[test]
    fn advance_str_moves_only_on_a_full_match() {
        let mut cur = Cursor::new("https://x");
        assert!(!cur.advance_str("http://"));
        assert_eq!(cur.pos(), 0);
        assert!(cur.advance_str("https://"));
        assert_eq!(cur.rest(), "x");
    }

    #[test]
    #[should_panic(expected = "not a character boundary")]
    fn set_pos_rejects_the_middle_of_a_character() {
        Cursor::new("é").set_pos(1);
    }
}
