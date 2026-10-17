#import "StilltermView.h"
#import "StilltermSettings.h"
#import "StilltermRenderer.h"
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

@implementation StilltermView {
    StilltermCanvasView *_canvas;
    StilltermSettings *_settings;
    StilltermOptionsController *_optionsController;
    id _dismissObserver;
    NSArray<STRainLayer *> *_renderLayers;
    CGContextRef _backgroundContext;
    CGImageRef _backgroundImage;
    CFTimeInterval _lastTime, _lastBackground;
    NSUInteger _drawnFrames, _backgroundUpdates;
    BOOL _running, _sleeping, _failed, _reportedFrame, _layoutDirty, _backgroundDirty;
    BOOL _disableBatchDrawing, _backgroundReference;
}
- (NSData *)cells { return _renderLayers.lastObject.cells; }
- (uint32_t)columns { return _renderLayers.lastObject.columns; }
- (uint32_t)rows { return _renderLayers.lastObject.rows; }
- (NSUInteger)backgroundBytes {
    return _backgroundContext ? CGBitmapContextGetBytesPerRow(_backgroundContext) * CGBitmapContextGetHeight(_backgroundContext) : 0;
}
- (void)releaseBackground {
    if (_backgroundImage) CGImageRelease(_backgroundImage);
    if (_backgroundContext) CGContextRelease(_backgroundContext);
    _backgroundImage = NULL; _backgroundContext = NULL;
    _backgroundDirty = YES; _lastBackground = 0;
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
    [_canvas removeFromSuperview]; _canvas = nil;
    _renderLayers = nil; [self releaseBackground];
    _lastTime = 0; _reportedFrame = NO; _layoutDirty = YES;
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
    _layoutDirty = YES; _lastTime = 0; [self releaseBackground];
    self.needsDisplay = YES; _canvas.needsDisplay = YES;
}
- (BOOL)prepareFrame {
    if (!isfinite(NSWidth(self.bounds)) || !isfinite(NSHeight(self.bounds))) return NO;
    if (!_renderLayers) {
        NSMutableArray *layers = [NSMutableArray array];
        NSUInteger count = _settings.options.theme == 2 ? 3 : 1;
        for (NSUInteger i = 0; i < count; i++) {
            StDepthLayer depth = {_settings.options, 1, 0};
            if (count == 3 && st_depth_defaults(_settings.options, (uint32_t)i, &depth) != ST_OK) return NO;
            [layers addObject:[[STRainLayer alloc] initWithOptions:depth.options fontScale:depth.font_scale
                offset:depth.offset characters:_settings.characters words:i + 1 == count ? _settings.words : @""]];
        }
        _renderLayers = layers; _layoutDirty = YES;
    }
    if (_layoutDirty) {
        CGFloat scale = MIN(1.0, MIN(NSWidth(self.bounds) / 800.0, NSHeight(self.bounds) / 500.0));
        for (STRainLayer *layer in _renderLayers) {
            CGFloat size = MAX(_renderLayers.count > 1 ? 5.0 : 7.0, _settings.fontSize * scale * layer.fontScale);
            if (![layer prepareBounds:self.bounds pointSize:size]) return NO;
        }
        _layoutDirty = NO; _canvas.needsDisplay = YES;
    }
    if (!_canvas && self.columns > 0 && self.rows > 0) {
        _canvas = [[StilltermCanvasView alloc] initWithFrame:self.bounds];
        _canvas.owner = self; _canvas.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        _canvas.wantsLayer = YES; [self addSubview:_canvas];
    }
    return YES;
}
- (void)refreshBackground {
    if (_renderLayers.count < 2 || NSWidth(self.bounds) <= 0 || NSHeight(self.bounds) <= 0) return;
    if (_backgroundImage) { CGImageRelease(_backgroundImage); _backgroundImage = NULL; }
    if (!_backgroundContext) {
        // One shared, at-most-4MP bitmap. Distant glyphs are softer at 1x and
        // refresh at 15 FPS; the near layer stays sharp at the chosen frame rate.
        double pixels = NSWidth(self.bounds) * NSHeight(self.bounds);
        double scale = MIN(1.0, sqrt(4000000.0 / pixels));
        size_t width = MAX(1, floor(NSWidth(self.bounds) * scale));
        size_t height = MAX(1, floor(NSHeight(self.bounds) * scale));
        CGColorSpaceRef colorSpace = CGColorSpaceCreateDeviceRGB();
        _backgroundContext = CGBitmapContextCreate(NULL, width, height, 8, 0, colorSpace, (CGBitmapInfo)kCGImageAlphaPremultipliedLast);
        CGColorSpaceRelease(colorSpace);
        if (!_backgroundContext) return;
        CGContextScaleCTM(_backgroundContext, width / NSWidth(self.bounds), height / NSHeight(self.bounds));
    }
    CGContextSetBlendMode(_backgroundContext, kCGBlendModeCopy);
    CGContextSetRGBFillColor(_backgroundContext, 0, 0, 0, 1);
    CGContextFillRect(_backgroundContext, self.bounds);
    CGContextSetBlendMode(_backgroundContext, kCGBlendModeScreen);
    for (NSUInteger i = 0; i + 1 < _renderLayers.count; i++)
        [_renderLayers[i] drawWithContext:_backgroundContext canvas:nil batching:!_disableBatchDrawing glow:NO];
    _backgroundImage = CGBitmapContextCreateImage(_backgroundContext);
    _backgroundReference = _disableBatchDrawing; _backgroundDirty = NO; _backgroundUpdates++;
    _lastBackground = CACurrentMediaTime();
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
    for (STRainLayer *layer in _renderLayers) {
        if (![layer advance:elapsed canvas:layer == _renderLayers.lastObject ? _canvas : nil]) {
            os_log_error(OS_LOG_DEFAULT, "Stillterm engine update failed");
            _failed = YES; [self releaseEngine]; self.needsDisplay = YES; return;
        }
        if (layer != _renderLayers.lastObject && layer.changed) _backgroundDirty = YES;
        if (_renderLayers.count > 1 && layer.changed) _canvas.needsDisplay = YES;
    }
    if (!_reportedFrame && self.columns > 0 && self.rows > 0) {
        os_log(OS_LOG_DEFAULT, "Stillterm frame ready: %u x %u, layers=%lu", self.columns, self.rows, (unsigned long)_renderLayers.count);
        _reportedFrame = YES;
    }
    if (_renderLayers.count > 1 && (!_backgroundImage || (_backgroundDirty && now - _lastBackground >= 1.0 / 15))) {
        [self refreshBackground]; _canvas.needsDisplay = YES;
    }
}
- (void)drawFrameInRect:(NSRect)rect canvas:(NSView *)canvas {
    _drawnFrames++;
    [NSColor.blackColor setFill]; NSRectFill(rect);
    CGContextRef context = NSGraphicsContext.currentContext.CGContext;
    CGContextSaveGState(context);
    if (_renderLayers.count > 1) {
        if (!_backgroundImage || _backgroundReference != _disableBatchDrawing) [self refreshBackground];
        if (_backgroundImage) {
            CGContextSetInterpolationQuality(context, kCGInterpolationLow);
            CGContextDrawImage(context, self.bounds, _backgroundImage);
        } else {
            for (NSUInteger i = 0; i + 1 < _renderLayers.count; i++)
                [_renderLayers[i] drawWithContext:context canvas:canvas batching:!_disableBatchDrawing glow:NO];
        }
        CGContextSetBlendMode(context, kCGBlendModeScreen);
    }
    [_renderLayers.lastObject drawWithContext:context canvas:canvas batching:!_disableBatchDrawing glow:_renderLayers.count > 1];
    CGContextRestoreGState(context);
}
- (void)dealloc {
    [NSWorkspace.sharedWorkspace.notificationCenter removeObserver:self];
    if (_dismissObserver) [NSDistributedNotificationCenter.defaultCenter removeObserver:_dismissObserver];
    [self releaseBackground];
}
@end
