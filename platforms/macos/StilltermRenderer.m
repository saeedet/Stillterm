#import "StilltermRenderer.h"
#import <CoreText/CoreText.h>
#include <math.h>

// Dense frames share fonts and colors across thousands of cells. Draw each
// font/color group in one Core Text call, retaining buffers between frames.
@interface STGlyphBatch : NSObject
@property(nonatomic, strong) id font;
@property(nonatomic, strong) NSMutableData *glyphs, *positions;
@property(nonatomic) NSUInteger count;
@property(nonatomic) uint32_t color;
- (void)addGlyph:(CGGlyph)glyph at:(CGPoint)position;
@end
@implementation STGlyphBatch
- (void)addGlyph:(CGGlyph)glyph at:(CGPoint)position {
    if (self.count >= self.glyphs.length / sizeof(CGGlyph)) {
        NSUInteger capacity = MAX(32, self.count * 2);
        if (!self.glyphs) self.glyphs = [NSMutableData data];
        if (!self.positions) self.positions = [NSMutableData data];
        self.glyphs.length = capacity * sizeof(CGGlyph);
        self.positions.length = capacity * sizeof(CGPoint);
    }
    ((CGGlyph *)self.glyphs.mutableBytes)[self.count] = glyph;
    ((CGPoint *)self.positions.mutableBytes)[self.count++] = position;
}
@end
@interface STGlyphFont : NSObject
@property(nonatomic, strong) id font;
@property(nonatomic, strong) NSMutableDictionary<NSNumber *, STGlyphBatch *> *colors;
@end
@implementation STGlyphFont
@end
@interface STGlyphInfo : NSObject
@property(nonatomic) CGGlyph glyph;
@property(nonatomic) CGPoint offset;
@property(nonatomic, strong) STGlyphFont *font;
@end
@implementation STGlyphInfo
@end

