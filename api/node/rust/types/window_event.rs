// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::platform::WindowEvent;
use slint_interpreter::{LogicalPosition, LogicalSize};

use crate::types::{JsPointerEventButton, SlintPoint, SlintSize};

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
    ScaleFactorChanged {
        #[napi(js_name = "scaleFactor")]
        scale_factor: f64,
    },
    Resized {
        size: SlintSize,
    },
    CloseRequested,
    WindowActiveChanged {
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
