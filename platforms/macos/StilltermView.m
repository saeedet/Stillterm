#import "StilltermView.h"
#import "StilltermSettings.h"
#import <CoreText/CoreText.h>
#import <QuartzCore/QuartzCore.h>
#include <math.h>
#import <os/log.h>

static NSString *const STDefaultsDomain = @"io.github.saeedet.Stillterm";

@implementation StilltermView {
    StEngine *_engine;
    StilltermSettings *_settings;
    StilltermOptionsController *_optionsController;
    NSMutableData *_cells, *_nextCells;
    NSMutableDictionary<NSNumber *, id> *_lines;
    NSFont *_font;
    uint32_t _columns, _rows;
    CGFloat _cellWidth, _cellHeight;
    CFTimeInterval _lastTime;
    BOOL _running, _sleeping, _failed, _reportedFrame;
}
- (instancetype)initWithFrame:(NSRect)frame isPreview:(BOOL)isPreview {
    if ((self = [super initWithFrame:frame isPreview:isPreview])) {
        [self reloadSettings];
        NSNotificationCenter *center = NSWorkspace.sharedWorkspace.notificationCenter;
        [center addObserver:self selector:@selector(willSleep:) name:NSWorkspaceWillSleepNotification object:nil];
        [center addObserver:self selector:@selector(didWake:) name:NSWorkspaceDidWakeNotification object:nil];
    }
    return self;
}
- (BOOL)isOpaque { return YES; }
- (BOOL)hasConfigureSheet { return YES; }
- (void)releaseEngine {
    st_destroy(_engine); _engine = NULL;
    _cells = nil; _nextCells = nil; _lines = nil; _font = nil;
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
    os_log(OS_LOG_DEFAULT, "Stillterm start: build=%{public}@ preview=%d windowVisible=%d occlusion=%lu",
        [[NSBundle bundleForClass:StilltermView.class] objectForInfoDictionaryKey:@"CFBundleVersion"],
        self.isPreview, self.window.visible, (unsigned long)self.window.occlusionState);
    [super startAnimation];
}
- (void)stopAnimation {
    // Apple may deliver one more animation callback after this method.
    _running = NO;
    [super stopAnimation];
    [self releaseEngine];
    self.needsDisplay = YES;
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
    _lastTime = 0;
    self.needsDisplay = YES;
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
    }
    double columns = ceil(MAX(0, NSWidth(self.bounds)) / _cellWidth);
    double rows = ceil(MAX(0, NSHeight(self.bounds)) / _cellHeight);
    if (!isfinite(columns) || !isfinite(rows) || columns > UINT16_MAX || rows > UINT16_MAX || columns * rows > 262144) return NO;
    uint32_t width = (uint32_t)columns, height = (uint32_t)rows;
    if (!_engine) {
        NSData *characters = [_settings.characters dataUsingEncoding:NSUTF8StringEncoding];
        _engine = st_create(width, height, _settings.options, characters.bytes, characters.length);
        if (!_engine) return NO;
    } else if ((_columns != width || _rows != height) && st_resize(_engine, width, height) != ST_OK) return NO;
    if (!_cells || _columns != width || _rows != height) {
        _columns = width; _rows = height;
        NSUInteger bytes = (NSUInteger)width * height * sizeof(StCell);
        _cells = [NSMutableData dataWithLength:bytes]; _nextCells = [NSMutableData dataWithLength:bytes];
        _lastTime = 0; self.needsDisplay = YES;
    }
    return YES;
}
- (void)animateOneFrame {
    if (!_running || _sleeping || _failed) return;
    // macOS can present a remote surface whose NSWindow is neither visible nor
    // unoccluded locally. ScreenSaverView's lifecycle controls animation; only
    // explicit view hiding or detachment suppresses an active host's callbacks.
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
            [self setNeedsDisplayInRect:NSMakeRect(0, NSHeight(self.bounds) - (row + 1) * _cellHeight,
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
- (void)drawRect:(NSRect)rect {
    [NSColor.blackColor setFill]; NSRectFill(rect);
    if (!_engine || !_cells || !_font) return;
    CGContextRef context = NSGraphicsContext.currentContext.CGContext;
    CGContextSaveGState(context);
    CGContextSetTextMatrix(context, CGAffineTransformIdentity);
    const StCell *cells = _cells.bytes;
    for (uint32_t row = 0; row < _rows; row++) {
        CGFloat y = NSHeight(self.bounds) - (row + 1) * _cellHeight;
        if (![self needsToDrawRect:NSMakeRect(0, y, NSWidth(self.bounds), _cellHeight)]) continue;
        for (uint32_t column = 0; column < _columns; column++) {
            StCell cell = cells[row * _columns + column];
            if (!cell.visible) continue;
            NSRect box = NSMakeRect(column * _cellWidth, y, _cellWidth, _cellHeight);
            if (![self needsToDrawRect:box]) continue;
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
    CGContextRestoreGState(context);
}
- (void)dealloc {
    [NSWorkspace.sharedWorkspace.notificationCenter removeObserver:self];
    st_destroy(_engine);
}
@end
