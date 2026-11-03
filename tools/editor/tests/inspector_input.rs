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
    #[edition = "1.19"]
    import { InspectorTextFieldBase } from "../ui/components/inspector/text-field-base.slint";
    import { InspectorComboBox } from "../ui/components/inspector/combo-box.slint";
    import { EditorWindow } from "../ui/components/index.slint";
    import { ComboBoxBase, ComboBoxPopupBase } from "../controls/src/headless.slint";
    import { Style, FieldStyle } from "../ui/style.slint";

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
    export component MultilineInputTest inherits Window {
        width: 240px;
        height: 120px;
        in property <length> test-font-size: 12px;
        in property <string> test-font-family: "serif";
        default-font-family: root.test-font-family;
        private property <FieldStyle> input-style: Style.field;
        init => {
            root.input-style.text.size = root.test-font-size;
            root.input-style.appearance.muted = root.input-style.appearance.normal.foreground;
        }
        changed test-font-size => { root.input-style.text.size = root.test-font-size; }
        public function blur() { focus-target.focus(); }
        focus-target := FocusScope { x: 230px; width: 1px; height: 1px; }
        field := InspectorTextFieldBase {
            width: 220px;
            height: 100px;
            style: root.input-style;
            multiline: true;
            accessible-name: "Multiline";
            placeholder: "Type here";
            accepted(text) => { return true; }
        }
    }
    export component ComboSelectionTest inherits Window {
        width: 240px;
        height: 160px;
        in property <bool> allow-selection;
        out property <int> selection: combo.current-index;
        out property <bool> expanded: combo.expanded;
        out property <int> openings;
        out property <bool> opening-was-closed;
        out property <int> selections;
        public function open() { combo.open(); }
        public function select(index: int) { combo.select(index); }

        combo := ComboBoxBase {
            width: 140px;
            height: 32px;
            item-height: 28px;
            model: ["One", "Two", "Three"];
            opening => {
                root.openings += 1;
                root.opening-was-closed = !self.expanded;
            }
            accept-selection(index) => { return root.allow-selection; }
            selected(value) => { root.selections += 1; }
            background << Rectangle { background: white; }
            content << Text { text: combo.current-value; }
            popup << ComboBoxPopupBase { Rectangle { background: white; } }
        }
    }
    export component InspectorComboPlacementTest inherits Window {
        width: 400px;
        height: 400px;
        in property <bool> show-preview-on-open;
        out property <bool> preview-visible;
        out property <bool> focus-visible: combo.focus-visible;
        public function focus-before() { before.focus(); }
        init => { EditorWindow.width = root.width; EditorWindow.height = root.height; }
        before := FocusScope { x: 0px; y: 0px; width: 1px; height: 1px; }
        combo := InspectorComboBox {
            x: 20px; y: 180px; width: 220px;
            accessible-name: "Placement combo";
            wide: true;
            options: [{ value: "one", label: "One" }, { value: "two", label: "Two" }, { value: "three", label: "Three" }];
            model: ["one", "two", "three"];
            value: "Two";
            selected-index: 1;
            popup-upwards: root.preview-visible;
            popup-opening => { root.preview-visible = root.show-preview-on-open; }
            accepted(value) => { return true; }
        }
        if root.preview-visible: preview := Rectangle {
            x: 20px; y: combo.y + combo.height + 8px; width: 220px; height: 100px;
            accessible-role: image;
            accessible-label: "Placement preview";
        }
    }
}

#[test]
fn inspector_combo_overlaps_selection_and_keeps_preview_clear() {
    use i_slint_backend_testing::ElementHandle;
    use i_slint_core::window::{PopupWindowLocation, WindowInner};
    use slint::platform::Key;

    i_slint_backend_testing::init_no_event_loop();
    for show_preview in [false, true] {
        let editor = InspectorComboPlacementTest::new().unwrap();
        editor.set_show_preview_on_open(show_preview);
        editor.show().unwrap();
        let combo =
            ElementHandle::find_by_accessible_label(&editor, "Placement combo").next().unwrap();
        combo.mock_single_click(PointerEventButton::Left);
        slint::platform::update_timers_and_animations();
        assert!(!editor.get_focus_visible());
        let popup_position = {
            let window = WindowInner::from_pub(editor.window());
            let popups = window.active_popups();
            match popups.last().unwrap().location {
                PopupWindowLocation::ChildWindow(position) => position,
                _ => panic!("expected an embedded popup"),
            }
        };
        let popup = ElementHandle::find_by_element_id(&editor, "InspectorComboBox::popup-surface")
            .next()
            .unwrap();
        let popup_bottom = popup_position.y + popup.size().height;
        let control_center = combo.absolute_position().y + combo.size().height / 2.;
        if show_preview {
            let preview = ElementHandle::find_by_accessible_label(&editor, "Placement preview")
                .next()
                .unwrap();
            assert_eq!(popup_bottom, combo.absolute_position().y + combo.size().height);
            assert!(popup_bottom < preview.absolute_position().y);
        } else {
            let selected = ElementHandle::find_by_accessible_label(&editor, "Two")
                .find(|item| {
                    item.accessible_role()
                        == Some(i_slint_backend_testing::AccessibleRole::ListItem)
                })
                .unwrap();
            assert_eq!(
                popup_position.y + selected.absolute_position().y + selected.size().height / 2.,
                control_center
            );
            assert!(popup_bottom > combo.absolute_position().y + combo.size().height);
        }
        editor.window().dispatch_event(WindowEvent::KeyPressed { text: Key::Escape.into() });
        editor.window().dispatch_event(WindowEvent::KeyReleased { text: Key::Escape.into() });
        assert!(!editor.get_focus_visible());
        editor.invoke_focus_before();
        editor.window().dispatch_event(WindowEvent::KeyPressed { text: Key::Tab.into() });
        editor.window().dispatch_event(WindowEvent::KeyReleased { text: Key::Tab.into() });
        assert!(editor.get_focus_visible());
    }
}

