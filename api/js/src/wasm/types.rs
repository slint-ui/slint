// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The value types the wasm binding exposes to JS: `Window`, `Keys` and
//! `StyledText`, mirroring the Node.js binding's classes of the same names.
//!
//! `Keys` and `StyledText` keep their Rust value in a thread-local registry
//! keyed by an integer id (the same pattern as `WasmSharedModelNotify`):
//! wasm-bindgen classes cannot be recovered from a `JsValue` on the Rust
//! side, but a hidden id getter can be read via `Reflect::get`, which is how
//! `js_to_value` converts them back.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use i_slint_core::window::WindowAdapterRc;
use slint_interpreter::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use wasm_bindgen::prelude::*;

fn get_f64(obj: &JsValue, key: &str) -> Option<f64> {
    js_sys::Reflect::get(obj, &key.into()).ok()?.as_f64()
}

fn point_to_js(x: f64, y: f64) -> JsValue {
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &"x".into(), &x.into()).unwrap_throw();
    js_sys::Reflect::set(&obj, &"y".into(), &y.into()).unwrap_throw();
    obj.into()
}

fn size_to_js(width: f64, height: f64) -> JsValue {
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &"width".into(), &width.into()).unwrap_throw();
    js_sys::Reflect::set(&obj, &"height".into(), &height.into()).unwrap_throw();
    obj.into()
}

/// This type represents a window towards the windowing system, that's used to render the
/// scene of a component. It provides API to control windowing system specific aspects such
/// as the position on the screen.
#[wasm_bindgen(js_name = "Window")]
pub struct WrappedWindow {
    pub(crate) inner: WindowAdapterRc,
}

#[wasm_bindgen(js_class = "Window")]
impl WrappedWindow {
    /// Shows the window on the screen. An additional strong reference on the
    /// associated component is maintained while the window is visible.
    #[wasm_bindgen]
    pub fn show(&self) -> Result<(), JsValue> {
        self.inner.window().show().map_err(|_| js_sys::Error::new("Cannot show window.").into())
    }

    /// Hides the window, so that it is not visible anymore.
    #[wasm_bindgen]
    pub fn hide(&self) -> Result<(), JsValue> {
        self.inner.window().hide().map_err(|_| js_sys::Error::new("Cannot hide window.").into())
    }

    /// Returns the visibility state of the window. This function can return false even if you previously called show()
    /// on it, for example if the user minimized the window.
    #[wasm_bindgen(getter)]
    pub fn visible(&self) -> bool {
        self.inner.window().is_visible()
    }

    /// Returns the logical position of the window on the screen.
    #[wasm_bindgen(getter, js_name = "logicalPosition")]
    pub fn logical_position(&self) -> JsValue {
        let pos = self.inner.window().position().to_logical(self.inner.window().scale_factor());
        point_to_js(pos.x as f64, pos.y as f64)
    }

    /// Sets the logical position of the window on the screen.
    #[wasm_bindgen(setter, js_name = "logicalPosition")]
    pub fn set_logical_position(&self, position: JsValue) {
        let (Some(x), Some(y)) = (get_f64(&position, "x"), get_f64(&position, "y")) else {
            return;
        };
        self.inner.window().set_position(LogicalPosition { x: x as f32, y: y as f32 });
    }

    /// Returns the physical position of the window on the screen.
    #[wasm_bindgen(getter, js_name = "physicalPosition")]
    pub fn physical_position(&self) -> JsValue {
        let pos = self.inner.window().position();
        point_to_js(pos.x as f64, pos.y as f64)
    }

    /// Sets the physical position of the window on the screen.
    #[wasm_bindgen(setter, js_name = "physicalPosition")]
    pub fn set_physical_position(&self, position: JsValue) {
        let (Some(x), Some(y)) = (get_f64(&position, "x"), get_f64(&position, "y")) else {
            return;
        };
        self.inner
            .window()
            .set_position(PhysicalPosition { x: x.floor() as i32, y: y.floor() as i32 });
    }

    /// Returns the logical size of the window on the screen,
    #[wasm_bindgen(getter, js_name = "logicalSize")]
    pub fn logical_size(&self) -> JsValue {
        let size = self.inner.window().size().to_logical(self.inner.window().scale_factor());
        size_to_js(size.width as f64, size.height as f64)
    }