@implementation STRainLayer {
    StEngine *_engine;
    StOptions _options;
    NSString *_characters, *_words;
    NSMutableData *_cells, *_nextCells;
    NSMutableDictionary<NSNumber *, id> *_lines;
    NSMutableDictionary<NSNumber *, STGlyphInfo *> *_denseGlyphs;
    NSMutableArray<STGlyphFont *> *_denseFonts;
    NSMutableArray<STGlyphBatch *> *_denseBatches;
    NSFont *_font;
    uint32_t _columns, _rows;
    CGFloat _cellWidth, _cellHeight, _fontScale, _offset;
    NSRect _bounds;
    BOOL _changed;
}
- (instancetype)initWithOptions:(StOptions)options fontScale:(CGFloat)fontScale
    offset:(CGFloat)offset characters:(NSString *)characters words:(NSString *)words {
    if ((self = [super init])) {
        _options = options; _fontScale = fontScale; _offset = offset;
        _characters = [characters copy]; _words = [words copy];
    }
    return self;
}
- (NSData *)cells { return _cells; }
- (uint32_t)columns { return _columns; }
- (uint32_t)rows { return _rows; }
- (CGFloat)fontScale { return _fontScale; }
- (BOOL)changed { return _changed; }
- (BOOL)prepareBounds:(NSRect)bounds pointSize:(CGFloat)pointSize {
    _bounds = bounds;
    if (!_font || _font.pointSize != pointSize) {
        _font = [NSFont monospacedSystemFontOfSize:pointSize weight:NSFontWeightRegular];
        _cellWidth = ceil([@"M" sizeWithAttributes:@{NSFontAttributeName: _font}].width);
        _cellHeight = ceil(_font.ascender - _font.descender + _font.leading + (_options.theme == 2 ? 0 : 2));
        _lines = [NSMutableDictionary dictionary];
        _denseGlyphs = [NSMutableDictionary dictionary];
        _denseFonts = [NSMutableArray array]; _denseBatches = [NSMutableArray array];
    }
    double columns = ceil(MAX(0, NSWidth(bounds)) / _cellWidth);
    double rows = ceil(MAX(0, NSHeight(bounds)) / _cellHeight);
    if (!isfinite(columns) || !isfinite(rows) || columns > UINT16_MAX || rows > UINT16_MAX || columns * rows > 262144) return NO;
    uint32_t width = (uint32_t)columns, height = (uint32_t)rows;
    if (!_engine) {
        NSData *characters = [_characters dataUsingEncoding:NSUTF8StringEncoding];
        NSData *words = [_words dataUsingEncoding:NSUTF8StringEncoding];
        _engine = st_create_words(width, height, _options, characters.bytes, characters.length, words.bytes, words.length);
        if (!_engine) return NO;
    } else if ((_columns != width || _rows != height) && st_resize(_engine, width, height) != ST_OK) return NO;
    if (!_cells || _columns != width || _rows != height) {
        _columns = width; _rows = height;
        NSUInteger bytes = (NSUInteger)width * height * sizeof(StCell);
        _cells = [NSMutableData dataWithLength:bytes]; _nextCells = [NSMutableData dataWithLength:bytes];
    }
    return YES;
}
- (BOOL)advance:(uint64_t)elapsed canvas:(NSView *)canvas {
    if (st_advance(_engine, elapsed) != ST_OK ||
        st_copy_frame(_engine, _nextCells.mutableBytes, (NSUInteger)_columns * _rows) != ST_OK) return NO;
    const StCell *previous = _cells.bytes, *next = _nextCells.bytes;
    _changed = NO;
    if (_columns == 0 || _rows == 0) return YES;
    for (uint32_t row = 0; row < _rows; row++) {
        if (memcmp(previous + row * _columns, next + row * _columns, _columns * sizeof(StCell)) != 0) {
            _changed = YES;
            [canvas setNeedsDisplayInRect:NSMakeRect(0, NSHeight(_bounds) - (row + 1) * _cellHeight,
                NSWidth(_bounds), _cellHeight)];
        }
    }
    NSMutableData *swap = _cells; _cells = _nextCells; _nextCells = swap;
    return YES;
}
- (CTLineRef)lineForScalar:(uint32_t)scalar {
    NSNumber *key = @(scalar);
    id cached = _lines[key];
    if (!cached) {
        UTF32Char character = scalar;
        NSString *text = [[NSString alloc] initWithBytes:&character length:sizeof(character) encoding:NSUTF32LittleEndianStringEncoding];
        NSAttributedString *attributed = [[NSAttributedString alloc] initWithString:text ?: @"?" attributes:@{
            NSFontAttributeName: _font, (__bridge NSString *)kCTForegroundColorFromContextAttributeName: @YES}];
        cached = CFBridgingRelease(CTLineCreateWithAttributedString((__bridge CFAttributedStringRef)attributed));
        _lines[key] = cached;
    }
    return (__bridge CTLineRef)cached;
}
- (STGlyphInfo *)denseGlyphForScalar:(uint32_t)scalar {
    NSNumber *key = @(scalar);
    STGlyphInfo *info = _denseGlyphs[key];
    if (info) return info;
    info = [STGlyphInfo new]; _denseGlyphs[key] = info;
    CTLineRef line = [self lineForScalar:scalar];
    CFArrayRef runs = CTLineGetGlyphRuns(line);
    if (CFArrayGetCount(runs) != 1) return info;
    CTRunRef run = CFArrayGetValueAtIndex(runs, 0);
    if (CTRunGetGlyphCount(run) != 1 || !CGAffineTransformIsIdentity(CTRunGetTextMatrix(run))) return info;
    CTFontRef font = CFDictionaryGetValue(CTRunGetAttributes(run), kCTFontAttributeName);
    if (!font) return info;
    CGGlyph glyph; CGPoint position;
    CTRunGetGlyphs(run, CFRangeMake(0, 1), &glyph);
    CTRunGetPositions(run, CFRangeMake(0, 1), &position);
    position.x += (_cellWidth - CTLineGetTypographicBounds(line, NULL, NULL, NULL)) / 2;
    position.y += -_font.descender + 1;
    CGRect bounds = CTFontGetBoundingRectsForGlyphs(font, kCTFontOrientationDefault, &glyph, NULL, 1);
    bounds = CGRectOffset(bounds, position.x, position.y);
    // Keep per-cell clipping for unusual fallback glyphs that cross a cell edge.
    if (CGRectGetMinX(bounds) < 0 || CGRectGetMaxX(bounds) > _cellWidth ||
        CGRectGetMinY(bounds) < 0 || CGRectGetMaxY(bounds) > _cellHeight) return info;
    STGlyphFont *group;
    for (STGlyphFont *candidate in _denseFonts) {
        if (CFEqual((__bridge CTFontRef)candidate.font, font)) { group = candidate; break; }
    }
    if (!group) {
        group = [STGlyphFont new]; group.font = (__bridge id)font;
        group.colors = [NSMutableDictionary dictionary]; [_denseFonts addObject:group];
    }
    info.font = group; info.glyph = glyph; info.offset = position;
    return info;
}
- (void)drawWithContext:(CGContextRef)context canvas:(NSView *)canvas batching:(BOOL)batching glow:(BOOL)glow {
    if (!_engine || !_cells || !_font) return;
    CGContextSaveGState(context);
    CGContextSetTextMatrix(context, CGAffineTransformIdentity);
    BOOL dense = _options.theme == 2 && batching;
    for (STGlyphBatch *batch in _denseBatches) batch.count = 0;
    const StCell *cells = _cells.bytes;
    for (uint32_t row = 0; row < _rows; row++) {
        CGFloat y = NSHeight(_bounds) - (row + 1) * _cellHeight;
        if (canvas && ![canvas needsToDrawRect:NSMakeRect(0, y, NSWidth(_bounds), _cellHeight)]) continue;
        for (uint32_t column = 0; column < _columns; column++) {
            StCell cell = cells[row * _columns + column];
            if (!cell.visible) continue;
            NSRect box = NSMakeRect((column + _offset) * _cellWidth, y, _cellWidth, _cellHeight);
            if (canvas && ![canvas needsToDrawRect:box]) continue;
            if (dense) {
                STGlyphInfo *glyph = [self denseGlyphForScalar:cell.scalar];
                if (glyph.font) {
                    uint32_t color = ((uint32_t)cell.red << 16) | ((uint32_t)cell.green << 8) | cell.blue;
                    STGlyphBatch *batch = glyph.font.colors[@(color)];
                    if (!batch) {
                        batch = [STGlyphBatch new]; batch.font = glyph.font.font; batch.color = color;
                        glyph.font.colors[@(color)] = batch; [_denseBatches addObject:batch];
                    }
                    [batch addGlyph:glyph.glyph at:CGPointMake(box.origin.x + glyph.offset.x, y + glyph.offset.y)];
                    continue;
                }
            }
            CGContextSaveGState(context);
            CGContextClipToRect(context, NSRectToCGRect(box));
            CGContextSetRGBFillColor(context, cell.red / 255.0, cell.green / 255.0, cell.blue / 255.0, 1);
            CTLineRef line = [self lineForScalar:cell.scalar];
            double width = CTLineGetTypographicBounds(line, NULL, NULL, NULL);
            CGContextSetTextPosition(context, box.origin.x + (_cellWidth - width) / 2, y - _font.descender + 1);
            if (glow && cell.red > 100) {
                CGFloat rgba[] = {0.3, 1, 0.35, 0.7};
                CGColorRef color = CGColorCreateGenericRGB(rgba[0], rgba[1], rgba[2], rgba[3]);
                CGContextSetShadowWithColor(context, CGSizeZero, 2.0, color); CGColorRelease(color);
            }
            CTLineDraw(line, context);
            CGContextRestoreGState(context);
        }
    }
    for (STGlyphBatch *batch in _denseBatches) {
        if (!batch.count) continue;
        CGContextSetRGBFillColor(context, (batch.color >> 16) / 255.0,
            ((batch.color >> 8) & 255) / 255.0, (batch.color & 255) / 255.0, 1);
        CGContextSaveGState(context);
        if (glow && (batch.color >> 16) > 100) {
            CGColorRef color = CGColorCreateGenericRGB(0.3, 1, 0.35, 0.7);
            CGContextSetShadowWithColor(context, CGSizeZero, 2.0, color); CGColorRelease(color);
        }
        CTFontDrawGlyphs((__bridge CTFontRef)batch.font, batch.glyphs.bytes, batch.positions.bytes, batch.count, context);
        CGContextRestoreGState(context);
    }
    CGContextRestoreGState(context);
}
- (void)dealloc { st_destroy(_engine); }
@end
