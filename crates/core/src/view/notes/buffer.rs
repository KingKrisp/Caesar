use std::ops::Range;
use crate::geom::LinearDir;
use crate::view::{KeyboardEvent, TextKind};

#[derive(Clone, Default)]
pub(super) struct Buffer {
    pub text: String,
    pub cursor: usize,
    pub partial: String,
}

impl Buffer {
    pub fn new(text: String) -> Self {
        Self { cursor: text.len(), text, partial: String::new() }
    }

    fn neighbor(&self, target: TextKind, dir: LinearDir) -> usize {
        let backward = dir == LinearDir::Backward;
        match target {
            TextKind::Extremum => if backward { 0 } else { self.text.len() },
            TextKind::Char => if backward {
                self.text[..self.cursor].char_indices().next_back().map_or(0, |(i, _)| i)
            } else {
                self.text[self.cursor..].chars().next().map_or(self.cursor, |c| self.cursor + c.len_utf8())
            },
            TextKind::Word => {
                let mut position = self.cursor;
                let chars: Vec<_> = if backward {
                    self.text[..self.cursor].char_indices().rev().collect()
                } else {
                    self.text[self.cursor..].char_indices().map(|(i, c)| (self.cursor + i, c)).collect()
                };
                let mut in_word = false;
                for (i, c) in chars {
                    if in_word && c.is_whitespace() { break; }
                    in_word |= !c.is_whitespace();
                    position = if backward { i } else { i + c.len_utf8() };
                }
                position
            },
        }
    }

    pub fn insert(&mut self, text: &str) {
        self.text.insert_str(self.cursor, text);
        self.cursor += text.len();
        self.partial.clear();
    }

    pub fn keyboard(&mut self, event: KeyboardEvent) {
        match event {
            KeyboardEvent::Append(c) => self.insert(&c.to_string()),
            KeyboardEvent::Submit => self.insert("\n"),
            KeyboardEvent::Partial(c) => self.partial.push(c),
            KeyboardEvent::Move { target, dir } => {
                self.cursor = self.neighbor(target, dir);
                self.partial.clear();
            },
            KeyboardEvent::Delete { target, dir } => {
                let next = self.neighbor(target, dir);
                self.text.replace_range(self.cursor.min(next)..self.cursor.max(next), "");
                self.cursor = self.cursor.min(next);
                self.partial.clear();
            },
        }
    }

    pub fn emphasis(&mut self, marker: &str) {
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        let start = self.text[..self.cursor].char_indices().rev()
            .take_while(|(_, c)| is_word(*c)).last().map_or(self.cursor, |(i, _)| i);
        let end = self.text[self.cursor..].char_indices().take_while(|(_, c)| is_word(*c))
            .last().map_or(self.cursor, |(i, c)| self.cursor + i + c.len_utf8());
        if start == end {
            self.insert(&marker.repeat(2));
            self.cursor -= marker.len();
        } else if self.text[..start].ends_with(marker) && self.text[end..].starts_with(marker) &&
                  (marker.len() == 2 || (self.text[..start].chars().rev().take_while(|c| *c == '*').count() % 2 == 1 &&
                                        self.text[end..].chars().take_while(|c| *c == '*').count() % 2 == 1)) {
            self.text.replace_range(end..end + marker.len(), "");
            self.text.replace_range(start - marker.len()..start, "");
            self.cursor -= marker.len();
        } else {
            self.text.insert_str(end, marker);
            self.text.insert_str(start, marker);
            self.cursor += marker.len();
        }
        self.partial.clear();
    }

    pub fn heading(&mut self) {
        let start = self.text[..self.cursor].rfind('\n').map_or(0, |i| i + 1);
        let rest = &self.text[start..];
        let hashes = rest.bytes().take_while(|b| *b == b'#').count();
        let old = if (1..=6).contains(&hashes) && rest.as_bytes().get(hashes) == Some(&b' ') {
            hashes + 1
        } else { 0 };
        let prefix = if old == 0 { "# " } else if hashes == 1 { "## " }
                     else if hashes == 2 { "### " } else { "" };
        self.text.replace_range(start..start + old, prefix);
        self.cursor = start + (self.cursor - start).saturating_sub(old) + prefix.len();
        self.partial.clear();
    }
}

// Ranges retain every original byte, including spaces. Newlines are represented
// by the boundary between logical lines, and long words wrap without truncation.
pub(super) fn wrap_lines(text: &str, width: i32, mut measure: impl FnMut(&str) -> i32) -> Vec<Range<usize>> {
    let mut lines = Vec::new();
    let mut base = 0;
    for paragraph in text.split('\n') {
        if paragraph.is_empty() { lines.push(base..base); }
        let mut start = 0;
        while start < paragraph.len() {
            let mut end = start;
            let mut space = None;
            for (i, c) in paragraph[start..].char_indices() {
                let next = start + i + c.len_utf8();
                if end > start && measure(&paragraph[start..next]) > width { break; }
                end = next;
                if c.is_whitespace() { space = Some(end); }
            }
            if end < paragraph.len() { end = space.unwrap_or(end); }
            lines.push(base + start..base + end);
            start = end;
        }
        base += paragraph.len() + 1;
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_editing_and_newlines_preserve_character_boundaries() {
        let mut b = Buffer::new("Café 日本語".to_string());
        b.keyboard(KeyboardEvent::Move { target: TextKind::Word, dir: LinearDir::Backward });
        assert_eq!(&b.text[b.cursor..], "日本語");
        b.keyboard(KeyboardEvent::Delete { target: TextKind::Char, dir: LinearDir::Forward });
        assert_eq!(b.text, "Café 本語");
        b.keyboard(KeyboardEvent::Move { target: TextKind::Extremum, dir: LinearDir::Forward });
        b.keyboard(KeyboardEvent::Submit);
        b.keyboard(KeyboardEvent::Append('é'));
        b.keyboard(KeyboardEvent::Delete { target: TextKind::Char, dir: LinearDir::Backward });
        assert_eq!(b.text, "Café 本語\n");
        assert!(b.text.is_char_boundary(b.cursor));
    }

    #[test]
    fn formatting_wraps_words_inserts_pairs_and_cycles_headings() {
        let mut b = Buffer::new("Café".to_string());
        b.emphasis("**");
        assert_eq!(b.text, "**Café**");
        b.emphasis("*");
        assert_eq!(b.text, "***Café***");
        b.emphasis("*");
        assert_eq!(b.text, "**Café**");
        b.emphasis("**");
        assert_eq!(b.text, "Café");
        b.emphasis("*");
        assert_eq!(b.text, "*Café*");
        let mut b = Buffer::default();
        b.emphasis("**"); b.insert("bold");
        assert_eq!(b.text, "**bold**");
        for expected in ["# **bold**", "## **bold**", "### **bold**", "**bold**"] {
            b.heading(); assert_eq!(b.text, expected); assert!(b.text.is_char_boundary(b.cursor));
        }
    }

    #[test]
    fn wrapping_keeps_spaces_empty_lines_unicode_and_long_words() {
        let text = "ab cd\n\n日本語éé\n";
        let lines = wrap_lines(text, 3, |s| s.chars().count() as i32);
        assert_eq!(lines.iter().map(|r| &text[r.clone()]).collect::<Vec<_>>(),
                   ["ab ", "cd", "", "日本語", "éé", ""]);
        for range in lines { assert!(text.is_char_boundary(range.start) && text.is_char_boundary(range.end)); }
    }
}
