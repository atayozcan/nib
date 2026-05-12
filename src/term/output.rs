//! Cell-diff renderer.
//!
//! We keep two grids of [`RenderCell`]: a `back` grid that the application paints into
//! each frame, and a `front` grid that mirrors what the terminal has actually been told.
//! On `flush`, we diff `back` against `front` and emit only the minimum sequence of
//! cursor-move + SGR-style + UTF-8 writes that bring `front` in line with `back`.
//!
//! This is the same architecture helix / zed use; ratatui does the redraw differently
//! (full-frame diff) but the *idea* is the same.

use std::io::{self, Write};

use anyhow::Result;
use unicode_width::UnicodeWidthChar;

use super::size::Size;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Color {
    #[default]
    Reset,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RenderCell {
    pub(crate) ch: char,
    pub(crate) fg: Color,
    pub(crate) bg: Color,
    pub(crate) bold: bool,
    pub(crate) reverse: bool,
}

impl Default for RenderCell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: Color::Reset,
            bg: Color::Reset,
            bold: false,
            reverse: false,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Renderer {
    size: Size,
    front: Vec<RenderCell>,
    back: Vec<RenderCell>,
    /// Cursor position the application wants displayed after this frame, as
    /// `(x_col, y_row)` to match how callers naturally think of it.
    cursor: (u16, u16),
}

impl Renderer {
    pub(crate) fn new(size: Size) -> Self {
        let cells = (size.cols as usize) * (size.rows as usize);
        Self {
            size,
            front: vec![RenderCell::default(); cells],
            back: vec![RenderCell::default(); cells],
            cursor: (0, 0),
        }
    }

    pub(crate) fn size(&self) -> Size {
        self.size
    }

    pub(crate) fn resize(&mut self, size: Size) {
        if size == self.size {
            return;
        }
        let cells = (size.cols as usize) * (size.rows as usize);
        self.size = size;
        self.front = vec![RenderCell::default(); cells];
        self.back = vec![RenderCell::default(); cells];
        // Force a full repaint by writing CLS — the diff loop will then re-fill.
        let _ = io::stdout().write_all(b"\x1b[2J");
    }

    pub(crate) fn clear_back(&mut self) {
        for c in &mut self.back {
            *c = RenderCell::default();
        }
    }

    pub(crate) fn put(&mut self, x: u16, y: u16, cell: RenderCell) {
        if x >= self.size.cols || y >= self.size.rows {
            return;
        }
        let idx = (y as usize) * (self.size.cols as usize) + (x as usize);
        self.back[idx] = cell;
    }

    pub(crate) fn put_str(
        &mut self,
        x: u16,
        y: u16,
        s: &str,
        fg: Color,
        bg: Color,
        bold: bool,
    ) -> u16 {
        let mut col = x;
        for ch in s.chars() {
            let w = ch.width().unwrap_or(0) as u16;
            if w == 0 {
                continue;
            }
            if col >= self.size.cols {
                break;
            }
            self.put(
                col,
                y,
                RenderCell {
                    ch,
                    fg,
                    bg,
                    bold,
                    reverse: false,
                },
            );
            col = col.saturating_add(w);
        }
        col
    }

    pub(crate) fn set_cursor(&mut self, x: u16, y: u16) {
        self.cursor = (x, y);
    }

    /// Emit the diff between `back` and `front` to stdout, then swap.
    pub(crate) fn flush(&mut self) -> Result<()> {
        let mut out = Vec::<u8>::with_capacity(2048);
        out.extend_from_slice(b"\x1b[?25l"); // hide while we paint

        let mut active = Style::default();
        let mut last_pos: Option<(u16, u16)> = None;

        for y in 0..self.size.rows {
            for x in 0..self.size.cols {
                let idx = (y as usize) * (self.size.cols as usize) + (x as usize);
                let back = self.back[idx];
                if back == self.front[idx] {
                    continue;
                }

                if last_pos != Some((y, x.wrapping_sub(1))) {
                    write_move(&mut out, y, x);
                }

                let next = Style::from(back);
                if next != active {
                    write_sgr(&mut out, next);
                    active = next;
                }

                let mut tmp = [0u8; 4];
                out.extend_from_slice(back.ch.encode_utf8(&mut tmp).as_bytes());

                self.front[idx] = back;
                last_pos = Some((y, x));
            }
        }

        // Reset SGR + position cursor + show.
        out.extend_from_slice(b"\x1b[0m");
        let (cx, cy) = self.cursor;
        if cx < self.size.cols && cy < self.size.rows {
            write_move(&mut out, cy, cx);
            out.extend_from_slice(b"\x1b[?25h");
        }

        let mut stdout = io::stdout().lock();
        stdout.write_all(&out)?;
        stdout.flush()?;
        Ok(())
    }
}

fn write_move(out: &mut Vec<u8>, row: u16, col: u16) {
    // ANSI is 1-based.
    let _ = write!(out, "\x1b[{};{}H", row + 1, col + 1);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Style {
    fg: Color,
    bg: Color,
    bold: bool,
    reverse: bool,
}

impl From<RenderCell> for Style {
    fn from(c: RenderCell) -> Self {
        Self {
            fg: c.fg,
            bg: c.bg,
            bold: c.bold,
            reverse: c.reverse,
        }
    }
}

fn write_sgr(out: &mut Vec<u8>, s: Style) {
    out.extend_from_slice(b"\x1b[0");
    if s.bold {
        out.extend_from_slice(b";1");
    }
    if s.reverse {
        out.extend_from_slice(b";7");
    }
    match s.fg {
        Color::Reset => {}
        Color::Indexed(n) => {
            let _ = write!(out, ";38;5;{n}");
        }
        Color::Rgb(r, g, b) => {
            let _ = write!(out, ";38;2;{r};{g};{b}");
        }
    }
    match s.bg {
        Color::Reset => {}
        Color::Indexed(n) => {
            let _ = write!(out, ";48;5;{n}");
        }
        Color::Rgb(r, g, b) => {
            let _ = write!(out, ";48;2;{r};{g};{b}");
        }
    }
    out.push(b'm');
}
