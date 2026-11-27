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
        for mode in ["default", "shortcut", "external", "restart", "pump", "idle-quit"] {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .arg(format!("--mode={mode}"))
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while child.try_wait().unwrap().is_none() {
                if std::time::Instant::now() >= deadline {
                    child.kill().unwrap();
                    let output = child.wait_with_output().unwrap();
                    panic!(
                        "{mode}: subprocess timed out: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            let output = child.wait_with_output().unwrap();
            assert!(output.status.success(), "{mode}: {}", String::from_utf8_lossy(&output.stderr));
            let stdout = String::from_utf8(output.stdout).unwrap();
            assert!(
                stdout.contains("EVENT_LOOP_RETURNED")
                    && (stdout.contains("CLEANUP_DROPPED") != (mode == "idle-quit")),
                "{mode}: native Quit bypassed cleanup"
            );
            println!("{mode}: passed");
        }
        return;
    };

    if mode == "send-external" {
        unsafe {
            let pid: i32 = std::env::args().nth(2).unwrap().parse().unwrap();
            let target: *mut AnyObject = msg_send![class!(NSRunningApplication), runningApplicationWithProcessIdentifier: pid];
            assert!(!target.is_null());
            let sent: bool = msg_send![target, terminate];
            assert!(sent);
        }
        return;
    }
    slint::slint! {
        #[edition = "1.19"]
        export component PlainApp inherits Window {
            width: 100px;
            height: 100px;
        }
    }

    fn native_quit_item() -> (*mut AnyObject, *mut AnyObject, *mut AnyObject) {
        unsafe {
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
            let title: objc2::rc::Retained<objc2_foundation::NSString> = msg_send![quit, title];
            assert_eq!(title.to_string(), muda::PredefinedMenuItem::quit(None).text());
            (app, menu, quit)
        }
    }

    fn run(app: impl ComponentHandle, mode: &str) {
        let shortcut = mode == "shortcut";
        let external = matches!(mode, "external" | "pump");
        let sender = Rc::new(std::cell::RefCell::new(None::<std::process::Child>));
        let sender_handle = sender.clone();
        slint::Timer::single_shot(std::time::Duration::from_secs(5), || {
            panic!("native Quit did not exit the event loop")
        });
        slint::Timer::single_shot(std::time::Duration::from_millis(200), move || unsafe {
            let (app, menu, quit) = native_quit_item();
            let modifiers = 1usize << 20;
            // AppKit dispatches native menu actions outside Slint callbacks.
            if external {
                *sender_handle.borrow_mut() = Some(
                    std::process::Command::new(std::env::current_exe().unwrap())
                        .arg("--mode=send-external")
                        .arg(std::process::id().to_string())
                        .spawn()
                        .unwrap(),
                );
            } else if shortcut {
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
        if mode == "pump" {
            app.show().unwrap();
            i_slint_core::context::with_existing_context(|context| {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
                loop {
                    assert!(std::time::Instant::now() < deadline, "pump timed out");
                    if context
                        .platform()
                        .process_events(
                            Some(std::time::Duration::from_millis(10)),
                            i_slint_core::InternalToken,
                        )
                        .unwrap()
                        .is_break()
                    {
                        break;
                    }
                }
            })
            .unwrap();
            app.hide().unwrap();
        } else {
            app.run().unwrap();
        }
        if let Some(child) = sender.borrow_mut().as_mut() {
            assert!(child.wait().unwrap().success(), "external Quit sender failed");
        }
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
    let app = PlainApp::new().unwrap();
    run(app.clone_strong(), &mode);
    if mode == "idle-quit" {
        let (application, _, _) = native_quit_item();
        unsafe {
            let _: () = msg_send![application, terminate: std::ptr::null::<AnyObject>()];
        }
        panic!("idle Quit must terminate the process");
    }
    if mode == "restart" {
        let continued = Rc::new(Cell::new(false));
        let timer_continued = continued.clone();
        slint::Timer::single_shot(std::time::Duration::from_millis(200), move || {
            timer_continued.set(true);
            slint::quit_event_loop().unwrap();
        });
        app.run().unwrap();
        assert!(continued.get(), "stale native Quit ended the next run");
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {}
