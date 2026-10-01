// Loads the actual bundle for a windowed preview and native lifecycle checks.
#import <Cocoa/Cocoa.h>
#import <ScreenSaver/ScreenSaver.h>
#import "StilltermSettings.h"
#include <sys/resource.h>

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
static NSBitmapImageRep *snapshot(NSView *view) {
    NSBitmapImageRep *rep = [view bitmapImageRepForCachingDisplayInRect:view.bounds];
    [view cacheDisplayInRect:view.bounds toBitmapImageRep:rep];
    return rep;
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
        if (argc < 2) { fprintf(stderr, "Usage: preview-host bundle [--check [snapshot.png]]\n"); return 2; }
        BOOL check = argc >= 3 && strcmp(argv[2], "--check") == 0;
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
        NSBundle *bundle = [NSBundle bundleWithPath:[NSString stringWithUTF8String:argv[1]]];
        NSError *error;
        require([bundle loadAndReturnError:&error], error.localizedDescription ?: @"Bundle did not load");
        Class viewClass = bundle.principalClass;
        require([viewClass isSubclassOfClass:ScreenSaverView.class], @"Principal class must be a ScreenSaverView");
        STPreviewDelegate *delegate = [STPreviewDelegate new]; NSApp.delegate = delegate;
        [NSApp finishLaunching];
        STRemoteVisibilityWindow *window = [[STRemoteVisibilityWindow alloc] initWithContentRect:NSMakeRect(120, 120, 800, 540)
            styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskResizable
            backing:NSBackingStoreBuffered defer:NO];
        window.title = @"Stillterm Preview"; window.releasedWhenClosed = NO; window.delegate = delegate;
        ScreenSaverView *view = [[viewClass alloc] initWithFrame:NSMakeRect(0, 0, 800, 500) isPreview:YES];
        view.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        [window.contentView addSubview:view];
        NSButton *button = [NSButton buttonWithTitle:@"Options…" target:delegate action:@selector(options:)];
        button.frame = NSMakeRect(680, 505, 100, 28); button.autoresizingMask = NSViewMinXMargin | NSViewMinYMargin;
        [window.contentView addSubview:button];
        delegate.window = window; delegate.saver = view;
        [window makeKeyAndOrderFront:nil];
        [NSApp activateIgnoringOtherApps:YES];
        [view startAnimation];
        if (!check) { [NSApp run]; return 0; }

        // Change only this view's in-memory settings; never write user preferences.
        StilltermSettings *settings = [view valueForKey:@"settings"];
        StOptions options = settings.options;
        options.theme = 1; options.speed = 1.6; options.density = 0.35; options.intensity = 1;
        settings.options = options; settings.characters = @"";
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
        window.simulateRemoteVisibility = YES;
        [view animateOneFrame];
        require([view valueForKey:@"cells"] != nil, @"Remote host visibility must not suppress rendering");
        NSData *remoteBefore = [[view valueForKey:@"cells"] copy];
        pump(0.15);
        require(![remoteBefore isEqual:[view valueForKey:@"cells"]], @"Remote-hosted rain must advance");
        window.simulateRemoteVisibility = NO;
        require(visiblePixels(snapshot(view), YES) > 100, @"Remote-hosted frame must contain green glyphs");
        NSData *before = [[view valueForKey:@"cells"] copy];
        pump(0.15);
        require(![before isEqual:[view valueForKey:@"cells"]], @"Animation must advance");
        [view setFrameSize:NSMakeSize(320, 180)]; pump(0.1);
        require(visiblePixels(snapshot(view), YES) > 10, @"Resized preview must render");
        [view setFrameSize:NSZeroSize]; [view animateOneFrame];
        [view setFrameSize:NSMakeSize(800, 500)]; pump(0.1);
        for (int i = 0; i < 20; i++) {
            [view stopAnimation]; [view animateOneFrame];
            require([view valueForKey:@"cells"] == nil, @"Stop releases buffers; late callbacks remain idle");
            [view startAnimation]; [view animateOneFrame];
        }
        pump(0.1);
        ScreenSaverView *second = [[viewClass alloc] initWithFrame:NSMakeRect(0, 0, 200, 120) isPreview:NO];
        [window.contentView addSubview:second]; [second startAnimation]; pump(0.1);
        [view stopAnimation];
        require([second valueForKey:@"cells"] != nil, @"A second instance owns independent resources");
        [view startAnimation]; pump(0.1);
        ScreenSaverView *third = [[viewClass alloc] initWithFrame:NSMakeRect(200, 0, 200, 120) isPreview:NO];
        [window.contentView addSubview:third]; [third startAnimation]; pump(0.1);
        require([third valueForKey:@"cells"] != nil, @"Both full-screen instances start rendering");
        NSNotification *dismissal = [NSNotification notificationWithName:@"com.apple.screensaver.willstop" object:nil];
        // Reproduce a host retaining multiple attached views and omitting stopAnimation.
        for (ScreenSaverView *retained in @[second, third]) {
            [retained screenSaverWillStop:dismissal];
            [retained screenSaverWillStop:dismissal];
            [retained animateOneFrame];
            require(!retained.isAnimating && [retained valueForKey:@"cells"] == nil,
                @"Dismissal stops retained timers and releases buffers despite late callbacks");
        }
        [view screenSaverWillStop:dismissal];
        require(view.isAnimating && [view valueForKey:@"cells"] != nil, @"Dismissal leaves the settings preview active");
        [NSWorkspace.sharedWorkspace.notificationCenter postNotificationName:NSWorkspaceWillSleepNotification object:nil];
        [NSWorkspace.sharedWorkspace.notificationCenter postNotificationName:NSWorkspaceDidWakeNotification object:nil];
        pump(0.15);
        for (ScreenSaverView *retained in @[second, third]) {
            [retained animateOneFrame];
            require(!retained.isAnimating && [retained valueForKey:@"cells"] == nil,
                @"Wake must not revive dismissed instances");
        }
        [second startAnimation]; pump(0.1);
        require(second.isAnimating && [second valueForKey:@"cells"] != nil, @"An explicit host restart resumes rendering");
        require([third valueForKey:@"cells"] == nil, @"Restarting one instance leaves older instances stopped");
        [second stopAnimation]; [second removeFromSuperview]; [third removeFromSuperview];
        [NSWorkspace.sharedWorkspace.notificationCenter postNotificationName:NSWorkspaceWillSleepNotification object:nil];
        [view animateOneFrame];
        require([view valueForKey:@"cells"] == nil, @"Sleep releases buffers");
        [NSWorkspace.sharedWorkspace.notificationCenter postNotificationName:NSWorkspaceDidWakeNotification object:nil];
        pump(0.1);
        require([view valueForKey:@"cells"] != nil, @"Wake restarts rendering while active");
        window.contentView.hidden = YES; [view animateOneFrame];
        require([view valueForKey:@"cells"] == nil, @"Hidden view releases buffers");
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
        [view startAnimation]; pump(0.1);
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
