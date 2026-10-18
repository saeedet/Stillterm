// Loads the actual bundle for a windowed preview and native lifecycle checks.
#import <Cocoa/Cocoa.h>
#import <ScreenSaver/ScreenSaver.h>
#import "StilltermSettings.h"
#import "StilltermCorridor.h"
#include <sys/resource.h>
#include <mach/mach.h>

// Invoke the dismissal handler locally; never broadcast a system notification
// that could affect other running screensavers during tests.
@interface ScreenSaverView (STLifecycleChecks)
- (void)screenSaverWillStop:(NSNotification *)notification;
@end

static void require(BOOL condition, NSString *message) {
    if (!condition) { fprintf(stderr, "FAIL: %s\n", message.UTF8String); exit(1); }
}
static void pump(NSTimeInterval seconds) {
    NSDate *end = [NSDate dateWithTimeIntervalSinceNow:seconds];
    while (end.timeIntervalSinceNow > 0) {
        @autoreleasepool {
            NSEvent *event = [NSApp nextEventMatchingMask:NSEventMaskAny untilDate:
                [NSDate dateWithTimeIntervalSinceNow:MIN(0.01, end.timeIntervalSinceNow)]
                inMode:NSDefaultRunLoopMode dequeue:YES];
            if (event) [NSApp sendEvent:event];
            [NSApp updateWindows];
        }
    }
}
// Native timers can be coalesced on busy/headless CI machines. Wait for the
// observable state with a deadline rather than assuming a callback in 100 ms.
static void waitFor(BOOL (^condition)(void), NSString *message) {
    CFTimeInterval deadline = NSProcessInfo.processInfo.systemUptime + 2.0;
    while (!condition() && NSProcessInfo.processInfo.systemUptime < deadline) pump(0.02);
    require(condition(), message);
}
static void waitForFrame(ScreenSaverView *view) {
    waitFor(^BOOL { return [(NSData *)[view valueForKey:@"cells"] length] > 0; },
        @"Active instance must receive an animation callback within two seconds");
}
static NSBitmapImageRep *snapshot(NSView *view) {
    NSBitmapImageRep *rep = [view bitmapImageRepForCachingDisplayInRect:view.bounds];
    [view cacheDisplayInRect:view.bounds toBitmapImageRep:rep];
    return rep;
}
static void compareDenseDrawing(ScreenSaverView *view) {
    NSBitmapImageRep *batched = snapshot(view);
    [view setValue:@YES forKey:@"disableBatchDrawing"];
    NSBitmapImageRep *reference = snapshot(view);
    [view setValue:@NO forKey:@"disableBatchDrawing"];
    require(batched.pixelsWide == reference.pixelsWide && batched.pixelsHigh == reference.pixelsHigh &&
        batched.bitsPerSample == 8 && reference.bitsPerSample == 8 && batched.samplesPerPixel == 4 &&
        reference.samplesPerPixel == 4 && batched.bitmapFormat == reference.bitmapFormat,
        @"Compare native bitmap formats");
    NSUInteger lit = 0, changed = 0;
    NSUInteger alpha = batched.bitmapFormat & NSBitmapFormatAlphaFirst ? 0 : 3;
    for (NSInteger y = 0; y < batched.pixelsHigh; y++) {
        const uint8_t *a = batched.bitmapData + y * batched.bytesPerRow;
        const uint8_t *b = reference.bitmapData + y * reference.bytesPerRow;
        for (NSInteger x = 0; x < batched.pixelsWide; x++) {
            BOOL visible = NO, different = NO;
            for (NSUInteger c = 0; c < 4; c++) if (c != alpha) {
                visible |= MAX(a[x * 4 + c], b[x * 4 + c]) > 8;
                different |= abs((int)a[x * 4 + c] - (int)b[x * 4 + c]) > 16;
            }
            lit += visible; changed += visible && different;
        }
    }
    printf("Dense drawing comparison: %lu lit pixels, %.3f%% differ by more than 16/255\n",
        (unsigned long)lit, lit ? changed * 100.0 / lit : 100.0);
    require(lit > 100 && changed * 100 < lit * 5,
        @"Batch drawing must preserve clipped Core Text glyphs, colors, and positions");
}
static BOOL containsGlowingPhrase(ScreenSaverView *view, const char *phrase) {
    NSData *data = [view valueForKey:@"cells"];
    const StCell *cells = data.bytes;
    NSUInteger count = data.length / sizeof(StCell), length = strlen(phrase);
    NSUInteger columns = [[view valueForKey:@"columns"] unsignedIntegerValue];
    if (!columns) return NO;
    for (NSUInteger i = 0; i + (length - 1) * columns < count; i++) {
        BOOL matches = YES;
        for (NSUInteger j = 0; j < length; j++) matches &= cells[i + j * columns].scalar == (uint8_t)phrase[j] && cells[i + j * columns].visible &&
            cells[i + j * columns].red > 0 && cells[i + j * columns].blue > 0;
        if (matches) return YES;
    }
    return NO;
}
static NSUInteger visiblePixels(NSBitmapImageRep *rep, BOOL greenOnly) {
    NSUInteger count = 0;
    for (NSInteger y = 0; y < rep.pixelsHigh; y++) {
      @autoreleasepool { for (NSInteger x = 0; x < rep.pixelsWide; x++) {
        NSColor *color = [[rep colorAtX:x y:y] colorUsingColorSpace:NSColorSpace.deviceRGBColorSpace];
        if (color.greenComponent > 0.05 && (!greenOnly || color.greenComponent > color.redComponent * 1.5)) count++;
      } }
    }
    return count;
}
static void startRain(ScreenSaverView *view) {
    [view startAnimation];
    StilltermSettings *settings = [view valueForKey:@"settings"];
    StOptions options = settings.options;
    options.theme = 1; options.speed = 1.6; options.density = 0.35; options.intensity = 1;
    settings.options = options; settings.fontSize = 18; settings.characters = @""; settings.words = @"";
}
static NSBitmapImageRep *corridorSnapshot(ScreenSaverView *view) {
    STCorridorView *corridor = [view valueForKey:@"corridor"];
    NSData *pixels = [corridor snapshot];
    if (!pixels) return nil;
    NSBitmapImageRep *rep = [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:NULL pixelsWide:corridor.pixelWidth
        pixelsHigh:corridor.pixelHeight bitsPerSample:8 samplesPerPixel:4 hasAlpha:YES isPlanar:NO
        colorSpaceName:NSDeviceRGBColorSpace bytesPerRow:corridor.pixelWidth * 4 bitsPerPixel:32];
    memcpy(rep.bitmapData, pixels.bytes, pixels.length);
    return rep;
}
static double footprintMiB(void) {
    task_vm_info_data_t info = {0}; mach_msg_type_number_t count = TASK_VM_INFO_COUNT;
    require(task_info(mach_task_self(), TASK_VM_INFO, (task_info_t)&info, &count) == KERN_SUCCESS, @"Read footprint");
    return info.phys_footprint / 1048576.0;
}
static void configureCorridor(ScreenSaverView *view) {
    StilltermSettings *settings = [view valueForKey:@"settings"];
    StOptions options = settings.options;
    options.theme = 3; options.speed = 1; options.density = 0.95; options.intensity = 1;
    options.seed = 42; options.word_frequency = 2;
    settings.options = options; settings.characters = @""; settings.words = @"FREE"; settings.fontSize = 18;
    view.animationTimeInterval = 1.0 / 30;
}
static void checkCorridor(ScreenSaverView *view, const char *filename) {
    configureCorridor(view); pump(2.0);
    if ([[view valueForKey:@"gpuUnavailable"] boolValue]) {
        require(visiblePixels(snapshot(view), YES) > 100, @"GPU unavailable fallback still renders rain");
        [view stopAnimation];
        puts("SKIP GPU drawing: Metal unavailable; Dense Matrix fallback passed"); return;
    }
    NSBitmapImageRep *first = corridorSnapshot(view);
    require(first != nil && visiblePixels(first, YES) > 1000, @"GPU corridor draws green code");
    NSUInteger backWallPixels = 0;
    for (NSInteger y = first.pixelsHigh * 2 / 5; y < first.pixelsHigh * 3 / 5; y++) {
        for (NSInteger x = first.pixelsWide * 2 / 5; x < first.pixelsWide * 3 / 5; x++) {
            NSUInteger pixel[4]; [first getPixel:pixel atX:x y:y];
            if (pixel[1] > 20 && pixel[1] > pixel[0] && pixel[1] > pixel[2]) backWallPixels++;
        }
    }
    require(backWallPixels > 100, @"A visible code-covered back wall fills the center");
    NSData *before = [NSData dataWithBytes:first.bitmapData length:first.bytesPerRow * first.pixelsHigh];
    struct rusage begin, end; getrusage(RUSAGE_SELF, &begin);
    CFTimeInterval start = NSProcessInfo.processInfo.systemUptime;
    NSUInteger frames = [[view valueForKey:@"drawnFrames"] unsignedIntegerValue];
    pump(5.0); getrusage(RUSAGE_SELF, &end);
    double elapsed = NSProcessInfo.processInfo.systemUptime - start;
    double cpu = (end.ru_utime.tv_sec - begin.ru_utime.tv_sec) + (end.ru_utime.tv_usec - begin.ru_utime.tv_usec) / 1e6
        + (end.ru_stime.tv_sec - begin.ru_stime.tv_sec) + (end.ru_stime.tv_usec - begin.ru_stime.tv_usec) / 1e6;
    STCorridorView *corridor = [view valueForKey:@"corridor"];
    printf("Matrix 3D: %ux%u GPU pixels; %.2f%% of one CPU core; %.1f submissions/s; footprint %.1f MiB; peak RSS %.1f MiB\n",
        corridor.pixelWidth, corridor.pixelHeight, cpu / elapsed * 100,
        ([[view valueForKey:@"drawnFrames"] unsignedIntegerValue] - frames) / elapsed, footprintMiB(), end.ru_maxrss / 1048576.0);
    NSBitmapImageRep *next = corridorSnapshot(view);
    require(next && ![before isEqual:[NSData dataWithBytes:next.bitmapData length:next.bytesPerRow * next.pixelsHigh]],
        @"Rain animates in the stationary room");
    [view.window setValue:@YES forKey:@"simulateRemoteVisibility"];
    [view animateOneFrame];
    require(corridorSnapshot(view) != nil, @"Remote host visibility does not suppress GPU rendering");
    [view.window setValue:@NO forKey:@"simulateRemoteVisibility"];
    if (filename) require([[next representationUsingType:NSBitmapImageFileTypePNG properties:@{}]
        writeToFile:[NSString stringWithUTF8String:filename] atomically:YES], @"Write actual GPU snapshot");
    corridor = nil;
    [view setFrameSize:NSMakeSize(320, 180)]; [view animateOneFrame]; pump(0.1);
    require(corridorSnapshot(view) != nil, @"GPU resize renders");
    [view setFrameSize:NSZeroSize]; [view animateOneFrame];
    require(![(STCorridorView *)[view valueForKey:@"corridor"] ready], @"Zero size releases the GPU renderer");
    [view setFrameSize:NSMakeSize(800, 500)]; [view animateOneFrame]; pump(0.1);
    [NSWorkspace.sharedWorkspace.notificationCenter postNotificationName:NSWorkspaceWillSleepNotification object:nil];
    require([view valueForKey:@"corridor"] == nil, @"Sleep releases GPU view and renderer");
    [NSWorkspace.sharedWorkspace.notificationCenter postNotificationName:NSWorkspaceDidWakeNotification object:nil];
    [view animateOneFrame]; pump(0.1);
    require(corridorSnapshot(view) != nil, @"GPU resumes after wake");
    [view stopAnimation];
    double warm = 0;
    for (int i = 0; i < 6; i++) {
        __weak STCorridorView *old;
        @autoreleasepool {
            [view startAnimation]; configureCorridor(view); [view animateOneFrame]; pump(0.1);
            require([(STCorridorView *)[view valueForKey:@"corridor"] ready], @"GPU restarts");
            old = [view valueForKey:@"corridor"];
            [view stopAnimation]; [view animateOneFrame];
            require([view valueForKey:@"corridor"] == nil, @"Stop and late callback keep GPU resources released");
            require(!old.ready, @"Stop releases the renderer even if a host retains its view");
        }
        pump(0.05); require(old == nil, @"GPU view is destroyed after deferred UI cleanup");
        pump(0.15);
        if (i == 1) { pump(2); warm = footprintMiB(); }
    }
    pump(2);
    printf("Stopped GPU footprint %.1f MiB; settled growth %.1f MiB\n", footprintMiB(), footprintMiB() - warm);
    require(footprintMiB() - warm < 64, @"Repeated GPU restarts do not retain drawing surfaces");
    [view startAnimation]; configureCorridor(view);
    [view setValue:@YES forKey:@"gpuUnavailable"]; [view animateOneFrame]; pump(0.1);
    require(visiblePixels(snapshot(view), YES) > 100, @"GPU failure fallback renders Dense Matrix");
    [view stopAnimation];
    puts("PASS GPU drawing, animation, resize, zero size, sleep/wake, and repeated stop cleanup");
}

