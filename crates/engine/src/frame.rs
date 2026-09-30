use unicode_width::UnicodeWidthChar;

use crate::ConfigError;

/// Upper bound on frame storage, including sizes supplied by native hosts.
pub const MAX_CELLS: usize = 262_144;

/// A printable scalar occupying one column in both common width policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyph(char);

impl Glyph {
    pub const SPACE: Self = Self(' ');

    pub fn new(ch: char) -> Result<Self, ConfigError> {
        if ch.is_control() || ch.width() != Some(1) || ch.width_cjk() != Some(1) {
            return Err(ConfigError(format!(
                "character {ch:?} must be printable and single-column"
            )));
        }
        Ok(Self(ch))
    }

    pub const fn as_char(self) -> char {
        self.0
    }
}

/// Visual state, independent of any palette or font.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub glyph: Glyph,
    pub intensity: u8,
    /// Highlights a stream head; renderers may ignore this presentation hint.
    pub emphasis: bool,
}

impl Cell {
    pub const BLANK: Self = Self {
        glyph: Glyph::SPACE,
        intensity: 0,
        emphasis: false,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridSize {
    columns: u16,
    rows: u16,
}

impl GridSize {
    pub fn new(columns: u16, rows: u16) -> Result<Self, ConfigError> {
        let count = usize::from(columns).checked_mul(usize::from(rows));
        if count.is_none_or(|n| n > MAX_CELLS) {
            return Err(ConfigError(format!(
                "frame must contain at most {MAX_CELLS} cells"
            )));
        }
        Ok(Self { columns, rows })
    }

    pub const fn columns(self) -> u16 {
        self.columns
    }
    pub const fn rows(self) -> u16 {
        self.rows
    }
    pub fn len(self) -> usize {
        usize::from(self.columns) * usize::from(self.rows)
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
}

/// Reusable row-major storage with checked access and private dimensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    size: GridSize,
    cells: Vec<Cell>,
}

impl Frame {
    pub fn new(size: GridSize) -> Self {
        Self {
            size,
            cells: vec![Cell::BLANK; size.len()],
        }
    }
    pub fn size(&self) -> GridSize {
        self.size
    }
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }
    pub fn clear(&mut self) {
        self.cells.fill(Cell::BLANK);
    }

    pub fn set(&mut self, column: u16, row: u16, cell: Cell) {
        if column < self.size.columns && row < self.size.rows {
            self.cells[usize::from(row) * usize::from(self.size.columns) + usize::from(column)] =
                cell;
        }
    }

    pub(crate) fn resize(&mut self, size: GridSize) {
        self.size = size;
        self.cells.resize(size.len(), Cell::BLANK);
        self.clear();
    }
}
