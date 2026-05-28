use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, unbounded};
use crossterm::event::{self, Event as CtEvent};

const POLL: Duration = Duration::from_millis(100);

pub enum AppEvent {
    Input(CtEvent),
    VaultChanged(Vec<PathBuf>),
}

pub struct Events {
    pub rx: Receiver<AppEvent>,
    tx: Sender<AppEvent>,
    suspended: Arc<AtomicBool>,
}

impl Events {
    pub fn set_suspended(&self, value: bool) {
        self.suspended.store(value, Ordering::Relaxed);
    }

    pub fn sender(&self) -> Sender<AppEvent> {
        self.tx.clone()
    }
}

pub fn spawn() -> Events {
    let (tx, rx): (Sender<AppEvent>, Receiver<AppEvent>) = unbounded();
    let suspended = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&suspended);
    let input_tx = tx.clone();
    thread::spawn(move || input_loop(input_tx, flag));
    Events { rx, tx, suspended }
}

fn input_loop(tx: Sender<AppEvent>, suspended: Arc<AtomicBool>) {
    loop {
        if suspended.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(50));
            continue;
        }
        match event::poll(POLL) {
            Ok(true) => match event::read() {
                Ok(ev) => {
                    if tx.send(AppEvent::Input(ev)).is_err() {
                        return;
                    }
                }
                Err(_) => return,
            },
            Ok(false) => {}
            Err(_) => return,
        }
    }
}
