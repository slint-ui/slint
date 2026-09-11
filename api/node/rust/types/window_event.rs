// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::platform::WindowEvent;
use slint_interpreter::{LogicalPosition, LogicalSize};

use crate::types::{JsPointerEventButton, SlintPoint, SlintSize};

// api/node/build.rs generates the TypeScript interfaces for these variants, documenting them
// from the matching `i_slint_core::platform::WindowEvent` variant. Add JSDoc here only where
// JavaScript needs to be told something different.

/// An event that describes user input or a windowing system change.
///
/// The `type` field selects the variant and determines which other fields apply.
/// Dispatch an event to a window with `Window.dispatchEvent`,
/// which reports whether the scene accepted or rejected it.
///
/// @example
/// ```js
/// import * as slint from "slint-ui";
///
/// const result = window.dispatchEvent({
///     type: "pointer-pressed",
///     position: { x: 51, y: 51 },
///     button: "left",
/// });
///
/// if (result === slint.platform.WindowEventDispatchResult.Accepted) {
///     console.log("the scene handled the press");
/// }
/// ```
#[repr(u32)]
#[napi(js_name = "WindowEvent", discriminant_case = "kebab-case")]
pub enum JsWindowEvent {
    PointerPressed {
        position: SlintPoint,

        button: JsPointerEventButton,
    },
    PointerReleased {
        position: SlintPoint,

        button: JsPointerEventButton,
    },
    PointerMoved {
        position: SlintPoint,
    },
    PointerScrolled {
        position: SlintPoint,

        #[napi(js_name = "deltaX")]
        delta_x: f64,

        #[napi(js_name = "deltaY")]
        delta_y: f64,
    },
    /// The pointer exited the window.
    ///
    /// Dispatching this event always returns `Accepted`.
    PointerExited,
    KeyPressed {
        text: String,
    },
    KeyPressRepeated {
        text: String,
    },
    KeyReleased {
        text: String,
    },
    /// The window's scale factor has changed.
    /// This can happen for example when the display's resolution changes,
    /// the user selects a new scale factor in the system settings,
    /// or the window is moved to a different screen.
    ScaleFactorChanged {
        #[napi(js_name = "scaleFactor")]
        scale_factor: f64,
    },
    /// The window was resized.
    ///
    /// Dispatching this event updates the `width` and `height` properties of the root window element.
    Resized {
        size: SlintSize,
    },
    /// The user requested to close the window.
    ///
    /// Dispatching this event invokes the `close-requested` callback of the window element,
    /// and hides the window unless that callback returns `reject`.
    CloseRequested,
    /// The window was activated or de-activated.
    WindowActiveChanged {
        /// True when the window gained focus, false when it lost focus.
        active: bool,
    },
}

impl From<JsWindowEvent> for WindowEvent {
    fn from(value: JsWindowEvent) -> Self {
        match value {
            JsWindowEvent::PointerPressed { position, button } => WindowEvent::PointerPressed {
                position: LogicalPosition { x: position.x as f32, y: position.y as f32 },
                button: button.into(),
            },
            JsWindowEvent::PointerReleased { position, button } => WindowEvent::PointerReleased {
                position: LogicalPosition { x: position.x as f32, y: position.y as f32 },
                button: button.into(),
            },
            JsWindowEvent::PointerMoved { position } => WindowEvent::PointerMoved {
                position: LogicalPosition { x: position.x as f32, y: position.y as f32 },
            },
            JsWindowEvent::PointerScrolled { position, delta_x, delta_y } => {
                WindowEvent::PointerScrolled {
                    position: LogicalPosition { x: position.x as f32, y: position.y as f32 },
                    delta_x: delta_x as f32,
                    delta_y: delta_y as f32,
                }
            }
            JsWindowEvent::PointerExited => WindowEvent::PointerExited,
            JsWindowEvent::KeyPressed { text } => WindowEvent::KeyPressed { text: text.into() },
            JsWindowEvent::KeyPressRepeated { text } => {
                WindowEvent::KeyPressRepeated { text: text.into() }
            }
            JsWindowEvent::KeyReleased { text } => WindowEvent::KeyReleased { text: text.into() },
            JsWindowEvent::ScaleFactorChanged { scale_factor } => {
                WindowEvent::ScaleFactorChanged { scale_factor: scale_factor as f32 }
            }
            JsWindowEvent::Resized { size } => WindowEvent::Resized {
                size: LogicalSize { width: size.width as f32, height: size.height as f32 },
            },
            JsWindowEvent::CloseRequested => WindowEvent::CloseRequested,
            JsWindowEvent::WindowActiveChanged { active } => {
                WindowEvent::WindowActiveChanged(active)
            }
        }
    }
}
