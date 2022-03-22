// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! Helper for wasm that adds a hidden `<input>`  and process its events
//!
//! Without it, the key event are sent to the canvas and processed by winit.
//! But this winit handling doesn't show the keyboard on mobile devices, and
//! also has bugs as the modifiers are not reported the same way and we don't
//! record them.
//!
//! This just interpret the keyup and keydown events. But this is not working
//! on mobile either as we only get these for a bunch of non-printable key
//! that do not interact with the composing input. For anything else we
//! check that we get input event when no normal key are pressed, and we send
//! that as text. Ignore the composition event until the end.

use std::cell::Cell;
use std::rc::{Rc, Weak};

use i_slint_core::input::{KeyEvent, KeyEventType, KeyboardModifiers};
use i_slint_core::SharedString;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::convert::FromWasmAbi;
use wasm_bindgen::JsCast;

pub struct WasmInputHelper {
    input: web_sys::HtmlInputElement,
}

impl WasmInputHelper {
    #[allow(unused)]
    pub fn new(
        window: Weak<i_slint_core::window::Window>,
        canvas: web_sys::HtmlCanvasElement,
    ) -> Self {
        let input = web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .create_element("input")
            .unwrap()
            .dyn_into::<web_sys::HtmlInputElement>()
            .unwrap();
        let style = input.style();
        style.set_property("z-index", "-1").unwrap();
        style.set_property("position", "absolute").unwrap();
        style.set_property("left", &format!("{}px", canvas.offset_left())).unwrap();
        style.set_property("top", &format!("{}px", canvas.offset_top())).unwrap();
        style.set_property("width", &format!("{}px", canvas.offset_width())).unwrap();
        style.set_property("height", &format!("{}px", canvas.offset_height())).unwrap();
        canvas.before_with_node_1(&input).unwrap();
        let mut h = Self { input };

        let has_key_down = Rc::new(Cell::new(false));

        let win = window.clone();
        h.add_event_listener("blur", move |_: web_sys::Event| {
            // Make sure that the window gets marked as unfocused when the focus leaves the input
            if let Some(window) = win.upgrade() {
                if !canvas.matches(":focus").unwrap_or(false) {
                    window.set_active(false);
                    window.set_focus(false);
                }
            }
        });
        let win = window.clone();
        let has_key_down2 = has_key_down.clone();
        h.add_event_listener("keydown", move |e: web_sys::KeyboardEvent| {
            if let (Some(window), Some(text)) = (win.upgrade(), event_text(&e)) {
                has_key_down2.set(true);
                window.process_key_input(&KeyEvent {
                    modifiers: modifiers(&e),
                    text,
                    event_type: KeyEventType::KeyPressed,
                });
            }
        });

        let win = window.clone();
        let has_key_down2 = has_key_down.clone();
        h.add_event_listener("keyup", move |e: web_sys::KeyboardEvent| {
            if let (Some(window), Some(text)) = (win.upgrade(), event_text(&e)) {
                has_key_down2.set(false);
                window.process_key_input(&KeyEvent {
                    modifiers: modifiers(&e),
                    text,
                    event_type: KeyEventType::KeyReleased,
                });
            }
        });

        let win = window.clone();
        let has_key_down2 = has_key_down.clone();
        let input = h.input.clone();
        h.add_event_listener("input", move |e: web_sys::InputEvent| {
            if let (Some(window), Some(data)) = (win.upgrade(), e.data()) {
                if !has_key_down2.get() && !e.is_composing() {
                    let text = SharedString::from(data.as_str());
                    window.clone().process_key_input(&KeyEvent {
                        modifiers: Default::default(),
                        text: text.clone(),
                        event_type: KeyEventType::KeyPressed,
                    });
                    window.process_key_input(&KeyEvent {
                        modifiers: Default::default(),
                        text,
                        event_type: KeyEventType::KeyReleased,
                    });
                    input.set_value("");
                    has_key_down2.set(false);
                }
            }
        });

        let win = window.clone();
        h.add_event_listener("compositionend", move |e: web_sys::CompositionEvent| {
            if let (Some(window), Some(data)) = (win.upgrade(), e.data()) {
                let text = SharedString::from(data.as_str());
                window.clone().process_key_input(&KeyEvent {
                    modifiers: Default::default(),
                    text: text.clone(),
                    event_type: KeyEventType::KeyPressed,
                });
                window.process_key_input(&KeyEvent {
                    modifiers: Default::default(),
                    text,
                    event_type: KeyEventType::KeyReleased,
                });
            }
        });

        h
    }

    /// Returns wether the fake input element has focus
    pub fn has_focus(&self) -> bool {
        self.input.matches(":focus").unwrap_or(false)
    }

    pub fn show(&self) {
        self.input.style().set_property("visibility", "visible").unwrap();
        self.input.focus().unwrap();
    }

    pub fn hide(&self) {
        self.input.blur().unwrap();
        self.input.style().set_property("visibility", "hidden").unwrap();
    }

    fn add_event_listener<Arg: FromWasmAbi + 'static>(
        &mut self,
        event: &str,
        closure: impl Fn(Arg) + 'static,
    ) {
        let closure = Closure::wrap(Box::new(closure) as Box<dyn Fn(_)>);
        self.input
            .add_event_listener_with_callback(event, closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }
}

fn event_text(e: &web_sys::KeyboardEvent) -> Option<SharedString> {
    if e.is_composing() {
        return None;
    }

    let key = e.key();

    let convert = |char: char| {
        let mut buffer = [0; 6];
        Some(SharedString::from(char.encode_utf8(&mut buffer) as &str))
    };

    macro_rules! check_non_printable_code {
        ($($char:literal # $name:ident # $($_qt:ident)|* # $($_winit:ident)|* ;)*) => {
            match key.as_str() {
                $(stringify!($name) => {
                    return convert($char);
                })*
                // Why did we diverge from DOM there?
                "ArrowLeft" => return convert(i_slint_core::input::key_codes::LeftArrow),
                "ArrowUp" => return convert(i_slint_core::input::key_codes::UpArrow),
                "ArrowRight" => return convert(i_slint_core::input::key_codes::RightArrow),
                "ArrowDown" => return convert(i_slint_core::input::key_codes::DownArrow),
                "Enter" => return convert(i_slint_core::input::key_codes::Return),
                _ => (),
            }
        };
    }
    i_slint_common::for_each_special_keys!(check_non_printable_code);
    if key.chars().count() == 1 {
        Some(key.as_str().into())
    } else {
        None
    }
}

fn modifiers(e: &web_sys::KeyboardEvent) -> KeyboardModifiers {
    KeyboardModifiers {
        alt: e.alt_key(),
        control: e.ctrl_key(),
        meta: e.meta_key(),
        shift: e.shift_key(),
    }
}
