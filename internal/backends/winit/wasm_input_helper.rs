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
//! that as text.
//! Since the slint core lib doesn't support composition yet, when we get
//! composition event, we just send that as key, and if the composition changes,
//! we just simulate a few backspaces.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use i_slint_core::input::{KeyEvent, KeyEventType, KeyboardModifiers};
use i_slint_core::window::{WindowAdapter, WindowInner};
use i_slint_core::SharedString;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::convert::FromWasmAbi;
use wasm_bindgen::JsCast;

pub struct WasmInputHelper {
    input: web_sys::HtmlInputElement,
    canvas: web_sys::HtmlCanvasElement,
}

#[derive(Default)]
struct WasmInputState {
    /// If there was a "keydown" event received that is not part of a composition
    has_key_down: bool,
    /// The current composing text
    composition: String,
}

impl WasmInputState {
    /// Update the composition text and return the number of character to rollback and the string to add
    fn text_from_compose(&mut self, data: String, is_end: bool) -> (SharedString, usize) {
        let mut data_iter = data.char_indices().peekable();
        let mut composition_iter = self.composition.chars().peekable();
        // Skip common prefix
        while let (Some(c), Some((_, d))) = (composition_iter.peek(), data_iter.peek()) {
            if c != d {
                break;
            }
            composition_iter.next();
            data_iter.next();
        }
        let to_delete = composition_iter.count();
        let result = if let Some((idx, _)) = data_iter.next() {
            SharedString::from(&data[idx..])
        } else {
            SharedString::default()
        };
        self.composition = if is_end { String::new() } else { data };
        (result, to_delete)
    }
}

impl WasmInputHelper {
    #[allow(unused)]
    pub fn new(
        window_adapter: Weak<dyn WindowAdapter>,
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
        style.set_property("opacity", "0").unwrap(); // Hide the cursor on mobile Safari
        input.set_attribute("autocapitalize", "none").unwrap(); // Otherwise everything would be capitalized as we need to clear the input
        canvas.before_with_node_1(&input).unwrap();
        let mut h = Self { input, canvas: canvas.clone() };

        let shared_state = Rc::new(RefCell::new(WasmInputState::default()));

        let win = window_adapter.clone();
        h.add_event_listener("blur", move |_: web_sys::Event| {
            // Make sure that the window gets marked as unfocused when the focus leaves the input
            if let Some(window_adapter) = win.upgrade() {
                let window_inner = WindowInner::from_pub(window_adapter.window());
                if !canvas.matches(":focus").unwrap_or(false) {
                    window_inner.set_active(false);
                    window_inner.set_focus(false);
                }
            }
        });
        let win = window_adapter.clone();
        let shared_state2 = shared_state.clone();
        h.add_event_listener("keydown", move |e: web_sys::KeyboardEvent| {
            if let (Some(window_adapter), Some(text)) = (win.upgrade(), event_text(&e)) {
                e.prevent_default();
                shared_state2.borrow_mut().has_key_down = true;
                WindowInner::from_pub(window_adapter.window()).process_key_input(&KeyEvent {
                    modifiers: modifiers(&e),
                    text,
                    event_type: KeyEventType::KeyPressed,
                    ..Default::default()
                });
            }
        });

        let win = window_adapter.clone();
        let shared_state2 = shared_state.clone();
        h.add_event_listener("keyup", move |e: web_sys::KeyboardEvent| {
            if let (Some(window_adapter), Some(text)) = (win.upgrade(), event_text(&e)) {
                e.prevent_default();
                shared_state2.borrow_mut().has_key_down = false;
                WindowInner::from_pub(window_adapter.window()).process_key_input(&KeyEvent {
                    modifiers: modifiers(&e),
                    text,
                    event_type: KeyEventType::KeyReleased,
                    ..Default::default()
                });
            }
        });

        let win = window_adapter.clone();
        let shared_state2 = shared_state.clone();
        let input = h.input.clone();
        h.add_event_listener("input", move |e: web_sys::InputEvent| {
            if let (Some(window_adapter), Some(data)) = (win.upgrade(), e.data()) {
                if !e.is_composing() && e.input_type() != "insertCompositionText" {
                    if !shared_state2.borrow_mut().has_key_down {
                        let window_inner = WindowInner::from_pub(window_adapter.window());
                        let text = SharedString::from(data.as_str());
                        window_inner.process_key_input(&KeyEvent {
                            modifiers: Default::default(),
                            text: text.clone(),
                            event_type: KeyEventType::KeyPressed,
                            ..Default::default()
                        });
                        window_inner.process_key_input(&KeyEvent {
                            modifiers: Default::default(),
                            text,
                            event_type: KeyEventType::KeyReleased,
                            ..Default::default()
                        });
                        shared_state2.borrow_mut().has_key_down = false;
                    }
                    input.set_value("");
                }
            }
        });

        for event in ["compositionend", "compositionupdate"] {
            let win = window_adapter.clone();
            let shared_state2 = shared_state.clone();
            let input = h.input.clone();
            h.add_event_listener(event, move |e: web_sys::CompositionEvent| {
                if let (Some(window_adapter), Some(data)) = (win.upgrade(), e.data()) {
                    let window_inner = WindowInner::from_pub(window_adapter.window());
                    let is_end = event == "compositionend";
                    let (text, to_delete) =
                        shared_state2.borrow_mut().text_from_compose(data, is_end);
                    if to_delete > 0 {
                        let mut buffer = [0; 6];
                        let backspace = SharedString::from(
                            i_slint_core::input::key_codes::Backspace.encode_utf8(&mut buffer)
                                as &str,
                        );
                        for _ in 0..to_delete {
                            window_inner.process_key_input(&KeyEvent {
                                modifiers: Default::default(),
                                text: backspace.clone(),
                                event_type: KeyEventType::KeyPressed,
                                ..Default::default()
                            });
                        }
                    }
                    window_inner.process_key_input(&KeyEvent {
                        modifiers: Default::default(),
                        text: text.clone(),
                        event_type: KeyEventType::KeyPressed,
                        ..Default::default()
                    });
                    window_inner.process_key_input(&KeyEvent {
                        modifiers: Default::default(),
                        text,
                        event_type: KeyEventType::KeyReleased,
                        ..Default::default()
                    });
                    if is_end {
                        input.set_value("");
                    }
                }
            });
        }

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
        if self.has_focus() {
            self.canvas.focus().unwrap()
        }
        self.input.style().set_property("visibility", "hidden").unwrap();
    }

    fn add_event_listener<Arg: FromWasmAbi + 'static>(
        &mut self,
        event: &str,
        closure: impl Fn(Arg) + 'static,
    ) {
        let closure = move |arg: Arg| {
            closure(arg);
            crate::event_loop::GLOBAL_PROXY.with(|global_proxy| {
                if let Ok(mut x) = global_proxy.try_borrow_mut() {
                    if let Some(proxy) = &mut *x {
                        let _ = proxy
                            .send_event(crate::event_loop::CustomEvent::WakeEventLoopWorkaround);
                    }
                }
            });
        };
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
                "Tab" if e.shift_key() => return convert(i_slint_core::input::key_codes::Backtab),
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
