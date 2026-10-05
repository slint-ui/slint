// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// cspell:ignore instancetype nonatomic

#import "TouchSynthesis.h"
#import <UIKit/UIKit.h>

// Private XCTest classes; they deliver touches with the requested timing.
@interface XCPointerEventPath : NSObject
- (instancetype)initForTouchAtPoint:(CGPoint)point offset:(double)offset;
- (void)moveToPoint:(CGPoint)point atOffset:(double)offset;
- (void)liftUpAtOffset:(double)offset;
@end

@interface XCSynthesizedEventRecord : NSObject
@property (nonatomic) pid_t targetProcessID;
- (instancetype)initWithName:(NSString *)name
        interfaceOrientation:(UIInterfaceOrientation)orientation;
- (void)addPointerEventPath:(XCPointerEventPath *)path;
- (BOOL)synthesizeWithError:(NSError **)error;
@end

BOOL synthesizeTouchPaths(pid_t processID, int pathCount, const int *pointCounts,
                          const double *times, const double *xs, const double *ys,
                          const double *liftTimes)
{
    XCSynthesizedEventRecord *record =
            [[XCSynthesizedEventRecord alloc] initWithName:@"Scroll case gesture"
                                      interfaceOrientation:UIInterfaceOrientationPortrait];
    record.targetProcessID = processID;
    int first = 0;
    for (int path = 0; path < pathCount; path++) {
        XCPointerEventPath *touch =
                [[XCPointerEventPath alloc] initForTouchAtPoint:CGPointMake(xs[first], ys[first])
                                                         offset:times[first]];
        for (int point = first + 1; point < first + pointCounts[path]; point++)
            [touch moveToPoint:CGPointMake(xs[point], ys[point]) atOffset:times[point]];
        [touch liftUpAtOffset:liftTimes[path]];
        [record addPointerEventPath:touch];
        first += pointCounts[path];
    }
    NSError *error = nil;
    BOOL result = [record synthesizeWithError:&error];
    if (!result)
        NSLog(@"Touch synthesis failed: %@", error);
    return result;
}
