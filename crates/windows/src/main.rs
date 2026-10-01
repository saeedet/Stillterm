#![cfg_attr(windows, windows_subsystem = "windows")]
#[cfg(windows)]
mod native;

fn main() {
    #[cfg(windows)]
    {
        use stillterm_windows::invocation::{self, Mode};
        let args: Vec<String> = match std::env::args_os()
            .skip(1)
            .map(|s| s.into_string())
            .collect()
        {
            Ok(args) => args,
            Err(_) => return,
        };
        let Ok(mode) = invocation::parse(&args) else {
            return;
        };
        if let Err(error) = native::run(mode) {
            // Fullscreen and embedded preview failures must exit without blocking the host.
            if matches!(mode, Mode::Configure(_)) {
                native::show_error(&error);
            }
        }
    }
    #[cfg(not(windows))]
    {
        eprintln!("The native screensaver runs on Windows; use stillterm for terminal animation.");
        std::process::exit(1);
    }
}
