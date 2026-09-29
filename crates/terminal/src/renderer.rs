use std::io::{self, Write};

use crossterm::{
    Command,
    cursor::MoveTo,
    style::{Color, SetBackgroundColor, SetForegroundColor},
    terminal::{Clear, ClearType},
};
use stillterm_engine::{Cell, Frame, GridSize};

#[derive(Debug, Clone, Copy)]
pub enum Palette {
    Grayscale,
    Basic,
    Monochrome,
}

impl Palette {
    pub fn detect() -> Self {
        if std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty()) {
            return Self::Monochrome;
        }
        let term = std::env::var("TERM").unwrap_or_default();
        let color = std::env::var("COLORTERM").unwrap_or_default();
        if cfg!(windows) || term.contains("256color") || color == "truecolor" || color == "24bit" {
            Self::Grayscale
        } else {
            Self::Basic
        }
    }

    fn present(self, cell: Cell) -> PresentedCell {
        let level = match self {
            Self::Grayscale => u16::from(cell.intensity) * 15 / 255,
            Self::Basic => u16::from(cell.intensity) * 3 / 255,
            Self::Monochrome => u16::from(cell.intensity > 16),
        } as u8;
        if level == 0 || cell.glyph.as_char() == ' ' {
            PresentedCell::BLANK
        } else {
            PresentedCell {
                glyph: cell.glyph.as_char(),
                level,
            }
        }
    }

    fn color(self, level: u8) -> Color {
        match self {
            Self::Grayscale => Color::AnsiValue(232 + level * 23 / 15),
            Self::Basic => {
                [Color::Black, Color::DarkGrey, Color::Grey, Color::White][usize::from(level)]
            }
            Self::Monochrome => Color::Reset,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct PresentedCell {
    glyph: char,
    level: u8,
}

impl PresentedCell {
    const BLANK: Self = Self {
        glyph: ' ',
        level: 0,
    };
}

pub struct Renderer {
    palette: Palette,
    size: Option<GridSize>,
    previous: Vec<PresentedCell>,
    bytes: String,
}

impl Renderer {
    pub fn new(palette: Palette) -> Self {
        Self {
            palette,
            size: None,
            previous: Vec::new(),
            bytes: String::new(),
        }
    }

    pub fn invalidate(&mut self) {
        self.size = None;
    }

    fn encode(&mut self, command: impl Command) -> io::Result<()> {
        // queue! can bypass the buffer and call WinAPI on Windows. Encoding
        // explicitly keeps frame generation pure, including when tested in CI.
        command
            .write_ansi(&mut self.bytes)
            .map_err(|_| io::Error::other("cannot encode terminal command"))
    }

    pub fn draw(&mut self, frame: &Frame, output: &mut impl Write) -> io::Result<()> {
        self.bytes.clear();
        let size = frame.size();
        if self.size != Some(size) {
            self.previous.resize(size.len(), PresentedCell::BLANK);
            self.previous.fill(PresentedCell::BLANK);
            self.size = Some(size);
            // Keep NO_COLOR meaningful while still clearing stale glyphs on resize.
            if !matches!(self.palette, Palette::Monochrome) {
                self.encode(SetBackgroundColor(Color::Black))?;
            }
            self.encode(Clear(ClearType::All))?;
        }
        let mut cursor = None;
        let mut foreground = None;
        for (index, cell) in frame.cells().iter().enumerate() {
            // Avoid triggering scroll on terminals with delayed auto-wrap.
            if index + 1 == size.len() {
                continue;
            }
            let presented = self.palette.present(*cell);
            if self.previous[index] == presented {
                continue;
            }
            let column = (index % usize::from(size.columns())) as u16;
            let row = (index / usize::from(size.columns())) as u16;
            if cursor != Some((column, row)) {
                self.encode(MoveTo(column, row))?;
            }
            if presented != PresentedCell::BLANK && foreground != Some(presented.level) {
                if !matches!(self.palette, Palette::Monochrome) {
                    self.encode(SetForegroundColor(self.palette.color(presented.level)))?;
                }
                foreground = Some(presented.level);
            }
            self.bytes.push(presented.glyph);
            self.previous[index] = presented;
            cursor = Some((column + 1, row));
        }
        if !self.bytes.is_empty() {
            // A failed write leaves the cached screen uncertain; repaint on retry.
            if let Err(error) = output
                .write_all(self.bytes.as_bytes())
                .and_then(|()| output.flush())
            {
                self.invalidate();
                return Err(error);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_write_forces_a_complete_repaint_on_retry() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut frame = Frame::new(GridSize::new(3, 1).unwrap());
        frame.set(
            0,
            0,
            Cell {
                glyph: Glyph::new('x').unwrap(),
                intensity: 255,
            },
        );
        let mut renderer = Renderer::new(Palette::Monochrome);
        assert!(renderer.draw(&frame, &mut Broken).is_err());
        let mut bytes = Vec::new();
        renderer.draw(&frame, &mut bytes).unwrap();
        assert_eq!(bytes, b"\x1b[2J\x1b[1;1Hx");
    }
    use stillterm_engine::Glyph;

    #[test]
    fn unchanged_frames_emit_nothing_and_removed_glyphs_are_erased() {
        let mut frame = Frame::new(GridSize::new(3, 1).unwrap());
        frame.set(
            0,
            0,
            Cell {
                glyph: Glyph::new('x').unwrap(),
                intensity: 200,
            },
        );
        let mut renderer = Renderer::new(Palette::Grayscale);
        renderer.draw(&frame, &mut Vec::new()).unwrap();
        let mut bytes = Vec::new();
        renderer.draw(&frame, &mut bytes).unwrap();
        assert!(bytes.is_empty());
        frame.clear();
        renderer.draw(&frame, &mut bytes).unwrap();
        assert_eq!(bytes, b"\x1b[1;1H ");
    }

    #[test]
    fn intensity_changes_within_one_palette_level_do_not_redraw() {
        let mut frame = Frame::new(GridSize::new(3, 1).unwrap());
        let glyph = Glyph::new('x').unwrap();
        frame.set(
            0,
            0,
            Cell {
                glyph,
                intensity: 200,
            },
        );
        let mut renderer = Renderer::new(Palette::Grayscale);
        renderer.draw(&frame, &mut Vec::new()).unwrap();
        frame.set(
            0,
            0,
            Cell {
                glyph,
                intensity: 201,
            },
        );
        let mut bytes = Vec::new();
        renderer.draw(&frame, &mut bytes).unwrap();
        assert!(bytes.is_empty());
    }

    #[test]
    fn adjacent_cells_share_cursor_movement_and_bottom_right_is_reserved() {
        let mut frame = Frame::new(GridSize::new(3, 1).unwrap());
        for col in 0..3 {
            frame.set(
                col,
                0,
                Cell {
                    glyph: Glyph::new('x').unwrap(),
                    intensity: 255,
                },
            );
        }
        let mut bytes = Vec::new();
        Renderer::new(Palette::Monochrome)
            .draw(&frame, &mut bytes)
            .unwrap();
        assert_eq!(bytes, b"\x1b[2J\x1b[1;1Hxx");
    }

    #[test]
    fn resize_invalidates_cached_cells() {
        let mut renderer = Renderer::new(Palette::Monochrome);
        renderer
            .draw(&Frame::new(GridSize::new(4, 2).unwrap()), &mut Vec::new())
            .unwrap();
        let mut bytes = Vec::new();
        renderer
            .draw(&Frame::new(GridSize::new(0, 0).unwrap()), &mut bytes)
            .unwrap();
        assert_eq!(bytes, b"\x1b[2J");
    }
}
