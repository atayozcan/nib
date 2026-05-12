//! `nib` — a minimal, highly-configurable modal terminal text editor.
//!
//! Linux `x86_64` only. Pure vi-style controls, KDL config, hand-rolled terminal
//! layer on top of `vte` + `rustix` + `signal-hook`.
//!
//! Module layout:
//! - [`buffer`]  — rope-backed text storage with grapheme cursor + transactional undo
//! - [`mode`]    — `Mode` enum (Normal / Insert / Command)
//! - [`keymap`]  — chord-trie keymap; `<C-x>`-style parser
//! - [`command`] — named-command registry (`fn(&mut Context)`) including the `:` parser
//! - [`config`]  — KDL loader, compiled-in defaults + user overlay
//! - [`term`]    — terminal layer (raw mode, escape parsing, cell-diff renderer)
//! - [`editor`]  — main loop: poll → dispatch → draw → flush
//! - [`cli`]     — `clap` arg parser

#[cfg(not(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
compile_error!(
    "nib targets Linux on x86_64 (x86-64-v3 baseline) or aarch64 (Armv9-A / cortex-a520 baseline) only \
     — see src/main.rs / .cargo/config.toml"
);

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
    let (config, config_warning) = Config::load();
    Editor::open(cli.path, config, config_warning)?.run()
}
