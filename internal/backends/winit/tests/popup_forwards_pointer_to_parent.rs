// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! While a menu grabs the pointer, the platform may deliver pointer events over the parent window
//! to the menu's popup window, with positions outside the popup.
//! Hovering another menu bar entry then must still switch the open menu.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use slint::winit_031::{CustomApplicationHandler, EventResult, WinitWindowAccessor, winit};
use winit::event::{ButtonSource, ElementState, MouseButton, PointerSource, WindowEvent};
use winit::raw_window_handle::RawWindowHandle;
use winit::window::WindowId;

slint::slint! {
    #[edition = "1.20"]
    export component App inherits Window {
        width: 300px;
        height: 200px;

        MenuBar {
            Menu {
                title: "F";
                MenuItem { title: "Open"; }
            }
            // Long, so that x = 150px is inside it whatever the font.
            Menu {
                title: "Edit Edit Edit Edit Edit Edit Edit Edit";
                MenuItem { title: "Copy"; }
            }
        }
    }
}

fn pointer_events(scale_factor: f32, x: f32, y: f32, click: bool) -> Vec<WindowEvent> {
    let position = slint::LogicalPosition::new(x, y).to_physical(scale_factor);
    let position = winit::dpi::PhysicalPosition::new(position.x as f64, position.y as f64);
    let button = |state| WindowEvent::PointerButton {
        device_id: None,
        state,
        position,
        primary: true,
        button: ButtonSource::Mouse(MouseButton::Left),
        is_macos_activation_click: false,
    };
    let mut events = vec![WindowEvent::PointerMoved {
        device_id: None,
        position,
        primary: true,
        source: PointerSource::Mouse,
    }];
    if click {
        events.extend([button(ElementState::Pressed), button(ElementState::Released)]);
    }
    events
}

/// Records the windows in the order they receive their first event:
/// the main window, the "F" menu, and the "Edit" menu.
struct OpenMenuThenHoverMenuBar {
    windows: Rc<RefCell<Vec<WindowId>>>,
    skipped: Rc<Cell<bool>>,
}

impl CustomApplicationHandler for OpenMenuThenHoverMenuBar {
    fn window_event(
        &mut self,
        event_loop: &dyn winit::event_loop::ActiveEventLoop,
        window_id: WindowId,
        winit_window: Option<&dyn winit::window::Window>,
        slint_window: Option<&slint::Window>,
        event: &WindowEvent,
    ) -> EventResult {
        let (Some(winit_window), Some(slint_window)) = (winit_window, slint_window) else {
            return EventResult::Propagate;
        };
        let mut windows = self.windows.borrow_mut();
        if windows.contains(&window_id) {
            return EventResult::Propagate;
        }
        let events = match windows.len() {
            0 if !matches!(event, WindowEvent::RedrawRequested) => return EventResult::Propagate,
            0 => {
                // X11 shows popups inside their parent window.
                let handle = winit_window.rwh_06_window_handle().window_handle().unwrap();
                if matches!(handle.as_raw(), RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_)) {
                    self.skipped.set(true);
                    slint::quit_event_loop().unwrap();
                    return EventResult::Propagate;
                }
                // Click the "F" entry of the menu bar.
                pointer_events(slint_window.scale_factor(), 15., 20., true)
            }
            // Wait until the platform has placed the menu.
            1 if winit_window.outer_position().is_err() => return EventResult::Propagate,
            // The "F" menu opens below the menu bar, so this is above it, over "Edit".
            1 => pointer_events(slint_window.scale_factor(), 150., -20., false),
            _ => {
                slint::quit_event_loop().unwrap();
                vec![]
            }
        };
        windows.push(window_id);
        drop(windows);
        for event in events {
            slint_window.dispatch_winit_window_event(event_loop, &event).unwrap();
        }
        EventResult::Propagate
    }
}

fn main() {
    let windows = Rc::new(RefCell::new(Vec::new()));
    let skipped = Rc::new(Cell::new(false));
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .with_winit_custom_application_handler(OpenMenuThenHoverMenuBar {
            windows: windows.clone(),
            skipped: skipped.clone(),
        })
        .select()
        .unwrap();

    let app = App::new().unwrap();
    app.show().unwrap();
    slint::Timer::single_shot(Duration::from_secs(10), || slint::quit_event_loop().unwrap());
    slint::run_event_loop().unwrap();

    if skipped.get() {
        eprintln!("skipped: popups aren't native windows on X11");
        return;
    }
    assert_eq!(windows.borrow().len(), 3, "hovering the menu bar didn't open the other menu");
}
