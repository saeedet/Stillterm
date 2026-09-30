// Exercise ScreenSaverDefaults across processes, as System Settings and the
// full-screen host do. Never use the user's Stillterm preference domain.
#import <ScreenSaver/ScreenSaver.h>
#import "StilltermSettings.h"

static void require(BOOL condition, const char *message) {
    if (!condition) { fprintf(stderr, "FAIL: %s\n", message); exit(1); }
}
static void writeTheme(ScreenSaverDefaults *defaults, uint32_t theme) {
    StilltermSettings *settings = [[StilltermSettings alloc] initWithDefaults:defaults];
    StOptions options;
    st_defaults(theme, &options);
    settings.options = options; settings.characters = @"";
    [settings save];
}
int main(int argc, const char *argv[]) {
    @autoreleasepool {
        if (argc == 4 && strcmp(argv[1], "--write") == 0) {
            NSString *domain = [NSString stringWithUTF8String:argv[2]];
            require([domain hasPrefix:@"io.github.saeedet.Stillterm.tests."], "isolated writer domain");
            writeTheme([ScreenSaverDefaults defaultsForModuleWithName:domain], (uint32_t)atoi(argv[3]));
            return 0;
        }
        require(argc == 1, "no unexpected arguments");
        NSString *domain = [@"io.github.saeedet.Stillterm.tests." stringByAppendingString:NSUUID.UUID.UUIDString];
        ScreenSaverDefaults *defaults = [ScreenSaverDefaults defaultsForModuleWithName:domain];
        writeTheme(defaults, 1);
        for (NSNumber *theme in @[@0, @1, @0]) {
            NSTask *writer = [NSTask new];
            writer.executableURL = [NSURL fileURLWithPath:[NSString stringWithUTF8String:argv[0]]];
            writer.arguments = @[@"--write", domain, theme.stringValue];
            NSError *error;
            require([writer launchAndReturnError:&error], "launch independent preferences writer");
            [writer waitUntilExit];
            require(writer.terminationStatus == 0, "independent preferences write");
            StilltermSettings *loaded = [[StilltermSettings alloc] initWithDefaults:
                [ScreenSaverDefaults defaultsForModuleWithName:domain]];
            require(loaded.options.theme == theme.unsignedIntValue, "reader sees theme saved by another process");
            StOptions expected;
            st_defaults(theme.unsignedIntValue, &expected);
            require(loaded.options.speed == expected.speed && loaded.options.density == expected.density &&
                loaded.options.intensity == expected.intensity, "reader sees matching theme presets");
        }
        [defaults removeObjectForKey:@"SettingsV1"]; [defaults synchronize];
        puts("PASS independent screensaver processes reload Matrix, Monochrome, and their presets");
    }
    return 0;
}
