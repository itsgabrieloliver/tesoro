mod app;
mod cli;
mod config;
mod editor;
mod event;
mod graphview;
mod logging;
mod markdown;
mod picker;
mod templates;
mod theme;
mod tui;
mod ui;
mod vault;
mod watcher;

use anyhow::Result;
use clap::Parser;

use vault::Vault;

fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    let _log_guard = logging::init(cli.verbose);
    let config = config::Config::load()?;
    let vault_path = config::resolve_vault(cli.vault, &config)?;
    tracing::info!(vault = %vault_path.display(), "loading vault");
    let vault = Vault::load(vault_path)?;
    tracing::info!(notes = vault.notes.len(), "vault loaded");
    let mut app = app::App::new(vault);
    let leader = app::parse_leader(config.leader.as_deref().unwrap_or("ctrl"));
    app.set_leader(leader);
    app.open_welcome_in_preview();
    tui::run(app)
}
