// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore deinit

use i_slint_core::{items::ColorScheme, Brush};

use super::*;

cpp! {{
namespace {
struct PaletteStyleChangeListener : QWidget {
    const void *qtStylePalette = nullptr;
    PaletteStyleChangeListener(const void *qtStylePalette) : qtStylePalette(qtStylePalette) {}
    bool event(QEvent *event) override {
        auto ty = event->type();
        if (ty == QEvent::StyleChange || ty == QEvent::PaletteChange || ty == QEvent::FontChange) {
            rust!(Slint_qt_style_change_event [qtStylePalette: Pin<&NativePalette> as "const void*"] {
                qtStylePalette.init_impl();
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
pub struct NativePalette {
    pub background: Property<Brush>,
    pub foreground: Property<Brush>,
    pub alternate_background: Property<Brush>,
    pub alternate_foreground: Property<Brush>,
    pub accent_background: Property<Brush>,
    pub accent_foreground: Property<Brush>,
    pub control_background: Property<Brush>,
    pub control_foreground: Property<Brush>,
    pub selection_background: Property<Brush>,
    pub selection_foreground: Property<Brush>,
    pub border: Property<Brush>,
    pub color_scheme: Property<ColorScheme>,
    pub style_change_listener: core::cell::Cell<*const u8>,
}

impl const_field_offset::PinnedDrop for NativePalette {
    fn drop(self: Pin<&mut Self>) {
        slint_native_palette_deinit(self);
    }
}

impl NativePalette {
    pub fn new() -> Pin<Rc<Self>> {
        Rc::pin(NativePalette {
            background: Default::default(),
            alternate_background: Default::default(),
            alternate_foreground: Default::default(),
            foreground: Default::default(),
            accent_background: Default::default(),
            accent_foreground: Default::default(),
            control_background: Default::default(),
            control_foreground: Default::default(),
            border: Default::default(),
            selection_background: Default::default(),
            selection_foreground: Default::default(),
            color_scheme: Default::default(),
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

        let background = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::Window).rgba();
        });
        let background = Color::from_argb_encoded(background);
        self.background.set(Brush::from(background));

        let alternate_background = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::Base).rgba();
        });
        let alternate_background = Color::from_argb_encoded(alternate_background);
        self.alternate_background.set(Brush::from(alternate_background));

        let alternate_foreground = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::Text).rgba();
        });
        let alternate_foreground = Color::from_argb_encoded(alternate_foreground);
        self.alternate_foreground.set(Brush::from(alternate_foreground));

        let foreground = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::WindowText).rgba();
        });
        let foreground = Color::from_argb_encoded(foreground);
        self.foreground.set(Brush::from(foreground));

        let accent_background = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::Link).rgba();
        });
        let accent_background = Color::from_argb_encoded(accent_background);
        self.accent_background.set(Brush::from(accent_background));

        let accent_foreground = cpp!(unsafe[] -> u32 as "QRgb" {
            #if QT_VERSION >= QT_VERSION_CHECK(6, 6, 0)
                return qApp->palette().color(QPalette::Accent).rgba();
            #else
                return qApp->palette().color(QPalette::Highlight).rgba();
            #endif
        });
        let accent_foreground = Color::from_argb_encoded(accent_foreground);
        self.accent_foreground.set(Brush::from(accent_foreground));

        let control_background = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::Button).rgba();
        });
        let control_background = Color::from_argb_encoded(control_background);
        self.control_background.set(Brush::from(control_background));

        let control_foreground = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::ButtonText).rgba();
        });
        let control_foreground = Color::from_argb_encoded(control_foreground);
        self.control_foreground.set(Brush::from(control_foreground));

        let border = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::Midlight).rgba();
        });
        let border = Color::from_argb_encoded(border);
        self.border.set(Brush::from(border));

        let selection_background = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::Highlight).rgba();
        });
        let selection_background = Color::from_argb_encoded(selection_background);
        self.selection_background.set(Brush::from(selection_background));

        let selection_foreground = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::HighlightedText).rgba();
        });
        let selection_foreground = Color::from_argb_encoded(selection_foreground);
        self.selection_foreground.set(Brush::from(selection_foreground));

        self.color_scheme.set(
            if (background.red() as u32 + background.green() as u32 + background.blue() as u32) / 3
                < 128
            {
                ColorScheme::Dark
            } else {
                ColorScheme::Light
            },
        );

        if self.style_change_listener.get().is_null() {
            self.style_change_listener.set(cpp!(unsafe [self as "void*"] -> *const u8 as "void*"{
                return new PaletteStyleChangeListener(self);
            }));
        }
    }
}

#[cfg(feature = "rtti")]
impl i_slint_core::rtti::BuiltinGlobal for NativePalette {
    fn new() -> Pin<Rc<Self>> {
        let r = NativePalette::new();
        r.as_ref().init_impl();
        r
    }
}

#[no_mangle]
pub extern "C" fn slint_native_palette_init(self_: Pin<&NativePalette>) {
    self_.style_change_listener.set(core::ptr::null()); // because the C++ code don't initialize it
    self_.init_impl();
}

#[no_mangle]
pub extern "C" fn slint_native_palette_deinit(self_: Pin<&mut NativePalette>) {
    let scl = self_.style_change_listener.get();
    cpp!(unsafe [scl as "PaletteStyleChangeListener*"] { delete scl; });
    self_.style_change_listener.set(core::ptr::null());
}
