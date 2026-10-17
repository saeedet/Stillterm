#import "StilltermSettings.h"
#import <ScreenSaver/ScreenSaver.h>
#include <errno.h>
#include <math.h>

@implementation StilltermSettings {
    NSUserDefaults *_defaults;
}
- (instancetype)initWithDefaults:(NSUserDefaults *)defaults {
    if ((self = [super init])) {
        _defaults = defaults;
        st_defaults(0, &_options);
        _fps = 30;
        _fontSize = 18;
        _characters = @"";
        _words = @"";
        // ScreenSaverDefaults caches its own module dictionary across view
        // instances. Refresh it before reading changes saved by another host.
        // This is specific to ScreenSaverDefaults, not ordinary NSUserDefaults.
        if ([defaults isKindOfClass:ScreenSaverDefaults.class]) [defaults synchronize];
        NSDictionary *saved = [defaults dictionaryForKey:@"SettingsV1"];
        if (saved) {
            NSArray *numeric = @[@"theme", @"seed", @"speed", @"density", @"intensity", @"fps", @"fontSize"];
            BOOL valid = [saved[@"characters"] isKindOfClass:NSString.class];
            for (NSString *key in numeric) valid &= [saved[key] isKindOfClass:NSNumber.class];
            valid &= !saved[@"words"] || [saved[@"words"] isKindOfClass:NSString.class];
            valid &= !saved[@"wordFrequency"] || [saved[@"wordFrequency"] isKindOfClass:NSNumber.class];
            if (valid) {
                _options = (StOptions){[saved[@"theme"] unsignedIntValue], [saved[@"seed"] unsignedLongLongValue],
                    [saved[@"speed"] doubleValue], [saved[@"density"] doubleValue], [saved[@"intensity"] doubleValue],
                    saved[@"wordFrequency"] ? [saved[@"wordFrequency"] unsignedIntValue] : 1};
                _fps = [saved[@"fps"] integerValue];
                _fontSize = [saved[@"fontSize"] doubleValue];
                _characters = [saved[@"characters"] copy];
                _words = [saved[@"words"] copy] ?: @"";
            }
            if (!valid || ![self isValid]) {
                st_defaults(0, &_options); _fps = 30; _fontSize = 18; _characters = @""; _words = @"";
            }
        }
    }
    return self;
}
- (BOOL)isValid {
    if (_fps < 10 || _fps > 60 || !isfinite(_fontSize) || _fontSize < 10 || _fontSize > 48) return NO;
    NSData *data = [_characters dataUsingEncoding:NSUTF8StringEncoding];
    NSData *words = [_words dataUsingEncoding:NSUTF8StringEncoding];
    if (!data || !words) return NO;
    StEngine *probe = st_create_words(0, 0, _options, data.bytes, data.length, words.bytes, words.length);
    if (!probe) return NO;
    st_destroy(probe);
    return YES;
}
- (void)save {
    if (![self isValid]) return;
    [_defaults setObject:@{@"theme": @(_options.theme), @"seed": @(_options.seed),
        @"speed": @(_options.speed), @"density": @(_options.density), @"intensity": @(_options.intensity),
        @"fps": @(_fps), @"fontSize": @(_fontSize), @"characters": _characters,
        @"words": _words, @"wordFrequency": @(_options.word_frequency)} forKey:@"SettingsV1"];
    [_defaults synchronize];
}
@end