// Remote screensaver surfaces need not report desktop-window visibility.
@interface STRemoteVisibilityWindow : NSWindow
@property(nonatomic) BOOL simulateRemoteVisibility;
@end
@implementation STRemoteVisibilityWindow
- (BOOL)isVisible { return self.simulateRemoteVisibility ? NO : [super isVisible]; }
- (NSWindowOcclusionState)occlusionState {
    return self.simulateRemoteVisibility ? 0 : [super occlusionState];
}
@end

@interface STPreviewDelegate : NSObject <NSApplicationDelegate, NSWindowDelegate>
@property(nonatomic, strong) NSWindow *window;
@property(nonatomic, strong) ScreenSaverView *saver;
@end
@implementation STPreviewDelegate
- (BOOL)applicationSupportsSecureRestorableState:(NSApplication *)app { (void)app; return YES; }
- (BOOL)applicationShouldTerminateAfterLastWindowClosed:(NSApplication *)app { (void)app; return YES; }
- (void)windowWillClose:(NSNotification *)notification { (void)notification; [self.saver stopAnimation]; }
- (void)options:(id)sender {
    (void)sender;
    NSWindow *sheet = self.saver.configureSheet;
    if (sheet) [self.window beginSheet:sheet completionHandler:nil];
}
@end

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        if (argc < 2) { fprintf(stderr, "Usage: preview-host bundle [--check|--dense|--measure-dense|--corridor|--check-corridor [snapshot.png]]\n"); return 2; }
        BOOL check = argc >= 3 && strcmp(argv[2], "--check") == 0;
        BOOL check3D = argc >= 3 && strcmp(argv[2], "--check-corridor") == 0;
        BOOL corridorMode = check3D || (argc >= 3 && strcmp(argv[2], "--corridor") == 0);
        BOOL measureDense = argc >= 3 && strcmp(argv[2], "--measure-dense") == 0;
        BOOL dense = measureDense || (argc >= 3 && strcmp(argv[2], "--dense") == 0);
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
        NSBundle *bundle = [NSBundle bundleWithPath:[NSString stringWithUTF8String:argv[1]]];
        NSError *error;
        require([bundle loadAndReturnError:&error], error.localizedDescription ?: @"Bundle did not load");
        Class viewClass = bundle.principalClass;
        require([viewClass isSubclassOfClass:ScreenSaverView.class], @"Principal class must be a ScreenSaverView");
        STPreviewDelegate *delegate = [STPreviewDelegate new]; NSApp.delegate = delegate;
        [NSApp finishLaunching];
        NSSize surface = (dense || corridorMode) ? NSMakeSize(MIN(1440, NSWidth(NSScreen.mainScreen.visibleFrame) - 80),
            MIN(900, NSHeight(NSScreen.mainScreen.visibleFrame) - 100)) : NSMakeSize(800, 500);
        STRemoteVisibilityWindow *window = [[STRemoteVisibilityWindow alloc] initWithContentRect:NSMakeRect(40, 40, surface.width, surface.height + 40)
            styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskResizable
            backing:NSBackingStoreBuffered defer:NO];
        window.title = @"Stillterm Preview"; window.releasedWhenClosed = NO; window.delegate = delegate;
        ScreenSaverView *view = [[viewClass alloc] initWithFrame:NSMakeRect(0, 0, surface.width, surface.height) isPreview:YES];
        view.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        [window.contentView addSubview:view];
        NSButton *button = [NSButton buttonWithTitle:@"Options…" target:delegate action:@selector(options:)];
        button.frame = NSMakeRect(surface.width - 120, surface.height + 5, 100, 28); button.autoresizingMask = NSViewMinXMargin | NSViewMinYMargin;
        [window.contentView addSubview:button];
        delegate.window = window; delegate.saver = view;
        [window makeKeyAndOrderFront:nil];
        [NSApp activateIgnoringOtherApps:YES];
        [view startAnimation];
        if (corridorMode) {
            configureCorridor(view);
            if (check3D) { checkCorridor(view, argc >= 4 ? argv[3] : NULL); [window orderOut:nil]; return 0; }
            [NSApp run]; return 0;
        }
        if (dense) {
            // A temporary per-view demo; leave the user's saved theme and words intact.
            StilltermSettings *settings = [view valueForKey:@"settings"];
            StOptions options = settings.options;
            options.theme = 2; options.speed = 1.2; options.density = 0.95; options.intensity = 1;
            if (measureDense) { options.seed = 42; options.word_frequency = 2; settings.words = @"FREE"; }
            settings.options = options; settings.characters = @""; settings.fontSize = 10; settings.fps = 30;
            view.animationTimeInterval = 1.0 / 30;
            if (measureDense) {
                pump(2.0);
                NSArray *layers = [view valueForKey:@"renderLayers"];
                require(layers.count == 3, @"Dense Matrix has three independent depth grids");
                for (NSUInteger i = 1; i < layers.count; i++) require(
                    [[layers[i - 1] valueForKey:@"columns"] unsignedIntValue] > [[layers[i] valueForKey:@"columns"] unsignedIntValue],
                    @"Distant grids use smaller, more densely spaced characters");
                NSUInteger backgroundFrames = [[view valueForKey:@"backgroundUpdates"] unsignedIntegerValue];
                struct rusage begin, end; getrusage(RUSAGE_SELF, &begin);
                NSUInteger frames = [[view valueForKey:@"drawnFrames"] unsignedIntegerValue];
                CFTimeInterval start = NSProcessInfo.processInfo.systemUptime;
                pump(5.0); getrusage(RUSAGE_SELF, &end);
                double elapsed = NSProcessInfo.processInfo.systemUptime - start;
                double cpu = (end.ru_utime.tv_sec - begin.ru_utime.tv_sec) + (end.ru_utime.tv_usec - begin.ru_utime.tv_usec) / 1e6
                           + (end.ru_stime.tv_sec - begin.ru_stime.tv_sec) + (end.ru_stime.tv_usec - begin.ru_stime.tv_usec) / 1e6;
                task_vm_info_data_t info = {0}; mach_msg_type_number_t count = TASK_VM_INFO_COUNT;
                require(task_info(mach_task_self(), TASK_VM_INFO, (task_info_t)&info, &count) == KERN_SUCCESS, @"Read footprint");
                NSData *data = [view valueForKey:@"cells"]; const StCell *cells = data.bytes; NSUInteger visible = 0;
                for (NSUInteger i = 0; i < data.length / sizeof(StCell); i++) visible += cells[i].visible != 0;
                printf("Dense Matrix: %.0fx%.0f points, near grid %ux%u; %.1f%% near cells visible; %.2f%% of one CPU core; %.1f draws/s; footprint %.1f MiB; peak RSS %.1f MiB\n",
                    NSWidth(view.bounds), NSHeight(view.bounds), [[view valueForKey:@"columns"] unsignedIntValue], [[view valueForKey:@"rows"] unsignedIntValue],
                    data.length ? visible * 100.0 / (data.length / sizeof(StCell)) : 0,
                    cpu / elapsed * 100, ([[view valueForKey:@"drawnFrames"] unsignedIntegerValue] - frames) / elapsed,
                    info.phys_footprint / 1048576.0, end.ru_maxrss / 1048576.0);
                require(data.length > 0 && visible > data.length / sizeof(StCell) / 2, @"Dense native preview fills most cells");
                printf("Depth layers: %lu; background %.1f updates/s; shared bitmap %.1f MiB\n", (unsigned long)layers.count,
                    ([[view valueForKey:@"backgroundUpdates"] unsignedIntegerValue] - backgroundFrames) / elapsed,
                    [[view valueForKey:@"backgroundBytes"] unsignedIntegerValue] / 1048576.0);
                require([[view valueForKey:@"backgroundBytes"] unsignedIntegerValue] <= 17000000, @"Shared background bitmap stays bounded");
                layers = nil; // Do not keep layer engines alive during cleanup checks.
                compareDenseDrawing(view);
                if (argc >= 4) require([[snapshot(view) representationUsingType:NSBitmapImageFileTypePNG properties:@{}]
                    writeToFile:[NSString stringWithUTF8String:argv[3]] atomically:YES], @"Write dense snapshot");
                [view setFrameSize:NSMakeSize(320, 180)]; [view animateOneFrame]; pump(0.1);
                require(visiblePixels(snapshot(view), YES) > 100, @"Layered rain survives resize");
                [view setFrameSize:NSZeroSize]; [view animateOneFrame];
                [view setFrameSize:NSMakeSize(800, 500)]; [view animateOneFrame]; pump(0.1);
                [NSWorkspace.sharedWorkspace.notificationCenter postNotificationName:NSWorkspaceWillSleepNotification object:nil];
                require([view valueForKey:@"renderLayers"] == nil && [[view valueForKey:@"backgroundBytes"] unsignedIntegerValue] == 0,
                    @"Sleep releases all depth grids and the shared bitmap");
                [NSWorkspace.sharedWorkspace.notificationCenter postNotificationName:NSWorkspaceDidWakeNotification object:nil];
                [view animateOneFrame]; pump(0.1);
                [view stopAnimation];
                require([view valueForKey:@"cells"] == nil && [view valueForKey:@"canvas"] == nil, @"Dense stop releases resources");
                require([view valueForKey:@"renderLayers"] == nil,
                    @"Dense stop releases glyph caches and drawing buffers");
                require([[view valueForKey:@"backgroundBytes"] unsignedIntegerValue] == 0, @"Stop releases the depth bitmap");
                [window orderOut:nil];
                return 0;
            }
        }
        if (!check) { [NSApp run]; return 0; }

        // Change only this view's in-memory settings; never write user preferences.
        StilltermSettings *settings = [view valueForKey:@"settings"];
        StOptions options = settings.options;
        options.theme = 1; options.speed = 1.6; options.density = 0.35; options.intensity = 1;
        options.word_frequency = 2;
        settings.options = options; settings.characters = @""; settings.words = @"FREE";
        pump(0.4);
        [view animateOneFrame];
        NSBitmapImageRep *rep = snapshot(view);
        NSUInteger greenPixels = visiblePixels(rep, YES);
        fprintf(stderr, "Native render: visible=%d occlusion=%lu frameBytes=%lu greenPixels=%lu\n",
            window.visible, (unsigned long)window.occlusionState,
            (unsigned long)[(NSData *)[view valueForKey:@"cells"] length], (unsigned long)greenPixels);
        require(greenPixels > 100, @"Matrix must draw green glyphs through Core Text");
        if (argc >= 4) require([[rep representationUsingType:NSBitmapImageFileTypePNG properties:@{}]
            writeToFile:[NSString stringWithUTF8String:argv[3]] atomically:YES], @"Write snapshot");
        CFTimeInterval wordDeadline = NSProcessInfo.processInfo.systemUptime + 10.0;
        while (!containsGlowingPhrase(view, "FREE") && NSProcessInfo.processInfo.systemUptime < wordDeadline) {
            pump(0.02); [view animateOneFrame];
        }
        require(containsGlowingPhrase(view, "FREE"), @"FREE must build up into a complete glowing word");
        if (argc >= 4) {
            pump(0.15); [view animateOneFrame];
            NSString *wordPath = [[NSString stringWithUTF8String:argv[3]] stringByDeletingLastPathComponent];
            require([[snapshot(view) representationUsingType:NSBitmapImageFileTypePNG properties:@{}]
                writeToFile:[wordPath stringByAppendingPathComponent:@"words-preview.png"] atomically:YES], @"Write phrase snapshot");
        }
        puts("PASS FREE builds up into a complete glowing word");
        window.simulateRemoteVisibility = YES;
        [view animateOneFrame];
        require([view valueForKey:@"cells"] != nil, @"Remote host visibility must not suppress rendering");
        NSData *remoteBefore = [[view valueForKey:@"cells"] copy];
        pump(0.15);
        // A real remote host supplies callbacks. Drive one explicitly because
        // desktop AppKit may throttle the synthetic invisible window's timer.
        [view animateOneFrame];
        require(![remoteBefore isEqual:[view valueForKey:@"cells"]], @"Remote-hosted rain must advance");
        window.simulateRemoteVisibility = NO;
        require(visiblePixels(snapshot(view), YES) > 100, @"Remote-hosted frame must contain green glyphs");
        NSData *before = [[view valueForKey:@"cells"] copy];
        pump(0.15);
        waitFor(^BOOL { return ![before isEqual:[view valueForKey:@"cells"]]; }, @"Animation must advance");
        [view setFrameSize:NSMakeSize(320, 180)]; pump(0.1);
        require(visiblePixels(snapshot(view), YES) > 10, @"Resized preview must render");
        [view setFrameSize:NSZeroSize]; [view animateOneFrame];
        [view setFrameSize:NSMakeSize(800, 500)]; pump(0.1);
        [view stopAnimation];
        for (int i = 0; i < 20; i++) {
            __weak NSView *canvas;
            @autoreleasepool {
                startRain(view); [view animateOneFrame]; pump(0.01);
                canvas = [view valueForKey:@"canvas"];
                require(canvas != nil, @"Active animation owns a drawing surface");
                [view stopAnimation]; [view animateOneFrame];
                require([view valueForKey:@"cells"] == nil && [view valueForKey:@"canvas"] == nil,
                    @"Stop releases resources; late callbacks remain idle");
            }
            pump(0.01);
            require(canvas == nil, @"Stop destroys the drawing view while the host retains its parent");
        }
        startRain(view); waitForFrame(view);
        ScreenSaverView *second = [[viewClass alloc] initWithFrame:NSMakeRect(0, 0, 200, 120) isPreview:NO];
        [window.contentView addSubview:second]; startRain(second); waitForFrame(second);
        [view stopAnimation];
        require([second valueForKey:@"cells"] != nil, @"A second instance owns independent resources");
        startRain(view); waitForFrame(view);
        ScreenSaverView *third = [[viewClass alloc] initWithFrame:NSMakeRect(200, 0, 200, 120) isPreview:NO];
        [window.contentView addSubview:third]; startRain(third); waitForFrame(third);
        require([third valueForKey:@"cells"] != nil, @"Both full-screen instances start rendering");
        NSNotification *dismissal = [NSNotification notificationWithName:@"com.apple.screensaver.willstop" object:nil];
        // Reproduce a host retaining multiple attached views and omitting stopAnimation.
        for (ScreenSaverView *retained in @[second, third]) {
            [retained screenSaverWillStop:dismissal];
            [retained screenSaverWillStop:dismissal];
            [retained animateOneFrame];
            require(!retained.isAnimating && [retained valueForKey:@"cells"] == nil && [retained valueForKey:@"canvas"] == nil,
                @"Dismissal stops retained timers and releases buffers despite late callbacks");
        }
        [view screenSaverWillStop:dismissal];
        require(view.isAnimating && [view valueForKey:@"cells"] != nil, @"Dismissal leaves the settings preview active");
        [NSWorkspace.sharedWorkspace.notificationCenter postNotificationName:NSWorkspaceWillSleepNotification object:nil];
        [NSWorkspace.sharedWorkspace.notificationCenter postNotificationName:NSWorkspaceDidWakeNotification object:nil];
        pump(0.15);
        for (ScreenSaverView *retained in @[second, third]) {
            [retained animateOneFrame];
            require(!retained.isAnimating && [retained valueForKey:@"cells"] == nil && [retained valueForKey:@"canvas"] == nil,
                @"Wake must not revive dismissed instances");
        }
        startRain(second); waitForFrame(second);
        require(second.isAnimating && [second valueForKey:@"cells"] != nil, @"An explicit host restart resumes rendering");
        require([third valueForKey:@"cells"] == nil, @"Restarting one instance leaves older instances stopped");
        [second stopAnimation]; [second removeFromSuperview]; [third removeFromSuperview];
        [NSWorkspace.sharedWorkspace.notificationCenter postNotificationName:NSWorkspaceWillSleepNotification object:nil];
        [view animateOneFrame];
        require([view valueForKey:@"cells"] == nil && [view valueForKey:@"canvas"] == nil, @"Sleep releases buffers and drawing surface");
        [NSWorkspace.sharedWorkspace.notificationCenter postNotificationName:NSWorkspaceDidWakeNotification object:nil];
        pump(0.1);
        waitForFrame(view);
        window.contentView.hidden = YES; [view animateOneFrame];
        require([view valueForKey:@"cells"] == nil && [view valueForKey:@"canvas"] == nil, @"Hidden view releases buffers and drawing surface");
        window.contentView.hidden = NO; pump(0.1);
        require(view.hasConfigureSheet && view.configureSheet != nil, @"Configuration sheet loads");
        NSWindow *sheet = view.configureSheet;
        [window beginSheet:sheet completionHandler:nil]; pump(0.1);
        if (argc >= 4) {
            NSString *path = [[NSString stringWithUTF8String:argv[3]] stringByDeletingPathExtension];
            [[snapshot(sheet.contentView) representationUsingType:NSBitmapImageFileTypePNG properties:@{}]
                writeToFile:[path stringByAppendingString:@"-options.png"] atomically:YES];
        }
        [window endSheet:sheet]; [sheet orderOut:nil];
        startRain(view); waitForFrame(view);
        struct rusage begin, end; getrusage(RUSAGE_SELF, &begin); CFTimeInterval start = NSProcessInfo.processInfo.systemUptime;
        pump(2.0); getrusage(RUSAGE_SELF, &end);
        double cpu = (end.ru_utime.tv_sec - begin.ru_utime.tv_sec) + (end.ru_utime.tv_usec - begin.ru_utime.tv_usec) / 1e6
                   + (end.ru_stime.tv_sec - begin.ru_stime.tv_sec) + (end.ru_stime.tv_usec - begin.ru_stime.tv_usec) / 1e6;
        printf("Native host: %.2f%% of one CPU core at 800x500 points; peak RSS %.1f MiB\n",
            cpu / (NSProcessInfo.processInfo.systemUptime - start) * 100, end.ru_maxrss / 1048576.0);
        [view stopAnimation]; pump(0.1); [view animateOneFrame];
        getrusage(RUSAGE_SELF, &begin); start = NSProcessInfo.processInfo.systemUptime;
        pump(1.0); getrusage(RUSAGE_SELF, &end);
        cpu = (end.ru_utime.tv_sec - begin.ru_utime.tv_sec) + (end.ru_utime.tv_usec - begin.ru_utime.tv_usec) / 1e6
            + (end.ru_stime.tv_sec - begin.ru_stime.tv_sec) + (end.ru_stime.tv_usec - begin.ru_stime.tv_usec) / 1e6;
        printf("Stopped host: %.2f%% of one CPU core (includes the test run loop)\n",
            cpu / (NSProcessInfo.processInfo.systemUptime - start) * 100);
        require([view valueForKey:@"cells"] == nil && !view.isAnimating, @"Stopped host remains idle");
        [window orderOut:nil];
        puts("PASS bundle loading, native drawing, resize, independent instances, lifecycle, and options sheet");
    }
    return 0;
}
