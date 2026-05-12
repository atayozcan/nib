use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "nib",
    version,
    about = "A minimal, highly-configurable modal terminal text editor",
    long_about = None,
)]
pub(crate) struct Cli {
    /// File to open. Created on first save if it does not exist.
    pub(crate) path: PathBuf,
}
