// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! With a backend that creates native popup windows, a context menu and its sub-menus are native popups.
//! Covers the interpreter; `tests/cases/popups/contextmenu_native_popup.slint` covers the generated code.

use i_slint_core::platform::{Key, PointerEventButton, WindowAdapter, WindowEvent};
use i_slint_core::window::{PopupWindowLocation, WindowInner};
use i_slint_core::{SharedString, api::LogicalPosition};
use slint_interpreter::{ComponentHandle, Value};
use std::rc::Rc;

fn popup_windows(window: &i_slint_core::api::Window) -> Vec<Rc<dyn WindowAdapter>> {
    WindowInner::from_pub(window)
        .active_popups()
        .iter()
        .map(|p| match &p.location {
            PopupWindowLocation::TopLevel(adapter) => adapter.clone(),
            PopupWindowLocation::ChildWindow(_) => panic!("the menu should be a native popup"),
        })
        .collect()
}

fn key(window: &i_slint_core::api::Window, key: Key) {
    let text = SharedString::from(key);
    window.dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    window.dispatch_event(WindowEvent::KeyReleased { text });
}

#[test]
fn context_menu_is_native_popup() {
    i_slint_backend_testing::init_no_event_loop();

    let mut compiler = slint_interpreter::Compiler::default();
    compiler.set_style("fluent".into());
    let code = r#"
        export component TestCase inherits Window {
            in-out property <string> result;
            width: 300px;
            height: 300px;
            ContextMenuArea {
                Menu {
                    MenuItem { title: "Entry1"; }
                    Menu {
                        title: "SubMenu";
                        MenuItem { title: "SubEntry"; activated => { result += "SubEntry;"; } }
                    }
                }
            }
        }
    "#;
    let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let instance = result.component("TestCase").unwrap().create().unwrap();
    i_slint_backend_testing::access_testing_window(instance.window(), |w| w.use_native_popup(true));

    let position = LogicalPosition::new(15.0, 15.0);
    let button = PointerEventButton::Right;
    instance.window().dispatch_event(WindowEvent::PointerPressed { position, button });
    instance.window().dispatch_event(WindowEvent::PointerReleased { position, button });
    let [menu] = &popup_windows(instance.window())[..] else { panic!("one menu") };

    key(menu.window(), Key::UpArrow);
    key(menu.window(), Key::RightArrow);
    let [sub_menu] = &popup_windows(menu.window())[..] else { panic!("one sub-menu") };
    let parent = sub_menu.internal(i_slint_core::InternalToken).and_then(|i| i.get_parent());
    assert!(parent.is_some_and(|p| Rc::ptr_eq(&p, menu)), "the menu is the sub-menu's parent");

    // Escape closes only the sub-menu.
    key(sub_menu.window(), Key::Escape);
    assert!(popup_windows(menu.window()).is_empty());
    assert_eq!(popup_windows(instance.window()).len(), 1);

    key(menu.window(), Key::RightArrow);
    let [sub_menu] = &popup_windows(menu.window())[..] else { panic!("one sub-menu") };
    key(sub_menu.window(), Key::DownArrow);
    key(sub_menu.window(), Key::Return);
    assert_eq!(instance.get_property("result").unwrap(), Value::String("SubEntry;".into()));
    assert!(popup_windows(instance.window()).is_empty());
}
