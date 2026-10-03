// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// The scene delegate that apps name in their `UIApplicationSceneManifest`, so
// that they adopt the UIScene lifecycle. iOS 27 aborts an app that connects a
// scene without adopting it, and UIKit decides adoption from the manifest's
// `UISceneConfigurations` alone: a delegate installed at runtime is too late.
//
// A window scene owns the windows drawn on its screen, so the windows winit
// creates have to be handed to the scene. winit may create a window before or
// after UIKit connects the scene, so both orders attach it.
//
// Apple's TN3187 describes the migration and the manifest keys:
// https://developer.apple.com/documentation/technotes/tn3187-migrating-to-the-uikit-scene-based-life-cycle
//
// None of this is specific to Slint, so it belongs next to the windows winit
// creates.
// TODO: Upstream it when upgrading to the next winit version.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ptr::NonNull;
use std::rc::Weak;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{ClassType as _, MainThreadMarker, MainThreadOnly, Message as _, define_class};
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSObject, NSObjectProtocol, NSString,
};
use objc2_ui_kit::{
    UIApplication, UISceneDelegate, UISceneWillConnectNotification, UIView, UIWindow,
    UIWindowScene, UIWindowSceneDelegate, UIWindowSceneSessionRoleApplication,
};
use winit::window::WindowId;

use crate::winitwindowadapter::WinitWindowAdapter;

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[name = "SlintWindowSceneDelegate"]
    struct SlintWindowSceneDelegate;

    unsafe impl NSObjectProtocol for SlintWindowSceneDelegate {}

    // UIKit only needs this class to exist and to conform. The windows are
    // attached from `register_scene_notifications`, which can reach them.
    unsafe impl UISceneDelegate for SlintWindowSceneDelegate {}

    unsafe impl UIWindowSceneDelegate for SlintWindowSceneDelegate {}
);

/// Attaches the window hosting `view` to the app's window scene, for windows
/// created after the scene connected. A window created before that is attached
/// by [`register_scene_notifications`].
pub(crate) fn attach_window_to_scene(view: &UIView) {
    if let Some(mtm) = MainThreadMarker::new()
        && let Some(window_scene) = window_scene(mtm)
        && let Some(window) = view.window()
    {
        attach(&window, &window_scene);
    }
}

/// The `UIView` winit renders into.
pub(crate) fn content_view(winit_window: &winit::window::Window) -> Retained<UIView> {
    use raw_window_handle::HasWindowHandle as _;

    let raw_window_handle::RawWindowHandle::UiKit(handle) = winit_window
        .window_handle()
        .expect("a winit window on iOS always has a window handle")
        .as_raw()
    else {
        unreachable!("winit hands out UIKit window handles on iOS")
    };
    unsafe { &*(handle.ui_view.as_ptr() as *const UIView) }.retain()
}

/// Keeps the scene-connection observer alive. Dropping it stops attaching
/// windows to the scenes that connect afterwards.
pub(crate) struct SceneLifecycle(Retained<ProtocolObject<dyn NSObjectProtocol>>);

impl Drop for SceneLifecycle {
    fn drop(&mut self) {
        unsafe { NSNotificationCenter::defaultCenter().removeObserver(self.0.as_ref()) };
    }
}

/// Puts everything the UIScene lifecycle needs in place: the delegate class the
/// app's `UIApplicationSceneManifest` names, and the observer that attaches the
/// windows winit created before the scene connected. Such a window belongs to
/// no scene, which keeps it out of `UIApplication.windows` and so out of reach
/// of the delegate.
///
/// Call this before `UIApplicationMain`, which is where UIKit looks the class up
/// and connects the first scene.
pub(crate) fn install_scene_lifecycle(
    active_windows: Weak<RefCell<HashMap<WindowId, Weak<WinitWindowAdapter>>>>,
) -> SceneLifecycle {
    // UIKit looks the delegate up by the name in the manifest, so the class has
    // to be registered with the runtime by then.
    let _ = SlintWindowSceneDelegate::class();

    let block = RcBlock::new(move |notification: NonNull<NSNotification>| {
        let Some(active_windows) = active_windows.upgrade() else { return };
        let Some(window_scene) = unsafe { notification.as_ref() }
            .object()
            .and_then(|scene| scene.downcast::<UIWindowScene>().ok())
        else {
            return;
        };
        if !is_app_scene(&window_scene) {
            return;
        }
        let active_windows = active_windows.borrow();
        for adapter in active_windows.values().filter_map(Weak::upgrade) {
            let Some(window) = adapter
                .winit_window()
                .map(|winit_window| content_view(&winit_window))
                .and_then(|view| view.window())
            else {
                continue;
            };
            // A hidden window belongs to whoever hid it, and attaching it here
            // would put it on screen.
            if !window.isHidden() {
                attach(&window, &window_scene);
            }
        }
    });
    // Pass no queue, so that the block runs while the scene connects rather than
    // a run loop turn later. UIKit posts this on the main thread.
    SceneLifecycle(unsafe {
        NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
            Some(UISceneWillConnectNotification),
            None,
            None,
            &block,
        )
    })
}

/// The scene that Slint's windows live on. A Slint app declares a single scene
/// configuration, so the one connected application scene is it.
fn window_scene(mtm: MainThreadMarker) -> Option<Retained<UIWindowScene>> {
    UIApplication::sharedApplication(mtm)
        .connectedScenes()
        .iter()
        .filter_map(|scene| scene.downcast_ref::<UIWindowScene>().map(UIWindowScene::retain))
        .find(|scene| is_app_scene(scene))
}

/// The system connects window scenes of its own next to the app's.
/// An iOS app running on macOS gets one with the role
/// `FUWindowSceneSessionRoleSystemUI`, after the app's scene.
/// A window attached to it never reaches the screen.
fn is_app_scene(scene: &UIWindowScene) -> bool {
    let application_role: &NSString = unsafe { UIWindowSceneSessionRoleApplication };
    *scene.session().role() == *application_role
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
