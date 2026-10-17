#ifndef STILLTERM_BRIDGE_H
#define STILLTERM_BRIDGE_H
#include <stdint.h>
#include <stddef.h>

// Private ABI: build the header and Rust archive from the same checkout.
// Each handle has one owner. Serialize calls; never use it after destruction.
// Rust copies input strings and writes caller-owned frames. No borrowed storage
// crosses the boundary. Null handles return ST_INVALID; destroy(NULL) is safe.
// Panics poison a handle (destroy it); invalid pointers/OOM are not recoverable.
typedef struct StEngine StEngine;
typedef struct {
    uint32_t theme; // 0 monochrome, 1 Matrix, 2 Dense Matrix, 3 Matrix 3D
    uint64_t seed;
    double speed, density, intensity;
    uint32_t word_frequency; // 0 rare, 1 occasional, 2 frequent
} StOptions;
typedef struct {
    StOptions options;
    double font_scale, offset;
} StDepthLayer;
typedef struct {
    uint32_t scalar; // Unicode scalar, not UTF-16
    uint8_t red, green, blue, visible;
} StCell;
enum { ST_OK = 0, ST_INVALID = -1, ST_BUFFER_TOO_SMALL = -2, ST_PANIC = -3 };
int32_t st_defaults(uint32_t theme, StOptions *output);
// Dense Matrix layers 0..2, distant to near; words belong only to layer 2.
int32_t st_depth_defaults(StOptions base, uint32_t layer, StDepthLayer *output);
// UTF-8 bytes; length 0 selects theme characters. Returns NULL on failure.
StEngine *st_create(uint32_t columns, uint32_t rows, StOptions options,
                    const uint8_t *characters, size_t length);
// Both UTF-8 buffers are copied. Characters: at most 1024 bytes; words: 8192.
// Words are optional newline-separated phrases. NULL is allowed only at length 0.
StEngine *st_create_words(uint32_t columns, uint32_t rows, StOptions options,
    const uint8_t *characters, size_t length, const uint8_t *words, size_t words_length);
int32_t st_resize(StEngine *, uint32_t columns, uint32_t rows);
int32_t st_advance(StEngine *, uint64_t elapsed_ns);
// Capacity in cells, at least columns * rows. Zero-sized frames allow NULL.
int32_t st_copy_frame(StEngine *, StCell *output, size_t capacity);
void st_destroy(StEngine *);
// GPU handles use the same main-thread ownership rules as the native layer.
typedef struct StGpu StGpu;
typedef struct { const uint8_t *characters; size_t characters_length;
    const uint8_t *words; size_t words_length; } StGpuText;
typedef struct { const uint8_t *pixels; size_t length;
    uint32_t width, height, tile_width, tile_height; } StAtlas;
size_t st_corridor_glyphs(StGpuText text, uint32_t *output, size_t capacity);
StGpu *st_corridor_create(void *layer, uint32_t width, uint32_t height,
    StOptions options, StGpuText text, StAtlas atlas, double character_size);
int32_t st_corridor_resize(StGpu *, uint32_t width, uint32_t height);
int32_t st_corridor_render(StGpu *, uint64_t elapsed_ns);
// Diagnostic RGBA pixels only; capacity >= width * height * 4.
int32_t st_corridor_snapshot(StGpu *, uint8_t *output, size_t capacity);
void st_corridor_destroy(StGpu *);
#endif
