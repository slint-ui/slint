// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// The scene delegate that apps name in their `UIApplicationSceneManifest`, so
// that they adopt the UIScene lifecycle. iOS 27 aborts an app that connects a
// scene without adopting it, and UIKit decides adoption from the manifest's
// `UISceneConfigurations` alone: a delegate installed at runtime is too late.
//
// A window scene owns the windows drawn on its screen, so the windows winit
// creates have to be handed to the scene that connects here.
//
// Apple's TN3187 describes the migration and the manifest keys:
// https://developer.apple.com/documentation/technotes/tn3187-migrating-to-the-uikit-scene-based-life-cycle
//
// None of this is specific to Slint, so it belongs next to the windows winit
// creates.
// TODO: Upstream it when upgrading to the next winit version.

use objc2::rc::Retained;
use objc2::{ClassType as _, MainThreadMarker, MainThreadOnly, Message as _, define_class};
use objc2_foundation::{NSObject, NSObjectProtocol};
use objc2_ui_kit::{
    UIApplication, UIScene, UISceneConnectionOptions, UISceneDelegate, UISceneSession, UIView,
    UIWindow, UIWindowScene, UIWindowSceneDelegate,
};

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[name = "SlintWindowSceneDelegate"]
    struct SlintWindowSceneDelegate;

    unsafe impl NSObjectProtocol for SlintWindowSceneDelegate {}

    unsafe impl UISceneDelegate for SlintWindowSceneDelegate {
        #[unsafe(method(scene:willConnectToSession:options:))]
        fn scene_will_connect_to_session(
            &self,
            scene: &UIScene,
            _session: &UISceneSession,
            _options: &UISceneConnectionOptions,
        ) {
            let Some(window_scene) = scene.downcast_ref::<UIWindowScene>() else { return };

            // winit creates its windows while handling the launch notification,
            // which UIKit posts before it connects this scene.
            #[allow(deprecated)]
            let windows = UIApplication::sharedApplication(self.mtm()).windows();
            for window in windows.iter() {
                // A hidden window belongs to whoever hid it, and attaching it
                // here would put it on screen.
                if !window.isHidden() {
                    attach(&window, window_scene);
                }
            }
        }
    }

    unsafe impl UIWindowSceneDelegate for SlintWindowSceneDelegate {}
);

/// Registers the scene delegate class under the name apps use in their
/// `UIApplicationSceneManifest`. UIKit looks the class up by name when it
/// connects a scene, so it has to exist by then.
pub(crate) fn register_scene_delegate_class() {
    let _ = SlintWindowSceneDelegate::class();
}

/// Attaches the window hosting `view` to the app's window scene, for windows
/// created after the scene connected.
pub(crate) fn attach_window_to_scene(view: &UIView) {
    if let Some(mtm) = MainThreadMarker::new()
        && let Some(window_scene) = window_scene(mtm)
        && let Some(window) = view.window()
    {
        attach(&window, &window_scene);
    }
}

/// The scene that Slint's windows live on. A Slint app declares a single scene
/// configuration, so the one connected window scene is it.
fn window_scene(mtm: MainThreadMarker) -> Option<Retained<UIWindowScene>> {
    UIApplication::sharedApplication(mtm)
        .connectedScenes()
        .iter()
        .find_map(|scene| scene.downcast_ref::<UIWindowScene>().map(UIWindowScene::retain))
}

fn attach(window: &UIWindow, window_scene: &UIWindowScene) {
    if window.windowScene().as_deref() == Some(window_scene) {
        return;
    }
    window.setWindowScene(Some(window_scene));
    // winit already made the window visible, but a window without a scene never
    // reached the screen.
    window.makeKeyAndVisible();
}