    /// Sets the logical size of the window on the screen,
    #[wasm_bindgen(setter, js_name = "logicalSize")]
    pub fn set_logical_size(&self, size: JsValue) {
        let (Some(width), Some(height)) = (get_f64(&size, "width"), get_f64(&size, "height"))
        else {
            return;
        };
        self.inner.window().set_size(LogicalSize::from_physical(
            PhysicalSize { width: width.floor() as u32, height: height.floor() as u32 },
            self.inner.window().scale_factor(),
        ));
    }

    /// Returns the physical size of the window on the screen,
    #[wasm_bindgen(getter, js_name = "physicalSize")]
    pub fn physical_size(&self) -> JsValue {
        let size = self.inner.window().size();
        size_to_js(size.width as f64, size.height as f64)
    }

    /// Sets the physical size of the window on the screen,
    #[wasm_bindgen(setter, js_name = "physicalSize")]
    pub fn set_physical_size(&self, size: JsValue) {
        let (Some(width), Some(height)) = (get_f64(&size, "width"), get_f64(&size, "height"))
        else {
            return;
        };
        self.inner
            .window()
            .set_size(PhysicalSize { width: width.floor() as u32, height: height.floor() as u32 });
    }

    /// Issues a request to the windowing system to re-render the contents of the window.
    #[wasm_bindgen(js_name = "requestRedraw")]
    pub fn request_redraw(&self) {
        self.inner.request_redraw();
    }

    /// Returns if the window is currently fullscreen
    #[wasm_bindgen(getter)]
    pub fn fullscreen(&self) -> bool {
        self.inner.window().is_fullscreen()
    }

    /// Set or unset the window to display fullscreen.
    #[wasm_bindgen(setter)]
    pub fn set_fullscreen(&self, enable: bool) {
        self.inner.window().set_fullscreen(enable);
    }

    /// Returns if the window is currently maximized
    #[wasm_bindgen(getter)]
    pub fn maximized(&self) -> bool {
        self.inner.window().is_maximized()
    }

    /// Maximize or unmaximize the window.
    #[wasm_bindgen(setter)]
    pub fn set_maximized(&self, maximized: bool) {
        self.inner.window().set_maximized(maximized);
    }

    /// Returns if the window is currently minimized
    #[wasm_bindgen(getter)]
    pub fn minimized(&self) -> bool {
        self.inner.window().is_minimized()
    }

    /// Minimize or unminimize the window.
    #[wasm_bindgen(setter)]
    pub fn set_minimized(&self, minimized: bool) {
        self.inner.window().set_minimized(minimized);
    }
}

thread_local! {
    static KEYS_REGISTRY: RefCell<HashMap<u32, i_slint_core::input::Keys>> = Default::default();
    static STYLED_TEXT_REGISTRY: RefCell<HashMap<u32, i_slint_core::styled_text::StyledText>> =
        Default::default();
    static NEXT_TYPE_ID: Cell<u32> = const { Cell::new(1) };
}

fn next_type_id() -> u32 {
    NEXT_TYPE_ID.with(|c| {
        let id = c.get();
        c.set(id + 1);
        id
    })
}

/// `Keys` represent a key combined with a list of modifiers (the `keys` type in the Slint language).
///
/// To construct a `Keys` instance from JavaScript, use the `Keys.fromParts()` method.
///
/// Use `toString()` to get a platform-native representation of the key binding
/// (e.g. "Ctrl+A" on Linux/Windows, "⌘A" on macOS).
#[wasm_bindgen(js_name = "Keys")]
pub struct WasmKeys {
    id: u32,
}

impl WasmKeys {
    pub(crate) fn new(keys: i_slint_core::input::Keys) -> Self {
        let id = next_type_id();
        KEYS_REGISTRY.with(|r| {
            r.borrow_mut().insert(id, keys);
        });
        Self { id }
    }

    fn inner(&self) -> Option<i_slint_core::input::Keys> {
        KEYS_REGISTRY.with(|r| r.borrow().get(&self.id).cloned())
    }
}

impl Drop for WasmKeys {
    fn drop(&mut self) {
        KEYS_REGISTRY.with(|r| {
            r.borrow_mut().remove(&self.id);
        });
    }
}

