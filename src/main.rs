mod buffer;
mod cli;
mod editor;
mod terminal;
mod ui;

use anyhow::Result;
use clap::Parser;

use crate::cli::Cli;
use crate::editor::Editor;
use crate::terminal::TerminalGuard;

fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut editor = Editor::open(cli.path)?;
    let mut terminal = TerminalGuard::enter()?;
    let outcome = editor.run(terminal.terminal());

    drop(terminal);
    outcome
}
