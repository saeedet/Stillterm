#import "StilltermView.h"
#import "StilltermSettings.h"
#import <CoreText/CoreText.h>
#import <QuartzCore/QuartzCore.h>
#include <math.h>
#import <os/log.h>

// Sonoma can retain full-screen views without calling stopAnimation. This
// undocumented notification is a best-effort cleanup signal, not a visibility
// or lock-state test. Only our own animation and resources are stopped.
static NSString *const STScreenSaverWillStop = @"com.apple.screensaver.willstop";
static NSString *const STDefaultsDomain = @"io.github.saeedet.Stillterm";

@interface StilltermView (STDrawing)
- (void)drawFrameInRect:(NSRect)rect canvas:(NSView *)canvas;
@end

// macOS may retain the ScreenSaverView after dismissal. Keep its root layer
// bitmap-free, and give the drawing surface a lifetime we can end on stop.
@interface StilltermCanvasView : NSView
@property(nonatomic, weak) StilltermView *owner;
@end
@implementation StilltermCanvasView
- (BOOL)isOpaque { return YES; }
- (void)drawRect:(NSRect)rect { [self.owner drawFrameInRect:rect canvas:self]; }
@end

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

@implementation StilltermView {
    StEngine *_engine;
    StilltermCanvasView *_canvas;
    StilltermSettings *_settings;
    StilltermOptionsController *_optionsController;
    id _dismissObserver;
    NSMutableData *_cells, *_nextCells;
    NSMutableDictionary<NSNumber *, id> *_lines;
    NSMutableDictionary<NSNumber *, STGlyphInfo *> *_denseGlyphs;
    NSMutableArray<STGlyphFont *> *_denseFonts;
    NSMutableArray<STGlyphBatch *> *_denseBatches;
    NSFont *_font;
    uint32_t _columns, _rows;
    CGFloat _cellWidth, _cellHeight;
    CFTimeInterval _lastTime;
    NSUInteger _drawnFrames;
    BOOL _running, _sleeping, _failed, _reportedFrame;
    BOOL _disableBatchDrawing; // Native host checks compare against the clipped line path.
}
- (instancetype)initWithFrame:(NSRect)frame isPreview:(BOOL)isPreview {
    if ((self = [super initWithFrame:frame isPreview:isPreview])) {
        self.wantsLayer = YES;
        self.layerContentsRedrawPolicy = NSViewLayerContentsRedrawOnSetNeedsDisplay;
        [self reloadSettings];
        NSNotificationCenter *center = NSWorkspace.sharedWorkspace.notificationCenter;
        [center addObserver:self selector:@selector(willSleep:) name:NSWorkspaceWillSleepNotification object:nil];
        [center addObserver:self selector:@selector(didWake:) name:NSWorkspaceDidWakeNotification object:nil];
        if (!isPreview) {
            __weak StilltermView *weakSelf = self;
            _dismissObserver = [NSDistributedNotificationCenter.defaultCenter
                addObserverForName:STScreenSaverWillStop object:nil queue:NSOperationQueue.mainQueue
                usingBlock:^(NSNotification *notification) { [weakSelf screenSaverWillStop:notification]; }];
        }
    }
    return self;
}
- (BOOL)isOpaque { return YES; }
- (BOOL)wantsUpdateLayer { return YES; }
- (void)updateLayer {
    self.layer.backgroundColor = NSColor.blackColor.CGColor;
}
- (BOOL)hasConfigureSheet { return YES; }
- (void)releaseEngine {
    [_canvas removeFromSuperview];
    _canvas = nil;
    st_destroy(_engine); _engine = NULL;
    _cells = nil; _nextCells = nil; _lines = nil; _font = nil;
    _denseGlyphs = nil; _denseFonts = nil; _denseBatches = nil;
    _columns = 0; _rows = 0; _lastTime = 0; _reportedFrame = NO;
}
- (void)reloadSettings {
    [self releaseEngine];
    _settings = [[StilltermSettings alloc] initWithDefaults:
        [ScreenSaverDefaults defaultsForModuleWithName:STDefaultsDomain]];
    self.animationTimeInterval = 1.0 / _settings.fps;
    _failed = NO;
    self.needsDisplay = YES;
}
- (NSWindow *)configureSheet {
    // Fresh values when System Settings opens the sheet again.
    [self reloadSettings];
    __weak StilltermView *weakSelf = self;
    _optionsController = [[StilltermOptionsController alloc] initWithSettings:_settings didApply:^{
        [weakSelf reloadSettings];
    }];
    return _optionsController.window;
}
- (void)startAnimation {
    if (_running) return;
    [self reloadSettings];
    _running = YES;
    os_log(OS_LOG_DEFAULT, "Stillterm start: build=%{public}@ theme=%u preview=%d windowVisible=%d occlusion=%lu",
        [[NSBundle bundleForClass:StilltermView.class] objectForInfoDictionaryKey:@"CFBundleVersion"],
        _settings.options.theme, self.isPreview, self.window.visible, (unsigned long)self.window.occlusionState);
    [super startAnimation];
}
- (void)stopAnimation {
    // Apple may deliver one more animation callback after this method.
    if (_running) os_log(OS_LOG_DEFAULT, "Stillterm stop: preview=%d", self.isPreview);
    _running = NO;
    [super stopAnimation];
    [self releaseEngine];
    self.needsDisplay = YES;
}
- (void)screenSaverWillStop:(NSNotification *)notification {
    (void)notification;
    // The observer delivers on the main queue. Do not resume old
    // instances on a global start/wake signal: only startAnimation may do that.
    if (self.isPreview || !_running) return;
    os_log(OS_LOG_DEFAULT, "Stillterm dismissal: stopping retained full-screen view");
    [self stopAnimation];
}
- (void)willSleep:(NSNotification *)notification {
    (void)notification; _sleeping = YES; [self releaseEngine];
}
- (void)didWake:(NSNotification *)notification {
    (void)notification; _sleeping = NO; _lastTime = 0;
}
- (void)viewDidMoveToWindow {
    [super viewDidMoveToWindow];
    if (!self.window) [self releaseEngine];
    _lastTime = 0;
}
- (void)setFrameSize:(NSSize)size {
    [super setFrameSize:size];
    _font = nil; _lines = nil;
    _denseGlyphs = nil; _denseFonts = nil; _denseBatches = nil;
    _lastTime = 0;
    self.needsDisplay = YES;
    _canvas.needsDisplay = YES;
}
- (BOOL)prepareFrame {
    if (!isfinite(NSWidth(self.bounds)) || !isfinite(NSHeight(self.bounds))) return NO;
    if (!_font) {
        // Scale small surfaces by their actual size, even if the host mislabels previews.
        CGFloat scale = MIN(1.0, MIN(NSWidth(self.bounds) / 800.0, NSHeight(self.bounds) / 500.0));
        CGFloat pointSize = MAX(7.0, _settings.fontSize * scale);
        _font = [NSFont monospacedSystemFontOfSize:pointSize weight:NSFontWeightRegular];
        _cellWidth = ceil([@"M" sizeWithAttributes:@{NSFontAttributeName: _font}].width);
        _cellHeight = ceil(_font.ascender - _font.descender + _font.leading + 2);
        _lines = [NSMutableDictionary dictionary];
        _denseGlyphs = [NSMutableDictionary dictionary];
        _denseFonts = [NSMutableArray array]; _denseBatches = [NSMutableArray array];
    }
    double columns = ceil(MAX(0, NSWidth(self.bounds)) / _cellWidth);
    double rows = ceil(MAX(0, NSHeight(self.bounds)) / _cellHeight);
    if (!isfinite(columns) || !isfinite(rows) || columns > UINT16_MAX || rows > UINT16_MAX || columns * rows > 262144) return NO;
    uint32_t width = (uint32_t)columns, height = (uint32_t)rows;
    if (!_engine) {
        NSData *characters = [_settings.characters dataUsingEncoding:NSUTF8StringEncoding];
        NSData *words = [_settings.words dataUsingEncoding:NSUTF8StringEncoding];
        _engine = st_create_words(width, height, _settings.options, characters.bytes, characters.length, words.bytes, words.length);
        if (!_engine) return NO;
    } else if ((_columns != width || _rows != height) && st_resize(_engine, width, height) != ST_OK) return NO;
    if (!_cells || _columns != width || _rows != height) {
        _columns = width; _rows = height;
        NSUInteger bytes = (NSUInteger)width * height * sizeof(StCell);
        _cells = [NSMutableData dataWithLength:bytes]; _nextCells = [NSMutableData dataWithLength:bytes];
        _lastTime = 0; _canvas.needsDisplay = YES;
    }
    if (!_canvas && width > 0 && height > 0) {
        _canvas = [[StilltermCanvasView alloc] initWithFrame:self.bounds];
        _canvas.owner = self;
        _canvas.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        _canvas.wantsLayer = YES;
        [self addSubview:_canvas];
    }
    return YES;
}
- (void)animateOneFrame {
    if (!_running || _sleeping || _failed) return;
    // macOS can present a remote surface whose NSWindow is neither visible nor
    // unoccluded locally. Host lifecycle and the dismissal fallback control
    // animation; explicit view hiding or detachment also suppresses callbacks.
    if (!self.window || self.hiddenOrHasHiddenAncestor) {
        [self releaseEngine]; return;
    }
    if (![self prepareFrame]) {
        os_log_error(OS_LOG_DEFAULT, "Stillterm could not prepare its frame");
        _failed = YES; [self releaseEngine]; self.needsDisplay = YES; return;
    }
    CFTimeInterval now = CACurrentMediaTime();
    uint64_t elapsed = _lastTime > 0 ? (uint64_t)(MAX(0.0, MIN(0.25, now - _lastTime)) * 1e9) : 0;
    _lastTime = now;
    if (st_advance(_engine, elapsed) != ST_OK ||
        st_copy_frame(_engine, _nextCells.mutableBytes, (NSUInteger)_columns * _rows) != ST_OK) {
        os_log_error(OS_LOG_DEFAULT, "Stillterm engine update failed");
        _failed = YES; [self releaseEngine]; self.needsDisplay = YES; return;
    }
    if (!_reportedFrame && _columns > 0 && _rows > 0) {
        os_log(OS_LOG_DEFAULT, "Stillterm frame ready: %u x %u", _columns, _rows);
        _reportedFrame = YES;
    }
    if (_columns == 0 || _rows == 0) return;
    const StCell *previous = _cells.bytes, *next = _nextCells.bytes;
    for (uint32_t row = 0; row < _rows; row++) {
        if (memcmp(previous + row * _columns, next + row * _columns, _columns * sizeof(StCell)) != 0)
            [_canvas setNeedsDisplayInRect:NSMakeRect(0, NSHeight(self.bounds) - (row + 1) * _cellHeight,
                NSWidth(self.bounds), _cellHeight)];
    }
    NSMutableData *swap = _cells; _cells = _nextCells; _nextCells = swap;
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
    if (CTRunGetGlyphCount(run) != 1) return info;
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
- (void)drawFrameInRect:(NSRect)rect canvas:(NSView *)canvas {
    _drawnFrames++;
    [NSColor.blackColor setFill]; NSRectFill(rect);
    if (!_engine || !_cells || !_font) return;
    CGContextRef context = NSGraphicsContext.currentContext.CGContext;
    CGContextSaveGState(context);
    CGContextSetTextMatrix(context, CGAffineTransformIdentity);
    BOOL dense = _settings.options.theme == 2 && !_disableBatchDrawing;
    for (STGlyphBatch *batch in _denseBatches) batch.count = 0;
    const StCell *cells = _cells.bytes;
    for (uint32_t row = 0; row < _rows; row++) {
        CGFloat y = NSHeight(self.bounds) - (row + 1) * _cellHeight;
        if (![canvas needsToDrawRect:NSMakeRect(0, y, NSWidth(self.bounds), _cellHeight)]) continue;
        for (uint32_t column = 0; column < _columns; column++) {
            StCell cell = cells[row * _columns + column];
            if (!cell.visible) continue;
            NSRect box = NSMakeRect(column * _cellWidth, y, _cellWidth, _cellHeight);
            if (![canvas needsToDrawRect:box]) continue;
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
            CTLineDraw(line, context);
            CGContextRestoreGState(context);
        }
    }
    for (STGlyphBatch *batch in _denseBatches) {
        if (!batch.count) continue;
        CGContextSetRGBFillColor(context, (batch.color >> 16) / 255.0,
            ((batch.color >> 8) & 255) / 255.0, (batch.color & 255) / 255.0, 1);
        CTFontDrawGlyphs((__bridge CTFontRef)batch.font, batch.glyphs.bytes, batch.positions.bytes, batch.count, context);
    }
    CGContextRestoreGState(context);
}
- (void)dealloc {
    [NSWorkspace.sharedWorkspace.notificationCenter removeObserver:self];
    if (_dismissObserver) [NSDistributedNotificationCenter.defaultCenter removeObserver:_dismissObserver];
    st_destroy(_engine);
}
@end
