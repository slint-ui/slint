// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Native drag-and-drop handoff, driven by a custom backend whose `start_drag` takes over the
//! drag (like Qt). The testing backend declines `start_drag`, so the `dragarea_*` cases only
//! cover the in-window fallback.

use slint::platform::software_renderer::SoftwareRenderer;
use slint::platform::{PlatformError, PointerEventButton, WindowAdapter, WindowEvent};
use slint::{LogicalPosition, PhysicalSize};
use std::cell::RefCell;
use std::rc::{Rc, Weak};

use i_slint_core::InternalToken;
use i_slint_core::window::{DragRequest, WindowAdapterInternal, WindowInner};
use slint::private_unstable_api::re_exports::{
    AllowedDragActions, DragAction, DropEvent, MouseEvent,
};

thread_local! {
    static NEXT_WINDOW: RefCell<Option<Rc<TestWindow>>> = const { RefCell::new(None) };
}

struct TestPlatform;
impl slint::platform::Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(NEXT_WINDOW
            .with(|c| c.borrow_mut().take())
            .expect("queue a TestWindow before creating a component"))
    }
}

/// A window adapter that takes over native drags by recording them. It never renders; the
/// renderer only exists to satisfy the trait.
struct TestWindow {
    window: slint::Window,
    renderer: SoftwareRenderer,
    /// The data captured by `start_drag`; the cross-window test uses it to build the drop, and
    /// its presence confirms the backend took the drag over.
    dragged_data: RefCell<Option<slint::DataTransfer>>,
}

impl TestWindow {
    fn new() -> Rc<Self> {
        Rc::new_cyclic(|w: &Weak<Self>| Self {
            window: slint::Window::new(w.clone()),
            renderer: SoftwareRenderer::new(),
            dragged_data: Default::default(),
        })
    }
}

impl WindowAdapter for TestWindow {
    fn window(&self) -> &slint::Window {
        &self.window
    }

    fn size(&self) -> PhysicalSize {
        Default::default()
    }

    fn renderer(&self) -> &dyn slint::platform::Renderer {
        &self.renderer
    }

    fn internal(&self, _: InternalToken) -> Option<&dyn WindowAdapterInternal> {
        Some(self)
    }
}

impl WindowAdapterInternal for TestWindow {
    // Take over the drag and record its data; the test routes the drop itself.
    fn start_drag(&self, request: &DragRequest) -> bool {
        *self.dragged_data.borrow_mut() = Some(request.data().clone());
        true
    }
}

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
            allow-move: true;
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
        VerticalLayout {
            DragArea {
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

/// Make a window adapter for the next component creation to pick up.
fn queue_window() -> Rc<TestWindow> {
    let window = TestWindow::new();
    NEXT_WINDOW.with(|c| *c.borrow_mut() = Some(window.clone()));
    window
}

/// The OS negotiates the allowed actions, so for an incoming drop let any DropArea choice through.
const ALL_ACTIONS: AllowedDragActions = AllowedDragActions { copy: true, move_: true, link: true };

#[test]
fn native_drag_across_windows() {
    slint::platform::set_platform(Box::new(TestPlatform)).ok();

    let source_window = queue_window();
    let source = Source::new().unwrap();
    source.on_to_transfer(slint::DataTransfer::from);

    queue_window();
    let target = Target::new().unwrap();
    target.on_accept(|data| data.has_plain_text());
    target.on_text_of(|data| data.plain_text().unwrap_or_default());

    // Press, then move past the 8px drag threshold: this calls our `start_drag`, which takes
    // over (so no in-window drag is armed) and records the dragged data.
    source.window().dispatch_event(WindowEvent::PointerPressed {
        position: LogicalPosition::new(50.0, 50.0),
        button: PointerEventButton::Left,
    });
    source
        .window()
        .dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(50.0, 90.0) });

    let data = source_window
        .dragged_data
        .borrow_mut()
        .take()
        .expect("start_drag should have been invoked and taken over the drag");
    assert!(!source.get_finished(), "drag isn't finished until reported");

    // Deliver the drag to the *target* window as a backend's receive path does: DragMove to
    // hover, then Drop.
    let mut event = DropEvent::default();
    event.data = data;
    event.position = LogicalPosition::new(50.0, 50.0);
    event.proposed_action = DragAction::Copy;

    let target_inner = WindowInner::from_pub(target.window());
    target_inner
        .process_mouse_input(MouseEvent::DragMove { event: event.clone(), allowed: ALL_ACTIONS });
    assert!(target.get_has_drag(), "the DropArea should report the hovering drag");

    let action = target_inner
        .process_mouse_input(MouseEvent::Drop { event, allowed: ALL_ACTIONS })
        .and_then(|r| r.drag_action)
        .unwrap_or(DragAction::None);

    // Report completion to the source, as the backend does when the OS drag ends.
    WindowInner::from_pub(source.window()).report_drag_finished(action);

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
    slint::platform::set_platform(Box::new(TestPlatform)).ok();

    let window = queue_window();
    let ui = SameWindow::new().unwrap();
    ui.on_to_transfer(slint::DataTransfer::from);
    ui.on_accept(|data| data.has_plain_text());
    ui.on_text_of(|data| data.plain_text().unwrap_or_default());

    // Start the drag on the DragArea (top half): the backend takes over and records it.
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position: LogicalPosition::new(50.0, 25.0),
        button: PointerEventButton::Left,
    });
    ui.window()
        .dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(50.0, 60.0) });
    assert!(
        window.dragged_data.borrow().is_some(),
        "start_drag should have been invoked and taken over the drag"
    );

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
