//! Named command registry.
//!
//! Every editable action — cursor motions, mode changes, buffer ops, quit — is a named
//! command registered here. Keybinds in config dispatch to commands by name; the rest
//! of the editor never wires keys to logic directly.

use std::collections::HashMap;

use crate::buffer::Buffer;
use crate::mode::Mode;

/// Mutable context passed to every command. Holds everything a command might want to
/// mutate. Kept deliberately wide (one type, not a trait) so commands stay one-line
/// closures and don't need to plumb generic parameters.
#[derive(Debug)]
pub(crate) struct Context<'a> {
    pub(crate) buffer: &'a mut Buffer,
    pub(crate) mode: &'a mut Mode,
    pub(crate) status: &'a mut String,
    pub(crate) quit: &'a mut bool,
}

pub(crate) type CommandFn = fn(&mut Context<'_>);

#[derive(Debug, Default)]
pub(crate) struct Registry {
    by_name: HashMap<&'static str, CommandFn>,
}

impl Registry {
    pub(crate) fn with_builtins() -> Self {
        let mut r = Self::default();
        r.register_builtins();
        r
    }

    pub(crate) fn get(&self, name: &str) -> Option<CommandFn> {
        self.by_name.get(name).copied()
    }

    fn put(&mut self, name: &'static str, f: CommandFn) {
        self.by_name.insert(name, f);
    }

    fn register_builtins(&mut self) {
        // --- motions ---
        self.put("cursor.left", |c| c.buffer.move_left());
        self.put("cursor.right", |c| c.buffer.move_right());
        self.put("cursor.up", |c| c.buffer.move_up());
        self.put("cursor.down", |c| c.buffer.move_down());
        self.put("cursor.line_start", |c| c.buffer.move_line_start());
        self.put("cursor.line_end", |c| c.buffer.move_line_end());
        self.put("cursor.word_forward", |c| c.buffer.move_word_forward());
        self.put("cursor.word_back", |c| c.buffer.move_word_back());
        self.put("buffer.goto_start", |c| c.buffer.move_buffer_start());
        self.put("buffer.goto_end", |c| c.buffer.move_buffer_end());

        // --- editing (work in any mode but typing comes in via mode dispatch) ---
        self.put("edit.backspace", |c| c.buffer.backspace());
        self.put("edit.delete_forward", |c| c.buffer.delete_char_forward());
        self.put("edit.delete_line", |c| c.buffer.delete_line());
        self.put("edit.newline", |c| c.buffer.insert_newline());
        self.put("edit.undo", |c| {
            if !c.buffer.undo() {
                c.status.replace_range(.., "nothing to undo");
            }
        });
        self.put("edit.redo", |c| {
            if !c.buffer.redo() {
                c.status.replace_range(.., "nothing to redo");
            }
        });

        // --- mode changes ---
        self.put("mode.normal", |c| *c.mode = Mode::Normal);
        self.put("mode.insert", |c| *c.mode = Mode::Insert);
        self.put("mode.insert_after", |c| {
            c.buffer.move_right();
            *c.mode = Mode::Insert;
        });
        self.put("mode.insert_line_start", |c| {
            c.buffer.move_line_start();
            *c.mode = Mode::Insert;
        });
        self.put("mode.insert_line_end", |c| {
            c.buffer.move_line_end();
            *c.mode = Mode::Insert;
        });
        self.put("mode.open_below", |c| {
            c.buffer.move_line_end();
            c.buffer.insert_newline();
            *c.mode = Mode::Insert;
        });
        self.put("mode.command", |c| *c.mode = Mode::Command);

        // --- file / lifecycle ---
        self.put("buffer.save", |c| match c.buffer.save() {
            Ok(()) => {
                let msg = format!("saved {}", c.buffer.path().display());
                c.status.replace_range(.., &msg);
            }
            Err(e) => {
                let msg = format!("save failed: {e}");
                c.status.replace_range(.., &msg);
            }
        });
        self.put("editor.quit", |c| *c.quit = true);
    }
}
