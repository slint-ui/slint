// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// cspell:ignore Autoresizing fabs instancetype nonatomic NSEC NSJSON NSURL NSUTF Subview Subviews subviews uikit

#import <UIKit/UIKit.h>
#import <UIKit/UIGestureRecognizerSubclass.h>

extern float slint_scroll_offset(void);
extern float slint_animation_clock_lag_ms(void);
extern void set_slint_scroll_offset(float offset);

typedef struct {
    float x, y, width, height, content_width, content_height;
} SlintScrollGeometry;
extern SlintScrollGeometry slint_scroll_geometry(void);

@class ForwardingScrollView;

@interface PassiveTouchForwarder : UIGestureRecognizer <UIGestureRecognizerDelegate>
- (instancetype)initWithHost:(UIView *)host scrollView:(ForwardingScrollView *)scrollView;
@property (nonatomic, weak) UIView *host;
@property (nonatomic, weak) ForwardingScrollView *scrollView;
@end

@interface ForwardingScrollView : UIScrollView
@property (nonatomic, weak) UILabel *metricsLabel;
@property (nonatomic, strong) CADisplayLink *displayLink;
@property (nonatomic, strong) NSMutableString *trace;
@property (nonatomic, strong) NSMutableString *inputTrace;
@property (nonatomic) CFTimeInterval startTime;
@property (nonatomic) CFTimeInterval previousSampleTime;
@property (nonatomic) CGFloat previousNativeOffset;
@property (nonatomic) CGFloat previousSlintOffset;
@property (nonatomic) CGFloat fingerY;
@property (nonatomic) NSInteger phase;
@property (nonatomic) NSInteger saveGeneration;
@property (nonatomic) CGFloat nativeReleaseVelocity;
@property (nonatomic) CGFloat maxOffsetDifference;
@property (nonatomic) CGFloat maxNativeOverscroll;
@property (nonatomic) CGFloat maxSlintOverscroll;
@property (nonatomic) NSInteger requestedSampleRate;
@property (nonatomic) BOOL hasPreviousSample;
@property (nonatomic) NSUInteger inputSequence;
@property (nonatomic) NSUInteger eventBatch;
@property (nonatomic) NSUInteger deliveredMoveBatches;
@property (nonatomic) NSUInteger deliveredMoveSamples;
@property (nonatomic) CGPoint pressLocation;
@property (nonatomic) BOOL inputTracingEnabled;
@property (nonatomic) BOOL uniqueTraceFiles;
@property (nonatomic) NSTimeInterval traceSaveDelay;
@property (nonatomic) UIGestureRecognizerState lastLoggedPanState;
@property (nonatomic) BOOL hasLoggedPanState;
@property (nonatomic, copy) NSString *scenario;
- (void)recordTouches:(NSSet<UITouch *> *)touches event:(UIEvent *)event phase:(NSInteger)phase;
- (void)recordContentOffset;
- (void)recordSlintDrag;
@end

static __weak ForwardingScrollView *recordingScrollView;

void record_slint_drag(void)
{
    [recordingScrollView recordSlintDrag];
}

@implementation PassiveTouchForwarder
- (instancetype)initWithHost:(UIView *)host scrollView:(ForwardingScrollView *)scrollView
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
    if (![NSProcessInfo.processInfo.environment[@"NATIVE_ONLY_INPUT"] boolValue])
        [self.host touchesBegan:touches withEvent:event];
}
- (void)touchesMoved:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event
{
    [self.scrollView recordTouches:touches event:event phase:1];
    if (![NSProcessInfo.processInfo.environment[@"NATIVE_ONLY_INPUT"] boolValue])
        [self.host touchesMoved:touches withEvent:event];
}
- (void)touchesEnded:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event
{
    self.scrollView.nativeReleaseVelocity =
            [self.scrollView.panGestureRecognizer velocityInView:self.scrollView].y;
    [self.scrollView recordTouches:touches event:event phase:2];
    if (![NSProcessInfo.processInfo.environment[@"NATIVE_ONLY_INPUT"] boolValue])
        [self.host touchesEnded:touches withEvent:event];
}
- (void)touchesCancelled:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event
{
    [self.scrollView recordTouches:touches event:event phase:3];
    if (![NSProcessInfo.processInfo.environment[@"NATIVE_ONLY_INPUT"] boolValue])
        [self.host touchesCancelled:touches withEvent:event];
}
- (BOOL)gestureRecognizer:(UIGestureRecognizer *)gestureRecognizer
        shouldRecognizeSimultaneouslyWithGestureRecognizer:
                (UIGestureRecognizer *)otherGestureRecognizer
{
    return YES;
}
@end

