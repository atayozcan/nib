use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use ropey::Rope;
use unicode_segmentation::UnicodeSegmentation;

/// A single primitive edit operation, expressed in `char` indices (matches `ropey`).
///
/// `Insert { at, text }` and `Delete { at, text }` are inverses of each other when
/// the same `at` is used — that's the property [`Edit::inverse`] relies on for undo.
#[derive(Debug, Clone)]
pub(crate) enum Edit {
    Insert { at: usize, text: String },
    Delete { at: usize, text: String },
}

impl Edit {
    pub(crate) fn inverse(&self) -> Self {
        match self {
            Self::Insert { at, text } => Self::Delete {
                at: *at,
                text: text.clone(),
            },
            Self::Delete { at, text } => Self::Insert {
                at: *at,
                text: text.clone(),
            },
        }
    }

    fn apply(&self, rope: &mut Rope) {
        match self {
            Self::Insert { at, text } => rope.insert(*at, text),
            Self::Delete { at, text } => {
                let len = text.chars().count();
                rope.remove(*at..*at + len);
            }
        }
    }
}

/// A batch of edits applied as one undo step.
///
/// Right now we only ever push a single edit per transaction; the type exists so the
/// history layer can grow into coalesced typing-runs / multi-cursor edits without
/// reworking the buffer API.
#[derive(Debug, Clone, Default)]
pub(crate) struct Transaction {
    edits: Vec<Edit>,
}

impl Transaction {
    pub(crate) fn single(edit: Edit) -> Self {
        Self { edits: vec![edit] }
    }

    pub(crate) fn inverse(&self) -> Self {
        Self {
            edits: self.edits.iter().rev().map(Edit::inverse).collect(),
        }
    }

    fn apply(&self, rope: &mut Rope) {
        for edit in &self.edits {
            edit.apply(rope);
        }
    }
}

/// Cursor position expressed in grapheme columns, not bytes — so emoji and combining
/// accents advance the cursor by what a human reads as one character.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Cursor {
    pub(crate) line: usize,
    pub(crate) col: usize,
}

