// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

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

use i_slint_core::input::{KeyEvent, KeyEventType};
use i_slint_core::platform::WindowEvent;
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
}

impl WasmInputHelper {
    #[allow(unused)]
    pub fn new(
        window_adapter: Weak<dyn WindowAdapter>,
        canvas: web_sys::HtmlCanvasElement,
    ) -> Self {
        let window = web_sys::window().unwrap();
        let input = window
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

        // macos, or ipad with an attached keyboard, etc.
        let is_apple = window.navigator().platform().ok().map_or(false, |platform| {
            let platform = platform.to_ascii_lowercase();
            platform.contains("mac") || platform.contains("iphone") || platform.contains("ipad")
        });

        let shared_state = Rc::new(RefCell::new(WasmInputState::default()));
        #[cfg(web_sys_unstable_apis)]
        {
            let win = window_adapter.clone();
            h.add_event_listener("paste", move |e: web_sys::ClipboardEvent| {
                if let Some(window_adapter) = win.upgrade() {
                    let Some(text) = e.clipboard_data().and_then(|data| data.get_data("text").ok())
                    else {
                        return;
                    };
                    e.prevent_default();
                    let synthetic_clipboard_data = RefCell::new(text);
                    CURRENT_WASM_CLIPBOARD_DATA.set(&synthetic_clipboard_data, || {
                        if let Some(focus_item) = WindowInner::from_pub(&window_adapter.window())
                            .focus_item
                            .borrow()
                            .upgrade()
                        {
                            if let Some(text_input) =
                                focus_item.downcast::<i_slint_core::items::TextInput>()
                            {
                                text_input.as_pin_ref().paste(&window_adapter, &focus_item);
                            }
                        }
                    })
                }
            });
            let win = window_adapter.clone();
            h.add_event_listener("copy", move |e: web_sys::ClipboardEvent| {
                if let Some(window_adapter) = win.upgrade() {
                    e.prevent_default();

                    let synthetic_clipboard_data = RefCell::new(String::default());
                    CURRENT_WASM_CLIPBOARD_DATA.set(&synthetic_clipboard_data, || {
                        if let Some(focus_item) = WindowInner::from_pub(&window_adapter.window())
                            .focus_item
                            .borrow()
                            .upgrade()
                        {
                            if let Some(text_input) =
                                focus_item.downcast::<i_slint_core::items::TextInput>()
                            {
                                let text =
                                    text_input.as_pin_ref().copy(&window_adapter, &focus_item);
                            }
                        }
                    });
                    if let Some(data) = e.clipboard_data() {
                        data.set_data("text", &synthetic_clipboard_data.into_inner()).ok();
                    }
                }
            });

            let win = window_adapter.clone();
            h.add_event_listener("cut", move |e: web_sys::ClipboardEvent| {
                if let Some(window_adapter) = win.upgrade() {
                    e.prevent_default();
                    if let Some(focus_item) = WindowInner::from_pub(&window_adapter.window())
                        .focus_item
                        .borrow()
                        .upgrade()
                    {
                        if let Some(text_input) =
                            focus_item.downcast::<i_slint_core::items::TextInput>()
                        {
                            let (anchor, cursor) =
                                text_input.as_pin_ref().selection_anchor_and_cursor();
                            if anchor == cursor {
                                return;
                            }
                            let text = text_input.as_pin_ref().text();
                            if let Some(data) = e.clipboard_data() {
                                data.set_data("text", &text[anchor..cursor]).ok();
                            }
                            text_input.as_pin_ref().delete_selection(
                                &window_adapter,
                                &focus_item,
                                i_slint_core::items::TextChangeNotify::TriggerCallbacks,
                            );
                        }
                    }
                }
            });
        }

        let win = window_adapter.clone();
        h.add_event_listener("blur", move |_: web_sys::Event| {
            // Make sure that the window gets marked as unfocused when the focus leaves the input
            if let Some(window_adapter) = win.upgrade() {
                if !canvas.matches(":focus").unwrap_or(false) {
                    window_adapter.window().dispatch_event(WindowEvent::WindowActiveChanged(false));
                }
            }
        });
        let win = window_adapter.clone();
        let shared_state2 = shared_state.clone();
        h.add_event_listener("keydown", move |e: web_sys::KeyboardEvent| {
            if let (Some(window_adapter), Some(mut text)) =
                (win.upgrade(), event_text(&e, is_apple))
            {
                // Same logic as in winit to prevent the default <https://github.com/rust-windowing/winit/blob/master/src/platform_impl/web/web_sys/canvas.rs#L202-L213>
                let event_key = &e.key();
                let is_key_string = event_key.len() == 1 || !event_key.is_ascii();
                let ctrl_key = if is_apple { e.meta_key() } else { e.ctrl_key() };
                let is_shortcut_modifiers =
                    (ctrl_key || e.alt_key()) && !e.get_modifier_state("AltGr");
                if !is_key_string || is_shortcut_modifiers {
                    // Also let copy/paste/cut through
                    if !matches!(text.as_str(), "c" | "v" | "x") {
                        e.prevent_default();
                    }
                }

                shared_state2.borrow_mut().has_key_down = true;
                let win_event = if e.repeat() {
                    WindowEvent::KeyPressRepeated { text }
                } else {
                    WindowEvent::KeyPressed { text }
                };
                window_adapter.window().dispatch_event(win_event);
            }
        });

        let win = window_adapter.clone();
        let shared_state2 = shared_state.clone();
        h.add_event_listener("keyup", move |e: web_sys::KeyboardEvent| {
            if let (Some(window_adapter), Some(mut text)) =
                (win.upgrade(), event_text(&e, is_apple))
            {
                e.prevent_default();
                shared_state2.borrow_mut().has_key_down = false;
                window_adapter.window().dispatch_event(WindowEvent::KeyReleased { text });
            }
        });

        let win = window_adapter.clone();
        let shared_state2 = shared_state.clone();
        let input = h.input.clone();
        h.add_event_listener("input", move |e: web_sys::InputEvent| {
            if let (Some(window_adapter), Some(data)) = (win.upgrade(), e.data()) {
                if !e.is_composing() && e.input_type() != "insertCompositionText" {
                    if !shared_state2.borrow_mut().has_key_down {
                        let text: SharedString = data.into();
                        window_adapter
                            .window()
                            .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
                        window_adapter.window().dispatch_event(WindowEvent::KeyReleased { text });
                        shared_state2.borrow_mut().has_key_down = false;
                    }
                    input.set_value("");
                }
            }
        });

        let win = window_adapter.clone();
        let input = h.input.clone();
        h.add_event_listener("compositionend", move |e: web_sys::CompositionEvent| {
            if let (Some(window_adapter), Some(data)) = (win.upgrade(), e.data()) {
                let window_inner = WindowInner::from_pub(window_adapter.window());
                window_inner.process_key_input(KeyEvent {
                    text: data.into(),
                    event_type: KeyEventType::CommitComposition,
                    ..Default::default()
                });
                input.set_value("");
            }
        });

        let win = window_adapter.clone();
        h.add_event_listener("compositionupdate", move |e: web_sys::CompositionEvent| {
            if let (Some(window_adapter), Some(data)) = (win.upgrade(), e.data()) {
                let window_inner = WindowInner::from_pub(window_adapter.window());
                window_inner.process_key_input(KeyEvent {
                    preedit_text: data.into(),
                    event_type: KeyEventType::UpdateComposition,
                    ..Default::default()
                });
            }
        });

        h
    }

