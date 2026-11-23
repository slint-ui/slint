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
        for mode in [
            "default",
            "shortcut",
            "multiple",
            "menubar",
            "custom",
            "restart",
            "terminate",
            "external",
            "receiver",
            "handler",
            "until-quit",
            "pump",
            "pump-between",
            "idle-quit",
        ] {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .arg(format!("--mode={mode}"))
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
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

    static FOREIGN_CALLED: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);
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
    if mode == "handler" {
        muda::MenuEvent::set_event_handler(Some(|event: muda::MenuEvent| {
            assert_eq!(event.id().0, "foreign-item");
            FOREIGN_CALLED.store(true, std::sync::atomic::Ordering::Relaxed);
        }));
    }

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

    fn native_quit_item(custom: bool) -> (*mut AnyObject, *mut AnyObject, *mut AnyObject) {
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
            let expected_title = if custom {
                "Custom Quit".to_owned()
            } else {
                muda::PredefinedMenuItem::quit(None).text()
            };
            assert_eq!(title.to_string(), expected_title);
            (app, menu, quit)
        }
    }

    fn run(app: impl ComponentHandle, mode: &str) {
        let shortcut = mode == "shortcut";
        let custom = mode == "custom";
        let terminate = mode == "terminate";
        let external = mode == "external";
        let foreign = matches!(mode, "receiver" | "handler");
        let handler = mode == "handler";
        let between_pumps = mode == "pump-between";
        let sender = Rc::new(std::cell::RefCell::new(None::<std::process::Child>));
        let sender_handle = sender.clone();
        slint::Timer::single_shot(std::time::Duration::from_secs(5), || {
            panic!("native Quit did not exit the event loop")
        });
        slint::Timer::single_shot(std::time::Duration::from_millis(200), move || unsafe {
            if between_pumps {
                return;
            }
            let (app, menu, quit) = native_quit_item(custom);
            let modifiers = 1usize << 20;
            if foreign {
                use muda::ContextMenu;
                let submenu = muda::Submenu::new("Foreign", true);
                let item = muda::MenuItem::with_id("foreign-item", "Foreign", true, None);
                submenu.append(&item).unwrap();
                let native = submenu.ns_menu().cast::<AnyObject>();
                let _: () = msg_send![native, performActionForItemAtIndex: 0isize];
                if handler {
                    assert!(FOREIGN_CALLED.load(std::sync::atomic::Ordering::Relaxed));
                } else {
                    assert_eq!(
                        muda::MenuEvent::receiver().try_recv().unwrap().id().0,
                        "foreign-item"
                    );
                }
            }
            // AppKit dispatches native menu actions outside Slint callbacks.
            if external {
                *sender_handle.borrow_mut() = Some(
                    std::process::Command::new(std::env::current_exe().unwrap())
                        .arg("--mode=send-external")
                        .arg(std::process::id().to_string())
                        .spawn()
                        .unwrap(),
                );
            } else if terminate {
                let _: () = msg_send![app, performSelector: sel!(terminate:), withObject: std::ptr::null::<AnyObject>(), afterDelay: 0.0f64];
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
        if matches!(mode, "pump" | "pump-between") {
            app.show().unwrap();
            i_slint_core::context::with_existing_context(|context| {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
                let mut sent_between_pumps = false;
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
                    let menu_ready = unsafe {
                        let application: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
                        let menu: *mut AnyObject = msg_send![application, mainMenu];
                        !menu.is_null() && { let count: isize = msg_send![menu, numberOfItems]; count > 0 }
                    };
                    if mode == "pump-between" && !sent_between_pumps && menu_ready {
                        let (application, _, quit) = native_quit_item(false);
                        unsafe {
                            let action: Option<Sel> = msg_send![quit, action];
                            let target: *mut AnyObject = msg_send![quit, target];
                            let sent: bool = msg_send![application, sendAction: action.unwrap(), to: target, from: quit];
                            assert!(sent);
                        }
                        sent_between_pumps = true;
                    }
                }
                assert!(mode != "pump-between" || sent_between_pumps);
            })
            .unwrap();
            app.hide().unwrap();
        } else if mode == "until-quit" {
            app.show().unwrap();
            slint::run_event_loop_until_quit().unwrap();
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
    if !matches!(mode.as_str(), "menubar" | "custom") {
        let second = (mode == "multiple").then(|| PlainApp::new().unwrap());
        if let Some(second) = &second {
            second.show().unwrap();
        }
        let app = PlainApp::new().unwrap();
        run(app.clone_strong(), &mode);
        if mode == "idle-quit" {
            let (application, _, _) = native_quit_item(false);
            unsafe {
                let _: () = msg_send![application, terminate: std::ptr::null::<AnyObject>()];
            }
            panic!("idle Quit must terminate the process");
        }
        if mode == "restart" {
            let (application, _, _) = native_quit_item(false);
            unsafe {
                let delegate: *mut AnyObject = msg_send![application, delegate];
                let reply: usize = msg_send![delegate, applicationShouldTerminate: application];
                assert_eq!(reply, 1, "idle Quit must permit process termination");
            }
            let continued = Rc::new(Cell::new(false));
            let timer_continued = continued.clone();
            slint::Timer::single_shot(std::time::Duration::from_millis(200), move || {
                timer_continued.set(true);
                slint::quit_event_loop().unwrap();
            });
            app.run().unwrap();
            assert!(continued.get(), "stale native Quit ended the next run");
        }
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
        run(app, &mode);
        assert_eq!(invoked.get(), if mode == "custom" { 2 } else { 0 });
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {}
