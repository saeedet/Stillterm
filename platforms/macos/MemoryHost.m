// Retain stopped views like the legacy host, and measure the whole process footprint.
#import <Cocoa/Cocoa.h>
#import <ScreenSaver/ScreenSaver.h>
#import <QuartzCore/QuartzCore.h>
#include <mach/mach.h>

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
static double footprintMiB(void) {
    task_vm_info_data_t info = {0};
    mach_msg_type_number_t count = TASK_VM_INFO_COUNT;
    require(task_info(mach_task_self(), TASK_VM_INFO, (task_info_t)&info, &count) == KERN_SUCCESS,
        @"Read process memory footprint");
    return info.phys_footprint / 1048576.0;
}
int main(int argc, const char *argv[]) {
    @autoreleasepool {
        require(argc == 2, @"Usage: memory-host bundle");
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
        [NSApp finishLaunching];
        NSBundle *bundle = [NSBundle bundleWithPath:[NSString stringWithUTF8String:argv[1]]];
        NSError *error;
        require([bundle loadAndReturnError:&error], error.localizedDescription ?: @"Load bundle");
        Class viewClass = bundle.principalClass;
        require([viewClass isSubclassOfClass:ScreenSaverView.class], @"Load screensaver class");
        NSWindow *window = [[NSWindow alloc] initWithContentRect:NSMakeRect(50, 50, 1600, 900)
            styleMask:NSWindowStyleMaskTitled backing:NSBackingStoreBuffered defer:NO];
        window.title = @"Stillterm memory check";
        window.contentView.wantsLayer = YES;
        [window makeKeyAndOrderFront:nil];
        [NSApp activateIgnoringOtherApps:YES];
        pump(0.1);
        double warm = 0, peakGrowth = 0;
        for (int i = 0; i < 12; i++) {
            @autoreleasepool {
                ScreenSaverView *view = [[viewClass alloc] initWithFrame:window.contentView.bounds isPreview:YES];
                // Give each retained instance its own backing layer, as a remote host can.
                view.wantsLayer = YES;
                [window.contentView addSubview:view];
                [view startAnimation]; [view animateOneFrame]; pump(0.15);
                require([view valueForKey:@"cells"] != nil, @"Active retained-view test must render");
                [view stopAnimation]; [view animateOneFrame];
                require([view valueForKey:@"cells"] == nil && !view.isAnimating, @"Stopped view stays idle");
                [CATransaction flush];
            }
            pump(0.1);
            double footprint = footprintMiB();
            fprintf(stderr, "Retained views: %d; footprint %.1f MiB\n", i + 1, footprint);
            if (i == 3) {
                // Core Animation may retire removed surfaces asynchronously.
                // Compare settled batches, not a transient driver allocation peak.
                pump(2.0);
                warm = footprintMiB();
            }
            if (i > 3) peakGrowth = MAX(peakGrowth, footprint - warm);
        }
        pump(2.0);
        double settled = footprintMiB();
        fprintf(stderr, "Settled footprint: %.1f MiB; settled growth: %.1f MiB; transient peak growth: %.1f MiB\n",
            settled, settled - warm, peakGrowth);
        // Allow AppKit/font caches and graphics-driver variability, but not a full
        // Retina backing surface per stopped instance (~22 MiB at this size).
        require(settled - warm < 64, @"Stopped views must not retain a full-size drawing surface each");
        [window orderOut:nil];
        puts("PASS retained-view graphics memory check");
    }
    return 0;
}
