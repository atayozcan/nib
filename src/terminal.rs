use std::io::{self, Stdout};

use anyhow::{Context, Result};
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

pub(crate) type Tui = Terminal<CrosstermBackend<Stdout>>;

/// RAII guard that puts the terminal into raw mode + alternate screen on construction
/// and restores it on drop — including on panic, which the 2021 version didn't handle.
pub(crate) struct TerminalGuard {
    terminal: Tui,
}

impl TerminalGuard {
    pub(crate) fn enter() -> Result<Self> {
        enable_raw_mode().context("failed to enable raw mode")?;
        let mut stdout = io::stdout();
        crossterm::execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)
            .context("failed to enter alternate screen")?;
        let terminal = Terminal::new(CrosstermBackend::new(stdout))
            .context("failed to construct terminal backend")?;
        Ok(Self { terminal })
    }

    pub(crate) fn terminal(&mut self) -> &mut Tui {
        &mut self.terminal
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableBracketedPaste
        );
        let _ = self.terminal.show_cursor();
    }
}

impl std::fmt::Debug for TerminalGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalGuard").finish_non_exhaustive()
    }
}
