use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Quiet character animation for your terminal",
    after_help = "Press q, Escape, or Ctrl+C to quit. This application does not lock your session."
)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Command>,
    /// Exit on any key press, mouse button, or scroll (for screensaver hosts)
    #[arg(long, global = true)]
    pub exit_on_input: bool,
    /// Read settings from a TOML file
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,
    /// Animation to run [default: rain]
    #[arg(long, global = true)]
    pub effect: Option<String>,
    /// Visual preset [default: monochrome]
    #[arg(long, global = true, value_parser = ["monochrome", "matrix"])]
    pub theme: Option<String>,
    /// Presentation frames per second (10–60) [default: 30]
    #[arg(long, global = true)]
    pub fps: Option<u16>,
    /// Animation speed (0.1–4.0); overrides the theme preset
    #[arg(long, global = true)]
    pub speed: Option<f64>,
    /// Stream activation probability (0.0–1.0); overrides the theme preset
    #[arg(long, global = true)]
    pub density: Option<f64>,
    /// Maximum brightness (0.0–1.0); overrides the theme preset
    #[arg(long, global = true)]
    pub intensity: Option<f64>,
    /// Printable single-column characters; overrides the theme preset
    #[arg(long, global = true)]
    pub characters: Option<String>,
    /// Reproducible random seed [default: 42]
    #[arg(long, global = true)]
    pub seed: Option<u64>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the animation (also the default when no command is supplied)
    Run,
    /// List available effects without opening the terminal renderer
    List,
}
