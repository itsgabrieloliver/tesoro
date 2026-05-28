use std::io::{self, Stdout};
use std::time::Duration;

use anyhow::Result;
use crossbeam_channel::RecvTimeoutError;
use crossterm::event::{Event as CtEvent, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::app::App;
use crate::editor;
use crate::event::{self, AppEvent};
use crate::ui;
use crate::watcher;

const TICK: Duration = Duration::from_millis(250);

type Term = Terminal<CrosstermBackend<Stdout>>;

pub struct TerminalGuard {
    terminal: Term,
}

impl TerminalGuard {
    fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let terminal = Terminal::new(CrosstermBackend::new(stdout))?;
        Ok(Self { terminal })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

pub fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original(info);
    }));
}

pub fn run(mut app: App) -> Result<()> {
    install_panic_hook();
    let mut guard = TerminalGuard::new()?;
    let events = event::spawn();

    let root = app.vault.root.clone();
    let _watcher = match watcher::spawn(root, events.sender()) {
        Ok(w) => Some(w),
        Err(e) => {
            tracing::warn!(error = %e, "file watcher disabled");
            None
        }
    };

    while !app.should_quit {
        guard.terminal.draw(|f| ui::draw(f, &mut app))?;
        match events.rx.recv_timeout(TICK) {
            Ok(ev) => {
                handle(&mut app, ev);
                while let Ok(ev) = events.rx.try_recv() {
                    handle(&mut app, ev);
                }
            }
            Err(RecvTimeoutError::Timeout) => app.on_tick(),
            Err(RecvTimeoutError::Disconnected) => break,
        }

        if let Some(path) = app.edit_request.take() {
            app.save_open();
            match editor::edit(&mut guard.terminal, &events, &path) {
                Ok(()) => {
                    app.after_edit(&path);
                    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("note");
                    app.status = Some(format!("reloaded {name}"));
                }
                Err(e) => app.status = Some(format!("editor error: {e}")),
            }
            while events.rx.try_recv().is_ok() {}
        }
    }
    Ok(())
}

fn handle(app: &mut App, ev: AppEvent) {
    match ev {
        AppEvent::Input(CtEvent::Key(key)) if key.kind == KeyEventKind::Press => app.on_key(key),
        AppEvent::Input(_) => {}
        AppEvent::VaultChanged(paths) => {
            for p in &paths {
                app.on_external_change(p);
            }
        }
    }
}
