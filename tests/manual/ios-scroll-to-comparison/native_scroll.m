// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// cspell:ignore Autoresizing fabs instancetype nonatomic NSEC NSJSON NSURL NSUTF Subview subviews

#import <QuartzCore/QuartzCore.h>
#import <UIKit/UIKit.h>

extern float slint_scroll_offset(void);
extern float slint_animation_clock_lag_ms(void);
extern void set_slint_scroll_offset(float offset);
extern void slint_scroll_to(float offset);

typedef struct {
    float x, y, width, height, content_width, content_height;
} SlintScrollGeometry;
extern SlintScrollGeometry slint_scroll_geometry(void);

static NSString *environmentValue(NSString *name)
{
    return NSProcessInfo.processInfo.environment[name];
}

@class RecordingScrollView;

/// Forwards UIKit's touches to Slint's view, so both lists get the same delivered events.
@interface PassiveTouchForwarder : UIGestureRecognizer <UIGestureRecognizerDelegate>
- (instancetype)initWithHost:(UIView *)host scrollView:(RecordingScrollView *)scrollView;
@property (nonatomic, weak) UIView *host;
@property (nonatomic, weak) RecordingScrollView *scrollView;
@end

/// A `UIScrollView` that runs the scroll-to plan on itself and on Slint's list, and records both.
///
/// `SCROLL_TO_COMMANDS` is the plan: `delay:target` pairs separated by `;`, with the delay in
/// seconds from the plan origin and the target content offset in points.
/// With `SCROLL_TO_ANCHOR=launch`, the origin is `PLAN_START_DELAY_MS` after the first layout
/// plus a lead-in, during which both lists are recorded at rest.
/// With `SCROLL_TO_ANCHOR=release`, the origin is the first touch release.
/// The traces are saved `TRACE_SAVE_DELAY_MS` after the origin.
@interface RecordingScrollView : UIScrollView
@property (nonatomic, weak) UILabel *metricsLabel;
@property (nonatomic, weak) UILabel *statusLabel;
@property (nonatomic, strong) CADisplayLink *displayLink;
@property (nonatomic, strong) NSMutableString *trace;
@property (nonatomic, strong) NSMutableString *events;
@property (nonatomic, strong) NSMutableString *inputTrace;
@property (nonatomic) CFTimeInterval previousSampleTime;
@property (nonatomic) CGFloat previousNativeOffset;
@property (nonatomic) CGFloat previousSlintOffset;
@property (nonatomic) CGFloat nativeVelocity;
@property (nonatomic) CGFloat slintVelocity;
@property (nonatomic) BOOL hasPreviousSample;
@property (nonatomic) CGFloat fingerY;
@property (nonatomic) NSInteger phase;
@property (nonatomic) NSInteger requestedSampleRate;
@property (nonatomic) NSInteger saveGeneration;
@property (nonatomic) NSUInteger inputSequence;
@property (nonatomic) BOOL inputTracingEnabled;
@property (nonatomic) NSTimeInterval traceSaveDelay;
@property (nonatomic, copy) NSString *scenario;
@property (nonatomic, copy) NSString *anchor;
@property (nonatomic, copy) NSArray<NSNumber *> *commandDelays;
@property (nonatomic, copy) NSArray<NSNumber *> *commandTargets;
@property (nonatomic) NSUInteger nextCommand;
@property (nonatomic) CFTimeInterval commandOrigin;
@property (nonatomic) BOOL commandsArmed;
- (void)parseCommands:(NSString *)plan;
- (void)startTrace;
- (void)armCommandsAt:(CFTimeInterval)origin;
- (void)recordEvent:(NSString *)event link:(CADisplayLink *)link command:(NSInteger)command
             target:(CGFloat)target;
- (void)recordTouches:(NSSet<UITouch *> *)touches event:(UIEvent *)event phase:(NSInteger)phase;
@end

