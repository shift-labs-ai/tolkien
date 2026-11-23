//! A position in a text that matchers move.

/// A cursor over a `&str`. Positions are byte offsets and always sit on a
/// character boundary.
///
/// A matcher reads with [`peek`](Cursor::peek) and moves with the `advance*`
/// methods. It may leave the cursor anywhere when it reports no match: the
/// lexer gives every matcher its own cursor.
///
/// A cursor does not show what stands before the place it was created at:
/// [`prev`](Cursor::prev) and the `back*` methods stop there. A token so
/// depends only on the text from its start on, which is what lets a text be
/// lexed from any token boundary, and in pieces.
#[derive(Debug, Clone)]
pub struct Cursor<'t> {
    text: &'t str,
    pos: usize,
    /// Where the cursor was created. Nothing before it is shown.
    start: usize,
}

impl<'t> Cursor<'t> {
    /// A cursor at the start of `text`.
    pub fn new(text: &'t str) -> Self {
        Self::at(text, 0)
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
        Self {
            text,
            pos,
            start: pos,
        }
    }

    /// The current byte offset.
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Moves to byte offset `pos`, usually one saved earlier from [`pos`](Cursor::pos).
    ///
    /// # Panics
    /// If `pos` is not on a character boundary of the text, or is before
    /// the place the cursor was created at.
    pub fn set_pos(&mut self, pos: usize) {
        assert!(
            pos >= self.start && self.text.is_char_boundary(pos),
            "position {pos} is not a character boundary at or after {}",
            self.start
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

    /// The text from byte offset `start`, usually one saved earlier from
    /// [`pos`](Cursor::pos), up to the cursor.
    ///
    /// # Panics
    /// If `start` is after the cursor, before the place the cursor was
    /// created at, or not on a character boundary.
    pub fn since(&self, start: usize) -> &'t str {
        assert!(
            start >= self.start,
            "position {start} is before {}, where the cursor was created",
            self.start
        );
        &self.text[start..self.pos]
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

    /// True when `first` accepts the character at the cursor and `second`
    /// the one after it. The cursor does not move: this is how a matcher
    /// asks whether its token is only the start of something longer.
    pub fn followed_by(
        &self,
        first: impl FnOnce(char) -> bool,
        second: impl FnOnce(char) -> bool,
    ) -> bool {
        let mut ahead = self.clone();
        ahead.advance_if(first) && ahead.peek().is_some_and(second)
    }

    /// The character just before the cursor, or `None` at the place the
    /// cursor was created at.
    pub fn prev(&self) -> Option<char> {
        self.text[self.start..self.pos].chars().next_back()
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

    /// Moves back while `pred` accepts the character before the cursor.
    /// Returns how many characters were passed.
    pub fn back_while(&mut self, pred: impl Fn(char) -> bool) -> usize {
        let mut count = 0;
        while let Some(c) = self.prev().filter(|&c| pred(c)) {
            self.pos -= c.len_utf8();
            count += 1;
        }
        count
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
        assert_eq!(cur.since(1), "é→");
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
    fn back_while_stops_at_the_first_rejected_character_and_at_the_start() {
        let mut cur = Cursor::new("aé..");
        cur.advance_while(|_| true);
        assert_eq!(cur.back_while(|c| c == '.'), 2);
        assert_eq!(cur.pos(), 3);
        assert_eq!(cur.back_while(|c| c == '.'), 0);
        assert_eq!(cur.back_while(|_| true), 2);
        assert_eq!(cur.pos(), 0);
    }

    #[test]
    fn followed_by_looks_at_two_characters_without_moving() {
        let cur = Cursor::at("a:é", 1);
        assert!(cur.followed_by(|c| c == ':', |c| c == 'é'));
        assert!(!cur.followed_by(|c| c == ':', |c| c == 'e'));
        assert!(!cur.followed_by(|c| c == ';', |_| true));
        assert!(!Cursor::at("a:", 1).followed_by(|c| c == ':', |_| true));
        assert_eq!(cur.pos(), 1);
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

    #[test]
    fn shows_nothing_before_the_place_it_was_created_at() {
        let mut cur = Cursor::at("ab.c", 2);
        assert_eq!(cur.prev(), None);
        assert_eq!(cur.back(), None);
        assert_eq!(cur.back_while(|_| true), 0);
        cur.advance();
        assert_eq!(cur.prev(), Some('.'));
        assert_eq!(cur.back_while(|_| true), 1);
        assert_eq!(cur.pos(), 2);
    }

    #[test]
    #[should_panic(expected = "position 1 is before 2")]
    fn since_rejects_a_position_before_the_start() {
        Cursor::at("ab.c", 2).since(1);
    }

    #[test]
    #[should_panic(expected = "not a character boundary at or after 2")]
    fn set_pos_rejects_a_position_before_the_start() {
        Cursor::at("ab.c", 2).set_pos(1);
    }

    #[test]
    #[should_panic(expected = "not a character boundary")]
    fn at_rejects_a_position_past_the_end() {
        Cursor::at("é", 3);
    }
}