    /// Returns whether the fake input element has focus
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
            // wake up event loop
            i_slint_core::api::invoke_from_event_loop(|| {}).ok();
        };
        let closure = Closure::wrap(Box::new(closure) as Box<dyn Fn(_)>);
        self.input
            .add_event_listener_with_callback(event, closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }
}

fn event_text(e: &web_sys::KeyboardEvent, is_apple: bool) -> Option<SharedString> {
    if e.is_composing() {
        return None;
    }

    let key = e.key();

    use i_slint_core::platform::Key;

    macro_rules! check_non_printable_code {
        ($($char:literal # $name:ident # $($_qt:ident)|* # $($_winit:ident $(($_pos:ident))?)|* # $($_xkb:ident)|* ;)*) => {
            match key.as_str() {
                "Tab" if e.shift_key() => return Some(Key::Backtab.into()),
                "Meta" if is_apple => return Some(Key::Control.into()),
                "Control" if is_apple => return Some(Key::Meta.into()),
                $(stringify!($name) => {
                    return Some($char.into());
                })*
                // Why did we diverge from DOM there?
                "ArrowLeft" => return Some(Key::LeftArrow.into()),
                "ArrowUp" => return Some(Key::UpArrow.into()),
                "ArrowRight" => return Some(Key::RightArrow.into()),
                "ArrowDown" => return Some(Key::DownArrow.into()),
                "Enter" => return Some(Key::Return.into()),
                _ => (),
            }
        };
    }
    i_slint_common::for_each_special_keys!(check_non_printable_code);

    let mut chars = key.chars();
    match chars.next() {
        Some(first_char) if chars.next().is_none() => Some(first_char.into()),
        _ => None,
    }
}

scoped_tls_hkt::scoped_thread_local!(static CURRENT_WASM_CLIPBOARD_DATA : for<'a> &'a RefCell<String>);

pub(crate) fn set_clipboard_text(data: String, clipboard: i_slint_core::platform::Clipboard) {
    if CURRENT_WASM_CLIPBOARD_DATA.is_set()
        && matches!(clipboard, i_slint_core::platform::Clipboard::DefaultClipboard)
    {
        CURRENT_WASM_CLIPBOARD_DATA.with(|current_data| *current_data.borrow_mut() = data)
    }
}

pub(crate) fn get_clipboard_text(clipboard: i_slint_core::platform::Clipboard) -> Option<String> {
    if CURRENT_WASM_CLIPBOARD_DATA.is_set()
        && matches!(clipboard, i_slint_core::platform::Clipboard::DefaultClipboard)
    {
        Some(CURRENT_WASM_CLIPBOARD_DATA.with(|current_data| current_data.borrow().clone()))
    } else {
        None
    }
}
