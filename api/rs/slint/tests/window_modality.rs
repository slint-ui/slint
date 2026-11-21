// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! `Window::show_modal` answers for itself on two counts that need no windowing system:
//! a backend that doesn't implement modality has to say so rather than show the window
//! anyway, and a window that is already up can't be given modality afterwards.

use i_slint_core::window::{WindowAdapter, WindowAdapterInternal};
use slint::PhysicalSize;
use slint::platform::software_renderer::SoftwareRenderer;
use slint::{PlatformError, WindowModality};
use std::cell::Cell;
use std::rc::Rc;

slint::slint! {
    export component Dialog inherits Window {
        width: 100px;
        height: 100px;
    }
}

/// A backend that does implement modality, so that a refusal can only come from the
/// check in `Window::show_modal` and not from the default that answers `Unsupported`.
struct ModalWindow {
    window: slint::Window,
    renderer: SoftwareRenderer,
    /// What the last `show_modal` was asked for: `None` for the application, the parent
    /// window otherwise.
    modal_to: Cell<Option<Option<*const slint::Window>>>,
}

impl ModalWindow {
    fn new() -> Rc<Self> {
        Rc::new_cyclic(|weak| ModalWindow {
            window: slint::Window::new(weak.clone() as _),
            renderer: SoftwareRenderer::new(),
            modal_to: Cell::new(None),
        })
    }
}

impl WindowAdapter for ModalWindow {
    fn window(&self) -> &slint::Window {
        &self.window
    }
    fn size(&self) -> PhysicalSize {
        PhysicalSize::new(100, 100)
    }
    fn renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }
    fn internal(&self, _: i_slint_core::InternalToken) -> Option<&dyn WindowAdapterInternal> {
        Some(self)
    }
}

impl WindowAdapterInternal for ModalWindow {
    fn show_modal(&self, modality: WindowModality<'_>) -> Result<(), PlatformError> {
        self.modal_to.set(Some(match modality {
            WindowModality::Application => None,
            WindowModality::Window(parent) => Some(parent as *const slint::Window),
            _ => return Err(PlatformError::Unsupported),
        }));
        Ok(())
    }
}

struct ModalPlatform;
impl slint::platform::Platform for ModalPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(ModalWindow::new())
    }
}

#[test]
fn a_backend_without_modality_says_so() {
    i_slint_backend_testing::init_no_event_loop();

    let dialog = Dialog::new().unwrap();
    assert!(matches!(
        dialog.window().show_modal(WindowModality::Application),
        Err(PlatformError::Unsupported)
    ));
    // Saying no means the window stays down; a caller that falls back to `show()` decides.
    assert!(!dialog.window().is_visible());
}

#[test]
fn a_window_that_is_already_shown_cannot_become_modal() {
    slint::platform::set_platform(Box::new(ModalPlatform)).unwrap();

    let dialog = Dialog::new().unwrap();
    // The backend takes modality, so this one succeeds.
    assert!(dialog.window().show_modal(WindowModality::Application).is_ok());

    let shown = Dialog::new().unwrap();
    shown.show().unwrap();
    assert!(shown.window().is_visible());
    assert!(matches!(
        shown.window().show_modal(WindowModality::Application),
        Err(PlatformError::Unsupported)
    ));
    shown.hide().unwrap();
}

#[test]
fn the_backend_is_told_which_window_to_be_modal_to() {
    slint::platform::set_platform(Box::new(ModalPlatform)).unwrap();

    let parent = Dialog::new().unwrap();
    let dialog = Dialog::new().unwrap();
    dialog.window().show_modal(WindowModality::Window(parent.window())).unwrap();
    assert_eq!(
        modal_to(dialog.window()),
        Some(Some(parent.window() as *const slint::Window)),
        "the parent window reached the backend"
    );

    let app_modal = Dialog::new().unwrap();
    app_modal.window().show_modal(WindowModality::Application).unwrap();
    assert_eq!(
        modal_to(app_modal.window()),
        Some(None),
        "no parent means modal to the application"
    );
}

fn modal_to(window: &slint::Window) -> Option<Option<*const slint::Window>> {
    i_slint_core::window::WindowInner::from_pub(window)
        .window_adapter()
        .internal(i_slint_core::InternalToken)
        .and_then(|internal| (internal as &dyn std::any::Any).downcast_ref::<ModalWindow>())
        .expect("the test platform makes ModalWindows")
        .modal_to
        .get()
}
