//! Private, versioned-with-the-bundle C ABI. See `platforms/macos/StilltermBridge.h`.
//! No Rust allocation or borrowed frame escapes this boundary. Calls on each
//! handle must be serialized. Ordinary unwinding panics poison the handle;
//! invalid pointers and allocation failure cannot be recovered by this API.
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    ptr, slice, str,
    time::Duration,
};
use stillterm_engine::{EffectConfig, Engine, GridSize, StepClock, Theme, WordFrequency};

pub const OK: i32 = 0;
pub const INVALID: i32 = -1;
pub const BUFFER_TOO_SMALL: i32 = -2;
pub const PANIC: i32 = -3;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct StOptions {
    pub theme: u32,
    pub seed: u64,
    pub speed: f64,
    pub density: f64,
    pub intensity: f64,
    pub word_frequency: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StCell {
    pub scalar: u32,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub visible: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct StDepthLayer {
    pub options: StOptions,
    pub font_scale: f64,
    pub offset: f64,
}

/// Dense Matrix presentation layers, ordered from distant to near. Words belong
/// only to the near layer. Separate seeds prevent repeated copies of one sheet.
/// # Safety
/// `output` must be null or a writable, aligned StDepthLayer pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn st_depth_defaults(
    base: StOptions,
    layer: u32,
    output: *mut StDepthLayer,
) -> i32 {
    if base.theme != 2
        || output.is_null()
        || base.word_frequency > 2
        || EffectConfig::new(base.speed, base.density, base.intensity, "01").is_err()
    {
        return INVALID;
    }
    let (font_scale, speed, brightness, offset, salt) = match layer {
        0 => (0.65, 0.5, 0.5, 0.3, 0x6a09e667f3bcc909),
        1 => (0.85, 0.8, 0.8, 0.65, 0xbb67ae8584caa73b),
        2 => (1.15, 1.3, 1.0, 0.0, 0),
        _ => return INVALID,
    };
    let options = StOptions {
        seed: base.seed ^ salt,
        speed: (base.speed * speed).clamp(0.1, 4.0),
        intensity: base.intensity * brightness,
        ..base
    };
    // SAFETY: the caller provides an aligned writable output, checked non-null.
    unsafe {
        output.write(StDepthLayer {
            options,
            font_scale,
            offset,
        });
    }
    OK
}

pub struct StEngine {
    engine: Engine,
    theme: Theme,
    clock: StepClock,
    poisoned: bool,
}

fn theme(value: u32) -> Option<Theme> {
    match value {
        0 => Some(Theme::Monochrome),
        1 => Some(Theme::Matrix),
        2 => Some(Theme::DenseMatrix),
        _ => None,
    }
}

fn size(columns: u32, rows: u32) -> Option<GridSize> {
    GridSize::new(columns.try_into().ok()?, rows.try_into().ok()?).ok()
}

/// Return a theme's numeric defaults. Unknown themes return INVALID.
/// # Safety
/// `output` must be null or a writable, aligned StOptions pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn st_defaults(value: u32, output: *mut StOptions) -> i32 {
    let Some(theme) = theme(value) else {
        return INVALID;
    };
    if output.is_null() {
        return INVALID;
    }
    let defaults = theme.rain_defaults();
    // SAFETY: the caller provides an aligned writable output, checked non-null.
    unsafe {
        output.write(StOptions {
            theme: value,
            seed: 42,
            speed: defaults.speed,
            density: defaults.density,
            intensity: defaults.intensity,
            word_frequency: 1,
        });
    }
    OK
}

/// Create an engine without optional phrases. See `st_create_words`.
/// # Safety
/// The character buffer follows the `st_create_words` contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn st_create(
    columns: u32,
    rows: u32,
    options: StOptions,
    characters: *const u8,
    length: usize,
) -> *mut StEngine {
    // SAFETY: forward the caller's character buffer; no words buffer is read.
    unsafe { st_create_words(columns, rows, options, characters, length, ptr::null(), 0) }
}

