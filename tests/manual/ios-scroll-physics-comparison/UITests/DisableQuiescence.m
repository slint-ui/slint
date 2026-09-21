// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// cspell:ignore instancetype nonatomic

#import <Foundation/Foundation.h>
#import <UIKit/UIKit.h>

@interface XCPointerEventPath : NSObject
- (instancetype)initForTouchAtPoint:(CGPoint)point offset:(double)offset;
- (void)moveToPoint:(CGPoint)point atOffset:(double)offset;
- (void)liftUpAtOffset:(double)offset;
@end

@interface XCSynthesizedEventRecord : NSObject
@property(nonatomic) pid_t targetProcessID;
- (instancetype)initWithName:(NSString *)name interfaceOrientation:(UIInterfaceOrientation)orientation;
- (void)addPointerEventPath:(XCPointerEventPath *)path;
- (BOOL)synthesizeWithError:(NSError **)error;
@end

BOOL synthesizeRapidFlicks(pid_t processID, double width, double height, int count) {
    XCSynthesizedEventRecord *record = [[XCSynthesizedEventRecord alloc]
        initWithName:@"Rapid repeated hard flicks"
        interfaceOrientation:UIInterfaceOrientationPortrait];
    record.targetProcessID = processID;

    double startTime = 0;
    CGPoint start = CGPointMake(width * 0.25, height * 0.75);
    CGPoint end = CGPointMake(width * 0.25, height * 0.30);
    for (int flick = 0; flick < count; flick++) {
        XCPointerEventPath *path = [[XCPointerEventPath alloc]
            initForTouchAtPoint:start offset:startTime];
        for (int step = 1; step <= 5; step++) {
            CGFloat progress = step / 5.0;
            CGPoint point = CGPointMake(start.x, start.y + (end.y - start.y) * progress);
            [path moveToPoint:point atOffset:startTime + step * 0.012];
        }
        [path liftUpAtOffset:startTime + 0.065];
        [record addPointerEventPath:path];
        startTime += 0.075;
    }

    NSError *error = nil;
    BOOL result = [record synthesizeWithError:&error];
    if (!result) NSLog(@"Rapid flick synthesis failed: %@", error);
    return result;
}
