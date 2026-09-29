use std::{
    io::{self, Write},
    sync::atomic::{AtomicBool, Ordering},
};

use crossterm::{
    cursor::{Hide, Show},
    execute,
    style::ResetColor,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};

static ACTIVE: AtomicBool = AtomicBool::new(false);

/// Restores modes on ordinary errors, partial setup, and stack unwinding.
pub struct Session;

impl Session {
    pub fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        ACTIVE.store(true, Ordering::SeqCst);
        let session = Self;
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        Ok(session)
    }

    pub fn finish(self) -> io::Result<()> {
        restore()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = restore();
    }
}

fn restore() -> io::Result<()> {
    if !ACTIVE.swap(false, Ordering::SeqCst) {
        return Ok(());
    }
    // Attempt every restoration even if an earlier write fails.
    let raw = terminal::disable_raw_mode();
    let mut out = io::stdout();
    let color = execute!(out, ResetColor);
    let screen = execute!(out, LeaveAlternateScreen);
    let cursor = execute!(out, Show);
    let flush = out.flush();
    raw.and(color).and(screen).and(cursor).and(flush)
}

pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore();
        previous(info);
    }));
}