@implementation PassiveTouchForwarder
- (instancetype)initWithHost:(UIView *)host scrollView:(RecordingScrollView *)scrollView
{
    self = [super initWithTarget:nil action:nil];
    if (!self)
        return nil;
    self.host = host;
    self.scrollView = scrollView;
    self.delegate = self;
    self.cancelsTouchesInView = NO;
    return self;
}
- (void)touchesBegan:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event
{
    [self.scrollView recordTouches:touches event:event phase:0];
    [self.host touchesBegan:touches withEvent:event];
}
- (void)touchesMoved:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event
{
    [self.scrollView recordTouches:touches event:event phase:1];
    [self.host touchesMoved:touches withEvent:event];
}
- (void)touchesEnded:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event
{
    [self.scrollView recordTouches:touches event:event phase:2];
    [self.host touchesEnded:touches withEvent:event];
}
- (void)touchesCancelled:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event
{
    [self.scrollView recordTouches:touches event:event phase:3];
    [self.host touchesCancelled:touches withEvent:event];
}
- (BOOL)gestureRecognizer:(UIGestureRecognizer *)gestureRecognizer
        shouldRecognizeSimultaneouslyWithGestureRecognizer:
                (UIGestureRecognizer *)otherGestureRecognizer
{
    return YES;
}
@end

