#import "StilltermCorridor.h"
#import <QuartzCore/CAMetalLayer.h>
#import <CoreText/CoreText.h>
#include <math.h>

@implementation STCorridorView {
    StGpu *_gpu;
    StilltermSettings *_settings;
    uint32_t _pixelWidth, _pixelHeight;
    BOOL _ready;
}
- (instancetype)initWithFrame:(NSRect)frame settings:(StilltermSettings *)settings {
    if ((self = [super initWithFrame:frame])) {
        _settings = settings;
        self.wantsLayer = YES;
        self.layerContentsRedrawPolicy = NSViewLayerContentsRedrawNever;
        self.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    }
    return self;
}
- (CALayer *)makeBackingLayer { return [CAMetalLayer layer]; }
- (BOOL)wantsUpdateLayer { return YES; }
- (void)updateLayer { /* Rust presents directly to the CAMetalLayer. */ }
- (BOOL)isOpaque { return YES; }
- (uint32_t)pixelWidth { return _pixelWidth; }
- (uint32_t)pixelHeight { return _pixelHeight; }
- (BOOL)ready { return _ready; }
- (BOOL)createGPU {
    NSData *characters = [_settings.characters dataUsingEncoding:NSUTF8StringEncoding];
    NSData *words = [_settings.words dataUsingEncoding:NSUTF8StringEncoding];
    StGpuText text = {characters.bytes, characters.length, words.bytes, words.length};
    uint32_t scalars[1280];
    size_t count = st_corridor_glyphs(text, scalars, 1280);
    if (!count) return NO;
    uint32_t tileWidth = 32, tileHeight = 48;
    uint32_t width = tileWidth * 16, height = (uint32_t)((count + 15) / 16) * tileHeight;
    NSMutableData *pixels = [NSMutableData dataWithLength:(NSUInteger)width * height * 4];
    CGColorSpaceRef colorSpace = CGColorSpaceCreateDeviceRGB();
    CGContextRef context = CGBitmapContextCreate(pixels.mutableBytes, width, height, 8, width * 4,
        colorSpace, (CGBitmapInfo)kCGImageAlphaPremultipliedLast);
    CGColorSpaceRelease(colorSpace);
    if (!context) return NO;
    CGContextSetRGBFillColor(context, 1, 1, 1, 1);
    CGContextSetTextMatrix(context, CGAffineTransformIdentity);
    NSFont *font = [NSFont monospacedSystemFontOfSize:28 weight:NSFontWeightRegular];
    for (size_t i = 0; i < count; i++) {
        NSString *glyph = [[NSString alloc] initWithBytes:&scalars[i] length:4 encoding:NSUTF32LittleEndianStringEncoding];
        NSAttributedString *attributed = [[NSAttributedString alloc] initWithString:glyph attributes:@{
            NSFontAttributeName:font, (__bridge NSString *)kCTForegroundColorFromContextAttributeName:@YES}];
        CTLineRef line = CTLineCreateWithAttributedString((__bridge CFAttributedStringRef)attributed);
        double advance = CTLineGetTypographicBounds(line, NULL, NULL, NULL);
        CGContextSetTextPosition(context, (i % 16) * tileWidth + (tileWidth - advance) / 2,
            (i / 16) * tileHeight + 11);
        CTLineDraw(line, context); CFRelease(line);
    }
    CGContextRelease(context);
    StAtlas atlas = {pixels.bytes, pixels.length, width, height, tileWidth, tileHeight};
    _gpu = st_corridor_create((__bridge void *)self.layer, _pixelWidth, _pixelHeight,
        _settings.options, text, atlas, _settings.fontSize);
    return _gpu != NULL;
}
- (BOOL)renderElapsed:(uint64_t)elapsed {
    double width = NSWidth(self.bounds), height = NSHeight(self.bounds);
    if (!isfinite(width) || !isfinite(height) || width < 0 || height < 0 ||
        width > 1000000 || height > 1000000 || !isfinite(width * height)) return NO;
    if (!width || !height) {
        [self stopRendering];
        return YES;
    }
    double backing = self.window.backingScaleFactor ?: 1;
    double scale = MIN(backing, sqrt(2073600.0 / (width * height)));
    uint32_t pixelsWide = (uint32_t)MAX(1, floor(width * scale));
    uint32_t pixelsHigh = (uint32_t)MAX(1, floor(height * scale));
    if (_pixelWidth != pixelsWide || _pixelHeight != pixelsHigh) {
        _pixelWidth = pixelsWide; _pixelHeight = pixelsHigh;
        if (_gpu && st_corridor_resize(_gpu, _pixelWidth, _pixelHeight) != ST_OK) return NO;
    }
    if (!_gpu && ![self createGPU]) return NO;
    if (st_corridor_render(_gpu, elapsed) != ST_OK) return NO;
    _ready = YES;
    return YES;
}
- (NSData *)snapshot {
    if (!_gpu || !_ready) return nil;
    NSMutableData *pixels = [NSMutableData dataWithLength:(NSUInteger)_pixelWidth * _pixelHeight * 4];
    return st_corridor_snapshot(_gpu, pixels.mutableBytes, pixels.length) == ST_OK ? pixels : nil;
}
- (void)stopRendering {
    st_corridor_destroy(_gpu); _gpu = NULL; _ready = NO; _pixelWidth = 0; _pixelHeight = 0;
}
- (void)dealloc { [self stopRendering]; }
@end
