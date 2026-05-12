use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use ropey::Rope;
use unicode_segmentation::UnicodeSegmentation;

/// A text buffer backed by a [`Rope`], plus a cursor expressed as (line, column-in-graphemes).
///
/// Column is grapheme-cluster based so a cursor sitting next to a flag emoji or a combining
/// accent advances by what a human reader would call "one character," not by a UTF-8 byte.
#[derive(Debug)]
pub(crate) struct Buffer {
    rope: Rope,
    path: PathBuf,
    dirty: bool,
    cursor: Cursor,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Cursor {
    pub(crate) line: usize,
    pub(crate) col: usize,
}

impl Buffer {
    pub(crate) fn open(path: PathBuf) -> Result<Self> {
        let rope = if path.exists() {
            let text = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            Rope::from_str(&text)
        } else {
            Rope::new()
        };
        Ok(Self {
            rope,
            path,
            dirty: false,
            cursor: Cursor::default(),
        })
    }

    pub(crate) fn save(&mut self) -> Result<()> {
        let tmp = self.path.with_extension("tmp~");
        fs::write(&tmp, self.rope.to_string())
            .with_context(|| format!("failed to write {}", tmp.display()))?;
        fs::rename(&tmp, &self.path).with_context(|| {
            format!(
                "failed to rename {} -> {}",
                tmp.display(),
                self.path.display()
            )
        })?;
        self.dirty = false;
        Ok(())
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub(crate) fn cursor(&self) -> Cursor {
        self.cursor
    }

    pub(crate) fn line_count(&self) -> usize {
        self.rope.len_lines().max(1)
    }

    pub(crate) fn line(&self, line: usize) -> String {
        self.rope
            .get_line(line)
            .map(|s| s.to_string().trim_end_matches('\n').to_string())
            .unwrap_or_default()
    }

    pub(crate) fn insert_char(&mut self, c: char) {
        let idx = self.cursor_char_idx();
        self.rope.insert_char(idx, c);
        self.dirty = true;
        if c == '\n' {
            self.cursor.line += 1;
            self.cursor.col = 0;
        } else {
            self.cursor.col += 1;
        }
    }

    pub(crate) fn insert_newline(&mut self) {
        self.insert_char('\n');
    }

    pub(crate) fn backspace(&mut self) {
        if self.cursor.col == 0 && self.cursor.line == 0 {
            return;
        }
        let end = self.cursor_char_idx();
        let (prev_line, prev_col) = if self.cursor.col > 0 {
            (self.cursor.line, self.cursor.col - 1)
        } else {
            let prev_line = self.cursor.line - 1;
            (prev_line, self.line_grapheme_count(prev_line))
        };
        let start = self.line_col_to_char_idx(prev_line, prev_col);
        self.rope.remove(start..end);
        self.cursor.line = prev_line;
        self.cursor.col = prev_col;
        self.dirty = true;
    }

    pub(crate) fn delete_forward(&mut self) {
        let len = self.rope.len_chars();
        let start = self.cursor_char_idx();
        if start >= len {
            return;
        }
        // Remove one grapheme cluster, not one char — protects ZWJ sequences.
        let line = self.line(self.cursor.line);
        let rest: String = line.graphemes(true).skip(self.cursor.col).take(1).collect();
        let chars_to_remove = if rest.is_empty() {
            1 // crossing a newline
        } else {
            rest.chars().count()
        };
        let end = (start + chars_to_remove).min(len);
        self.rope.remove(start..end);
        self.dirty = true;
    }

    pub(crate) fn move_left(&mut self) {
        if self.cursor.col > 0 {
            self.cursor.col -= 1;
        } else if self.cursor.line > 0 {
            self.cursor.line -= 1;
            self.cursor.col = self.line_grapheme_count(self.cursor.line);
        }
    }

    pub(crate) fn move_right(&mut self) {
        let line_len = self.line_grapheme_count(self.cursor.line);
        if self.cursor.col < line_len {
            self.cursor.col += 1;
        } else if self.cursor.line + 1 < self.line_count() {
            self.cursor.line += 1;
            self.cursor.col = 0;
        }
    }

    pub(crate) fn move_up(&mut self) {
        if self.cursor.line == 0 {
            self.cursor.col = 0;
            return;
        }
        self.cursor.line -= 1;
        self.cursor.col = self
            .cursor
            .col
            .min(self.line_grapheme_count(self.cursor.line));
    }

    pub(crate) fn move_down(&mut self) {
        if self.cursor.line + 1 >= self.line_count() {
            self.cursor.col = self.line_grapheme_count(self.cursor.line);
            return;
        }
        self.cursor.line += 1;
        self.cursor.col = self
            .cursor
            .col
            .min(self.line_grapheme_count(self.cursor.line));
    }

    pub(crate) fn move_home(&mut self) {
        self.cursor.col = 0;
    }

    pub(crate) fn move_end(&mut self) {
        self.cursor.col = self.line_grapheme_count(self.cursor.line);
    }

    fn line_grapheme_count(&self, line: usize) -> usize {
        self.line(line).graphemes(true).count()
    }

    fn cursor_char_idx(&self) -> usize {
        self.line_col_to_char_idx(self.cursor.line, self.cursor.col)
    }

    fn line_col_to_char_idx(&self, line: usize, col: usize) -> usize {
        let line_start = self.rope.line_to_char(line.min(self.rope.len_lines()));
        let line_text = self.line(line);
        let byte_offset: usize = line_text.graphemes(true).take(col).map(str::len).sum();
        let char_offset = line_text[..byte_offset.min(line_text.len())]
            .chars()
            .count();
        line_start + char_offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(text: &str) -> Buffer {
        Buffer {
            rope: Rope::from_str(text),
            path: PathBuf::from("/tmp/test"),
            dirty: false,
            cursor: Cursor::default(),
        }
    }

    #[test]
    fn insert_and_backspace_roundtrip() {
        let mut b = buf("");
        for c in "hello".chars() {
            b.insert_char(c);
        }
        assert_eq!(b.rope.to_string(), "hello");
        b.backspace();
        b.backspace();
        assert_eq!(b.rope.to_string(), "hel");
        assert_eq!(b.cursor.col, 3);
    }

    #[test]
    fn newline_moves_cursor_to_next_line() {
        let mut b = buf("");
        b.insert_char('a');
        b.insert_newline();
        b.insert_char('b');
        assert_eq!(b.rope.to_string(), "a\nb");
        assert_eq!(b.cursor.line, 1);
        assert_eq!(b.cursor.col, 1);
    }

    #[test]
    fn arrow_movement_clamps_to_line_length() {
        let mut b = buf("longline\nshort");
        b.move_end();
        b.move_down();
        assert_eq!(b.cursor.line, 1);
        assert_eq!(b.cursor.col, 5);
    }

    #[test]
    fn backspace_across_line_boundary() {
        let mut b = buf("ab\ncd");
        b.move_down();
        b.backspace();
        assert_eq!(b.rope.to_string(), "abcd");
        assert_eq!(b.cursor.line, 0);
        assert_eq!(b.cursor.col, 2);
    }

    #[test]
    fn grapheme_cursor_treats_flag_emoji_as_one_step() {
        // "🇹🇷" is two scalar values (regional indicators) forming one grapheme.
        let mut b = buf("🇹🇷x");
        b.move_right();
        assert_eq!(b.cursor.col, 1);
        b.move_right();
        assert_eq!(b.cursor.col, 2);
    }
}
