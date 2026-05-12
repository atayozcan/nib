use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "rusty-editor",
    version,
    about = "A small terminal text editor",
    long_about = None,
)]
pub(crate) struct Cli {
    /// File to open. If it does not exist, it will be created on first save.
    pub(crate) path: PathBuf,
}
