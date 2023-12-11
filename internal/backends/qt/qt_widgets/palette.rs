// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// cSpell: ignore deinit

use i_slint_core::Brush;

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
    pub background_alt: Property<Brush>,
    pub on_background: Property<Brush>,
    pub accent: Property<Brush>,
    pub on_accent: Property<Brush>,
    pub surface: Property<Brush>,
    pub on_surface: Property<Brush>,
    pub border: Property<Brush>,
    pub selection: Property<Brush>,
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
            background_alt: Default::default(),
            on_background: Default::default(),
            accent: Default::default(),
            on_accent: Default::default(),
            surface: Default::default(),
            on_surface: Default::default(),
            border: Default::default(),
            selection: Default::default(),
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

        let background_alt = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::Base).rgba();
        });
        let background_alt = Color::from_argb_encoded(background_alt);
        self.background_alt.set(Brush::from(background_alt));

        let on_background = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::WindowText).rgba();
        });
        let on_background = Color::from_argb_encoded(on_background);
        self.on_background.set(Brush::from(on_background));

        let accent = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::Link).rgba();
        });
        let accent = Color::from_argb_encoded(accent);
        self.accent.set(Brush::from(accent));

        let on_accent = cpp!(unsafe[] -> u32 as "QRgb" {
            #if QT_VERSION >= QT_VERSION_CHECK(6, 6, 0)
                return qApp->palette().color(QPalette::Accent).rgba();
            #else
                return qApp->palette().color(QPalette::Highlight).rgba();
            #endif
        });
        let on_accent = Color::from_argb_encoded(on_accent);
        self.on_accent.set(Brush::from(on_accent));

        let surface = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::Button).rgba();
        });
        let surface = Color::from_argb_encoded(surface);
        self.surface.set(Brush::from(surface));

        let on_surface = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::ButtonText).rgba();
        });
        let on_surface = Color::from_argb_encoded(on_surface);
        self.on_surface.set(Brush::from(on_surface));

        let border = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::Midlight).rgba();
        });
        let border = Color::from_argb_encoded(border);
        self.border.set(Brush::from(border));

        let selection = cpp!(unsafe[] -> u32 as "QRgb" {
            return qApp->palette().color(QPalette::HighlightedText).rgba();
        });
        let selection = Color::from_argb_encoded(selection);
        self.selection.set(Brush::from(selection));

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
