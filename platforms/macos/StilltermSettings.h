#import <Cocoa/Cocoa.h>
#import "StilltermBridge.h"

@interface StilltermSettings : NSObject
@property(nonatomic) StOptions options;
@property(nonatomic) NSInteger fps;
@property(nonatomic) CGFloat fontSize;
@property(nonatomic, copy) NSString *characters; // Empty uses the theme preset.
- (instancetype)initWithDefaults:(NSUserDefaults *)defaults;
- (BOOL)isValid;
- (void)save;
@end

@interface StilltermOptionsController : NSWindowController
- (instancetype)initWithSettings:(StilltermSettings *)settings
                       didApply:(void (^)(void))didApply;
@end
