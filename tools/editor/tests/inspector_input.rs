// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{cell::RefCell, rc::Rc};

use slint::{
    ComponentHandle, LogicalPosition,
    platform::{
        Clipboard, Platform, PlatformError, PointerEventButton, WindowAdapter, WindowEvent,
    },
};

slint::slint! {
    import { InspectorTextFieldBase } from "../ui/components/inspector/text-field-base.slint";

    export component InspectorInputTest inherits Window {
        width: 200px;
        height: 40px;
        out property <bool> input-focused: field.input-focused;
        out property <string> text: field.accessible-value;

        field := InspectorTextFieldBase {
            value: "Inter";
            accepted(text) => { return true; }
        }
    }
}

struct PrimarySelectionPlatform {
    backend: i_slint_backend_testing::TestingBackend,
    primary: Rc<RefCell<String>>,
}

impl Platform for PrimarySelectionPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        self.backend.create_window_adapter()
    }

    fn set_clipboard_text(&self, text: &str, clipboard: Clipboard) {
        if clipboard == Clipboard::SelectionClipboard {
            *self.primary.borrow_mut() = text.into();
        } else {
            self.backend.set_clipboard_text(text, clipboard);
        }
    }

    fn clipboard_text(&self, clipboard: Clipboard) -> Option<String> {
        if clipboard == Clipboard::SelectionClipboard {
            Some(self.primary.borrow().clone())
        } else {
            self.backend.clipboard_text(clipboard)
        }
    }
}

#[test]
fn middle_click_pastes_primary_selection_into_unfocused_inspector_input() {
    let primary = Rc::new(RefCell::new(" Mono".into()));
    let backend = i_slint_backend_testing::TestingBackend::new(Default::default());
    backend.set_clipboard_text("regular clipboard", Clipboard::DefaultClipboard);
    slint::platform::set_platform(Box::new(PrimarySelectionPlatform {
        backend,
        primary: primary.clone(),
    }))
    .unwrap();

    for move_before_release in [false, true] {
        let field = InspectorInputTest::new().unwrap();
        field.show().unwrap();
        assert!(!field.get_input_focused());
        assert_eq!(field.get_text(), "Inter");

        let input = i_slint_backend_testing::ElementHandle::find_by_element_id(
            &field,
            "InspectorTextFieldBase::input",
        )
        .next()
        .unwrap();
        let origin = input.absolute_position();
        let size = input.size();
        let position =
            LogicalPosition::new(origin.x + size.width - 1., origin.y + size.height / 2.);
        field.window().dispatch_event(WindowEvent::PointerPressed {
            position,
            button: PointerEventButton::Middle,
        });
        slint::platform::update_timers_and_animations();
        assert!(field.get_input_focused());
        assert_eq!(*primary.borrow(), " Mono");
        assert_eq!(field.get_text(), "Inter");

        if move_before_release {
            field.window().dispatch_event(WindowEvent::PointerMoved { position });
        }
        field.window().dispatch_event(WindowEvent::PointerReleased {
            position,
            button: PointerEventButton::Middle,
        });
        slint::platform::update_timers_and_animations();
        assert_eq!(field.get_text(), "Inter Mono");
        assert_eq!(*primary.borrow(), " Mono");
    }
}
