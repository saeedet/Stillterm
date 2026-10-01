//! Owned GDI back buffer. Changed cells are painted offscreen, then copied once.
use std::ptr::{null, null_mut};
use stillterm_engine::{Cell, Frame, GridSize, Theme};
use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*};

pub struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    old_bitmap: HGDIOBJ,
    font: HFONT,
    old_font: HGDIOBJ,
    pub width: i32,
    pub height: i32,
    cell_width: i32,
    cell_height: i32,
    pub size: GridSize,
    previous: Vec<Cell>,
}
impl Surface {
    pub fn new(width: i32, height: i32, font_size: i32) -> Result<Self, String> {
        if width <= 0 || height <= 0 || i64::from(width) * i64::from(height) > 16_777_216 {
            return Err("Display surface is empty or exceeds 16 million pixels".into());
        }
        // SAFETY: DCs and objects are created here, validated, and released by Drop.
        // Only GDI calls (no window messages) run while the surface is borrowed.
        unsafe {
            let screen = GetDC(null_mut());
            if screen.is_null() {
                return Err("Cannot acquire display context".into());
            }
            let dc = CreateCompatibleDC(screen);
            let bitmap = CreateCompatibleBitmap(screen, width, height);
            ReleaseDC(null_mut(), screen);
            let font = CreateFontW(
                -font_size,
                0,
                0,
                0,
                FW_NORMAL as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32,
                CLIP_DEFAULT_PRECIS as u32,
                ANTIALIASED_QUALITY as u32,
                FIXED_PITCH as u32,
                super::wide("Consolas").as_ptr(),
            );
            let mut surface = Self {
                dc,
                bitmap,
                font,
                old_bitmap: null_mut(),
                old_font: null_mut(),
                width,
                height,
                cell_width: 0,
                cell_height: 0,
                size: GridSize::new(0, 0).unwrap(),
                previous: vec![],
            };
            if dc.is_null() || bitmap.is_null() || font.is_null() {
                return Err("Cannot allocate drawing resources".into());
            }
            surface.old_bitmap = SelectObject(dc, bitmap);
            surface.old_font = SelectObject(dc, font);
            if surface.old_bitmap.is_null() || surface.old_font.is_null() {
                return Err("Cannot select drawing resources".into());
            }
            let mut metrics: TEXTMETRICW = std::mem::zeroed();
            if GetTextMetricsW(dc, &mut metrics) == 0 {
                return Err("Cannot measure font".into());
            }
            surface.cell_width = metrics.tmAveCharWidth.max(1);
            surface.cell_height = (metrics.tmHeight + 2).max(1);
            let columns = (width + surface.cell_width - 1) / surface.cell_width;
            let rows = (height + surface.cell_height - 1) / surface.cell_height;
            surface.size = GridSize::new(
                u16::try_from(columns).map_err(|_| "Too many columns")?,
                u16::try_from(rows).map_err(|_| "Too many rows")?,
            )
            .map_err(|e| e.to_string())?;
            surface.previous = vec![Cell::BLANK; surface.size.len()];
            SetBkColor(dc, 0);
            SetBkMode(dc, OPAQUE as i32);
            PatBlt(dc, 0, 0, width, height, BLACKNESS);
            Ok(surface)
        }
    }
    pub fn update(&mut self, frame: &Frame, theme: Theme) -> bool {
        assert_eq!(frame.size(), self.size);
        let mut changed = false;
        for (index, cell) in frame.cells().iter().enumerate() {
            if self.previous[index] == *cell {
                continue;
            }
            let x = index as i32 % i32::from(self.size.columns()) * self.cell_width;
            let y = index as i32 / i32::from(self.size.columns()) * self.cell_height;
            let rect = RECT {
                left: x,
                top: y,
                right: x + self.cell_width,
                bottom: y + self.cell_height,
            };
            let [r, g, b] = theme.rgb(cell.intensity, cell.emphasis);
            let mut utf16 = [0u16; 2];
            let glyph = cell.glyph.as_char().encode_utf16(&mut utf16);
            // SAFETY: this surface owns a live DC; glyph and rectangle outlive the call.
            unsafe {
                SetTextColor(
                    self.dc,
                    u32::from(r) | u32::from(g) << 8 | u32::from(b) << 16,
                );
                ExtTextOutW(
                    self.dc,
                    x,
                    y,
                    ETO_OPAQUE | ETO_CLIPPED,
                    &rect,
                    glyph.as_ptr(),
                    glyph.len() as u32,
                    null(),
                );
            }
            self.previous[index] = *cell;
            changed = true;
        }
        changed
    }
    pub fn paint(&self, target: HDC) {
        // SAFETY: target is the live BeginPaint DC; our memory DC remains owned.
        unsafe {
            BitBlt(
                target,
                0,
                0,
                self.width,
                self.height,
                self.dc,
                0,
                0,
                SRCCOPY,
            );
        }
    }
}
impl Drop for Surface {
    fn drop(&mut self) {
        // SAFETY: restore original selections before deleting owned GDI objects.
        unsafe {
            if !self.old_font.is_null() {
                SelectObject(self.dc, self.old_font);
            }
            if !self.old_bitmap.is_null() {
                SelectObject(self.dc, self.old_bitmap);
            }
            if !self.font.is_null() {
                DeleteObject(self.font);
            }
            if !self.bitmap.is_null() {
                DeleteObject(self.bitmap);
            }
            if !self.dc.is_null() {
                DeleteDC(self.dc);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stillterm_engine::Engine;
    #[test]
    fn gdi_draws_matrix_and_skips_unchanged_cells() {
        let mut surface = Surface::new(320, 200, 18).unwrap();
        let mut engine = Engine::rain(surface.size, Theme::Matrix.rain_config(), 42);
        for _ in 0..120 {
            engine.step();
        }
        let frame = engine.frame();
        assert!(surface.update(frame, Theme::Matrix));
        assert!(!surface.update(frame, Theme::Matrix));
        let mut green = 0;
        // SAFETY: reads from the test's owned memory DC, within bitmap bounds.
        unsafe {
            for y in 0..200 {
                for x in 0..320 {
                    let rgb = GetPixel(surface.dc, x, y);
                    let r = rgb & 255;
                    let g = (rgb >> 8) & 255;
                    if g > 20 && g > r * 2 {
                        green += 1;
                    }
                }
            }
        }
        assert!(green > 100, "Expected green glyphs, found {green} pixels");
        assert!(Surface::new(0, 10, 18).is_err());
        assert!(Surface::new(i32::MAX, i32::MAX, 18).is_err());
    }
}