#[derive(Debug)]
pub(crate) struct Buffer {
    rope: Rope,
    path: PathBuf,
    dirty: bool,
    pub(crate) cursor: Cursor,
    undo_stack: Vec<Transaction>,
    redo_stack: Vec<Transaction>,
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
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        })
    }

    pub(crate) fn save(&mut self) -> Result<()> {
        let tmp = self.path.with_extension("nib~");
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

    pub(crate) fn line_count(&self) -> usize {
        self.rope.len_lines().max(1)
    }

    pub(crate) fn line(&self, idx: usize) -> String {
        self.rope.get_line(idx).map_or_else(String::new, |s| {
            s.to_string().trim_end_matches('\n').to_string()
        })
    }

    pub(crate) fn line_grapheme_count(&self, idx: usize) -> usize {
        self.line(idx).graphemes(true).count()
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

    /// Apply a transaction and push its inverse on the undo stack.
    fn apply_transaction(&mut self, tx: &Transaction) {
        let inverse = tx.inverse();
        tx.apply(&mut self.rope);
        self.undo_stack.push(inverse);
        self.redo_stack.clear();
        self.dirty = true;
    }

    pub(crate) fn insert_char(&mut self, c: char) {
        let at = self.cursor_char_idx();
        let mut s = String::new();
        s.push(c);
        self.apply_transaction(&Transaction::single(Edit::Insert { at, text: s }));
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
        let removed: String = self.rope.slice(start..end).to_string();
        self.apply_transaction(&Transaction::single(Edit::Delete {
            at: start,
            text: removed,
        }));
        self.cursor.line = prev_line;
        self.cursor.col = prev_col;
    }

    pub(crate) fn delete_char_forward(&mut self) {
        let len = self.rope.len_chars();
        let start = self.cursor_char_idx();
        if start >= len {
            return;
        }
        let line = self.line(self.cursor.line);
        let rest: String = line.graphemes(true).skip(self.cursor.col).take(1).collect();
        let chars_to_remove = if rest.is_empty() {
            1
        } else {
            rest.chars().count()
        };
        let end = (start + chars_to_remove).min(len);
        let removed: String = self.rope.slice(start..end).to_string();
        self.apply_transaction(&Transaction::single(Edit::Delete {
            at: start,
            text: removed,
        }));
    }

    pub(crate) fn delete_line(&mut self) {
        let line_idx = self.cursor.line;
        let start = self.rope.line_to_char(line_idx);
        let end = self
            .rope
            .line_to_char((line_idx + 1).min(self.rope.len_lines()));
        if start == end {
            return;
        }
        let removed: String = self.rope.slice(start..end).to_string();
        self.apply_transaction(&Transaction::single(Edit::Delete {
            at: start,
            text: removed,
        }));
        self.cursor.line = self.cursor.line.min(self.line_count().saturating_sub(1));
        self.cursor.col = 0;
    }

    pub(crate) fn undo(&mut self) -> bool {
        let Some(inverse) = self.undo_stack.pop() else {
            return false;
        };
        let redo = inverse.inverse();
        inverse.apply(&mut self.rope);
        self.redo_stack.push(redo);
        self.dirty = true;
        self.clamp_cursor();
        true
    }

    pub(crate) fn redo(&mut self) -> bool {
        let Some(tx) = self.redo_stack.pop() else {
            return false;
        };
        let inverse = tx.inverse();
        tx.apply(&mut self.rope);
        self.undo_stack.push(inverse);
        self.dirty = true;
        self.clamp_cursor();
        true
    }

    fn clamp_cursor(&mut self) {
        let last_line = self.line_count().saturating_sub(1);
        self.cursor.line = self.cursor.line.min(last_line);
        self.cursor.col = self
            .cursor
            .col
            .min(self.line_grapheme_count(self.cursor.line));
    }

    // --- Motions (used by named commands; pure cursor mutation). ---

    pub(crate) fn move_left(&mut self) {
        if self.cursor.col > 0 {
            self.cursor.col -= 1;
        }
    }

    pub(crate) fn move_right(&mut self) {
        let line_len = self.line_grapheme_count(self.cursor.line);
        if self.cursor.col < line_len {
            self.cursor.col += 1;
        }
    }

    pub(crate) fn move_up(&mut self) {
        if self.cursor.line > 0 {
            self.cursor.line -= 1;
            self.cursor.col = self
                .cursor
                .col
                .min(self.line_grapheme_count(self.cursor.line));
        }
    }

    pub(crate) fn move_down(&mut self) {
        if self.cursor.line + 1 < self.line_count() {
            self.cursor.line += 1;
            self.cursor.col = self
                .cursor
                .col
                .min(self.line_grapheme_count(self.cursor.line));
        }
    }

    pub(crate) fn move_line_start(&mut self) {
        self.cursor.col = 0;
    }

    pub(crate) fn move_line_end(&mut self) {
        self.cursor.col = self.line_grapheme_count(self.cursor.line);
    }

    pub(crate) fn move_word_forward(&mut self) {
        let line = self.line(self.cursor.line);
        for w in line.unicode_words() {
            let pos = line.find(w).unwrap_or(0);
            let end = pos + w.chars().count();
            if end > self.cursor.col {
                self.cursor.col = end.min(self.line_grapheme_count(self.cursor.line));
                return;
            }
        }
        // No word right of cursor on this line — drop to next line's first word.
        if self.cursor.line + 1 < self.line_count() {
            self.cursor.line += 1;
            self.cursor.col = 0;
        } else {
            self.cursor.col = self.line_grapheme_count(self.cursor.line);
        }
    }

    pub(crate) fn move_word_back(&mut self) {
        if self.cursor.col == 0 {
            if self.cursor.line > 0 {
                self.cursor.line -= 1;
                self.cursor.col = self.line_grapheme_count(self.cursor.line);
            }
            return;
        }
        let line = self.line(self.cursor.line);
        let mut last_start = 0usize;
        for w in line.unicode_words() {
            let pos = line.find(w).unwrap_or(0);
            if pos >= self.cursor.col {
                break;
            }
            last_start = pos;
        }
        self.cursor.col = last_start;
    }

    pub(crate) fn move_buffer_start(&mut self) {
        self.cursor = Cursor::default();
    }

    pub(crate) fn move_buffer_end(&mut self) {
        self.cursor.line = self.line_count().saturating_sub(1);
        self.cursor.col = self.line_grapheme_count(self.cursor.line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(text: &str) -> Buffer {
        Buffer {
            rope: Rope::from_str(text),
            path: PathBuf::from("/tmp/nib-test"),
            dirty: false,
            cursor: Cursor::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    #[test]
    fn insert_pushes_undo() {
        let mut b = buf("");
        b.insert_char('a');
        b.insert_char('b');
        assert_eq!(b.rope.to_string(), "ab");
        assert!(b.undo());
        assert_eq!(b.rope.to_string(), "a");
        assert!(b.undo());
        assert_eq!(b.rope.to_string(), "");
        assert!(!b.undo());
    }

    #[test]
    fn redo_after_undo_restores_state() {
        let mut b = buf("");
        b.insert_char('x');
        b.undo();
        assert_eq!(b.rope.to_string(), "");
        assert!(b.redo());
        assert_eq!(b.rope.to_string(), "x");
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut b = buf("");
        b.insert_char('a');
        b.undo();
        b.insert_char('b');
        assert!(!b.redo());
        assert_eq!(b.rope.to_string(), "b");
    }

    #[test]
    fn backspace_across_line_boundary_undoes_cleanly() {
        let mut b = buf("ab\ncd");
        b.move_down();
        b.backspace();
        assert_eq!(b.rope.to_string(), "abcd");
        b.undo();
        assert_eq!(b.rope.to_string(), "ab\ncd");
    }

    #[test]
    fn delete_line_removes_whole_line() {
        let mut b = buf("one\ntwo\nthree");
        b.move_down();
        b.delete_line();
        assert_eq!(b.rope.to_string(), "one\nthree");
    }

    #[test]
    fn motion_within_line() {
        let mut b = buf("hello world");
        b.move_word_forward();
        assert_eq!(b.cursor.col, 5);
        b.move_word_forward();
        assert_eq!(b.cursor.col, 11);
    }

    #[test]
    fn grapheme_cursor_handles_flag_emoji() {
        let mut b = buf("🇹🇷x");
        b.move_right();
        assert_eq!(b.cursor.col, 1);
        b.move_right();
        assert_eq!(b.cursor.col, 2);
    }
}
