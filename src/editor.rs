//! Main editor loop. Pulls together buffer, config, keymap dispatch, terminal I/O.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::buffer::Buffer;
use crate::command::{Context, Registry};
use crate::config::Config;
use crate::keymap::{KeyChord, Lookup};
use crate::mode::Mode;
use crate::term::{Key, KeyMod, KeyReader, RenderCell, Renderer, Size, SizeWatcher, TerminalGuard};

#[derive(Debug)]
pub(crate) struct Editor {
    buffer: Buffer,
    mode: Mode,
    status: String,
    cmdline: String,
    quit: bool,
    config: Config,
    registry: Registry,
    pending: Vec<KeyChord>,
    last_chord_at: Instant,
}

impl Editor {
    pub(crate) fn open(path: PathBuf, config: Config) -> Result<Self> {
        let buffer = Buffer::open(path)?;
        Ok(Self {
            buffer,
            mode: Mode::Normal,
            status: String::from("-- NORMAL --   :q to quit  :w to save"),
            cmdline: String::new(),
            quit: false,
            config,
            registry: Registry::with_builtins(),
            pending: Vec::new(),
            last_chord_at: Instant::now(),
        })
    }

    pub(crate) fn run(mut self) -> Result<()> {
        let _guard = TerminalGuard::enter()?;
        let mut renderer = Renderer::new(Size::query()?);
        let watcher = SizeWatcher::new()?;
        let mut keys = KeyReader::new();

        self.draw(&mut renderer);
        renderer.flush()?;

        while !self.quit {
            if watcher.poll() {
                renderer.resize(Size::query()?);
            }

            for (key, mods) in keys.poll()? {
                self.handle_key(key, mods);
                if self.quit {
                    break;
                }
            }

            // Chord timeout — if the user paused mid-prefix, drop the prefix.
            if !self.pending.is_empty()
                && self.last_chord_at.elapsed()
                    > Duration::from_millis(u64::from(self.config.behavior.chord_timeout_ms))
            {
                self.pending.clear();
            }

            self.draw(&mut renderer);
            renderer.flush()?;
        }
        Ok(())
    }

    fn handle_key(&mut self, key: Key, mods: KeyMod) {
        // Insert mode has a fallthrough: any unmapped printable key inserts itself.
        // We still consult the keymap first so the user can bind <C-w> etc.
        let chord = KeyChord::from_event(key, mods);
        self.pending.push(chord);
        self.last_chord_at = Instant::now();

        let mode_for_lookup = self.mode;
        let lookup_result = self
            .config
            .keymaps
            .get(&mode_for_lookup)
            .map(|km| km.lookup(&self.pending));

        let outcome = lookup_result.as_ref().map(|l| match l {
            Lookup::Command(name) => Outcome::Run((*name).to_string()),
            Lookup::Pending => Outcome::Wait,
            Lookup::None => Outcome::Fallback,
        });

        match outcome.unwrap_or(Outcome::Fallback) {
            Outcome::Run(name) => {
                self.pending.clear();
                if let Some(cmd) = self.registry.get(&name) {
                    let mut ctx = Context {
                        buffer: &mut self.buffer,
                        mode: &mut self.mode,
                        status: &mut self.status,
                        quit: &mut self.quit,
                    };
                    cmd(&mut ctx);
                } else {
                    self.status = format!("unknown command: {name}");
                }
            }
            Outcome::Wait => {
                // keep accumulating
            }
            Outcome::Fallback => {
                // No binding. In insert mode, type the character. Otherwise drop.
                self.pending.clear();
                if self.mode == Mode::Insert {
                    if let Key::Char(c) = key {
                        if mods.is_empty() || mods == KeyMod::SHIFT {
                            self.buffer.insert_char(c);
                        }
                    } else if key == Key::Enter {
                        self.buffer.insert_newline();
                    } else if key == Key::Backspace {
                        self.buffer.backspace();
                    }
                }
            }
        }
    }

    fn draw(&self, r: &mut Renderer) {
        r.clear_back();
        let size = r.size();
        if size.rows < 3 || size.cols < 1 {
            return;
        }
        let text_rows = size.rows.saturating_sub(2);
        let text_cols = size.cols;

        let theme = &self.config.theme;
        let line_count = self.buffer.line_count();
        let cursor = self.buffer.cursor;

        // Vertical scroll so the cursor stays on-screen.
        let scroll_y = cursor.line.saturating_sub(text_rows as usize - 1);

        // Text area.
        for screen_row in 0..text_rows {
            let line_idx = scroll_y + screen_row as usize;
            if line_idx >= line_count {
                break;
            }
            let line = self.buffer.line(line_idx);
            r.put_str(
                0,
                screen_row,
                &line,
                theme.foreground,
                theme.background,
                false,
            );
        }

        // Statusline.
        let status_y = size.rows.saturating_sub(2);
        for x in 0..size.cols {
            r.put(
                x,
                status_y,
                RenderCell {
                    ch: ' ',
                    fg: theme.status_fg,
                    bg: theme.status_bg,
                    bold: false,
                    reverse: false,
                },
            );
        }
        let mode_label = match self.mode {
            Mode::Normal => " NORMAL ",
            Mode::Insert => " INSERT ",
            Mode::Command => " COMMAND ",
        };
        let path = self.buffer.path().display().to_string();
        let dirty = if self.buffer.is_dirty() { " ●" } else { "" };
        let line = format!(
            "{mode_label}{path}{dirty}   {}:{}",
            cursor.line + 1,
            cursor.col + 1
        );
        r.put_str(0, status_y, &line, theme.status_fg, theme.status_bg, true);

        // Cmdline / message line.
        let msg_y = size.rows.saturating_sub(1);
        let msg = if self.mode == Mode::Command {
            format!(":{}", self.cmdline)
        } else {
            self.status.clone()
        };
        r.put_str(0, msg_y, &msg, theme.cmdline_fg, theme.cmdline_bg, false);

        // Cursor.
        let screen_x = u16::try_from(cursor.col)
            .unwrap_or(u16::MAX)
            .min(text_cols.saturating_sub(1));
        let screen_y = u16::try_from(cursor.line.saturating_sub(scroll_y))
            .unwrap_or(u16::MAX)
            .min(text_rows.saturating_sub(1));
        r.set_cursor(screen_x, screen_y);
    }
}

enum Outcome {
    Run(String),
    Wait,
    Fallback,
}
