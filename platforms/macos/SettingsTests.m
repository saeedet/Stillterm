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
        options.word_frequency = 2;
        settings.words = @"anything\nwake up";
        settings.options = options; settings.characters = @"ｱ01"; settings.fps = 60; settings.fontSize = 24;
        [settings save];
        StilltermSettings *loaded = [[StilltermSettings alloc] initWithDefaults:defaults];
        require(loaded.options.theme == 1 && loaded.options.seed == UINT64_MAX && loaded.fps == 60 &&
            loaded.fontSize == 24 && [loaded.characters isEqual:@"ｱ01"], "settings round trip, including u64 seed");
        require([loaded.words isEqual:@"anything\nwake up"] && loaded.options.word_frequency == 2, "phrases and frequency round trip");
        loaded.words = @"界"; require(!loaded.isValid, "wide phrase characters rejected");
        loaded.words = @"anything";
        loaded.characters = @"界";
        require(!loaded.isValid, "wide characters rejected");
        NSMutableDictionary *legacy = [[defaults dictionaryForKey:@"SettingsV1"] mutableCopy];
        [legacy removeObjectForKey:@"words"]; [legacy removeObjectForKey:@"wordFrequency"];
        [defaults setObject:legacy forKey:@"SettingsV1"];
        loaded = [[StilltermSettings alloc] initWithDefaults:defaults];
        require(loaded.options.theme == 1 && loaded.words.length == 0 && loaded.options.word_frequency == 1,
            "legacy preferences preserve theme and disable words");
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
            if ([view isKindOfClass:NSPopUpButton.class] && [view.accessibilityLabel isEqual:@"Theme"]) theme = (NSPopUpButton *)view;
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
        [(NSTextField *)fields[@"Seed"] setStringValue:@"18446744073709551615"];
        NSTextView *words = [controller valueForKey:@"words"];
        NSPopUpButton *frequency = [controller valueForKey:@"wordFrequency"];
        require(words && frequency, "phrase controls exist");
        words.string = @"界"; [save performClick:nil];
        require(!applied && loaded.words.length == 0, "invalid phrases do not change saved settings");
        words.string = @"anything\nwake up"; [frequency selectItemAtIndex:2];
        [theme selectItemAtIndex:0]; [NSApp sendAction:theme.action to:theme.target from:theme];
        require([words.string isEqual:@"anything\nwake up"] && frequency.indexOfSelectedItem == 2,
            "theme changes preserve personal phrases and frequency");
        [theme selectItemAtIndex:1]; [NSApp sendAction:theme.action to:theme.target from:theme];
        [save performClick:nil];
        require(applied && loaded.options.theme == 1 && loaded.options.seed == UINT64_MAX, "valid options apply");
        loaded = [[StilltermSettings alloc] initWithDefaults:defaults];
        require([loaded.words isEqual:@"anything\nwake up"] && loaded.options.word_frequency == 2,
            "options save persists multiline phrases");
        [theme selectItemAtIndex:2]; [NSApp sendAction:theme.action to:theme.target from:theme];
        require([[(NSTextField *)fields[@"Character size"] stringValue] isEqual:@"10"] &&
            [[(NSTextField *)fields[@"Density"] stringValue] isEqual:@"0.95"], "dense preset uses small characters and high density");
        [save performClick:nil];
        loaded = [[StilltermSettings alloc] initWithDefaults:defaults];
        require(loaded.options.theme == 2 && loaded.fontSize == 10 && [loaded.words isEqual:@"anything\nwake up"] &&
            loaded.options.word_frequency == 2, "dense theme round trip preserves words");
        [theme selectItemAtIndex:3]; [NSApp sendAction:theme.action to:theme.target from:theme];
        [save performClick:nil];
        loaded = [[StilltermSettings alloc] initWithDefaults:defaults];
        require(loaded.options.theme == 3 && loaded.fontSize == 18 && [loaded.words isEqual:@"anything\nwake up"],
            "experimental 3D mode persists and preserves words");
        [theme selectItemAtIndex:1]; [NSApp sendAction:theme.action to:theme.target from:theme];
        require([[(NSTextField *)fields[@"Character size"] stringValue] isEqual:@"18"], "leaving dense restores regular character size");
        __block BOOL canceled = NO;
        StilltermOptionsController *cancelController = [[StilltermOptionsController alloc] initWithSettings:loaded didApply:^{ canceled = YES; }];
        ((NSTextView *)[cancelController valueForKey:@"words"]).string = @"changed";
        for (NSView *control in cancelController.window.contentView.subviews) {
            if ([control isKindOfClass:NSButton.class] && [[(NSButton *)control title] isEqual:@"Cancel"]) [(NSButton *)control performClick:nil];
        }
        StilltermSettings *afterCancel = [[StilltermSettings alloc] initWithDefaults:defaults];
        require(!canceled && [afterCancel.words isEqual:@"anything\nwake up"], "cancel preserves saved words");
        [controller.window.contentView layoutSubtreeIfNeeded];
        NSView *content = controller.window.contentView;
        NSBitmapImageRep *rep = [content bitmapImageRepForCachingDisplayInRect:content.bounds];
        [content cacheDisplayInRect:content.bounds toBitmapImageRep:rep];
        require([[rep representationUsingType:NSBitmapImageFileTypePNG properties:@{}]
            writeToFile:@"target/macos/words-options.png" atomically:YES], "write options snapshot");
        [defaults removePersistentDomainForName:domain];
        puts("PASS native settings, validation, persistence, and options controls");
    }
    return 0;
}
