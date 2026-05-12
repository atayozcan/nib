//! Input layer: read raw bytes from stdin, feed them to `vte`, hand out [`Key`] events.
//!
//! The parser handles everything `vte` does — single chars, escape-prefixed sequences,
//! CSI dispatches (arrows, F-keys, modifiers), bracketed-paste markers. The Perform
//! impl is the only place that knows about escape bytes; the rest of the editor sees
//! only typed [`Key`] values.

use std::io;

use anyhow::Result;
use rustix::stdio;
use vte::{Params, Parser, Perform};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Key {
    Char(char),
    Enter,
    Tab,
    BackTab,
    Backspace,
    Delete,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Function(u8),
    PasteStart,
    PasteEnd,
}

bitflags::bitflags! {
    /// Bit set of held modifier keys.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
    pub(crate) struct KeyMod: u8 {
        const SHIFT = 1 << 0;
        const ALT   = 1 << 1;
        const CTRL  = 1 << 2;
    }
}

/// Decode the `1;5` style of CSI modifier param (xterm convention).
fn decode_csi_modifier(raw: u32) -> KeyMod {
    let bits = raw.saturating_sub(1) as u8;
    let mut m = KeyMod::empty();
    if bits & 0b001 != 0 {
        m |= KeyMod::SHIFT;
    }
    if bits & 0b010 != 0 {
        m |= KeyMod::ALT;
    }
    if bits & 0b100 != 0 {
        m |= KeyMod::CTRL;
    }
    m
}

/// Translate a raw control byte (C0) into a [`Key`] + modifier set.
fn from_c0(byte: u8) -> (Key, KeyMod) {
    match byte {
        b'\r' | b'\n' => (Key::Enter, KeyMod::empty()),
        b'\t' => (Key::Tab, KeyMod::empty()),
        0x7f | 0x08 => (Key::Backspace, KeyMod::empty()),
        0x1b => (Key::Escape, KeyMod::empty()),
        // Ctrl+letter: 0x01..=0x1A → 'a'..='z'.
        0x01..=0x1a => (Key::Char((byte + 0x60) as char), KeyMod::CTRL),
        other => (Key::Char(other as char), KeyMod::empty()),
    }
}

#[derive(Debug, Default)]
struct EventSink {
    events: Vec<(Key, KeyMod)>,
}

impl Perform for EventSink {
    fn print(&mut self, c: char) {
        self.events.push((Key::Char(c), KeyMod::empty()));
    }

    fn execute(&mut self, byte: u8) {
        self.events.push(from_c0(byte));
    }

    fn csi_dispatch(
        &mut self,
        params: &Params,
        _intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        let mut iter = params.iter();
        let first = iter.next().and_then(|p| p.first().copied());
        let second = iter.next().and_then(|p| p.first().copied());
        let modifier = decode_csi_modifier(u32::from(second.unwrap_or(1)));

        let key = match action {
            'A' => Some(Key::Up),
            'B' => Some(Key::Down),
            'C' => Some(Key::Right),
            'D' => Some(Key::Left),
            'H' => Some(Key::Home),
            'F' => Some(Key::End),
            'Z' => Some(Key::BackTab),
            '~' => match first {
                Some(1 | 7) => Some(Key::Home),
                Some(2) => Some(Key::Insert),
                Some(3) => Some(Key::Delete),
                Some(4 | 8) => Some(Key::End),
                Some(5) => Some(Key::PageUp),
                Some(6) => Some(Key::PageDown),
                Some(11) => Some(Key::Function(1)),
                Some(12) => Some(Key::Function(2)),
                Some(13) => Some(Key::Function(3)),
                Some(14) => Some(Key::Function(4)),
                Some(15) => Some(Key::Function(5)),
                Some(n @ 17..=21) => Some(Key::Function(n as u8 - 11)),
                Some(n @ 23..=24) => Some(Key::Function(n as u8 - 12)),
                Some(200) => Some(Key::PasteStart),
                Some(201) => Some(Key::PasteEnd),
                _ => None,
            },
            _ => None,
        };

        if let Some(k) = key {
            self.events.push((k, modifier));
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8) {
        // SS3 sequences (\x1bO[ABCD], \x1bO[PQRS]) come through as esc_dispatch
        // when the parser sees a single intermediate-less byte after ESC O.
        let key = match byte {
            b'A' => Some(Key::Up),
            b'B' => Some(Key::Down),
            b'C' => Some(Key::Right),
            b'D' => Some(Key::Left),
            b'H' => Some(Key::Home),
            b'F' => Some(Key::End),
            b'P' => Some(Key::Function(1)),
            b'Q' => Some(Key::Function(2)),
            b'R' => Some(Key::Function(3)),
            b'S' => Some(Key::Function(4)),
            _ => None,
        };
        if let Some(k) = key {
            self.events.push((k, KeyMod::empty()));
        }
    }
}

/// Owns a `vte::Parser` + a small read buffer; turns raw stdin into [`Key`] events.
pub(crate) struct KeyReader {
    parser: Parser,
    sink: EventSink,
    buf: [u8; 256],
}

impl std::fmt::Debug for KeyReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyReader").finish_non_exhaustive()
    }
}

impl KeyReader {
    pub(crate) fn new() -> Self {
        Self {
            parser: Parser::new(),
            sink: EventSink::default(),
            buf: [0u8; 256],
        }
    }

    /// Drain any pending events. Calls `read(2)` once (VMIN=0/VTIME=1 in raw mode means
    /// this returns within ~100ms even if no input is pending), then feeds whatever
    /// bytes came back through the parser.
    pub(crate) fn poll(&mut self) -> Result<Vec<(Key, KeyMod)>> {
        let n = match rustix::io::read(stdio::stdin(), &mut self.buf) {
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => 0,
            Err(e) => return Err(e.into()),
        };
        for &b in &self.buf[..n] {
            self.parser.advance(&mut self.sink, &[b]);
        }
        Ok(std::mem::take(&mut self.sink.events))
    }
}

impl Default for KeyReader {
    fn default() -> Self {
        Self::new()
    }
}
