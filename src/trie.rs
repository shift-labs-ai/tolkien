//! A prefix tree of words, for matchers that look for one of many strings.

use crate::cursor::Cursor;

#[derive(Debug, Default)]
struct Node {
    /// Sorted by character.
    children: Vec<(char, u32)>,
    /// True when the path from the root to this node spells an inserted word.
    word: bool,
}

/// A set of words that can be matched at a cursor, longest word first.
#[derive(Debug)]
pub struct Trie {
    nodes: Vec<Node>,
}

impl Default for Trie {
    fn default() -> Self {
        Self {
            nodes: vec![Node::default()],
        }
    }
}

impl Trie {
    /// An empty trie.
    pub fn new() -> Self {
        Self::default()
    }

    /// A trie holding `words`.
    pub fn from_words<I>(words: I) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let mut trie = Self::new();
        for word in words {
            trie.insert(word.as_ref());
        }
        trie
    }

    /// A trie holding each of `words` in four spellings: as given, in lower
    /// case, in upper case, and with only its first letter raised to upper
    /// case. `Rs` gives `Rs`, `rs` and `RS`; `kr` gives `kr`, `KR` and `Kr`.
    pub fn from_words_in_common_cases<I>(words: I) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let mut trie = Self::new();
        for word in words {
            let word = word.as_ref();
            trie.insert(word);
            trie.insert(&word.to_lowercase());
            trie.insert(&word.to_uppercase());
            let mut rest = word.chars();
            if let Some(first) = rest.next() {
                trie.insert(&first.to_uppercase().chain(rest).collect::<String>());
            }
        }
        trie
    }

    /// Adds `word`. The empty string is ignored.
    pub fn insert(&mut self, word: &str) {
        if word.is_empty() {
            return;
        }
        let mut node = 0;
        for c in word.chars() {
            node = match self.nodes[node]
                .children
                .binary_search_by_key(&c, |&(ch, _)| ch)
            {
                Ok(i) => self.nodes[node].children[i].1 as usize,
                Err(i) => {
                    let next = self.nodes.len();
                    self.nodes.push(Node::default());
                    self.nodes[node].children.insert(i, (c, next as u32));
                    next
                }
            };
        }
        self.nodes[node].word = true;
    }

    fn child(&self, node: usize, c: char) -> Option<usize> {
        let children = &self.nodes[node].children;
        children
            .binary_search_by_key(&c, |&(ch, _)| ch)
            .ok()
            .map(|i| children[i].1 as usize)
    }

    /// Moves the cursor past the longest word that starts at it and returns
    /// `true`. With no such word the cursor stays where it is and the result
    /// is `false`.
    pub fn advance_longest(&self, cur: &mut Cursor<'_>) -> bool {
        let mut node = 0;
        let mut end = None;
        let mut ahead = cur.clone();
        while let Some(next) = ahead.peek().and_then(|c| self.child(node, c)) {
            ahead.advance();
            node = next;
            if self.nodes[node].word {
                end = Some(ahead.pos());
            }
        }
        if let Some(end) = end {
            cur.set_pos(end);
        }
        end.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn longest<'t>(trie: &Trie, text: &'t str) -> Option<&'t str> {
        let mut cur = Cursor::new(text);
        trie.advance_longest(&mut cur).then(|| &text[..cur.pos()])
    }

    #[test]
    fn a_prefix_of_a_word_is_not_a_word() {
        let trie = Trie::from_words(["http://", "https://", "www."]);
        assert_eq!(longest(&trie, "http://x"), Some("http://"));
        assert_eq!(longest(&trie, "https://x"), Some("https://"));
        assert_eq!(longest(&trie, "http:/x"), None);
        assert_eq!(longest(&trie, "ww"), None);
        assert_eq!(longest(&trie, ""), None);
    }

    #[test]
    fn takes_the_longest_word_and_steps_back_from_a_dead_end() {
        let trie = Trie::from_words(["Cent", "Cents", "Centimo"]);
        assert_eq!(longest(&trie, "Cents."), Some("Cents"));
        // "Centi" runs into the path to "Centimo" and has to come back.
        assert_eq!(longest(&trie, "Centi"), Some("Cent"));
        assert_eq!(longest(&trie, "Centimo"), Some("Centimo"));
    }

    #[test]
    fn leaves_the_cursor_alone_without_a_match() {
        let trie = Trie::from_words(["Feb"]);
        let mut cur = Cursor::new("Fe 1");
        assert!(!trie.advance_longest(&mut cur));
        assert_eq!(cur.pos(), 0);
    }

    #[test]
    fn matches_words_outside_ascii_and_ignores_the_empty_word() {
        let trie = Trie::from_words(["Öre", ""]);
        assert_eq!(longest(&trie, "Öre!"), Some("Öre"));
        assert_eq!(longest(&trie, "Ore"), None);
        assert_eq!(longest(&trie, "!"), None);
    }

    #[test]
    fn common_cases_are_as_given_lower_upper_and_capitalised() {
        let trie = Trie::from_words_in_common_cases(["maLoti", "öre"]);
        for word in ["maLoti", "maloti", "MALOTI", "MaLoti", "öre", "ÖRE", "Öre"] {
            assert_eq!(longest(&trie, word), Some(word));
        }
        for word in ["Maloti", "mALOTI", "öRE"] {
            assert_eq!(longest(&trie, word), None, "{word}");
        }
    }
}
