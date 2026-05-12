#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
compile_error!("nib targets Linux on x86_64 only — see src/main.rs / .cargo/config.toml");

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
