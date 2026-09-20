// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#import <UIKit/UIKit.h>
#import <UIKit/UIGestureRecognizerSubclass.h>

extern void mirror_native_touch(int phase, float x, float y);
extern float slint_scroll_offset(void);
extern void set_slint_scroll_offset(float offset);

@interface ForwardingScrollView : UIScrollView
@property(nonatomic, weak) UIView *host;
@property(nonatomic, strong) CADisplayLink *displayLink;
@property(nonatomic, strong) NSMutableString *trace;
@property(nonatomic) CFTimeInterval startTime;
@property(nonatomic) CGFloat fingerY;
@property(nonatomic) NSInteger phase;
@property(nonatomic) CGFloat nativeReleaseVelocity;
@property(nonatomic, copy) NSString *scenario;
@end

@implementation ForwardingScrollView
- (void)sample:(CADisplayLink *)link {
    [self.trace appendFormat:@"%.6f,%ld,%.3f,%.3f,%.3f,%.3f\n",
        CACurrentMediaTime() - self.startTime, (long)self.phase, self.fingerY,
        self.contentOffset.y, slint_scroll_offset(), self.nativeReleaseVelocity];
}
- (void)saveTraceForStart:(CFTimeInterval)start {
    if (self.startTime != start) return;
    [self.displayLink invalidate];
    NSString *path = [NSSearchPathForDirectoriesInDomains(
        NSDocumentDirectory, NSUserDomainMask, YES).firstObject
        stringByAppendingPathComponent:[NSString stringWithFormat:@"scroll-%@.csv", self.scenario]];
    [self.trace writeToFile:path atomically:YES encoding:NSUTF8StringEncoding error:nil];
}
- (void)forward:(NSSet<UITouch *> *)touches phase:(NSInteger)phase {
    CGPoint point = [touches.anyObject locationInView:self.host];
    self.fingerY = point.y;
    self.phase = phase;
    mirror_native_touch((int)phase, point.x + self.host.bounds.size.width / 2, point.y);
}
- (void)touchesBegan:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event {
    [self.displayLink invalidate];
    self.trace = [NSMutableString stringWithString:@"time,phase,finger_y,uikit_offset,slint_offset,native_velocity\n"];
    self.startTime = CACurrentMediaTime();
    self.displayLink = [CADisplayLink displayLinkWithTarget:self selector:@selector(sample:)];
    [self.displayLink addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];
    [self forward:touches phase:0];
    [super touchesBegan:touches withEvent:event];
}
- (void)touchesMoved:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event {
    [self forward:touches phase:1];
    [super touchesMoved:touches withEvent:event];
}
- (void)touchesEnded:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event {
    self.nativeReleaseVelocity = [self.panGestureRecognizer velocityInView:self].y;
    [self forward:touches phase:2];
    CFTimeInterval start = self.startTime;
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC), dispatch_get_main_queue(), ^{
        [self saveTraceForStart:start];
    });
    [super touchesEnded:touches withEvent:event];
}
- (void)touchesCancelled:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event {
    [self forward:touches phase:3];
    [super touchesCancelled:touches withEvent:event];
}
@end

@interface NativeScrollPane : UIView <UIScrollViewDelegate>
@property(nonatomic, strong) UILabel *title;
@property(nonatomic, strong) ForwardingScrollView *scroll;
@property(nonatomic, strong) NSArray<UIView *> *rows;
@property(nonatomic) BOOL initialPositionApplied;
@end