/// Convert a JS `Keys` instance back to the Rust value, detected through the
/// hidden id getter.
pub(crate) fn try_keys_from_js(js: &JsValue) -> Option<i_slint_core::input::Keys> {
    let id = js_sys::Reflect::get(js, &"__slintKeysId".into()).ok()?.as_f64()? as u32;
    KEYS_REGISTRY.with(|r| r.borrow().get(&id).cloned())
}

#[wasm_bindgen(js_class = "Keys")]
impl WasmKeys {
    /// Create a `Keys` from a list of string parts, e.g. `["Control", "Shift?", "Z"]`.
    ///
    /// Each element is either a modifier name or a key name. Throws an error on parse failure.
    #[wasm_bindgen(js_name = "fromParts")]
    pub fn from_parts(parts: Vec<String>) -> Result<WasmKeys, JsValue> {
        i_slint_core::input::Keys::from_parts(parts.iter().map(|s| s.as_str()))
            .map(Self::new)
            .map_err(|e| js_sys::Error::new(&e.to_string()).into())
    }

    /// Returns the platform-native string representation of this key binding.
    #[wasm_bindgen(js_name = "toString")]
    pub fn to_string(&self) -> String {
        self.inner().map(|k| k.to_string()).unwrap_or_default()
    }

    /// Returns `true` if this key binding is equal to `other`.
    #[wasm_bindgen]
    pub fn equals(&self, other: &WasmKeys) -> bool {
        self.inner() == other.inner()
    }

    /// @hidden Used to convert the instance back to a Slint value.
    #[wasm_bindgen(getter, js_name = "__slintKeysId")]
    pub fn slint_keys_id(&self) -> u32 {
        self.id
    }
}

/// Styled text parsed from markdown or plain text.
///
/// Use `StyledText.fromMarkdown()` or `StyledText.fromPlainText()` to create instances.
/// Assign the result to a `styled-text` property in a Slint component to display it.
#[wasm_bindgen(js_name = "StyledText")]
pub struct WasmStyledText {
    id: u32,
}

impl WasmStyledText {
    pub(crate) fn new(styled_text: i_slint_core::styled_text::StyledText) -> Self {
        let id = next_type_id();
        STYLED_TEXT_REGISTRY.with(|r| {
            r.borrow_mut().insert(id, styled_text);
        });
        Self { id }
    }

    fn inner(&self) -> Option<i_slint_core::styled_text::StyledText> {
        STYLED_TEXT_REGISTRY.with(|r| r.borrow().get(&self.id).cloned())
    }
}

impl Drop for WasmStyledText {
    fn drop(&mut self) {
        STYLED_TEXT_REGISTRY.with(|r| {
            r.borrow_mut().remove(&self.id);
        });
    }
}

/// Convert a JS `StyledText` instance back to the Rust value, detected through
/// the hidden id getter.
pub(crate) fn try_styled_text_from_js(
    js: &JsValue,
) -> Option<i_slint_core::styled_text::StyledText> {
    let id = js_sys::Reflect::get(js, &"__slintStyledTextId".into()).ok()?.as_f64()? as u32;
    STYLED_TEXT_REGISTRY.with(|r| r.borrow().get(&id).cloned())
}

#[wasm_bindgen(js_class = "StyledText")]
impl WasmStyledText {
    /// Creates styled text from plain text without applying markdown parsing.
    #[wasm_bindgen(js_name = "fromPlainText")]
    pub fn from_plain_text(text: String) -> WasmStyledText {
        Self::new(i_slint_core::styled_text::StyledText::from_plain_text(&text))
    }

    /// Parses markdown into styled text. Throws an error if the markdown
    /// contains unsupported syntax.
    #[wasm_bindgen(js_name = "fromMarkdown")]
    pub fn from_markdown(markdown: String) -> Result<WasmStyledText, JsValue> {
        i_slint_core::styled_text::StyledText::from_markdown(&markdown)
            .map(Self::new)
            .map_err(|e| js_sys::Error::new(&e.to_string()).into())
    }

    /// Returns `true` if this styled text is equal to `other`.
    #[wasm_bindgen]
    pub fn equals(&self, other: &WasmStyledText) -> bool {
        self.inner() == other.inner()
    }

    /// @hidden Used to convert the instance back to a Slint value.
    #[wasm_bindgen(getter, js_name = "__slintStyledTextId")]
    pub fn slint_styled_text_id(&self) -> u32 {
        self.id
    }
}