@implementation StilltermOptionsController {
    StilltermSettings *_settings;
    void (^_didApply)(void);
    NSPopUpButton *_theme;
    NSPopUpButton *_wordFrequency;
    NSTextView *_words;
    NSMutableDictionary<NSString *, NSTextField *> *_fields;
    NSTextField *_error;
}
- (instancetype)initWithSettings:(StilltermSettings *)settings didApply:(void (^)(void))didApply {
    NSWindow *window = [[NSWindow alloc] initWithContentRect:NSMakeRect(0, 0, 440, 650)
        styleMask:NSWindowStyleMaskTitled backing:NSBackingStoreBuffered defer:NO];
    if ((self = [super initWithWindow:window])) {
        _settings = settings;
        _didApply = [didApply copy];
        _fields = [NSMutableDictionary dictionary];
        window.title = @"Stillterm Options";
        window.releasedWhenClosed = NO;
        NSView *content = window.contentView;
        NSTextField *label = [NSTextField labelWithString:@"Theme"];
        label.frame = NSMakeRect(24, 596, 115, 24); [content addSubview:label];
        _theme = [[NSPopUpButton alloc] initWithFrame:NSMakeRect(145, 594, 265, 28) pullsDown:NO];
        [_theme addItemsWithTitles:@[@"Monochrome", @"Matrix"]];
        [_theme selectItemAtIndex:settings.options.theme];
        _theme.target = self; _theme.action = @selector(themeChanged:);
        _theme.accessibilityLabel = @"Theme";
        [content addSubview:_theme];
        NSArray *names = @[@"Speed", @"Density", @"Brightness", @"Frames per second", @"Character size", @"Seed", @"Characters"];
        NSArray *values = @[[NSString stringWithFormat:@"%g", settings.options.speed],
            [NSString stringWithFormat:@"%g", settings.options.density],
            [NSString stringWithFormat:@"%g", settings.options.intensity],
            @(settings.fps).stringValue, @(settings.fontSize).stringValue,
            @(settings.options.seed).stringValue, settings.characters];
        for (NSUInteger i = 0; i < names.count; i++) {
            CGFloat y = 554 - i * 38;
            NSTextField *title = [NSTextField labelWithString:names[i]];
            title.frame = NSMakeRect(24, y, 122, 24); [content addSubview:title];
            NSTextField *field = [[NSTextField alloc] initWithFrame:NSMakeRect(150, y, 260, 24)];
            field.stringValue = values[i]; field.accessibilityLabel = names[i];
            [content addSubview:field]; _fields[names[i]] = field;
        }
        _fields[@"Characters"].placeholderString = @"Blank uses theme characters";
        NSTextField *wordsLabel = [NSTextField labelWithString:@"Words / phrases"];
        wordsLabel.frame = NSMakeRect(24, 288, 122, 24); [content addSubview:wordsLabel];
        NSScrollView *scroll = [[NSScrollView alloc] initWithFrame:NSMakeRect(150, 218, 260, 94)];
        scroll.borderType = NSBezelBorder; scroll.hasVerticalScroller = YES;
        _words = [[NSTextView alloc] initWithFrame:NSMakeRect(0, 0, 240, 94)];
        _words.richText = NO; _words.font = [NSFont systemFontOfSize:13];
        _words.automaticQuoteSubstitutionEnabled = NO; _words.automaticDashSubstitutionEnabled = NO;
        _words.verticallyResizable = YES; _words.horizontallyResizable = NO;
        _words.maxSize = NSMakeSize(CGFLOAT_MAX, CGFLOAT_MAX);
        _words.textContainer.widthTracksTextView = YES;
        _words.string = settings.words; _words.accessibilityLabel = @"Words / phrases";
        scroll.documentView = _words; [content addSubview:scroll];
        NSTextField *frequencyLabel = [NSTextField labelWithString:@"Word frequency"];
        frequencyLabel.frame = NSMakeRect(24, 178, 122, 24); [content addSubview:frequencyLabel];
        _wordFrequency = [[NSPopUpButton alloc] initWithFrame:NSMakeRect(145, 176, 265, 28) pullsDown:NO];
        [_wordFrequency addItemsWithTitles:@[@"Rare", @"Occasional", @"Frequent"]];
        [_wordFrequency selectItemAtIndex:settings.options.word_frequency];
        _wordFrequency.accessibilityLabel = @"Word frequency"; [content addSubview:_wordFrequency];
        NSTextField *hint = [NSTextField wrappingLabelWithString:@"Words: one per line; blank disables. Up to 16 phrases, 64 single-width characters each.\nSpeed 0.1–4 · Density/brightness 0–1 · 10–60 FPS · Size 10–48"];
        hint.frame = NSMakeRect(24, 111, 390, 58); hint.textColor = NSColor.secondaryLabelColor;
        hint.font = [NSFont systemFontOfSize:11]; [content addSubview:hint];
        _error = [NSTextField wrappingLabelWithString:@""];
        _error.frame = NSMakeRect(24, 54, 390, 54); _error.textColor = NSColor.systemRedColor;
        _error.font = [NSFont systemFontOfSize:11]; [content addSubview:_error];
        NSButton *cancel = [NSButton buttonWithTitle:@"Cancel" target:self action:@selector(cancel:)];
        cancel.frame = NSMakeRect(232, 14, 86, 30); cancel.keyEquivalent = @"\e"; [content addSubview:cancel];
        NSButton *save = [NSButton buttonWithTitle:@"Save" target:self action:@selector(save:)];
        save.frame = NSMakeRect(324, 14, 86, 30); save.keyEquivalent = @"\r"; [content addSubview:save];
    }
    return self;
}
- (void)themeChanged:(id)sender {
    (void)sender;
    StOptions options;
    st_defaults((uint32_t)_theme.indexOfSelectedItem, &options);
    _fields[@"Speed"].stringValue = @(options.speed).stringValue;
    _fields[@"Density"].stringValue = @(options.density).stringValue;
    _fields[@"Brightness"].stringValue = @(options.intensity).stringValue;
    _fields[@"Characters"].stringValue = @"";
}
- (void)finish {
    if (self.window.sheetParent) [self.window.sheetParent endSheet:self.window];
    else [NSApp endSheet:self.window];
    [self.window orderOut:nil];
}
- (void)cancel:(id)sender { (void)sender; [self finish]; }
- (void)save:(id)sender {
    (void)sender;
    // Parse whole fields; Cocoa's doubleValue alone silently accepts bad suffixes.
    NSArray *keys = @[@"Speed", @"Density", @"Brightness", @"Frames per second", @"Character size"];
    double values[5];
    for (NSUInteger i = 0; i < keys.count; i++) {
        NSScanner *scanner = [NSScanner scannerWithString:_fields[keys[i]].stringValue];
        scanner.locale = [NSLocale localeWithLocaleIdentifier:@"en_US_POSIX"];
        if (![scanner scanDouble:&values[i]] || !scanner.isAtEnd || !isfinite(values[i])) {
            _error.stringValue = @"Enter valid numbers in all numeric fields."; return;
        }
    }
    NSString *seedText = [_fields[@"Seed"].stringValue stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceAndNewlineCharacterSet];
    if (seedText.length == 0 || [seedText rangeOfCharacterFromSet:
        [[NSCharacterSet characterSetWithCharactersInString:@"0123456789"] invertedSet]].location != NSNotFound) {
        _error.stringValue = @"Seed must be a whole number from 0 to 18446744073709551615."; return;
    }
    errno = 0; char *end;
    uint64_t seed = strtoull(seedText.UTF8String, &end, 10);
    if (errno || *end || values[3] != floor(values[3]) || values[3] < 10 || values[3] > 60) {
        _error.stringValue = @"Check the seed and use a whole FPS value from 10 to 60."; return;
    }
    StOptions old = _settings.options;
    NSInteger oldFPS = _settings.fps; CGFloat oldSize = _settings.fontSize;
    NSString *oldCharacters = _settings.characters;
    NSString *oldWords = _settings.words;
    _settings.options = (StOptions){(uint32_t)_theme.indexOfSelectedItem, seed, values[0], values[1], values[2], (uint32_t)_wordFrequency.indexOfSelectedItem};
    _settings.fps = (NSInteger)values[3]; _settings.fontSize = values[4];
    _settings.characters = _fields[@"Characters"].stringValue;
    _settings.words = _words.string;
    if (![_settings isValid]) {
        _settings.options = old; _settings.fps = oldFPS; _settings.fontSize = oldSize; _settings.characters = oldCharacters; _settings.words = oldWords;
        _error.stringValue = @"Check the ranges and use printable, single-width text. Characters: up to 256. Words: 16 lines, up to 64 characters per phrase."; return;
    }
    [_settings save]; if (_didApply) _didApply(); [self finish];
}
@end