@implementation NativeScrollPane
- (instancetype)initWithFrame:(CGRect)frame {
    self = [super initWithFrame:frame];
    if (!self) return nil;
    self.backgroundColor = [UIColor colorWithRed:244.0/255 green:246.0/255 blue:250.0/255 alpha:1];
    self.title = [[UILabel alloc] init];
    self.title.text = @"UIKit";
    self.title.font = [UIFont systemFontOfSize:20 weight:UIFontWeightBold];
    self.title.textAlignment = NSTextAlignmentCenter;
    self.title.textColor = [UIColor colorWithRed:23.0/255 green:35.0/255 blue:59.0/255 alpha:1];
    [self addSubview:self.title];
    self.scroll = [[ForwardingScrollView alloc] init];
    NSString *scenario = NSProcessInfo.processInfo.environment[@"SCROLL_SCENARIO"];
    self.scroll.scenario = scenario.length > 0 ? scenario : @"manual";
    self.scroll.contentInsetAdjustmentBehavior = UIScrollViewContentInsetAdjustmentNever;
    self.scroll.decelerationRate = UIScrollViewDecelerationRateNormal;
    self.scroll.delegate = self;
    self.scroll.alwaysBounceVertical = YES;
    [self addSubview:self.scroll];
    NSMutableArray *rows = [NSMutableArray arrayWithCapacity:1000];
    for (NSInteger row = 0; row < 1000; row++) {
        UIView *item = [[UIView alloc] init];
        item.userInteractionEnabled = NO;
        item.backgroundColor = row % 2 == 0
            ? [UIColor colorWithRed:228.0/255 green:235.0/255 blue:245.0/255 alpha:1]
            : UIColor.whiteColor;
        UILabel *label = [[UILabel alloc] init];
        label.text = [NSString stringWithFormat:@"Row %ld", (long)row + 1];
        label.font = [UIFont systemFontOfSize:20];
        label.textColor = self.title.textColor;
        [item addSubview:label];
        [self.scroll addSubview:item];
        [rows addObject:item];
    }
    self.rows = rows;
    return self;
}
- (void)scrollViewWillEndDragging:(UIScrollView *)scrollView
        withVelocity:(CGPoint)velocity
        targetContentOffset:(inout CGPoint *)targetContentOffset {
    self.scroll.nativeReleaseVelocity = velocity.y;
}

- (void)layoutSubviews {
    [super layoutSubviews];
    CGFloat width = self.bounds.size.width;
    self.title.frame = CGRectMake(8, 64, width - 16, 40);
    // Slint's Material ScrollView reserves 12px for the disabled horizontal bar
    // and its padding, so use the same effective viewport height in UIKit.
    self.scroll.frame = CGRectMake(8, 112, width - 16, self.bounds.size.height - 158);
    self.scroll.contentSize = CGSizeMake(width - 16, 1000 * 72);
    [self.rows enumerateObjectsUsingBlock:^(UIView *item, NSUInteger row, BOOL *stop) {
        item.frame = CGRectMake(0, row * 72, width - 16, 72);
        item.subviews.firstObject.frame = CGRectMake(12, 0, width - 40, 72);
    }];
    if (!self.initialPositionApplied) {
        NSDictionary<NSString *, NSString *> *environment = NSProcessInfo.processInfo.environment;
        CGFloat offset = [environment[@"START_OFFSET"] doubleValue];
        if ([environment[@"START_FROM_BOTTOM"] boolValue]) {
            offset = MAX(0, self.scroll.contentSize.height - self.scroll.bounds.size.height
                            - [environment[@"START_FROM_BOTTOM_DISTANCE"] doubleValue]);
        }
        offset = MIN(MAX(0, offset), MAX(0, self.scroll.contentSize.height - self.scroll.bounds.size.height));
        self.scroll.contentOffset = CGPointMake(0, offset);
        set_slint_scroll_offset((float)offset);
        self.initialPositionApplied = YES;
    }
}
@end

void install_native_scroll(void *hostPointer) {
    UIView *host = (__bridge UIView *)hostPointer;
    NativeScrollPane *pane = [[NativeScrollPane alloc] initWithFrame:
        CGRectMake(0, 0, host.bounds.size.width / 2, host.bounds.size.height)];
    pane.autoresizingMask = UIViewAutoresizingFlexibleHeight;
    [host addSubview:pane];
    pane.scroll.host = host;
    pane.scroll.panGestureRecognizer.cancelsTouchesInView = NO;
}
