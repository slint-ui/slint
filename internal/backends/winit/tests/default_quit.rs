// Copyright © 2026 Koichiro Ohba
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore Koichiro Ohba

#[cfg(target_os = "macos")]
fn main() {
    use objc2::runtime::{AnyObject, Sel};
    use objc2::{class, msg_send, sel};
    use slint::ComponentHandle;
    use std::cell::Cell;
    use std::rc::Rc;

    let Some(mode) =
        std::env::args().find_map(|arg| arg.strip_prefix("--mode=").map(str::to_owned))
    else {
        for mode in ["default", "shortcut", "multiple", "menubar", "custom"] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .arg(format!("--mode={mode}"))
                .output()
                .unwrap();
            assert!(output.status.success(), "{mode}: {}", String::from_utf8_lossy(&output.stderr));
            let stdout = String::from_utf8(output.stdout).unwrap();
            assert!(
                stdout.contains("EVENT_LOOP_RETURNED") && stdout.contains("CLEANUP_DROPPED"),
                "{mode}: native Quit bypassed cleanup"
            );
        }
        return;
    };

    slint::slint! {
        #[edition = "1.19"]
        export component PlainApp inherits Window {
            width: 100px;
            height: 100px;
        }
        export component MenuApp inherits Window {
            width: 100px;
            height: 100px;
            in property <bool> custom-quit;
            callback quit-requested();
            MenuBar {
                Menu {
                    title: "File";
                    MenuItem {
                        title: "Custom Quit";
                        role: root.custom-quit ? MenuItemRole.quit : MenuItemRole.none;
                        activated => { root.quit-requested(); }
                    }
                }
            }
        }
    }

    fn run(app: impl ComponentHandle, shortcut: bool) {
        slint::Timer::single_shot(std::time::Duration::from_secs(5), || {
            panic!("native Quit did not exit the event loop")
        });
        slint::Timer::single_shot(std::time::Duration::from_millis(200), move || unsafe {
            let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
            let menu: *mut AnyObject = msg_send![app, mainMenu];
            assert!(!menu.is_null(), "no application menu");
            let app_item: *mut AnyObject = msg_send![menu, itemAtIndex: 0isize];
            let submenu: *mut AnyObject = msg_send![app_item, submenu];
            let count: isize = msg_send![submenu, numberOfItems];
            assert!(count > 0);
            let quit: *mut AnyObject = msg_send![submenu, itemAtIndex: count - 1];
            let key: objc2::rc::Retained<objc2_foundation::NSString> =
                msg_send![quit, keyEquivalent];
            assert_eq!(key.to_string(), "q");
            let modifiers: usize = msg_send![quit, keyEquivalentModifierMask];
            assert_eq!(modifiers, 1 << 20, "expected Command-Q");
            // AppKit dispatches native menu actions outside Slint callbacks.
            if shortcut {
                let key = objc2_foundation::NSString::from_str("q");
                let event: *mut AnyObject = msg_send![class!(NSEvent),
                    keyEventWithType: objc2_app_kit::NSEventType::KeyDown,
                    location: objc2_foundation::NSPoint::new(0., 0.),
                    modifierFlags: modifiers,
                    timestamp: 0.0f64,
                    windowNumber: 0isize,
                    context: std::ptr::null::<AnyObject>(),
                    characters: &*key,
                    charactersIgnoringModifiers: &*key,
                    isARepeat: false,
                    keyCode: 12u16,
                ];
                assert!(!event.is_null());
                let _: () = msg_send![menu, performSelector: sel!(performKeyEquivalent:), withObject: event, afterDelay: 0.0f64];
            } else {
                let action: Option<Sel> = msg_send![quit, action];
                let target: *mut AnyObject = msg_send![quit, target];
                let target = if target.is_null() { app } else { target };
                let _: () = msg_send![target, performSelector: action.unwrap(), withObject: quit, afterDelay: 0.0f64];
            }
        });
        app.run().unwrap();
        println!("EVENT_LOOP_RETURNED");
    }

    struct Cleanup;
    impl Drop for Cleanup {
        fn drop(&mut self) {
            println!("CLEANUP_DROPPED");
        }
    }
    let _cleanup = Cleanup;
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()
        .unwrap();
    if matches!(mode.as_str(), "default" | "shortcut" | "multiple") {
        let second = (mode == "multiple").then(|| PlainApp::new().unwrap());
        if let Some(second) = &second {
            second.show().unwrap();
        }
        run(PlainApp::new().unwrap(), mode == "shortcut");
    } else {
        let app = MenuApp::new().unwrap();
        app.set_custom_quit(mode == "custom");
        let invoked = Rc::new(Cell::new(0));
        let called = invoked.clone();
        app.on_quit_requested(move || {
            called.set(1);
            let called = called.clone();
            slint::Timer::single_shot(std::time::Duration::from_millis(10), move || {
                called.set(2);
                slint::quit_event_loop().unwrap();
            });
        });
        run(app, false);
        assert_eq!(invoked.get(), if mode == "custom" { 2 } else { 0 });
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {}
