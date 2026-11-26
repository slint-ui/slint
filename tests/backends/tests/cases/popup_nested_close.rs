// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Clicks close nested popups the same way whether the backend shows them as native popup windows
// or inside the main window.
//
// main window
// └── outer (close-on-click-outside)
//     └── inner (close-on-click-outside)

use i_slint_core::window::{PopupWindowLocation, WindowInner};
use slint::LogicalPosition;
use slint::platform::{PointerEventButton, WindowAdapter, WindowEvent};
use std::rc::Rc;

slint::slint! {
    #[edition = "1.20"]
    export component App inherits Window {
        width: 300px;
        height: 300px;

        public function show-outer() { outer.show(); }

        outer := PopupWindow {
            width: 200px;
            height: 200px;
            close-policy: close-on-click-outside;

            TouchArea {
                x: 0;
                y: 150px;
                width: 50px;
                height: 50px;
                clicked => { inner.show(); }
            }

            inner := PopupWindow {
                x: 100px;
                y: 0;
                width: 50px;
                height: 50px;
                close-policy: close-on-click-outside;
            }
        }
    }
}

// The position of the inner popup in the outer one.
const INNER_X: f32 = 100.;

fn click(window: &slint::Window, x: f32, y: f32) {
    let position = LogicalPosition::new(x, y);
    let button = PointerEventButton::Left;
    window.dispatch_event(WindowEvent::PointerMoved { position });
    window.dispatch_event(WindowEvent::PointerPressed { position, button });
    window.dispatch_event(WindowEvent::PointerReleased { position, button });
}

/// The window of the native popup shown from `window`, if any.
fn popup_window(window: &slint::Window) -> Option<Rc<dyn WindowAdapter>> {
    let inner = WindowInner::from_pub(window);
    let popups = inner.active_popups();
    assert!(popups.len() <= 1, "at most one popup per window");
    popups.first().map(|p| match &p.location {
        PopupWindowLocation::TopLevel(adapter) => adapter.clone(),
        PopupWindowLocation::ChildWindow(_) => panic!("a native popup opens a native popup"),
    })
}

/// The number of open popups shown from `window` and from its native popups.
fn open_popups(window: &slint::Window) -> usize {
    let nested: Vec<_> = WindowInner::from_pub(window)
        .active_popups()
        .iter()
        .filter_map(|p| match &p.location {
            PopupWindowLocation::TopLevel(adapter) => Some(adapter.clone()),
            PopupWindowLocation::ChildWindow(_) => None,
        })
        .collect();
    WindowInner::from_pub(window).active_popups().len()
        + nested.iter().map(|a| open_popups(a.window())).sum::<usize>()
}

fn children(adapter: &Rc<dyn WindowAdapter>) -> Vec<Rc<dyn WindowAdapter>> {
    let mut children = Vec::new();
    adapter
        .internal(i_slint_core::InternalToken)
        .unwrap()
        .visit_children(&mut |child| children.push(child.clone()));
    children
}

#[satchel::test]
fn nested_popups_close() {
    let app = App::new().unwrap();
    app.show().unwrap();
    let main = app.window();

    app.invoke_show_outer();
    let native = matches!(
        WindowInner::from_pub(main).active_popups().first().map(|p| &p.location),
        Some(PopupWindowLocation::TopLevel(_))
    );
    WindowInner::from_pub(main).close_all_popups();

    // Clicks at a position in the outer or inner popup. Popups inside the main window share its
    // coordinates, with the outer popup at (0, 0).
    let click_outer = |x, y| match native {
        true => click(popup_window(main).unwrap().window(), x, y),
        false => click(main, x, y),
    };
    let click_inner = |x, y| match native {
        true => click(popup_window(popup_window(main).unwrap().window()).unwrap().window(), x, y),
        false => click(main, INNER_X + x, y),
    };
    let open_nested = || {
        app.invoke_show_outer();
        click_outer(25., 175.);
        assert_eq!(open_popups(main), 2, "the outer and the inner popup open");
    };

    // A click in the main window closes the outer popup and the inner one opened from it.
    open_nested();
    click(main, 250., 250.);
    assert_eq!(open_popups(main), 0, "all popups close");

    // A click in the outer popup, outside the inner one, closes only the inner one.
    open_nested();
    click_outer(10., 10.);
    assert_eq!(open_popups(main), 1, "the inner popup closes");
    click_outer(10., 10.);
    assert_eq!(open_popups(main), 1, "a click in the outer popup keeps it open");
    WindowInner::from_pub(main).close_all_popups();

    // A click in the inner popup keeps both open.
    open_nested();
    click_inner(10., 10.);
    assert_eq!(open_popups(main), 2, "both popups stay open");
    WindowInner::from_pub(main).close_all_popups();

    if native {
        native_popup_windows(&app);
    }

    app.hide().unwrap();
}

/// Each native popup is a child window of the window it's shown from, and closing it destroys it.
fn native_popup_windows(app: &App) {
    let main = WindowInner::from_pub(app.window()).window_adapter();
    app.invoke_show_outer();
    let outer = popup_window(app.window()).unwrap();
    click(outer.window(), 25., 175.);
    let inner = popup_window(outer.window()).unwrap();

    let is_only_child =
        |parent, child| matches!(&children(parent)[..], [c] if Rc::ptr_eq(c, child));
    assert!(is_only_child(&main, &outer), "the main window lists the outer popup as its child");
    assert!(is_only_child(&outer, &inner), "the outer popup lists the inner popup as its child");

    let outer_weak = Rc::downgrade(&outer);
    let inner_weak = Rc::downgrade(&inner);
    drop((outer, inner));
    click(app.window(), 250., 250.);
    assert!(inner_weak.upgrade().is_none(), "the inner popup's window is gone");
    assert!(outer_weak.upgrade().is_none(), "the outer popup's window is gone");
    assert!(children(&main).is_empty(), "the main window has no child windows left");
}
