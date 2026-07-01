// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Native drag-and-drop handoff, exercised through the testing backend's native-drag
//! simulation: `set_simulate_native_drag` makes `start_drag` take the drag over like a real
//! backend, and the `simulate_native_drag_*` helpers drive the OS receive path.

use i_slint_backend_testing::access_testing_window;
use i_slint_core::window::WindowInner;
use slint::LogicalPosition;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::private_unstable_api::re_exports::DragAction;

slint::slint! {
    export component Source inherits Window {
        pure callback to-transfer(string) -> data-transfer;
        width: 100px;
        height: 100px;
        out property <bool> finished;
        out property <DragAction> finished-action;
        DragArea {
            width: 100%;
            height: 100%;
            allow-copy: true;
            data: to-transfer("payload-text");
            drag-finished(action) => {
                root.finished = true;
                root.finished-action = action;
            }
        }
    }

    export component Target inherits Window {
        pure callback accept(data-transfer) -> bool;
        pure callback text-of(data-transfer) -> string;
        width: 100px;
        height: 100px;
        out property <bool> got-drop;
        out property <string> dropped-text;
        out property <bool> has-drag <=> dropper.has-drag;
        dropper := DropArea {
            width: 100%;
            height: 100%;
            can-drop(event) => {
                accept(event.data) ? DragAction.copy : DragAction.none
            }
            dropped(event) => {
                root.got-drop = true;
                root.dropped-text = text-of(event.data);
                return event.proposed-action;
            }
        }
    }

    // A DragArea (top) and a DropArea (bottom) in one window, to drop within itself.
    export component SameWindow inherits Window {
        pure callback to-transfer(string) -> data-transfer;
        pure callback accept(data-transfer) -> bool;
        pure callback text-of(data-transfer) -> string;
        width: 100px;
        height: 200px;
        out property <bool> finished;
        out property <DragAction> finished-action;
        out property <bool> got-drop;
        out property <string> dropped-text;
        out property <bool> has-drag <=> dropper.has-drag;
        out property <bool> dragging <=> dragger.dragging;
        VerticalLayout {
            dragger := DragArea {
                allow-copy: true;
                data: to-transfer("payload-text");
                drag-finished(action) => {
                    root.finished = true;
                    root.finished-action = action;
                }
            }
            dropper := DropArea {
                can-drop(event) => {
                    accept(event.data) ? DragAction.copy : DragAction.none
                }
                dropped(event) => {
                    root.got-drop = true;
                    root.dropped-text = text-of(event.data);
                    return event.proposed-action;
                }
            }
        }
    }
}

#[test]
fn native_drag_across_windows() {
    i_slint_backend_testing::init_no_event_loop();

    let source = Source::new().unwrap();
    source.on_to_transfer(slint::DataTransfer::from);
    access_testing_window(source.window(), |w| w.set_simulate_native_drag(true));

    let target = Target::new().unwrap();
    target.on_accept(|data| data.has_plain_text());
    target.on_text_of(|data| data.plain_text().unwrap_or_default());

    // Press, then move past the 8px drag threshold: this calls the backend's `start_drag`,
    // which (in simulate mode) takes the drag over and records the dragged data.
    source.window().dispatch_event(WindowEvent::PointerPressed {
        position: LogicalPosition::new(50.0, 50.0),
        button: PointerEventButton::Left,
    });
    source
        .window()
        .dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(50.0, 90.0) });
    assert!(!source.get_finished(), "drag isn't finished until reported");

    // Deliver the drag to the *target* window, as a backend does on the OS receive path:
    // DragMove to hover, then Drop with completion reported back to the source.
    let hover = LogicalPosition::new(50.0, 50.0);
    let action = access_testing_window(source.window(), |w| {
        w.simulate_native_drag_move(target.window(), hover);
        assert!(target.get_has_drag(), "the DropArea should report the hovering drag");
        w.simulate_native_drop(target.window(), hover)
    });

    assert!(target.get_got_drop(), "the drop should have reached the target DropArea");
    assert_eq!(target.get_dropped_text(), "payload-text");
    assert_eq!(action, DragAction::Copy);
    assert!(source.get_finished(), "drag-finished should have fired on the source");
    assert_eq!(source.get_finished_action(), DragAction::Copy);
}

// The backend takes over the drag, but the native start then fails, so it falls back to the
// in-window drag via `start_in_window_drag` — as a backend does when the native start errors.
// The in-window machinery then drives the drag to its drop.
#[test]
fn native_drag_falls_back_to_in_window() {
    i_slint_backend_testing::init_no_event_loop();

    let ui = SameWindow::new().unwrap();
    ui.on_to_transfer(slint::DataTransfer::from);
    ui.on_accept(|data| data.has_plain_text());
    ui.on_text_of(|data| data.plain_text().unwrap_or_default());
    access_testing_window(ui.window(), |w| w.set_simulate_native_drag(true));

    // Start the drag on the DragArea (top half): the backend takes over and records it.
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position: LogicalPosition::new(50.0, 25.0),
        button: PointerEventButton::Left,
    });
    ui.window()
        .dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(50.0, 60.0) });
    assert!(ui.get_dragging(), "start_drag should have taken the drag over");

    // The native start "fails": fall back to the in-window drag, as a backend does.
    WindowInner::from_pub(ui.window()).start_in_window_drag();

    // The in-window machinery now drives it: move over the DropArea (bottom half), then release.
    ui.window()
        .dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(50.0, 150.0) });
    assert!(ui.get_has_drag(), "the in-window drag should hover the DropArea");
    assert!(!ui.get_got_drop());

    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position: LogicalPosition::new(50.0, 150.0),
        button: PointerEventButton::Left,
    });

    assert!(ui.get_got_drop(), "the in-window drop should reach the DropArea");
    assert_eq!(ui.get_dropped_text(), "payload-text");
    assert!(ui.get_finished(), "drag-finished should fire when the in-window drag ends");
    assert_eq!(ui.get_finished_action(), DragAction::Copy);
}
