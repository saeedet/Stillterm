#import "StilltermSettings.h"

static void require(BOOL condition, const char *message) {
    if (!condition) { fprintf(stderr, "FAIL: %s\n", message); exit(1); }
}
int main(void) {
    @autoreleasepool {
        [NSApplication sharedApplication];
        NSString *domain = [@"io.github.saeedet.Stillterm.tests." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:domain];
        StilltermSettings *settings = [[StilltermSettings alloc] initWithDefaults:defaults];
        require(settings.options.theme == 0 && settings.fps == 30, "native defaults");
        StOptions options; st_defaults(1, &options); options.seed = UINT64_MAX;
        settings.options = options; settings.characters = @"ｱ01"; settings.fps = 60; settings.fontSize = 24;
        [settings save];
        StilltermSettings *loaded = [[StilltermSettings alloc] initWithDefaults:defaults];
        require(loaded.options.theme == 1 && loaded.options.seed == UINT64_MAX && loaded.fps == 60 &&
            loaded.fontSize == 24 && [loaded.characters isEqual:@"ｱ01"], "settings round trip, including u64 seed");
        loaded.characters = @"界";
        require(!loaded.isValid, "wide characters rejected");
        [defaults setObject:@{@"theme": @"broken"} forKey:@"SettingsV1"];
        loaded = [[StilltermSettings alloc] initWithDefaults:defaults];
        require(loaded.isValid && loaded.options.theme == 0, "corrupt preferences fall back safely");
        __block BOOL applied = NO;
        StilltermOptionsController *controller = [[StilltermOptionsController alloc] initWithSettings:loaded didApply:^{ applied = YES; }];
        NSMutableDictionary *fields = [NSMutableDictionary dictionary];
        NSPopUpButton *theme;
        NSButton *save;
        for (NSView *view in controller.window.contentView.subviews) {
            if ([view isKindOfClass:NSTextField.class] && [(NSTextField *)view isEditable]) fields[view.accessibilityLabel] = view;
            if ([view isKindOfClass:NSPopUpButton.class]) theme = (NSPopUpButton *)view;
            if ([view isKindOfClass:NSButton.class] && [[(NSButton *)view title] isEqual:@"Save"]) save = (NSButton *)view;
        }
        require(fields.count == 7 && theme && save, "options controls");
        [theme selectItemAtIndex:1]; [NSApp sendAction:theme.action to:theme.target from:theme];
        require([[(NSTextField *)fields[@"Speed"] stringValue] isEqual:@"1.6"], "theme presets populate fields");
        [(NSTextField *)fields[@"Speed"] setStringValue:@"1.6oops"]; [save performClick:nil];
        require(!applied, "invalid numeric suffix rejected");
        [(NSTextField *)fields[@"Speed"] setStringValue:@"1.6"];
        [(NSTextField *)fields[@"Seed"] setStringValue:@"18446744073709551616"]; [save performClick:nil];
        require(!applied, "overflow seed rejected");
        [(NSTextField *)fields[@"Seed"] setStringValue:@"18446744073709551615"]; [save performClick:nil];
        require(applied && loaded.options.theme == 1 && loaded.options.seed == UINT64_MAX, "valid options apply");
        [defaults removePersistentDomainForName:domain];
        puts("PASS native settings, validation, persistence, and options controls");
    }
    return 0;
}
