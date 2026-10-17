#import <Cocoa/Cocoa.h>
#import "StilltermBridge.h"

// One reusable grid/font/engine. Several grids can share one drawing surface.
@interface STRainLayer : NSObject
@property(nonatomic, readonly) NSData *cells;
@property(nonatomic, readonly) uint32_t columns, rows;
@property(nonatomic, readonly) CGFloat fontScale;
@property(nonatomic, readonly) BOOL changed;
- (instancetype)initWithOptions:(StOptions)options fontScale:(CGFloat)fontScale
    offset:(CGFloat)offset characters:(NSString *)characters words:(NSString *)words;
- (BOOL)prepareBounds:(NSRect)bounds pointSize:(CGFloat)pointSize;
- (BOOL)advance:(uint64_t)elapsed canvas:(NSView *)canvas;
- (void)drawWithContext:(CGContextRef)context canvas:(NSView *)canvas batching:(BOOL)batching glow:(BOOL)glow;
@end
