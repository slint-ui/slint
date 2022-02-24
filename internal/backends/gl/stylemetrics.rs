// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use super::*;

use const_field_offset::FieldOffsets;
use core::pin::Pin;
use i_slint_core::rtti::*;
use i_slint_core::Property;
use i_slint_core_macros::*;

#[repr(C)]
#[derive(FieldOffsets, SlintElement)]
#[pin]
#[pin_drop]
pub struct NativeStyleMetrics {
    pub layout_spacing: Property<f32>,
    pub layout_padding: Property<f32>,
    pub text_cursor_width: Property<f32>,
    pub window_background: Property<Color>,
    pub default_text_color: Property<Color>,
    pub textedit_background: Property<Color>,
    pub textedit_text_color: Property<Color>,
    pub textedit_background_disabled: Property<Color>,
    pub textedit_text_color_disabled: Property<Color>,

    pub dark_style: Property<bool>,

    pub placeholder_color: Property<Color>,
    pub placeholder_color_disabled: Property<Color>,
}

impl const_field_offset::PinnedDrop for NativeStyleMetrics {
    fn drop(self: Pin<&mut Self>) {
        native_style_metrics_deinit(self);
    }
}

impl NativeStyleMetrics {
    pub fn new() -> Pin<Rc<Self>> {
        let new = Rc::pin(NativeStyleMetrics {
            layout_spacing: Default::default(),
            layout_padding: Default::default(),
            text_cursor_width: Default::default(),
            window_background: Default::default(),
            default_text_color: Default::default(),
            textedit_background: Default::default(),
            textedit_text_color: Default::default(),
            textedit_background_disabled: Default::default(),
            textedit_text_color_disabled: Default::default(),
            dark_style: Default::default(),
            placeholder_color: Default::default(),
            placeholder_color_disabled: Default::default(),
        });
        new
    }

    pub fn init<T>(self: Pin<Rc<Self>>, _root: T) {
        self.as_ref().init_impl();
    }

    // TODO: Windows- and Linux-specific implementations

    // The palette on macOS is fixed, so the only property the macOS theme
    // actually uses is dark_style.
    // The actual colors are defined in the theme's .slint file.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn init_impl(self: Pin<&Self>) {
        use dark_light::Mode;

        self.dark_style.set(match dark_light::detect() {
            Mode::Light => false,
            Mode::Dark => true,
        });
    }

    // dark-light currently has no support for WASM
    #[cfg(target_arch = "wasm32")]
    pub fn init_impl(self: Pin<&Self>) {}
}

#[cfg(feature = "rtti")]
impl i_slint_core::rtti::BuiltinGlobal for NativeStyleMetrics {
    fn new() -> Pin<Rc<Self>> {
        NativeStyleMetrics::new()
    }
}

pub fn native_style_metrics_init(self_: Pin<&NativeStyleMetrics>) {
    self_.init_impl();
}

pub fn native_style_metrics_deinit(_self_: Pin<&mut NativeStyleMetrics>) {}
