//! Private GPU ABI. Native ownership keeps the CAMetalLayer alive until destroy.
use crate::{BUFFER_TOO_SMALL, INVALID, OK, PANIC, StOptions};
use std::{
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind},
    ptr, slice, str,
    time::Duration,
};
use stillterm_engine::{EffectConfig, Theme, WordFrequency};
use stillterm_gpu::{
    renderer::{Atlas, Renderer},
    scene,
};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct StGpuText {
    pub characters: *const u8,
    pub characters_length: usize,
    pub words: *const u8,
    pub words_length: usize,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct StAtlas {
    pub pixels: *const u8,
    pub length: usize,
    pub width: u32,
    pub height: u32,
    pub tile_width: u32,
    pub tile_height: u32,
}
pub struct StGpu {
    renderer: Renderer,
    poisoned: bool,
}

unsafe fn read_text(text: StGpuText) -> Option<(String, String)> {
    unsafe fn read(pointer: *const u8, length: usize, limit: usize) -> Option<String> {
        if length > limit || (length > 0 && pointer.is_null()) {
            return None;
        }
        if length == 0 {
            return Some(String::new());
        }
        // SAFETY: caller supplies readable bytes; null and maximum length checked above.
        Some(
            str::from_utf8(unsafe { slice::from_raw_parts(pointer, length) })
                .ok()?
                .to_owned(),
        )
    }
    // SAFETY: forwarded caller-owned input buffer contract, bounded in read().
    let characters = unsafe { read(text.characters, text.characters_length, 1024) }?;
    // SAFETY: forwarded caller-owned input buffer contract, bounded in read().
    let words = unsafe { read(text.words, text.words_length, 8192) }?;
    Some((
        if characters.is_empty() {
            Theme::Matrix.rain_defaults().characters.to_owned()
        } else {
            characters
        },
        words,
    ))
}
fn effect(options: StOptions, characters: &str, words: &str) -> Option<EffectConfig> {
    if options.theme != 3 {
        return None;
    }
    let frequency = match options.word_frequency {
        0 => WordFrequency::Rare,
        1 => WordFrequency::Occasional,
        2 => WordFrequency::Frequent,
        _ => return None,
    };
    EffectConfig::new(
        options.speed,
        options.density,
        options.intensity,
        characters,
    )
    .ok()?
    .with_theme_style(Theme::DenseMatrix)
    .with_vertical_fade(false)
    .with_words(words, frequency)
    .ok()
}

/// Fill the native glyph-atlas order; zero means invalid input or insufficient space.
/// # Safety
/// Text buffers must be null at length zero or readable for their byte lengths.
/// Output must be writable for capacity aligned u32 values. No input is retained.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn st_corridor_glyphs(
    text: StGpuText,
    output: *mut u32,
    capacity: usize,
) -> usize {
    catch_unwind(|| {
        // SAFETY: forwarded caller-owned readable buffers; read_text bounds lengths.
        let Some((characters, words)) = (unsafe { read_text(text) }) else {
            return 0;
        };
        let defaults = Theme::Matrix.rain_defaults();
        let options = StOptions {
            theme: 3,
            seed: 42,
            speed: defaults.speed,
            density: defaults.density,
            intensity: defaults.intensity,
            word_frequency: 1,
        };
        if effect(options, &characters, &words).is_none() {
            return 0;
        }
        let glyphs = scene::glyphs(&characters, &words);
        if output.is_null() || glyphs.len() > capacity || glyphs.len() > scene::MAX_GLYPHS {
            return 0;
        }
        for (i, glyph) in glyphs.iter().enumerate() {
            // SAFETY: caller guarantees writable capacity, checked above.
            unsafe {
                output.add(i).write(*glyph as u32);
            }
        }
        glyphs.len()
    })
    .unwrap_or(0)
}

