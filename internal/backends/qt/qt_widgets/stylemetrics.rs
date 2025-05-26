// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore deinit

use i_slint_core::items::LayoutAlignment;

use super::*;

cpp! {{
namespace {
struct StyleChangeListener : QWidget {
    const void *nativeStyleMetrics = nullptr;
    StyleChangeListener(const void *nativeStyleMetrics) : nativeStyleMetrics(nativeStyleMetrics) {}
    bool event(QEvent *event) override {
        auto ty = event->type();
        if (ty == QEvent::StyleChange || ty == QEvent::PaletteChange || ty == QEvent::FontChange) {
            rust!(Slint_style_change_event [nativeStyleMetrics: Pin<&NativeStyleMetrics> as "const void*"] {
                nativeStyleMetrics.init_impl();
            });
        }
        return QWidget::event(event);
    }
};
}
}}

#[repr(C)]
#[derive(FieldOffsets, SlintElement)]
#[pin]
#[pin_drop]
pub struct NativeStyleMetrics {
    pub layout_spacing: Property<LogicalLength>,
    pub layout_padding: Property<LogicalLength>,
    pub text_cursor_width: Property<LogicalLength>,
    pub window_background: Property<Color>,
    pub default_text_color: Property<Color>,
    pub default_font_size: Property<LogicalLength>,
    pub textedit_background: Property<Color>,
    pub textedit_text_color: Property<Color>,
    pub textedit_background_disabled: Property<Color>,
    pub textedit_text_color_disabled: Property<Color>,

    pub placeholder_color: Property<Color>,
    pub placeholder_color_disabled: Property<Color>,

    pub dark_color_scheme: Property<bool>,

    // Tab Bar metrics:
    pub tab_bar_alignment: Property<LayoutAlignment>,

    pub style_name: Property<SharedString>,

    pub style_change_listener: core::cell::Cell<*const u8>,
}

impl const_field_offset::PinnedDrop for NativeStyleMetrics {
    fn drop(self: Pin<&mut Self>) {
        slint_native_style_metrics_deinit(self);
    }
}

impl NativeStyleMetrics {
    pub fn new() -> Pin<Rc<Self>> {
        Rc::pin(NativeStyleMetrics {
            layout_spacing: Default::default(),
            layout_padding: Default::default(),
            text_cursor_width: Default::default(),
            window_background: Default::default(),
            default_text_color: Default::default(),
            default_font_size: Default::default(),
            textedit_background: Default::default(),
            textedit_text_color: Default::default(),
            textedit_background_disabled: Default::default(),
            textedit_text_color_disabled: Default::default(),
            placeholder_color: Default::default(),
            placeholder_color_disabled: Default::default(),
            dark_color_scheme: Default::default(),
            tab_bar_alignment: Default::default(),
            style_name: Default::default(),
            style_change_listener: core::cell::Cell::new(core::ptr::null()),
        })
    }

    pub fn init<T>(self: Pin<Rc<Self>>, _root: &T) {
        self.as_ref().init_impl();
    }

    fn init_impl(self: Pin<&Self>) {
        let wrong_thread = cpp!(unsafe [] -> bool as "bool" {
            static QMutex mtx;
            QMutexLocker locker(&mtx);
            ensure_initialized();
            return qApp->thread() != QThread::currentThread();
        });
        if wrong_thread {
            return;
        }

        if self.style_change_listener.get().is_null() {
            self.style_change_listener.set(cpp!(unsafe [self as "void*"] -> *const u8 as "void*"{
                return new StyleChangeListener(self);
            }));
        }

        let layout_spacing = cpp!(unsafe [] -> f32 as "float" {
            int spacing = qApp->style()->pixelMetric(QStyle::PM_LayoutHorizontalSpacing);
            if (spacing < 0)
                spacing = qApp->style()->layoutSpacing(QSizePolicy::DefaultType, QSizePolicy::DefaultType, Qt::Horizontal);
            return spacing;
        });
        self.layout_spacing.set(LogicalLength::new(layout_spacing.max(0.0)));
        let layout_padding = cpp!(unsafe [] -> f32 as "float" {
            return qApp->style()->pixelMetric(QStyle::PM_LayoutLeftMargin);
        });
        self.layout_padding.set(LogicalLength::new(layout_padding.max(0.0)));
        let text_cursor_width = cpp!(unsafe [] -> f32 as "float" {
            return qApp->style()->pixelMetric(QStyle::PM_TextCursorWidth);
        });
        self.text_cursor_width.set(LogicalLength::new(text_cursor_width.max(0.0)));
        let window_background = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::Window).rgba();
        });
        let window_background = Color::from_argb_encoded(window_background);
        self.window_background.set(window_background);
        let default_text_color = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::WindowText).rgba();
        });
        self.default_text_color.set(Color::from_argb_encoded(default_text_color));
        let default_font_size = cpp!(unsafe[] -> i32 as "int" {
            return QFontInfo(qApp->font()).pixelSize();
        });
        self.default_font_size.set(LogicalLength::new(default_font_size as f32));
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

        // This is sub-optimal: It should really be a binding to Palette.color-scheme == ColorScheme.dark, so that
        // writes to Palette.color-scheme are reflected, but we can't access the other global singleton here and
        // this is just a backwards-compat property that was never documented to be public.
        self.dark_color_scheme.set(
            (window_background.red() as u32
                + window_background.green() as u32
                + window_background.blue() as u32)
                / 3
                < 128,
        );

        let tab_bar_alignment = cpp!(unsafe[] -> u32 as "uint32_t" {
            switch (qApp->style()->styleHint(QStyle::SH_TabBar_Alignment)) {
                case Qt::AlignLeft: return 1;
                case Qt::AlignCenter: return 2;
                case Qt::AlignRight: return 3;
                default: return 0;
            }
        });
        self.tab_bar_alignment.set(match tab_bar_alignment {
            1 => LayoutAlignment::Start,
            2 => LayoutAlignment::Center,
            3 => LayoutAlignment::End,
            _ => LayoutAlignment::SpaceBetween, // Should not happen! If it does, it should be noticeable;-)
        });
    }
}

#[cfg(feature = "rtti")]
impl i_slint_core::rtti::BuiltinGlobal for NativeStyleMetrics {
    fn new() -> Pin<Rc<Self>> {
        let r = NativeStyleMetrics::new();
        r.as_ref().init_impl();
        r
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_native_style_metrics_init(self_: Pin<&NativeStyleMetrics>) {
    self_.style_change_listener.set(core::ptr::null()); // because the C++ code don't initialize it
    self_.init_impl();
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_native_style_metrics_deinit(self_: Pin<&mut NativeStyleMetrics>) {
    let scl = self_.style_change_listener.get();
    cpp!(unsafe [scl as "StyleChangeListener*"] { delete scl; });
    self_.style_change_listener.set(core::ptr::null());
}
