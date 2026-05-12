#[cfg(not(target_os = "linux"))]
compile_error!("nib is Linux-only by design — see src/main.rs");

mod buffer;
mod cli;
mod command;
mod config;
mod editor;
mod keymap;
mod mode;
mod term;

use anyhow::Result;
use clap::Parser;

use crate::cli::Cli;
use crate::config::Config;
use crate::editor::Editor;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load()?;
    Editor::open(cli.path, config)?.run()
}
