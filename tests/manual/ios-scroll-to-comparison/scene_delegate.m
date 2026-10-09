// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// cspell:ignore nonatomic
#import <UIKit/UIKit.h>

@interface ScrollComparisonSceneDelegate : NSObject <UIWindowSceneDelegate>
@property (nonatomic, strong) UIWindow *window;
@end

@implementation ScrollComparisonSceneDelegate
- (void)attachWindow:(UIScene *)scene
{
    if (![scene isKindOfClass:UIWindowScene.class]) return;
    for (UIWindow *window in UIApplication.sharedApplication.windows) {
        if (!window.windowScene) {
            window.windowScene = (UIWindowScene *)scene;
            self.window = window;
            [window makeKeyAndVisible];
        }
    }
}
- (void)scene:(UIScene *)scene willConnectToSession:(UISceneSession *)session
        options:(UISceneConnectionOptions *)options
{
    [self attachWindow:scene];
}
- (void)sceneDidBecomeActive:(UIScene *)scene
{
    [self attachWindow:scene];
}
@end

void register_scroll_scene_delegate(void)
{
    (void)ScrollComparisonSceneDelegate.class;
}
