// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

use super::*;

cpp! {{
namespace {
struct StyleChangeListener : QWidget {
    const void *nativeStyleMetrics = nullptr;
    StyleChangeListener(const void *nativeStyleMetrics) : nativeStyleMetrics(nativeStyleMetrics) {}
    bool event(QEvent *event) override {
        auto ty = event->type();
        if (ty == QEvent::StyleChange || ty == QEvent::PaletteChange || ty == QEvent::FontChange) {
            rust!(SFPS_style_change_event [nativeStyleMetrics: Pin<&NativeStyleMetrics> as "const void*"] {
                nativeStyleMetrics.init();
            });
        }
        return QWidget::event(event);
    }
};
}
}}

#[repr(C)]
#[derive(FieldOffsets, SixtyFPSElement)]
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

    pub placeholder_color: Property<Color>,
    pub placeholder_color_disabled: Property<Color>,

    pub style_change_listener: core::cell::Cell<*const u8>,
}

impl const_field_offset::PinnedDrop for NativeStyleMetrics {
    fn drop(self: Pin<&mut Self>) {
        sixtyfps_native_style_metrics_deinit(self);
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
            placeholder_color: Default::default(),
            placeholder_color_disabled: Default::default(),
            style_change_listener: core::cell::Cell::new(core::ptr::null()),
        });
        new.as_ref().init();
        new
    }

    fn init(self: Pin<&Self>) {
        if self.style_change_listener.get().is_null() {
            self.style_change_listener.set(cpp!(unsafe [self as "void*"] -> *const u8 as "void*"{
                ensure_initialized();
                return new StyleChangeListener(self);
            }));
        }

        let layout_spacing = cpp!(unsafe [] -> f32 as "float" {
            ensure_initialized();
            int spacing = qApp->style()->pixelMetric(QStyle::PM_LayoutHorizontalSpacing);
            if (spacing < 0)
                spacing = qApp->style()->layoutSpacing(QSizePolicy::DefaultType, QSizePolicy::DefaultType, Qt::Horizontal);
            return spacing;
        });
        self.layout_spacing.set(layout_spacing.max(0.0));
        let layout_padding = cpp!(unsafe [] -> f32 as "float" {
            ensure_initialized();
            return qApp->style()->pixelMetric(QStyle::PM_LayoutLeftMargin);
        });
        self.layout_padding.set(layout_padding.max(0.0));
        let text_cursor_width = cpp!(unsafe [] -> f32 as "float" {
            return qApp->style()->pixelMetric(QStyle::PM_TextCursorWidth);
        });
        self.text_cursor_width.set(text_cursor_width.max(0.0));
        let window_background = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::Window).rgba();
        });
        self.window_background.set(Color::from_argb_encoded(window_background));
        let default_text_color = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::WindowText).rgba();
        });
        self.default_text_color.set(Color::from_argb_encoded(default_text_color));
        let textedit_text_color = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::Text).rgba();
        });
        self.textedit_text_color.set(Color::from_argb_encoded(textedit_text_color));
        let textedit_text_color_disabled = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::Disabled, QPalette::Text).rgba();
        });
        self.textedit_text_color_disabled
            .set(Color::from_argb_encoded(textedit_text_color_disabled));
        let textedit_background = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::Base).rgba();
        });
        self.textedit_background.set(Color::from_argb_encoded(textedit_background));
        let textedit_background_disabled = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::Disabled, QPalette::Base).rgba();
        });
        self.textedit_background_disabled
            .set(Color::from_argb_encoded(textedit_background_disabled));
        let placeholder_color = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::PlaceholderText).rgba();
        });
        self.placeholder_color.set(Color::from_argb_encoded(placeholder_color));
        let placeholder_color_disabled = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::Disabled, QPalette::PlaceholderText).rgba();
        });
        self.placeholder_color_disabled.set(Color::from_argb_encoded(placeholder_color_disabled));
    }
}

#[cfg(feature = "rtti")]
impl sixtyfps_corelib::rtti::BuiltinGlobal for NativeStyleMetrics {
    fn new() -> Pin<Rc<Self>> {
        NativeStyleMetrics::new()
    }
}

#[no_mangle]
pub extern "C" fn sixtyfps_native_style_metrics_init(self_: Pin<&NativeStyleMetrics>) {
    self_.style_change_listener.set(core::ptr::null()); // because the C++ code don't initialize it
    self_.init();
}

#[no_mangle]
pub extern "C" fn sixtyfps_native_style_metrics_deinit(self_: Pin<&mut NativeStyleMetrics>) {
    let scl = self_.style_change_listener.get();
    cpp!(unsafe [scl as "StyleChangeListener*"] { delete scl; });
    self_.style_change_listener.set(core::ptr::null());
}
