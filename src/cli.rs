use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "teso", version, about = "Obsidian-compatible note-taking TUI")]
pub struct Cli {
    #[arg(
        help = "Path to the vault (a folder of .md files); defaults to $TESORO_VAULT or the current directory"
    )]
    pub vault: Option<PathBuf>,

    #[arg(short, long, help = "Enable debug logging to the cache directory")]
    pub verbose: bool,
}