@implementation ForwardingScrollView
- (NSString *)panStateName:(UIGestureRecognizerState)state
{
    switch (state) {
    case UIGestureRecognizerStatePossible:
        return @"possible";
    case UIGestureRecognizerStateBegan:
        return @"began";
    case UIGestureRecognizerStateChanged:
        return @"changed";
    case UIGestureRecognizerStateEnded:
        return @"ended";
    case UIGestureRecognizerStateCancelled:
        return @"cancelled";
    case UIGestureRecognizerStateFailed:
        return @"failed";
    }
    return @"unknown";
}
- (void)appendInputEvent:(NSString *)eventType
            callbackTime:(CFTimeInterval)callbackTime
          eventTimestamp:(NSTimeInterval)eventTimestamp
          touchTimestamp:(NSTimeInterval)touchTimestamp
              touchPhase:(NSInteger)touchPhase
                 touchID:(uintptr_t)touchID
          coalescedCount:(NSUInteger)coalescedCount
          coalescedIndex:(NSInteger)coalescedIndex
          predictedCount:(NSUInteger)predictedCount
                location:(CGPoint)location
        previousLocation:(CGPoint)previousLocation
{
    if (!self.inputTracingEnabled || !self.inputTrace)
        return;
    CGPoint translation = [self.panGestureRecognizer translationInView:self];
    CGPoint velocity = [self.panGestureRecognizer velocityInView:self];
    UIGestureRecognizerState panState = self.panGestureRecognizer.state;
    BOOL panStateChanged = !self.hasLoggedPanState || self.lastLoggedPanState != panState;
    self.lastLoggedPanState = panState;
    self.hasLoggedPanState = YES;
    CGPoint displacement =
            CGPointMake(location.x - self.pressLocation.x, location.y - self.pressLocation.y);
    [self.inputTrace
            appendFormat:@"%lu,%@,%.9f,%.9f,%.9f,%ld,%lu,0x%lx,%lu,%ld,%lu,%.3f,%.3f,"
                          "%.3f,%.3f,%.3f,%.3f,%@,%d,%.3f,%.3f,%.3f,%.3f,%.3f,%.3f,%lu,%lu,%.3f\n",
                         (unsigned long)++self.inputSequence, eventType, callbackTime,
                         eventTimestamp, touchTimestamp, (long)touchPhase,
                         (unsigned long)self.eventBatch, (unsigned long)touchID,
                         (unsigned long)coalescedCount, (long)coalescedIndex,
                         (unsigned long)predictedCount, location.x, location.y, previousLocation.x,
                         previousLocation.y, displacement.x, displacement.y,
                         [self panStateName:panState], panStateChanged, translation.x,
                         translation.y, velocity.x, velocity.y, self.contentOffset.y,
                         slint_scroll_offset(), (unsigned long)self.deliveredMoveBatches,
                         (unsigned long)self.deliveredMoveSamples, slint_animation_clock_lag_ms()];
}
- (void)recordPan:(UIPanGestureRecognizer *)recognizer
{
    if (!self.inputTracingEnabled || !self.inputTrace)
        return;
    CGPoint location = [recognizer locationInView:self.window];
    [self appendInputEvent:@"pan"
                callbackTime:CACurrentMediaTime()
              eventTimestamp:NAN
              touchTimestamp:NAN
                  touchPhase:-1
                     touchID:0
              coalescedCount:0
              coalescedIndex:-1
              predictedCount:0
                    location:location
            previousLocation:location];
}
- (void)recordContentOffset
{
    if (!self.inputTracingEnabled || !self.inputTrace)
        return;
    CGPoint location = CGPointMake(NAN, self.fingerY);
    [self appendInputEvent:@"content_offset"
                callbackTime:CACurrentMediaTime()
              eventTimestamp:NAN
              touchTimestamp:NAN
                  touchPhase:-1
                     touchID:0
              coalescedCount:0
              coalescedIndex:-1
              predictedCount:0
                    location:location
            previousLocation:location];
}
- (void)recordSlintDrag
{
    CGPoint location = CGPointMake(NAN, self.fingerY);
    [self appendInputEvent:@"slint_drag"
                callbackTime:CACurrentMediaTime()
              eventTimestamp:NAN
              touchTimestamp:NAN
                  touchPhase:-1
                     touchID:0
              coalescedCount:0
              coalescedIndex:-1
              predictedCount:0
                    location:location
            previousLocation:location];
}
- (void)startTrace
{
    if (self.displayLink)
        return;
    self.trace = [NSMutableString
            stringWithString:@"time,phase,finger_y,uikit_offset,slint_offset,native_velocity,"
                              "uikit_frame_velocity,slint_frame_velocity,offset_difference,"
                              "callback_time,display_time,display_target_time,requested_hz\n"];
    if (self.inputTracingEnabled) {
        self.inputTrace = [NSMutableString
                stringWithString:@"sequence,event_type,callback_time,event_timestamp,"
                                  "touch_timestamp,touch_phase,event_batch,touch_id,"
                                  "coalesced_count,coalesced_index,predicted_count,x,y,"
                                  "previous_x,previous_y,press_dx,press_dy,pan_state,"
                                  "pan_state_changed,"
                                  "pan_translation_x,pan_translation_y,pan_velocity_x,"
                                  "pan_velocity_y,uikit_content_y,slint_content_y,"
                                  "delivered_move_batches,delivered_move_samples,slint_clock_lag_ms\n"];
        self.inputSequence = 0;
        self.eventBatch = 0;
        self.deliveredMoveBatches = 0;
        self.deliveredMoveSamples = 0;
        self.hasLoggedPanState = NO;
    }
    self.startTime = CACurrentMediaTime();
    self.hasPreviousSample = NO;
    self.maxOffsetDifference = 0;
    self.maxNativeOverscroll = 0;
    self.maxSlintOverscroll = 0;
    self.displayLink = [CADisplayLink displayLinkWithTarget:self selector:@selector(sample:)];
    self.requestedSampleRate = MIN(120, self.window.screen.maximumFramesPerSecond);
    self.displayLink.preferredFrameRateRange = CAFrameRateRangeMake(
            self.requestedSampleRate, self.requestedSampleRate, self.requestedSampleRate);
    [self.displayLink addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];
}
- (void)sample:(CADisplayLink *)link
{
    CFTimeInterval now = CACurrentMediaTime();
    CGFloat nativeOffset = self.contentOffset.y;
    CGFloat slintOffset = slint_scroll_offset();
    CGFloat nativeVelocity = 0;
    CGFloat slintVelocity = 0;
    if (self.hasPreviousSample) {
        CFTimeInterval elapsed = now - self.previousSampleTime;
        if (elapsed > 0) {
            nativeVelocity = (nativeOffset - self.previousNativeOffset) / elapsed;
            slintVelocity = (slintOffset - self.previousSlintOffset) / elapsed;
        }
    }
    CGFloat difference = slintOffset - nativeOffset;
    self.maxOffsetDifference = MAX(self.maxOffsetDifference, fabs(difference));
    self.maxNativeOverscroll = MAX(self.maxNativeOverscroll, -nativeOffset);
    self.maxSlintOverscroll = MAX(self.maxSlintOverscroll, -slintOffset);
    CGFloat percent = nativeOffset == 0 ? 0 : difference / fabs(nativeOffset) * 100;
    self.metricsLabel.text =
            [NSString stringWithFormat:@"OFFSET     UIKit %8.1f   Slint %8.1f\n"
                                        "DIFFERENCE       %+8.1f   (%+6.1f%%)\n"
                                        "VELOCITY   UIKit %8.0f   Slint %8.0f\n"
                                        "VELOCITY Δ       %+8.0f\n"
                                        "MAX OFFSET Δ     %8.1f",
                                       nativeOffset, slintOffset, difference, percent,
                                       nativeVelocity, slintVelocity,
                                       slintVelocity - nativeVelocity, self.maxOffsetDifference];
    self.metricsLabel.accessibilityValue = [NSString
            stringWithFormat:@"UIKit=%.3f, Slint=%.3f, Difference=%.3f, Percent=%.3f, "
                              "UIKitVelocity=%.3f, SlintVelocity=%.3f, MaxDifference=%.3f, "
                              "UIKitPeakOverscroll=%.3f, SlintPeakOverscroll=%.3f, RequestedHz=%ld",
                             nativeOffset, slintOffset, difference, percent, nativeVelocity,
                             slintVelocity, self.maxOffsetDifference, self.maxNativeOverscroll,
                             self.maxSlintOverscroll, (long)self.requestedSampleRate];
    [self.trace appendFormat:@"%.6f,%ld,%.3f,%.3f,%.3f,%.3f,%.3f,%.3f,%.3f,%.9f,%.9f,%.9f,%ld\n", now - self.startTime,
                             (long)self.phase, self.fingerY, nativeOffset, slintOffset,
                             self.nativeReleaseVelocity, nativeVelocity, slintVelocity, difference,
                             now, link.timestamp, link.targetTimestamp, (long)self.requestedSampleRate];
    self.previousSampleTime = now;
    self.previousNativeOffset = nativeOffset;
    self.previousSlintOffset = slintOffset;
    self.hasPreviousSample = YES;
}
- (void)saveTraceForGeneration:(NSInteger)generation
{
    if (self.saveGeneration != generation)
        return;
    [self.displayLink invalidate];
    self.displayLink = nil;
    NSString *traceName = self.uniqueTraceFiles
            ? [NSString stringWithFormat:@"%@-%ld", self.scenario, (long)generation]
            : self.scenario;
    NSString *path =
            [NSSearchPathForDirectoriesInDomains(NSDocumentDirectory, NSUserDomainMask, YES)
                            .firstObject
                    stringByAppendingPathComponent:[NSString stringWithFormat:@"scroll-%@.csv",
                                                                              traceName]];
    [self.trace writeToFile:path atomically:YES encoding:NSUTF8StringEncoding error:nil];
    if (self.inputTracingEnabled) {
        NSString *inputPath =
                [NSSearchPathForDirectoriesInDomains(NSDocumentDirectory, NSUserDomainMask, YES)
                                .firstObject
                        stringByAppendingPathComponent:[NSString stringWithFormat:@"input-%@.csv",
                                                                                  traceName]];
        [self.inputTrace writeToFile:inputPath
                          atomically:YES
                            encoding:NSUTF8StringEncoding
                               error:nil];
    }
}
- (void)recordTouches:(NSSet<UITouch *> *)touches event:(UIEvent *)event phase:(NSInteger)phase
{
    if (phase == 0) {
        ++self.saveGeneration;
        [self startTrace];
    }
    self.eventBatch++;
    if (phase == 1)
        self.deliveredMoveBatches++;
    for (UITouch *touch in touches) {
        CGPoint location = [touch locationInView:self.window];
        CGPoint previousLocation = [touch previousLocationInView:self.window];
        if (phase == 0)
            self.pressLocation = location;
        NSArray<UITouch *> *coalescedTouches = [event coalescedTouchesForTouch:touch];
        if (!coalescedTouches)
            coalescedTouches = @[ touch ];
        NSArray<UITouch *> *predictedTouches = [event predictedTouchesForTouch:touch];
        if (phase == 1)
            self.deliveredMoveSamples += coalescedTouches.count;
        [self appendInputEvent:@"touch_callback"
                    callbackTime:CACurrentMediaTime()
                  eventTimestamp:event.timestamp
                  touchTimestamp:touch.timestamp
                      touchPhase:touch.phase
                         touchID:(uintptr_t)(__bridge void *)touch
                  coalescedCount:coalescedTouches.count
                  coalescedIndex:-1
                  predictedCount:predictedTouches.count
                        location:location
                previousLocation:previousLocation];
        [coalescedTouches enumerateObjectsUsingBlock:^(UITouch *sample, NSUInteger index,
                                                       BOOL *__unused stop) {
            [self appendInputEvent:@"coalesced_sample"
                        callbackTime:CACurrentMediaTime()
                      eventTimestamp:event.timestamp
                      touchTimestamp:sample.timestamp
                          touchPhase:sample.phase
                             touchID:(uintptr_t)(__bridge void *)touch
                      coalescedCount:coalescedTouches.count
                      coalescedIndex:index
                      predictedCount:predictedTouches.count
                            location:[sample locationInView:self.window]
                    previousLocation:[sample previousLocationInView:self.window]];
        }];
    }
    self.fingerY = [touches.anyObject locationInView:self.window].y;
    self.phase = phase;
    if (phase == 2 || phase == 3) {
        NSInteger generation = ++self.saveGeneration;
        int64_t delay = (int64_t)(self.traceSaveDelay * NSEC_PER_SEC);
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, delay), dispatch_get_main_queue(),
                       ^{ [self saveTraceForGeneration:generation]; });
    }
}
@end

