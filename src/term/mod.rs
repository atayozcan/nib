//! The whole terminal layer for `nib`.
//!
//! Linux-only by design. There's no `crossterm`, no `termion` — just `rustix` for the
//! termios syscalls, `vte` to parse keyboard escape sequences out of stdin, and
//! hand-written ANSI for output. Everything routes through this module so the rest of
//! the editor never touches a syscall or an escape sequence directly.

mod input;
mod output;
mod size;

pub(crate) use input::{Key, KeyMod, KeyReader};
pub(crate) use output::{Color, RenderCell, Renderer};
pub(crate) use size::{Size, SizeWatcher};

use std::io::{self, Write};

use anyhow::{Context, Result};
use rustix::stdio;
use rustix::termios::{
    self, ControlModes, InputModes, LocalModes, OptionalActions, OutputModes, SpecialCodeIndex,
    Termios,
};

const ENTER_ALT_SCREEN: &[u8] = b"\x1b[?1049h";
const LEAVE_ALT_SCREEN: &[u8] = b"\x1b[?1049l";
const HIDE_CURSOR: &[u8] = b"\x1b[?25l";
const SHOW_CURSOR: &[u8] = b"\x1b[?25h";
const ENABLE_BRACKETED_PASTE: &[u8] = b"\x1b[?2004h";
const DISABLE_BRACKETED_PASTE: &[u8] = b"\x1b[?2004l";
const RESET_STYLE: &[u8] = b"\x1b[0m";

/// RAII guard: puts the controlling TTY in raw mode + alternate screen on entry and
/// restores it on `Drop` — including on panic.
#[derive(Debug)]
pub(crate) struct TerminalGuard {
    original: Termios,
}

impl TerminalGuard {
    pub(crate) fn enter() -> Result<Self> {
        let stdin = stdio::stdin();
        let original = termios::tcgetattr(stdin).context("tcgetattr")?;

        let mut raw = original.clone();
        // Mirror the canonical "raw mode" mask used by every TUI library, written out
        // in full so the next person to touch this can see exactly what we're disabling.
        raw.input_modes.remove(
            InputModes::BRKINT
                | InputModes::ICRNL
                | InputModes::INPCK
                | InputModes::ISTRIP
                | InputModes::IXON,
        );
        raw.output_modes.remove(OutputModes::OPOST);
        raw.control_modes.insert(ControlModes::CS8);
        raw.local_modes
            .remove(LocalModes::ECHO | LocalModes::ICANON | LocalModes::IEXTEN | LocalModes::ISIG);
        // VMIN=0 / VTIME=1 → read returns within ~100ms with whatever is available.
        // Lets the main loop poll resize / external state without blocking forever on input.
        raw.special_codes[SpecialCodeIndex::VMIN] = 0;
        raw.special_codes[SpecialCodeIndex::VTIME] = 1;

        termios::tcsetattr(stdin, OptionalActions::Flush, &raw).context("tcsetattr (raw)")?;

        let mut stdout = io::stdout().lock();
        stdout.write_all(ENTER_ALT_SCREEN)?;
        stdout.write_all(HIDE_CURSOR)?;
        stdout.write_all(ENABLE_BRACKETED_PASTE)?;
        stdout.flush()?;

        Ok(Self { original })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout().lock();
        let _ = stdout.write_all(DISABLE_BRACKETED_PASTE);
        let _ = stdout.write_all(RESET_STYLE);
        let _ = stdout.write_all(SHOW_CURSOR);
        let _ = stdout.write_all(LEAVE_ALT_SCREEN);
        let _ = stdout.flush();
        let _ = termios::tcsetattr(stdio::stdin(), OptionalActions::Flush, &self.original);
    }
}
