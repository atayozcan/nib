use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::buffer::Buffer;
use crate::terminal::Tui;
use crate::ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Status {
    Running,
    Quitting,
}

#[derive(Debug)]
pub(crate) struct Editor {
    pub(crate) buffer: Buffer,
    pub(crate) message: String,
    status: Status,
}

impl Editor {
    pub(crate) fn open(path: PathBuf) -> Result<Self> {
        let buffer = Buffer::open(path)?;
        Ok(Self {
            buffer,
            message: String::from("Ctrl+S save · Ctrl+Q quit"),
            status: Status::Running,
        })
    }

    pub(crate) fn run(&mut self, terminal: &mut Tui) -> Result<()> {
        while self.status == Status::Running {
            terminal.draw(|frame| ui::render(frame, self))?;
            if event::poll(Duration::from_millis(50))?
                && let Event::Key(key) = event::read()?
            {
                self.handle_key(key);
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match (ctrl, key.code) {
            (true, KeyCode::Char('q')) | (_, KeyCode::Esc) => self.status = Status::Quitting,
            (true, KeyCode::Char('s')) => match self.buffer.save() {
                Ok(()) => self.message = format!("saved {}", self.buffer.path().display()),
                Err(e) => self.message = format!("save failed: {e}"),
            },
            (_, KeyCode::Char(c)) => self.buffer.insert_char(c),
            (_, KeyCode::Enter) => self.buffer.insert_newline(),
            (_, KeyCode::Backspace) => self.buffer.backspace(),
            (_, KeyCode::Delete) => self.buffer.delete_forward(),
            (_, KeyCode::Left) => self.buffer.move_left(),
            (_, KeyCode::Right) => self.buffer.move_right(),
            (_, KeyCode::Up) => self.buffer.move_up(),
            (_, KeyCode::Down) => self.buffer.move_down(),
            (_, KeyCode::Home) => self.buffer.move_home(),
            (_, KeyCode::End) => self.buffer.move_end(),
            _ => {}
        }
    }
}