/// Create an independently owned engine; null means invalid input or panic.
/// A zero byte length selects the theme's characters; UTF-8 is copied otherwise.
/// # Safety
/// For nonzero length, characters must point to that many readable bytes for this
/// call (maximum 1024). For nonzero words_length, words must point to that many
/// readable UTF-8 bytes (maximum 8192). No pointer is retained.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn st_create_words(
    columns: u32,
    rows: u32,
    options: StOptions,
    characters: *const u8,
    length: usize,
    words: *const u8,
    words_length: usize,
) -> *mut StEngine {
    catch_unwind(AssertUnwindSafe(|| {
        let theme = theme(options.theme)?;
        let size = size(columns, rows)?;
        if length > 1024
            || (length != 0 && characters.is_null())
            || words_length > 8192
            || (words_length != 0 && words.is_null())
        {
            return None;
        }
        let defaults = theme.rain_defaults();
        let characters = if length == 0 {
            defaults.characters
        } else {
            // SAFETY: caller guarantees readable bytes; length is bounded above.
            str::from_utf8(unsafe { slice::from_raw_parts(characters, length) }).ok()?
        };
        let frequency = match options.word_frequency {
            0 => WordFrequency::Rare,
            1 => WordFrequency::Occasional,
            2 => WordFrequency::Frequent,
            _ => return None,
        };
        let words = if words_length == 0 {
            ""
        } else {
            // SAFETY: caller guarantees readable bytes; length is bounded above.
            str::from_utf8(unsafe { slice::from_raw_parts(words, words_length) }).ok()?
        };
        let config = EffectConfig::new(
            options.speed,
            options.density,
            options.intensity,
            characters,
        )
        .map(|config| config.with_theme_style(theme))
        .and_then(|config| config.with_words(words, frequency))
        .ok()?;
        Some(Box::into_raw(Box::new(StEngine {
            engine: Engine::rain(size, config, options.seed),
            theme,
            clock: StepClock::default(),
            poisoned: false,
        })))
    }))
    .ok()
    .flatten()
    .unwrap_or(ptr::null_mut())
}

// The caller must provide a live handle and exclusive access for the whole call.
unsafe fn with_handle(handle: *mut StEngine, f: impl FnOnce(&mut StEngine) -> i32) -> i32 {
    // SAFETY: caller guarantees a live, aligned, exclusive handle, or null.
    let Some(state) = (unsafe { handle.as_mut() }) else {
        return INVALID;
    };
    if state.poisoned {
        return PANIC;
    }
    match catch_unwind(AssertUnwindSafe(|| f(state))) {
        Ok(result) => result,
        Err(_) => {
            state.poisoned = true;
            PANIC
        }
    }
}

/// Resize without replacing surviving rain columns. Resets fractional time.
/// # Safety
/// `handle` must be null or a live handle with exclusive access for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn st_resize(handle: *mut StEngine, columns: u32, rows: u32) -> i32 {
    let Some(size) = size(columns, rows) else {
        return INVALID;
    };
    // SAFETY: forwarded caller contract.
    unsafe {
        with_handle(handle, |state| {
            state.engine.resize(size);
            state.clock.reset();
            OK
        })
    }
}

/// Advance at 30 simulation ticks/second, discarding elapsed time above 250 ms.
/// # Safety
/// `handle` must be null or a live handle with exclusive access for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn st_advance(handle: *mut StEngine, elapsed_ns: u64) -> i32 {
    // SAFETY: forwarded caller contract.
    unsafe {
        with_handle(handle, |state| {
            for _ in 0..state.clock.advance(Duration::from_nanos(elapsed_ns)) {
                state.engine.step();
            }
            OK
        })
    }
}

/// Copy the current frame into caller-owned row-major storage; no partial writes
/// on insufficient capacity. Invisible cells should be painted black.
/// # Safety
/// `handle` must be null or live and exclusively accessible. For nonempty frames,
/// output must point to capacity writable, aligned StCells, disjoint from handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn st_copy_frame(
    handle: *mut StEngine,
    output: *mut StCell,
    capacity: usize,
) -> i32 {
    // SAFETY: forwarded handle and output contracts.
    unsafe {
        with_handle(handle, |state| {
            let count = state.engine.size().len();
            if capacity < count {
                return BUFFER_TOO_SMALL;
            }
            if count == 0 {
                return OK;
            }
            if output.is_null() {
                return INVALID;
            }
            let destination = slice::from_raw_parts_mut(output, count);
            for (destination, cell) in destination.iter_mut().zip(state.engine.frame().cells()) {
                let [red, green, blue] = state.theme.rgb(cell.intensity, cell.emphasis);
                *destination = StCell {
                    scalar: cell.glyph.as_char() as u32,
                    red,
                    green,
                    blue,
                    visible: u8::from(cell.intensity != 0 && cell.glyph.as_char() != ' '),
                };
            }
            OK
        })
    }
}