@implementation RecordingScrollView
- (void)parseCommands:(NSString *)plan
{
    NSMutableArray<NSNumber *> *delays = [NSMutableArray array];
    NSMutableArray<NSNumber *> *targets = [NSMutableArray array];
    for (NSString *command in [plan componentsSeparatedByString:@";"]) {
        NSArray<NSString *> *parts = [command componentsSeparatedByString:@":"];
        if (parts.count != 2)
            continue;
        [delays addObject:@(parts[0].doubleValue)];
        [targets addObject:@(parts[1].doubleValue)];
    }
    self.commandDelays = delays;
    self.commandTargets = targets;
    self.nextCommand = 0;
}
- (CGFloat)presentationOffset
{
    // `contentOffset` is the bounds origin; the presentation layer has what's on screen.
    CALayer *presentation = self.layer.presentationLayer;
    return presentation ? presentation.bounds.origin.y : NAN;
}
- (void)startTrace
{
    if (self.displayLink)
        return;
    self.trace = [NSMutableString
            stringWithString:@"callback_time,display_time,display_target_time,phase,finger_y,"
                              "uikit_offset,uikit_presentation_offset,slint_offset,"
                              "slint_clock_lag_ms,requested_hz\n"];
    self.events = [NSMutableString
            stringWithString:@"event,callback_time,display_time,display_target_time,command_index,"
                              "target_offset,uikit_offset,uikit_presentation_offset,slint_offset,"
                              "uikit_velocity,slint_velocity,slint_clock_lag_ms\n"];
    if (self.inputTracingEnabled) {
        self.inputTrace = [NSMutableString
                stringWithString:@"sequence,event_type,callback_time,event_timestamp,"
                                  "touch_timestamp,touch_phase,touch_id,coalesced_index,x,y,"
                                  "pan_state,pan_velocity_y,uikit_content_y,slint_content_y\n"];
        self.inputSequence = 0;
    }
    self.hasPreviousSample = NO;
    self.displayLink = [CADisplayLink displayLinkWithTarget:self selector:@selector(sample:)];
    self.requestedSampleRate = MIN(120, self.window.screen.maximumFramesPerSecond);
    self.displayLink.preferredFrameRateRange = CAFrameRateRangeMake(
            self.requestedSampleRate, self.requestedSampleRate, self.requestedSampleRate);
    [self.displayLink addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];
    self.statusLabel.text = @"recording";
}
- (void)armCommandsAt:(CFTimeInterval)origin
{
    if (self.commandsArmed)
        return;
    self.commandOrigin = origin;
    self.commandsArmed = YES;
    NSInteger generation = ++self.saveGeneration;
    NSTimeInterval delay = origin - CACurrentMediaTime() + self.traceSaveDelay;
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(delay * NSEC_PER_SEC)),
                   dispatch_get_main_queue(), ^{ [self saveTraceForGeneration:generation]; });
}
- (void)recordEvent:(NSString *)event link:(CADisplayLink *)link command:(NSInteger)command
             target:(CGFloat)target
{
    if (!self.events)
        return;
    [self.events appendFormat:@"%@,%.9f,%.9f,%.9f,%ld,%.3f,%.3f,%.3f,%.3f,%.3f,%.3f,%.3f\n",
                              event, CACurrentMediaTime(), link ? link.timestamp : NAN,
                              link ? link.targetTimestamp : NAN, (long)command, target,
                              self.contentOffset.y, [self presentationOffset],
                              slint_scroll_offset(), self.nativeVelocity, self.slintVelocity,
                              slint_animation_clock_lag_ms()];
}
- (void)sample:(CADisplayLink *)link
{
    CFTimeInterval now = CACurrentMediaTime();
    CGFloat nativeOffset = self.contentOffset.y;
    CGFloat slintOffset = slint_scroll_offset();
    if (self.hasPreviousSample && now > self.previousSampleTime) {
        CFTimeInterval elapsed = now - self.previousSampleTime;
        self.nativeVelocity = (nativeOffset - self.previousNativeOffset) / elapsed;
        self.slintVelocity = (slintOffset - self.previousSlintOffset) / elapsed;
    }
    self.metricsLabel.text = [NSString
            stringWithFormat:@"OFFSET     UIKit %8.1f   Slint %8.1f\n"
                              "DIFFERENCE       %+8.1f\n"
                              "VELOCITY   UIKit %8.0f   Slint %8.0f",
                             nativeOffset, slintOffset, slintOffset - nativeOffset,
                             self.nativeVelocity, self.slintVelocity];
    [self.trace appendFormat:@"%.9f,%.9f,%.9f,%ld,%.3f,%.3f,%.3f,%.3f,%.3f,%ld\n", now,
                             link.timestamp, link.targetTimestamp, (long)self.phase,
                             self.fingerY, nativeOffset, [self presentationOffset], slintOffset,
                             slint_animation_clock_lag_ms(), (long)self.requestedSampleRate];
    self.previousSampleTime = now;
    self.previousNativeOffset = nativeOffset;
    self.previousSlintOffset = slintOffset;
    self.hasPreviousSample = YES;

    // Both scroll-to calls start in the same callback, after the sample of the state before.
    while (self.commandsArmed && self.nextCommand < self.commandTargets.count
           && now >= self.commandOrigin + self.commandDelays[self.nextCommand].doubleValue) {
        CGFloat target = self.commandTargets[self.nextCommand].doubleValue;
        [self recordEvent:@"command" link:link command:self.nextCommand target:target];
        [self setContentOffset:CGPointMake(self.contentOffset.x, target) animated:YES];
        slint_scroll_to((float)target);
        self.nextCommand++;
    }
}
- (void)saveTraceForGeneration:(NSInteger)generation
{
    if (self.saveGeneration != generation)
        return;
    [self.displayLink invalidate];
    self.displayLink = nil;
    NSString *documents =
            NSSearchPathForDirectoriesInDomains(NSDocumentDirectory, NSUserDomainMask, YES)
                    .firstObject;
    NSString * (^path)(NSString *) = ^(NSString *prefix) {
        return [documents stringByAppendingPathComponent:
                                  [NSString stringWithFormat:@"%@-%@.csv", prefix, self.scenario]];
    };
    [self.trace writeToFile:path(@"scroll") atomically:YES encoding:NSUTF8StringEncoding error:nil];
    [self.events writeToFile:path(@"events")
                  atomically:YES
                    encoding:NSUTF8StringEncoding
                       error:nil];
    if (self.inputTracingEnabled)
        [self.inputTrace writeToFile:path(@"input")
                          atomically:YES
                            encoding:NSUTF8StringEncoding
                               error:nil];
    self.statusLabel.text = @"saved";
}
- (void)appendInput:(NSString *)type
          eventTime:(NSTimeInterval)eventTime
          touchTime:(NSTimeInterval)touchTime
              phase:(NSInteger)phase
            touchID:(uintptr_t)touchID
     coalescedIndex:(NSInteger)coalescedIndex
           location:(CGPoint)location
{
    if (!self.inputTrace)
        return;
    [self.inputTrace
            appendFormat:@"%lu,%@,%.9f,%.9f,%.9f,%ld,0x%lx,%ld,%.3f,%.3f,%ld,%.3f,%.3f,%.3f\n",
                         (unsigned long)++self.inputSequence, type, CACurrentMediaTime(),
                         eventTime, touchTime, (long)phase, (unsigned long)touchID,
                         (long)coalescedIndex, location.x, location.y,
                         (long)self.panGestureRecognizer.state,
                         [self.panGestureRecognizer velocityInView:self].y, self.contentOffset.y,
                         slint_scroll_offset()];
}
- (void)recordTouches:(NSSet<UITouch *> *)touches event:(UIEvent *)event phase:(NSInteger)phase
{
    if (phase == 0)
        [self startTrace];
    for (UITouch *touch in touches) {
        uintptr_t touchID = (uintptr_t)(__bridge void *)touch;
        [self appendInput:@"touch_callback"
                eventTime:event.timestamp
                touchTime:touch.timestamp
                    phase:touch.phase
                  touchID:touchID
           coalescedIndex:-1
                 location:[touch locationInView:self.window]];
        NSArray<UITouch *> *coalesced = [event coalescedTouchesForTouch:touch] ?: @[ touch ];
        [coalesced enumerateObjectsUsingBlock:^(UITouch *sample, NSUInteger index,
                                                BOOL *__unused stop) {
            [self appendInput:@"coalesced_sample"
                    eventTime:event.timestamp
                    touchTime:sample.timestamp
                        phase:sample.phase
                      touchID:touchID
               coalescedIndex:index
                     location:[sample locationInView:self.window]];
        }];
    }
    self.fingerY = [touches.anyObject locationInView:self.window].y;
    self.phase = phase;
    if ((phase == 2 || phase == 3) && [self.anchor isEqualToString:@"release"]) {
        [self recordEvent:@"release" link:nil command:-1 target:NAN];
        [self armCommandsAt:CACurrentMediaTime()];
    }
}
@end

