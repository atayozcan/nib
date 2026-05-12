use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use rustix::stdio;
use rustix::termios;
use signal_hook::consts::SIGWINCH;
use signal_hook::flag;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Size {
    pub(crate) cols: u16,
    pub(crate) rows: u16,
}

impl Size {
    pub(crate) fn query() -> Result<Self> {
        let ws = termios::tcgetwinsize(stdio::stdin()).context("tcgetwinsize")?;
        Ok(Self {
            cols: ws.ws_col,
            rows: ws.ws_row,
        })
    }
}

/// Wraps `signal-hook`'s atomic-flag registration for `SIGWINCH`. Call [`Self::poll`]
/// once per main-loop tick — it returns `true` if the terminal was resized since the
/// last call.
#[derive(Debug)]
pub(crate) struct SizeWatcher {
    flag: Arc<AtomicBool>,
}

impl SizeWatcher {
    pub(crate) fn new() -> Result<Self> {
        let flag = Arc::new(AtomicBool::new(false));
        flag::register(SIGWINCH, Arc::clone(&flag)).context("register SIGWINCH")?;
        Ok(Self { flag })
    }

    pub(crate) fn poll(&self) -> bool {
        self.flag.swap(false, Ordering::Relaxed)
    }
}
