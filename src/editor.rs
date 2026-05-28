use std::io::Stdout;
use std::path::Path;
use std::process::Command;

use anyhow::Result;
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::event::Events;

type Term = Terminal<CrosstermBackend<Stdout>>;

fn on_path(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(bin).is_file()))
        .unwrap_or(false)
}

fn editor_command() -> String {
    let from_env = std::env::var("VISUAL")
        .ok()
        .or_else(|| std::env::var("EDITOR").ok())
        .filter(|s| !s.trim().is_empty());
    if let Some(cmd) = from_env {
        return cmd;
    }
    for candidate in ["nvim", "vim", "hx", "nano"] {
        if on_path(candidate) {
            return candidate.to_string();
        }
    }
    "vi".to_string()
}

pub fn edit(terminal: &mut Term, events: &Events, file: &Path) -> Result<()> {
    let cmd = editor_command();
    let mut parts = cmd.split_whitespace();
    let program = parts.next().unwrap_or("vi").to_string();
    let args: Vec<String> = parts.map(str::to_string).collect();

    events.set_suspended(true);
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    disable_raw_mode()?;

    let status = Command::new(&program).args(&args).arg(file).status();

    enable_raw_mode()?;
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    terminal.clear()?;
    events.set_suspended(false);

    status?;
    Ok(())
}