/// Create a main-thread renderer. Null means invalid input or unavailable GPU.
/// # Safety
/// Layer must be a live CAMetalLayer kept alive until destroy. Text follows the
/// glyphs contract; atlas pixels must be readable for length bytes. Inputs are
/// copied during this call. All calls on the returned handle must be serialized.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn st_corridor_create(
    layer: *mut c_void,
    width: u32,
    height: u32,
    options: StOptions,
    text: StGpuText,
    atlas: StAtlas,
    character_size: f64,
) -> *mut StGpu {
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: caller-owned input buffers are valid for this call.
        let (characters, words) = unsafe { read_text(text) }?;
        let config = effect(options, &characters, &words)?;
        let glyphs = scene::glyphs(&characters, &words);
        if layer.is_null()
            || !character_size.is_finite()
            || !(10.0..=48.0).contains(&character_size)
            || atlas.pixels.is_null()
            || atlas.length > 20_000_000
            || atlas.tile_width == 0
            || atlas.tile_height == 0
            || atlas.width != atlas.tile_width.checked_mul(16)?
            || atlas.height
                != (glyphs.len() as u32)
                    .div_ceil(16)
                    .checked_mul(atlas.tile_height)?
            || u64::from(atlas.width) * u64::from(atlas.height) * 4 != atlas.length as u64
        {
            return None;
        }
        // SAFETY: caller provides atlas.length readable bytes, bounded and checked above.
        let pixels = unsafe { slice::from_raw_parts(atlas.pixels, atlas.length) };
        // SAFETY: native owner keeps the live CAMetalLayer for the renderer's lifetime.
        match unsafe {
            Renderer::new(
                layer,
                [width, height],
                Atlas {
                    rgba: pixels,
                    width: atlas.width,
                    height: atlas.height,
                    tile_width: atlas.tile_width,
                    tile_height: atlas.tile_height,
                },
                config,
                &glyphs,
                options.seed,
                (character_size / 18.0) as f32,
            )
        } {
            Ok(renderer) => Some(Box::into_raw(Box::new(StGpu {
                renderer,
                poisoned: false,
            }))),
            Err(error) => {
                eprintln!("Stillterm Matrix 3D: {error}");
                None
            }
        }
    }))
    .ok()
    .flatten()
    .unwrap_or(ptr::null_mut())
}
unsafe fn with_gpu(handle: *mut StGpu, f: impl FnOnce(&mut Renderer) -> i32) -> i32 {
    // SAFETY: caller guarantees a null or live, aligned, exclusive handle.
    let Some(state) = (unsafe { handle.as_mut() }) else {
        return INVALID;
    };
    if state.poisoned {
        return PANIC;
    }
    match catch_unwind(AssertUnwindSafe(|| f(&mut state.renderer))) {
        Ok(result) => result,
        Err(_) => {
            state.poisoned = true;
            PANIC
        }
    }
}
/// Resize a live renderer; invalid sizes leave the existing surface intact.
/// # Safety
/// Handle must be null or live and exclusively owned for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn st_corridor_resize(handle: *mut StGpu, width: u32, height: u32) -> i32 {
    // SAFETY: forwarded caller-owned exclusive handle contract.
    unsafe {
        with_gpu(handle, |gpu| {
            if gpu.resize([width, height]).is_ok() {
                OK
            } else {
                INVALID
            }
        })
    }
}
/// Draw one frame, limiting simulation catch-up to 250 ms.
/// # Safety
/// Handle must be null or live and exclusively owned for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn st_corridor_render(handle: *mut StGpu, elapsed: u64) -> i32 {
    // SAFETY: forwarded caller-owned exclusive handle contract.
    unsafe {
        with_gpu(handle, |gpu| {
            match gpu.render(Duration::from_nanos(elapsed)) {
                Ok(()) => OK,
                Err(error) => {
                    eprintln!("Stillterm Matrix 3D: {error}");
                    INVALID
                }
            }
        })
    }
}
/// Copy an RGBA diagnostic image. Normal rendering never reads GPU pixels back.
/// # Safety
/// Handle is exclusively owned. Output must point to capacity writable bytes,
/// disjoint from the handle. Caller supplies width * height * 4 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn st_corridor_snapshot(
    handle: *mut StGpu,
    output: *mut u8,
    capacity: usize,
) -> i32 {
    // SAFETY: forwarded caller-owned exclusive handle contract.
    unsafe {
        with_gpu(handle, |gpu| match gpu.snapshot() {
            Ok(pixels) => {
                if output.is_null() || capacity < pixels.len() {
                    return BUFFER_TOO_SMALL;
                }
                // SAFETY: caller supplies a disjoint writable buffer, checked to fit above.
                ptr::copy_nonoverlapping(pixels.as_ptr(), output, pixels.len());
                OK
            }
            Err(error) => {
                eprintln!("Stillterm GPU snapshot: {error}");
                INVALID
            }
        })
    }
}
/// Destroy once on the main thread, before releasing the CAMetalLayer.
/// # Safety
/// Non-null handle must be live and exclusively owned, and never used again.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn st_corridor_destroy(handle: *mut StGpu) {
    if !handle.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| {
            // SAFETY: caller transfers the uniquely owned handle exactly once.
            drop(unsafe { Box::from_raw(handle) });
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn glyph_buffers_are_bounded_and_invalid_gpu_handles_are_rejected() {
        // SAFETY: inputs are live buffers or null at length zero; outputs fit.
        unsafe {
            let text = StGpuText {
                characters: ptr::null(),
                characters_length: 0,
                words: "FREE".as_ptr(),
                words_length: 4,
            };
            let mut scalars = [0u32; 1280];
            let count = st_corridor_glyphs(text, scalars.as_mut_ptr(), scalars.len());
            assert!(count > 0 && scalars[..count].contains(&('F' as u32)));
            let mut sentinel = [123u32];
            assert_eq!(st_corridor_glyphs(text, sentinel.as_mut_ptr(), 1), 0);
            assert_eq!(sentinel, [123]);
            let bad = StGpuText {
                characters: ptr::null(),
                characters_length: 1025,
                ..text
            };
            assert_eq!(
                st_corridor_glyphs(bad, scalars.as_mut_ptr(), scalars.len()),
                0
            );
            assert_eq!(st_corridor_render(ptr::null_mut(), 0), INVALID);
            assert_eq!(st_corridor_resize(ptr::null_mut(), 800, 500), INVALID);
            assert_eq!(
                st_corridor_snapshot(ptr::null_mut(), ptr::null_mut(), 0),
                INVALID
            );
            st_corridor_destroy(ptr::null_mut());
        }
    }
}
