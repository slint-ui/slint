// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// cspell:ignore Autoresizing fabs instancetype nonatomic NSEC NSUTF Subview Subviews subviews uikit

#import <UIKit/UIKit.h>
#import <UIKit/UIGestureRecognizerSubclass.h>

extern float slint_scroll_offset(void);
extern void set_slint_scroll_offset(float offset);

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
@property (nonatomic) CFTimeInterval startTime;
@property (nonatomic) CFTimeInterval previousSampleTime;
@property (nonatomic) CGFloat previousNativeOffset;
@property (nonatomic) CGFloat previousSlintOffset;
@property (nonatomic) CGFloat fingerY;
@property (nonatomic) NSInteger phase;
@property (nonatomic) NSInteger saveGeneration;
@property (nonatomic) CGFloat nativeReleaseVelocity;
@property (nonatomic) CGFloat maxOffsetDifference;
@property (nonatomic) BOOL hasPreviousSample;
@property (nonatomic, copy) NSString *scenario;
- (void)recordTouches:(NSSet<UITouch *> *)touches phase:(NSInteger)phase;
@end

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
    [self.scrollView recordTouches:touches phase:0];
    [self.host touchesBegan:touches withEvent:event];
}
- (void)touchesMoved:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event
{
    [self.scrollView recordTouches:touches phase:1];
    [self.host touchesMoved:touches withEvent:event];
}
- (void)touchesEnded:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event
{
    self.scrollView.nativeReleaseVelocity =
            [self.scrollView.panGestureRecognizer velocityInView:self.scrollView].y;
    [self.scrollView recordTouches:touches phase:2];
    [self.host touchesEnded:touches withEvent:event];
}
- (void)touchesCancelled:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event
{
    [self.scrollView recordTouches:touches phase:3];
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
- (void)startTrace
{
    if (self.displayLink)
        return;
    self.trace = [NSMutableString
            stringWithString:@"time,phase,finger_y,uikit_offset,slint_offset,native_velocity,"
                              "uikit_frame_velocity,slint_frame_velocity,offset_difference\n"];
    self.startTime = CACurrentMediaTime();
    self.hasPreviousSample = NO;
    self.maxOffsetDifference = 0;
    self.displayLink = [CADisplayLink displayLinkWithTarget:self selector:@selector(sample:)];
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
                              "UIKitVelocity=%.3f, SlintVelocity=%.3f, MaxDifference=%.3f",
                             nativeOffset, slintOffset, difference, percent, nativeVelocity,
                             slintVelocity, self.maxOffsetDifference];
    [self.trace appendFormat:@"%.6f,%ld,%.3f,%.3f,%.3f,%.3f,%.3f,%.3f,%.3f\n", now - self.startTime,
                             (long)self.phase, self.fingerY, nativeOffset, slintOffset,
                             self.nativeReleaseVelocity, nativeVelocity, slintVelocity, difference];
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
    NSString *path =
            [NSSearchPathForDirectoriesInDomains(NSDocumentDirectory, NSUserDomainMask, YES)
                            .firstObject
                    stringByAppendingPathComponent:[NSString stringWithFormat:@"scroll-%@.csv",
                                                                              self.scenario]];
    [self.trace writeToFile:path atomically:YES encoding:NSUTF8StringEncoding error:nil];
}
- (void)recordTouches:(NSSet<UITouch *> *)touches phase:(NSInteger)phase
{
    if (phase == 0)
        [self startTrace];
    self.fingerY = [touches.anyObject locationInView:self].y;
    self.phase = phase;
    if (phase == 2 || phase == 3) {
        NSInteger generation = ++self.saveGeneration;
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC),
                       dispatch_get_main_queue(), ^{ [self saveTraceForGeneration:generation]; });
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
    NSString *scenario = NSProcessInfo.processInfo.environment[@"SCROLL_SCENARIO"];
    self.scroll.scenario = scenario.length > 0 ? scenario : @"manual";
    self.scroll.contentInsetAdjustmentBehavior = UIScrollViewContentInsetAdjustmentNever;
    self.scroll.decelerationRate = UIScrollViewDecelerationRateNormal;
    self.scroll.delegate = self;
    self.scroll.alwaysBounceVertical = YES;
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
- (void)layoutSubviews
{
    [super layoutSubviews];
    CGFloat width = self.bounds.size.width;
    self.header.frame = CGRectMake(8, 54, width - 16, 40);
    CGFloat headerWidth = self.header.bounds.size.width;
    self.slintTitle.frame = CGRectMake(0, 0, headerWidth / 2, 40);
    self.uikitTitle.frame = CGRectMake(headerWidth / 2, 0, headerWidth / 2, 40);
    self.scroll.frame = CGRectMake(8, 102, width - 16, self.bounds.size.height - 148);
    self.scroll.contentSize = CGSizeMake(width - 28, 1000 * 72);
    [self.rows enumerateObjectsUsingBlock:^(UIView *item, NSUInteger row, BOOL *__unused stop) {
        item.frame = CGRectMake(0, row * 72, width - 28, 72);
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
