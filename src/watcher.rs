use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use crossbeam_channel::Sender;
use notify_debouncer_full::notify::RecursiveMode;
use notify_debouncer_full::{DebounceEventResult, new_debouncer};

use crate::event::AppEvent;

pub fn spawn(root: PathBuf, tx: Sender<AppEvent>) -> Result<impl Send> {
    let mut debouncer = new_debouncer(
        Duration::from_millis(400),
        None,
        move |result: DebounceEventResult| {
            let Ok(events) = result else {
                return;
            };
            let mut paths: Vec<PathBuf> = Vec::new();
            for ev in &events {
                for p in &ev.paths {
                    let is_md = p
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("md"));
                    if is_md && !paths.contains(p) {
                        paths.push(p.clone());
                    }
                }
            }
            if !paths.is_empty() {
                let _ = tx.send(AppEvent::VaultChanged(paths));
            }
        },
    )?;
    debouncer.watch(&root, RecursiveMode::Recursive)?;
    Ok(debouncer)
}