#[test]
fn headless_combo_rejects_edits_before_changing_selection_or_closing() {
    use slint::platform::Key;

    i_slint_backend_testing::init_no_event_loop();
    let editor = ComboSelectionTest::new().unwrap();
    editor.show().unwrap();
    let key = |key: Key| {
        editor.window().dispatch_event(WindowEvent::KeyPressed { text: key.into() });
        editor.window().dispatch_event(WindowEvent::KeyReleased { text: key.into() });
        slint::platform::update_timers_and_animations();
    };
    editor.invoke_open();
    assert_eq!(editor.get_openings(), 1);
    assert!(editor.get_opening_was_closed());
    key(Key::DownArrow);
    key(Key::Return);
    assert!(editor.get_expanded());
    assert_eq!(editor.get_selection(), 0);
    assert_eq!(editor.get_selections(), 0);
    editor.set_allow_selection(true);
    key(Key::Return);
    assert!(!editor.get_expanded());
    assert_eq!(editor.get_selection(), 1);
    assert_eq!(editor.get_selections(), 1);
    editor.invoke_open();
    key(Key::Home);
    key(Key::Escape);
    assert_eq!(editor.get_selection(), 1);
    assert_eq!(editor.get_selections(), 1);
    editor.set_allow_selection(false);
    editor.invoke_select(2);
    assert_eq!(editor.get_selection(), 1);
    assert_eq!(editor.get_selections(), 1);
}

#[test]
fn multiline_placeholder_and_scrolling_follow_font_metrics() {
    use slint::platform::Key;

    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            mock_time: true,
            threading: false,
            renderer_name: Some("software".into()),
        },
    )))
    .unwrap();
    for family in ["serif", "monospace"] {
        for size in [12., 24.] {
            let editor = MultilineInputTest::new().unwrap();
            editor.set_test_font_family(family.into());
            editor.set_test_font_size(size);
            editor.show().unwrap();
            slint::platform::update_timers_and_animations();
            let find = |id| {
                i_slint_backend_testing::ElementHandle::find_by_element_id(&editor, id)
                    .next()
                    .unwrap()
            };
            let input = find("EditorInput::input");
            let viewport = find("EditorInput::viewport");
            let placeholder = find("EditorInput::placeholder-text");
            assert_eq!(placeholder.absolute_position().y, input.absolute_position().y);
            let before = editor.window().take_snapshot().unwrap();
            i_slint_backend_testing::ElementHandle::find_by_accessible_label(&editor, "Multiline")
                .next()
                .unwrap()
                .invoke_accessible_default_action();
            let key = |text: slint::SharedString| {
                editor.window().dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
                editor.window().dispatch_event(WindowEvent::KeyReleased { text });
                slint::platform::update_timers_and_animations();
            };
            key("Type here".into());
            editor.invoke_blur();
            slint::platform::update_timers_and_animations();
            let after = editor.window().take_snapshot().unwrap();
            assert!(
                before.as_bytes() == after.as_bytes(),
                "placeholder shifts for {family}, size {size}"
            );
            i_slint_backend_testing::ElementHandle::find_by_accessible_label(&editor, "Multiline")
                .next()
                .unwrap()
                .invoke_accessible_default_action();
            for _ in 0..20 {
                key("Line".into());
                key(Key::Return.into());
            }
            assert!(input.absolute_position().y < viewport.absolute_position().y);
            editor.window().dispatch_event(WindowEvent::KeyPressed { text: Key::Control.into() });
            key(if cfg!(target_os = "macos") { Key::UpArrow } else { Key::Home }.into());
            editor.window().dispatch_event(WindowEvent::KeyReleased { text: Key::Control.into() });
            assert_eq!(input.absolute_position().y, viewport.absolute_position().y);
            editor.window().dispatch_event(WindowEvent::KeyPressed { text: Key::Control.into() });
            key(if cfg!(target_os = "macos") { Key::DownArrow } else { Key::End }.into());
            editor.window().dispatch_event(WindowEvent::KeyReleased { text: Key::Control.into() });
            assert!(input.absolute_position().y < viewport.absolute_position().y);
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
            "EditorInput::input",
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
