mod args;
mod config_file;
mod renderer;
mod session;
mod timing;

use std::{
    error::Error,
    io::{self, IsTerminal},
    process::ExitCode,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal,
};
use stillterm_engine::{Engine, GridSize};

use args::{Args, Command};
use config_file::Settings;
use renderer::{Palette, Renderer};
use session::Session;
use timing::StepClock;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("stillterm: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    if matches!(args.command, Some(Command::List)) {
        println!("rain  Sparse streams of characters with fading trails");
        return Ok(());
    }
    let settings = Settings::load(&args)?;
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err("run in an interactive terminal (use --help or list for plain output)".into());
    }
    if std::env::var("TERM").is_ok_and(|term| term == "dumb") {
        return Err("this terminal does not support cursor-addressed animation".into());
    }
    let (columns, rows) = terminal::size()?;
    let mut engine = Engine::rain(
        GridSize::new(columns, rows)?,
        settings.effect,
        settings.seed,
    );
    let stop = Arc::new(AtomicBool::new(false));
    let signal_stop = Arc::clone(&stop);
    ctrlc::set_handler(move || signal_stop.store(true, Ordering::Relaxed))?;
    session::install_panic_hook();
    let session = Session::enter()?;
    let result = animate(&mut engine, settings.fps, &stop);
    let restored = session.finish();
    result?;
    restored?;
    Ok(())
}

fn animate(engine: &mut Engine, fps: u16, stop: &AtomicBool) -> Result<(), Box<dyn Error>> {
    let interval = Duration::from_secs_f64(1.0 / f64::from(fps));
    let mut renderer = Renderer::new(Palette::detect());
    let mut clock = StepClock::default();
    let mut previous_time = Instant::now();
    let mut next_frame = previous_time;
    let mut output = io::stdout();

    while !stop.load(Ordering::Relaxed) {
        let now = Instant::now();
        if now >= next_frame {
            if engine.size().is_empty() {
                clock.reset();
            } else {
                for _ in 0..clock.advance(now.duration_since(previous_time)) {
                    engine.step();
                }
                renderer.draw(engine.frame(), &mut output)?;
            }
            previous_time = now;
            // Skip missed presentation deadlines; never spin trying to catch up.
            next_frame = Instant::now() + interval;
        }
        let wait = next_frame
            .saturating_duration_since(Instant::now())
            .min(Duration::from_millis(50));
        if event::poll(wait)? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    if matches!(key.code, KeyCode::Esc | KeyCode::Char('q'))
                        || (key.code == KeyCode::Char('c')
                            && key.modifiers.contains(KeyModifiers::CONTROL))
                    {
                        break;
                    }
                }
                Event::Resize(columns, rows) => {
                    engine.resize(GridSize::new(columns, rows)?);
                    renderer.invalidate();
                    clock.reset();
                    previous_time = Instant::now();
                    // Presentation remains capped even during a resize storm.
                }
                _ => {}
            }
        }
    }
    Ok(())
}
