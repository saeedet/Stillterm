use std::{
    io::{self, Write},
    sync::atomic::{AtomicBool, Ordering},
};

use crossterm::{
    cursor::{Hide, Show},
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    style::ResetColor,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};

static ACTIVE: AtomicBool = AtomicBool::new(false);
static MOUSE_CAPTURED: AtomicBool = AtomicBool::new(false);

/// Restores modes on ordinary errors, partial setup, and stack unwinding.
pub struct Session;

impl Session {
    pub fn enter(capture_mouse: bool) -> io::Result<Self> {
        #[cfg(windows)]
        if !crossterm::ansi_support::supports_ansi() {
            return Err(io::Error::other(
                "an ANSI-capable Windows terminal is required",
            ));
        }
        terminal::enable_raw_mode()?;
        ACTIVE.store(true, Ordering::SeqCst);
        let session = Self;
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        if capture_mouse {
            MOUSE_CAPTURED.store(true, Ordering::SeqCst);
            execute!(io::stdout(), EnableMouseCapture)?;
        }
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
    let mouse = if MOUSE_CAPTURED.swap(false, Ordering::SeqCst) {
        execute!(out, DisableMouseCapture)
    } else {
        Ok(())
    };
    let color = execute!(out, ResetColor);
    let screen = execute!(out, LeaveAlternateScreen);
    let cursor = execute!(out, Show);
    let flush = out.flush();
    raw.and(mouse).and(color).and(screen).and(cursor).and(flush)
}

pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore();
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires an isolated PTY; run scripts/check-terminal.py --panic-check"]
    fn panic_restores_terminal() {
        install_panic_hook();
        let unwind = std::panic::catch_unwind(|| {
            let _session = Session::enter(true).expect("PTY setup");
            panic!("intentional terminal cleanup test");
        });
        assert!(unwind.is_err());
        assert!(!terminal::is_raw_mode_enabled().unwrap());
        assert!(!ACTIVE.load(Ordering::SeqCst));
        // Let the parent inspect actual termios before Cargo closes its PTY.
        println!("STILLTERM_CLEANUP_READY");
        io::stdout().flush().unwrap();
        io::stdin().read_line(&mut String::new()).unwrap();
    }
}
