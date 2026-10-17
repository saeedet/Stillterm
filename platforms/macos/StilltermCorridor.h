#import <Cocoa/Cocoa.h>
#import "StilltermSettings.h"

// Owns the CAMetalLayer and its Rust renderer. Releasing the view destroys the
// renderer before the layer, so surface/device resources cannot outlive the host.
@interface STCorridorView : NSView
@property(nonatomic, readonly) uint32_t pixelWidth, pixelHeight;
@property(nonatomic, readonly) BOOL ready;
- (instancetype)initWithFrame:(NSRect)frame settings:(StilltermSettings *)settings;
- (BOOL)renderElapsed:(uint64_t)elapsed;
- (void)stopRendering;
- (NSData *)snapshot;
@end