@interface NativeScrollPane : UIView <UIScrollViewDelegate>
@property (nonatomic, strong) UIView *header;
@property (nonatomic, strong) UILabel *slintTitle;
@property (nonatomic, strong) UILabel *uikitTitle;
@property (nonatomic, strong) UILabel *metrics;
@property (nonatomic, strong) UILabel *status;
@property (nonatomic, strong) RecordingScrollView *scroll;
@property (nonatomic, strong) NSArray<UIView *> *rows;
@property (nonatomic) BOOL initialPositionApplied;
@end

@implementation NativeScrollPane
- (UILabel *)titleLabel:(NSString *)text red:(CGFloat)red green:(CGFloat)green blue:(CGFloat)blue
{
    UILabel *label = [[UILabel alloc] init];
    label.text = text;
    label.font = [UIFont systemFontOfSize:18 weight:UIFontWeightBold];
    label.textAlignment = NSTextAlignmentCenter;
    label.textColor = [UIColor colorWithRed:red green:green blue:blue alpha:1];
    return label;
}
- (instancetype)initWithFrame:(CGRect)frame host:(UIView *)host
{
    self = [super initWithFrame:frame];
    if (!self)
        return nil;
    self.backgroundColor = UIColor.clearColor;
    self.header = [[UIView alloc] init];
    self.header.backgroundColor = [UIColor colorWithWhite:1 alpha:0.94];
    self.header.layer.cornerRadius = 9;
    self.header.layer.masksToBounds = YES;
    self.header.userInteractionEnabled = NO;
    self.slintTitle = [self titleLabel:@"Slint" red:20.0 / 255 green:90.0 / 255 blue:170.0 / 255];
    self.uikitTitle = [self titleLabel:@"UIKit" red:184.0 / 255 green:20.0 / 255 blue:10.0 / 255];
    [self.header addSubview:self.slintTitle];
    [self.header addSubview:self.uikitTitle];
    [self addSubview:self.header];

    RecordingScrollView *scroll = [[RecordingScrollView alloc] init];
    self.scroll = scroll;
    NSString *scenario = environmentValue(@"SCROLL_SCENARIO");
    scroll.scenario = scenario.length > 0 ? scenario : @"manual";
    NSString *anchor = environmentValue(@"SCROLL_TO_ANCHOR");
    scroll.anchor = anchor.length > 0 ? anchor : @"launch";
    [scroll parseCommands:environmentValue(@"SCROLL_TO_COMMANDS") ?: @""];
    NSString *saveDelay = environmentValue(@"TRACE_SAVE_DELAY_MS");
    scroll.traceSaveDelay = saveDelay.length > 0 ? saveDelay.doubleValue / 1000 : 4;
    scroll.inputTracingEnabled = [environmentValue(@"INPUT_TRACE") boolValue];
    scroll.contentInsetAdjustmentBehavior = UIScrollViewContentInsetAdjustmentNever;
    scroll.decelerationRate = UIScrollViewDecelerationRateNormal;
    scroll.delegate = self;
    scroll.alwaysBounceVertical = YES;
    scroll.showsVerticalScrollIndicator = NO;
    [scroll addGestureRecognizer:[[PassiveTouchForwarder alloc] initWithHost:host
                                                                  scrollView:scroll]];
    [self addSubview:scroll];
    UIApplication.sharedApplication.idleTimerDisabled = YES;

    NSMutableArray *rows = [NSMutableArray arrayWithCapacity:1000];
    for (NSInteger row = 0; row < 1000; row++) {
        UIView *item = [[UIView alloc] init];
        item.userInteractionEnabled = NO;
        item.backgroundColor = row % 2 == 0
                ? [UIColor colorWithRed:1 green:0.18 blue:0.12 alpha:0.14]
                : [UIColor colorWithWhite:1 alpha:0.06];
        UILabel *label = [[UILabel alloc] init];
        label.text = [NSString stringWithFormat:@"UIKit %ld", (long)row + 1];
        label.font = [UIFont systemFontOfSize:16 weight:UIFontWeightSemibold];
        label.textColor = [UIColor colorWithRed:0.72 green:0.08 blue:0.04 alpha:0.85];
        [item addSubview:label];
        [scroll addSubview:item];
        [rows addObject:item];
    }
    self.rows = rows;

    self.metrics = [[UILabel alloc] init];
    self.metrics.accessibilityIdentifier = @"Scroll comparison metrics";
    self.metrics.numberOfLines = 3;
    self.metrics.font = [UIFont monospacedDigitSystemFontOfSize:12 weight:UIFontWeightSemibold];
    self.metrics.textColor = UIColor.whiteColor;
    self.metrics.backgroundColor = [UIColor colorWithWhite:0.06 alpha:0.82];
    self.metrics.layer.cornerRadius = 9;
    self.metrics.layer.masksToBounds = YES;
    self.metrics.userInteractionEnabled = NO;
    self.metrics.text = @"OFFSET     UIKit      0.0   Slint      0.0";
    [self addSubview:self.metrics];
    scroll.metricsLabel = self.metrics;

    // The UI test polls this label, so it only reads the app after the trace is saved.
    self.status = [[UILabel alloc] init];
    self.status.accessibilityIdentifier = @"Trace status";
    self.status.font = [UIFont monospacedDigitSystemFontOfSize:12 weight:UIFontWeightSemibold];
    self.status.textColor = UIColor.whiteColor;
    self.status.backgroundColor = [UIColor colorWithWhite:0.06 alpha:0.82];
    self.status.textAlignment = NSTextAlignmentCenter;
    self.status.userInteractionEnabled = NO;
    self.status.text = @"waiting";
    [self addSubview:self.status];
    scroll.statusLabel = self.status;
    return self;
}
- (void)scrollViewDidScroll:(UIScrollView *__unused)scrollView
{
    [self.scroll recordEvent:@"uikit_did_scroll" link:nil command:-1 target:NAN];
}
- (void)scrollViewDidEndScrollingAnimation:(UIScrollView *__unused)scrollView
{
    [self.scroll recordEvent:@"uikit_animation_end" link:nil command:-1 target:NAN];
}
- (void)scrollViewDidEndDecelerating:(UIScrollView *__unused)scrollView
{
    [self.scroll recordEvent:@"uikit_deceleration_end" link:nil command:-1 target:NAN];
}
- (void)layoutSubviews
{
    [super layoutSubviews];
    CGFloat width = self.bounds.size.width;
    self.header.frame = CGRectMake(8, 54, width - 16, 40);
    CGFloat headerWidth = self.header.bounds.size.width;
    self.slintTitle.frame = CGRectMake(0, 0, headerWidth / 2, 40);
    self.uikitTitle.frame = CGRectMake(headerWidth / 2, 0, headerWidth / 2, 40);
    SlintScrollGeometry geometry = slint_scroll_geometry();
    if (geometry.width <= 0 || geometry.height <= 0)
        return;
    self.scroll.frame = CGRectMake(geometry.x, geometry.y, geometry.width, geometry.height);
    self.scroll.contentSize = CGSizeMake(geometry.content_width, geometry.content_height);
    [self.rows enumerateObjectsUsingBlock:^(UIView *item, NSUInteger row, BOOL *__unused stop) {
        item.frame = CGRectMake(0, row * 72, geometry.content_width, 72);
        item.subviews.firstObject.frame = CGRectMake(width / 2 + 8, 0, width / 2 - 28, 72);
    }];
    self.metrics.frame = CGRectMake(width - 310, 110, 292, 60);
    self.status.frame = CGRectMake(width - 310, 176, 120, 22);
    if (self.initialPositionApplied)
        return;
    self.initialPositionApplied = YES;

    CGFloat maximum = MAX(0, self.scroll.contentSize.height - self.scroll.bounds.size.height);
    CGFloat offset = MIN(MAX(0, [environmentValue(@"START_OFFSET") doubleValue]), maximum);
    self.scroll.contentOffset = CGPointMake(0, offset);
    set_slint_scroll_offset((float)offset);
    NSDictionary *record = @{
        @"slint_viewport" : @[ @(geometry.x), @(geometry.y), @(geometry.width), @(geometry.height) ],
        @"uikit_viewport" : @[
            @(self.scroll.frame.origin.x), @(self.scroll.frame.origin.y),
            @(self.scroll.bounds.size.width), @(self.scroll.bounds.size.height)
        ],
        @"slint_content" : @[ @(geometry.content_width), @(geometry.content_height) ],
        @"uikit_content" : @[ @(self.scroll.contentSize.width), @(self.scroll.contentSize.height) ],
        @"maximum_offset" : @(maximum),
        @"start_offset" : @(offset),
        @"anchor" : self.scroll.anchor,
        @"commands" : environmentValue(@"SCROLL_TO_COMMANDS") ?: @"",
        @"maximum_frames_per_second" : @(self.window.screen.maximumFramesPerSecond),
    };
    NSURL *documents = [[NSFileManager defaultManager] URLsForDirectory:NSDocumentDirectory
                                                              inDomains:NSUserDomainMask]
                               .firstObject;
    NSData *data = [NSJSONSerialization dataWithJSONObject:record options:0 error:nil];
    NSString *name = [NSString stringWithFormat:@"geometry-%@.json", self.scroll.scenario];
    [data writeToURL:[documents URLByAppendingPathComponent:name] atomically:YES];

    if ([self.scroll.anchor isEqualToString:@"launch"] && self.scroll.commandTargets.count > 0) {
        NSString *startDelay = environmentValue(@"PLAN_START_DELAY_MS");
        NSString *leadIn = environmentValue(@"PLAN_LEAD_IN_MS");
        double start = startDelay.length > 0 ? startDelay.doubleValue / 1000 : 1.5;
        double lead = leadIn.length > 0 ? leadIn.doubleValue / 1000 : 0.3;
        RecordingScrollView *scroll = self.scroll;
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(start * NSEC_PER_SEC)),
                       dispatch_get_main_queue(), ^{
                           [scroll startTrace];
                           [scroll armCommandsAt:CACurrentMediaTime() + lead];
                       });
    }
}
@end

void install_native_scroll(void *hostPointer)
{
    UIView *host = (__bridge UIView *)hostPointer;
    NativeScrollPane *pane = [[NativeScrollPane alloc] initWithFrame:host.bounds host:host];
    pane.autoresizingMask = UIViewAutoresizingFlexibleWidth | UIViewAutoresizingFlexibleHeight;
    [host addSubview:pane];
}