/// Destroy once; null is a no-op. A poisoned handle can still be destroyed.
/// # Safety
/// A non-null handle must come from st_create, still be live, and have no active
/// calls. It must never be used again after this function returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn st_destroy(handle: *mut StEngine) {
    if !handle.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| {
            // SAFETY: ownership is transferred back exactly once by the caller.
            drop(unsafe { Box::from_raw(handle) });
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn options() -> StOptions {
        let d = Theme::Matrix.rain_defaults();
        StOptions {
            theme: 1,
            seed: 42,
            speed: d.speed,
            density: d.density,
            intensity: d.intensity,
            word_frequency: 1,
        }
    }
    #[test]
    fn frame_matches_rust_engine_and_resizes() {
        // SAFETY: all handles are live, serialized, destroyed once; buffers fit.
        unsafe {
            let handle = st_create(40, 20, options(), ptr::null(), 0);
            assert!(!handle.is_null());
            let mut core = Engine::rain(
                GridSize::new(40, 20).unwrap(),
                Theme::Matrix.rain_config(),
                42,
            );
            for _ in 0..30 {
                assert_eq!(st_advance(handle, 100_000_000), OK);
                for _ in 0..3 {
                    core.step();
                }
            }
            let mut cells = vec![StCell::default(); 800];
            assert_eq!(st_copy_frame(handle, cells.as_mut_ptr(), cells.len()), OK);
            for (native, rust) in cells.iter().zip(core.frame().cells()) {
                assert_eq!(native.scalar, rust.glyph.as_char() as u32);
                assert_eq!(
                    [native.red, native.green, native.blue],
                    Theme::Matrix.rgb(rust.intensity, rust.emphasis)
                );
            }
            assert_eq!(st_resize(handle, 50, 30), OK);
            assert_eq!(
                st_copy_frame(handle, cells.as_mut_ptr(), cells.len()),
                BUFFER_TOO_SMALL
            );
            assert_eq!(st_resize(handle, 0, 0), OK);
            assert_eq!(st_copy_frame(handle, ptr::null_mut(), 0), OK);
            st_destroy(handle);
        }
    }
    #[test]
    fn invalid_input_does_not_create_or_write() {
        // SAFETY: inputs are null or readable/writable buffers of stated size.
        unsafe {
            assert!(st_create(u32::MAX, 1, options(), ptr::null(), 0).is_null());
            assert!(st_create(65535, 65535, options(), ptr::null(), 0).is_null());
            assert!(st_create(1, 1, options(), ptr::null(), 1).is_null());
            assert!(st_create(1, 1, options(), [255].as_ptr(), 1).is_null());
            assert!(st_create(1, 1, options(), "界".as_ptr(), 3).is_null());
            let mut bad = options();
            bad.speed = f64::NAN;
            assert!(st_create(1, 1, bad, ptr::null(), 0).is_null());
            bad = options();
            bad.theme = 99;
            assert!(st_create(1, 1, bad, ptr::null(), 0).is_null());
            assert_eq!(st_advance(ptr::null_mut(), 0), INVALID);
            st_destroy(ptr::null_mut());
        }
    }
    #[test]
    fn dense_theme_defaults_and_long_trails_cross_the_bridge() {
        // SAFETY: serialized live handle and correctly sized caller-owned buffers.
        unsafe {
            let mut options = options();
            assert_eq!(st_defaults(2, &mut options), OK);
            let handle = st_create(120, 60, options, ptr::null(), 0);
            assert!(!handle.is_null());
            let mut core = Engine::rain(
                GridSize::new(120, 60).unwrap(),
                Theme::DenseMatrix.rain_config(),
                42,
            );
            let mut cells = vec![StCell::default(); 7200];
            for _ in 0..30 {
                assert_eq!(st_advance(handle, 100_000_000), OK);
                for _ in 0..3 {
                    core.step();
                }
            }
            assert_eq!(st_copy_frame(handle, cells.as_mut_ptr(), cells.len()), OK);
            for (native, rust) in cells.iter().zip(core.frame().cells()) {
                assert_eq!(native.scalar, rust.glyph.as_char() as u32);
                assert_eq!(
                    [native.red, native.green, native.blue],
                    Theme::DenseMatrix.rgb(rust.intensity, rust.emphasis)
                );
            }
            st_destroy(handle);
        }
    }

    #[test]
    fn depth_layers_are_independent_bounded_and_respect_user_controls() {
        // SAFETY: valid writable output, and null pointers only for rejection checks.
        unsafe {
            let mut base = options();
            assert_eq!(st_defaults(2, &mut base), OK);
            base.seed = u64::MAX;
            let mut layers = Vec::new();
            for layer in 0..3 {
                let mut depth = StDepthLayer {
                    options: base,
                    font_scale: 0.0,
                    offset: 0.0,
                };
                assert_eq!(st_depth_defaults(base, layer, &mut depth), OK);
                assert_eq!(depth.options.density, base.density);
                assert_eq!(depth.options.word_frequency, base.word_frequency);
                assert!((0.1..=4.0).contains(&depth.options.speed));
                layers.push(depth);
            }
            assert!(
                layers
                    .windows(2)
                    .all(|pair| pair[0].font_scale < pair[1].font_scale
                        && pair[0].options.speed < pair[1].options.speed
                        && pair[0].options.intensity < pair[1].options.intensity
                        && pair[0].options.seed != pair[1].options.seed)
            );
            assert_eq!(layers[2].options.seed, base.seed);
            base.speed = 4.0;
            base.density = 0.0;
            base.intensity = 0.0;
            for layer in 0..3 {
                let mut depth = layers[0];
                assert_eq!(st_depth_defaults(base, layer, &mut depth), OK);
                assert_eq!(depth.options.density, 0.0);
                assert_eq!(depth.options.intensity, 0.0);
                assert!(depth.options.speed <= 4.0);
            }
            assert_eq!(st_depth_defaults(base, 3, &mut layers[0]), INVALID);
            assert_eq!(st_depth_defaults(base, 0, ptr::null_mut()), INVALID);
            base.speed = f64::NAN;
            assert_eq!(st_depth_defaults(base, 0, &mut layers[0]), INVALID);
        }
    }
    #[test]
    fn words_cross_the_bridge_and_match_the_core() {
        // SAFETY: buffers are readable for creation; handles and frame storage
        // remain live and exclusively owned, and the handle is destroyed once.
        unsafe {
            let text = "anything,wake up\nFREE";
            let handle =
                st_create_words(40, 20, options(), ptr::null(), 0, text.as_ptr(), text.len());
            assert!(!handle.is_null());
            let mut core = Engine::rain(
                GridSize::new(40, 20).unwrap(),
                Theme::Matrix
                    .rain_config()
                    .with_words(text, WordFrequency::Occasional)
                    .unwrap(),
                42,
            );
            let mut cells = vec![StCell::default(); 800];
            for _ in 0..160 {
                assert_eq!(st_advance(handle, 100_000_000), OK);
                for _ in 0..3 {
                    core.step();
                }
                assert_eq!(st_copy_frame(handle, cells.as_mut_ptr(), cells.len()), OK);
                for (native, rust) in cells.iter().zip(core.frame().cells()) {
                    assert_eq!(native.scalar, rust.glyph.as_char() as u32);
                    assert_eq!(
                        [native.red, native.green, native.blue],
                        Theme::Matrix.rgb(rust.intensity, rust.emphasis)
                    );
                }
            }
            st_destroy(handle);
            for (bytes, length) in [
                (ptr::null(), 1),
                ([255].as_ptr(), 1),
                ("界".as_ptr(), 3),
                (ptr::null(), 8193),
            ] {
                assert!(st_create_words(1, 1, options(), ptr::null(), 0, bytes, length).is_null());
            }
            let mut invalid = options();
            invalid.word_frequency = 99;
            assert!(st_create_words(1, 1, invalid, ptr::null(), 0, ptr::null(), 0).is_null());
        }
    }

    #[test]
    fn panic_is_contained_and_handle_is_poisoned() {
        // SAFETY: exclusively owned handle, destroyed once after poisoning.
        unsafe {
            let handle = st_create(1, 1, options(), ptr::null(), 0);
            assert_eq!(
                with_handle(handle, |_| panic!("injected bridge panic")),
                PANIC
            );
            assert_eq!(st_advance(handle, 1), PANIC);
            st_destroy(handle);
        }
    }
}