@interface NativeScrollPane : UIView <UIScrollViewDelegate>
@property (nonatomic, strong) UIView *header;
@property (nonatomic, strong) UILabel *slintTitle;
@property (nonatomic, strong) UILabel *uikitTitle;
@property (nonatomic, strong) UILabel *metrics;
@property (nonatomic, strong) ForwardingScrollView *scroll;
@property (nonatomic, strong) NSArray<UIView *> *rows;
@property (nonatomic) BOOL initialPositionApplied;
@end

@implementation NativeScrollPane
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
    self.slintTitle = [[UILabel alloc] init];
    self.slintTitle.text = @"Slint";
    self.slintTitle.font = [UIFont systemFontOfSize:18 weight:UIFontWeightBold];
    self.slintTitle.textAlignment = NSTextAlignmentCenter;
    self.slintTitle.textColor = [UIColor colorWithRed:20.0 / 255
                                                green:90.0 / 255
                                                 blue:170.0 / 255
                                                alpha:1];
    self.uikitTitle = [[UILabel alloc] init];
    self.uikitTitle.text = @"UIKit";
    self.uikitTitle.font = [UIFont systemFontOfSize:18 weight:UIFontWeightBold];
    self.uikitTitle.textAlignment = NSTextAlignmentCenter;
    self.uikitTitle.textColor = [UIColor colorWithRed:184.0 / 255
                                                green:20.0 / 255
                                                 blue:10.0 / 255
                                                alpha:1];
    [self.header addSubview:self.slintTitle];
    [self.header addSubview:self.uikitTitle];
    [self addSubview:self.header];
    self.scroll = [[ForwardingScrollView alloc] init];
    recordingScrollView = self.scroll;
    NSString *scenario = NSProcessInfo.processInfo.environment[@"SCROLL_SCENARIO"];
    self.scroll.scenario = scenario.length > 0 ? scenario : @"manual";
    self.scroll.contentInsetAdjustmentBehavior = UIScrollViewContentInsetAdjustmentNever;
    self.scroll.decelerationRate = UIScrollViewDecelerationRateNormal;
    self.scroll.delegate = self;
    self.scroll.alwaysBounceVertical = YES;
    self.scroll.inputTracingEnabled =
            [NSProcessInfo.processInfo.environment[@"INPUT_TRACE"] boolValue];
    if (self.scroll.inputTracingEnabled)
        UIApplication.sharedApplication.idleTimerDisabled = YES;
    self.scroll.uniqueTraceFiles =
            [NSProcessInfo.processInfo.environment[@"TRACE_UNIQUE_FILES"] boolValue];
    NSString *traceSaveDelay = NSProcessInfo.processInfo.environment[@"TRACE_SAVE_DELAY_MS"];
    self.scroll.traceSaveDelay = traceSaveDelay.length > 0 ? traceSaveDelay.doubleValue / 1000 : 5;
    [self.scroll.panGestureRecognizer addTarget:self.scroll action:@selector(recordPan:)];
    [self.scroll addGestureRecognizer:[[PassiveTouchForwarder alloc] initWithHost:host
                                                                       scrollView:self.scroll]];
    [self addSubview:self.scroll];
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
        [self.scroll addSubview:item];
        [rows addObject:item];
    }
    self.rows = rows;
    self.metrics = [[UILabel alloc] init];
    self.metrics.accessibilityIdentifier = @"Scroll comparison metrics";
    self.metrics.numberOfLines = 5;
    self.metrics.font = [UIFont monospacedDigitSystemFontOfSize:12 weight:UIFontWeightSemibold];
    self.metrics.textColor = UIColor.whiteColor;
    self.metrics.backgroundColor = [UIColor colorWithWhite:0.06 alpha:0.82];
    self.metrics.layer.cornerRadius = 9;
    self.metrics.layer.masksToBounds = YES;
    self.metrics.userInteractionEnabled = NO;
    self.metrics.text = @"OFFSET     UIKit      0.0   Slint      0.0\n"
                         "DIFFERENCE           +0.0   (  +0.0%)\n"
                         "VELOCITY   UIKit        0   Slint        0\n"
                         "VELOCITY Δ             +0\n"
                         "MAX OFFSET Δ          0.0";
    self.metrics.accessibilityValue =
            @"UIKit=0.000, Slint=0.000, Difference=0.000, Percent=0.000, "
             "UIKitVelocity=0.000, SlintVelocity=0.000, MaxDifference=0.000";
    [self addSubview:self.metrics];
    self.scroll.metricsLabel = self.metrics;
    return self;
}
- (void)scrollViewWillEndDragging:(UIScrollView *)scrollView
                     withVelocity:(CGPoint)velocity
              targetContentOffset:(inout CGPoint *)targetContentOffset
{
    self.scroll.nativeReleaseVelocity = velocity.y;
}
- (void)scrollViewDidScroll:(UIScrollView *__unused)scrollView
{
    [self.scroll recordContentOffset];
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
    self.metrics.frame = CGRectMake(width - 310, 110, 292, 116);
    if (!self.initialPositionApplied) {
        NSDictionary<NSString *, NSString *> *environment = NSProcessInfo.processInfo.environment;
        CGFloat offset = [environment[@"START_OFFSET"] doubleValue];
        if ([environment[@"START_FROM_BOTTOM"] boolValue]) {
            offset = MAX(0,
                         self.scroll.contentSize.height - self.scroll.bounds.size.height -
                                 [environment[@"START_FROM_BOTTOM_DISTANCE"] doubleValue]);
        }
        offset = MIN(MAX(0, offset),
                     MAX(0, self.scroll.contentSize.height - self.scroll.bounds.size.height));
        self.scroll.contentOffset = CGPointMake(0, offset);
        set_slint_scroll_offset((float)offset);
        NSDictionary *record = @{
            @"slint_viewport": @[@(geometry.x), @(geometry.y), @(geometry.width), @(geometry.height)],
            @"uikit_viewport": @[@(self.scroll.frame.origin.x), @(self.scroll.frame.origin.y),
                                  @(self.scroll.bounds.size.width), @(self.scroll.bounds.size.height)],
            @"slint_content": @[@(geometry.content_width), @(geometry.content_height)],
            @"uikit_content": @[@(self.scroll.contentSize.width), @(self.scroll.contentSize.height)],
            @"slint_offset": @(slint_scroll_offset()),
            @"uikit_offset": @(self.scroll.contentOffset.y)
        };
        NSURL *directory = [[NSFileManager defaultManager] URLsForDirectory:NSDocumentDirectory
                                                                inDomains:NSUserDomainMask].firstObject;
        NSData *data = [NSJSONSerialization dataWithJSONObject:record options:0 error:nil];
        [data writeToURL:[directory URLByAppendingPathComponent:@"viewport-geometry.json"] atomically:YES];
        NSString *scenarioGeometry =
                [NSString stringWithFormat:@"geometry-%@.json", self.scroll.scenario];
        [data writeToURL:[directory URLByAppendingPathComponent:scenarioGeometry] atomically:YES];
        self.initialPositionApplied = YES;
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
