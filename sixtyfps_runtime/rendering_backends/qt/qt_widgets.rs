/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

/*!

This module contains all the native Qt widget implementation that forwards to QStyle.

Same as in sixtyfps_corelib::items, when When adding an item or a property,
it needs to be kept in sync with different place.

 - It needs to be changed in this module
 - the Widget list in lib.rs
 - In the compiler: builtins.60
 - For the C++ code (new item only): the build.rs to export the new item, and the `using` declaration in sixtyfps.h
 - Don't forget to update the documentation
*/

#![allow(non_upper_case_globals)]

use const_field_offset::FieldOffsets;
use core::pin::Pin;
use cpp::cpp;
use sixtyfps_corelib::graphics::{Color, Rect};
use sixtyfps_corelib::input::{
    FocusEvent, InputEventFilterResult, InputEventResult, KeyEvent, KeyEventResult, MouseEvent,
};
use sixtyfps_corelib::item_rendering::{CachedRenderingData, ItemRenderer};
use sixtyfps_corelib::items::{Item, ItemConsts, ItemRc, ItemVTable, VoidArg};
use sixtyfps_corelib::layout::{LayoutInfo, Orientation};
use sixtyfps_corelib::rtti::*;
use sixtyfps_corelib::window::WindowRc;
use sixtyfps_corelib::{
    declare_item_vtable, Callback, ItemVTable_static, Property, SharedString, SharedVector,
};
use sixtyfps_corelib_macros::*;
use std::rc::Rc;

type ItemRendererRef<'a> = &'a mut dyn ItemRenderer;

use crate::qt_window::QPainter;

/// Helper macro to get the size from the width and height property,
/// and return Default::default in case the size is too small
macro_rules! get_size {
    ($self:ident) => {{
        let width = $self.width();
        let height = $self.height();
        if width < 1. || height < 1. {
            return Default::default();
        };
        qttypes::QSize { width: width as _, height: height as _ }
    }};
}

macro_rules! fn_render {
    ($this:ident $dpr:ident $size:ident $painter:ident $initial_state:ident => $($tt:tt)*) => {
        fn render(self: Pin<&Self>, backend: &mut &mut dyn ItemRenderer) {
            let $dpr: f32 = backend.scale_factor();

            let window = backend.window();
            let active: bool = window.active();
            // This should include self.enabled() as well, but not every native widget
            // has that property right now.
            let $initial_state = cpp!(unsafe [ active as "bool" ] -> i32 as "int" {
                QStyle::State state(QStyle::State_None);
                if (active)
                    state |= QStyle::State_Active;
                return (int)state;
            });

            if let Some(painter) = <dyn std::any::Any>::downcast_mut::<QPainter>(backend.as_any()) {
                let $size: qttypes::QSize = get_size!(self);
                let $this = self;
                painter.save_state();
                let $painter = painter;
                $($tt)*
                $painter.restore_state();
            } else {
                // Fallback: this happen when the Qt backend is not used and the gl backend is used instead
                backend.draw_cached_pixmap(
                    &self.cached_rendering_data,
                    &|callback| {
                        let width = self.width() * $dpr;
                        let height = self.height() * $dpr;
                        if width < 1. || height < 1. {
                            return Default::default();
                        };
                        let $size = qttypes::QSize { width: width as _, height: height as _ };
                        let mut imgarray = QImageWrapArray::new($size, $dpr);
                        let img = &mut imgarray.img;
                        let mut painter_ = cpp!(unsafe [img as "QImage*"] -> QPainter as "QPainter" { return QPainter(img); });
                        let $painter = &mut painter_;
                        let $this = self;
                        $($tt)*
                        drop(painter_);
                        imgarray.draw(callback);
                    },
                );
            }
        }
    };
}

struct QImageWrapArray {
    /// The image reference the array, so the array must outlive the image without being detached or accessed
    img: qttypes::QImage,
    array: SharedVector<u8>,
}

impl QImageWrapArray {
    pub fn new(size: qttypes::QSize, dpr: f32) -> Self {
        let mut array = SharedVector::default();
        array.resize((size.width * size.height * 4) as usize, 0);
        let array_ptr = array.make_mut_slice().as_mut_ptr();
        let img = cpp!(unsafe [size as "QSize", array_ptr as "uchar*", dpr as "float"] -> qttypes::QImage as "QImage" {
            ensure_initialized();
            QImage img(array_ptr, size.width(), size.height(), size.width() * 4, QImage::Format_RGBA8888_Premultiplied);
            img.setDevicePixelRatio(dpr);
            return img;
        });
        QImageWrapArray { img, array }
    }

    pub fn draw(&self, callback: &mut dyn FnMut(u32, u32, &[u8])) {
        let size = self.img.size();
        callback(size.width, size.height, self.array.as_slice());
    }
}

cpp! {{
    #include <QtWidgets/QApplication>
    #include <QtWidgets/QStyle>
    #include <QtWidgets/QStyleOption>
    #include <QtWidgets/QStyleFactory>
    #include <QtGui/QPainter>
    #include <QtGui/QClipboard>
    #include <QtCore/QMimeData>
    #include <QtCore/QDebug>
    #include <QtCore/QScopeGuard>

    void ensure_initialized()
    {
        static auto app [[maybe_unused]]  = []{
            if (qApp) {
                return qApp;
            }
            QCoreApplication::setAttribute(Qt::AA_PluginApplication, true);
            static int argc  = 1;
            static char argv[] = "sixtyfps";
            static char *argv2[] = { argv };
            // Leak the QApplication, otherwise it crashes on exit
            // (because the QGuiApplication destructor access some Q_GLOBAL_STATIC which are already gone)
            return new QApplication(argc, argv2);
        }();
    }
}}

mod button;
pub use button::*;

mod checkbox;
pub use checkbox::*;

mod spinbox;
pub use spinbox::*;

mod slider;
pub use slider::*;

mod groupbox;
pub use groupbox::*;

mod lineedit;
pub use lineedit::*;

mod scrollview;
pub use scrollview::*;

mod listviewitem;
pub use listviewitem::*;

mod combobox;
pub use combobox::*;

mod tabwidget;
pub use tabwidget::*;

#[repr(C)]
#[derive(FieldOffsets, SixtyFPSElement)]
#[pin]
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
}

impl Default for NativeStyleMetrics {
    fn default() -> Self {
        let s = NativeStyleMetrics {
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
        };
        sixtyfps_init_native_style_metrics(&s);
        s
    }
}

impl NativeStyleMetrics {
    pub fn new() -> Pin<Rc<Self>> {
        Rc::pin(Self::default())
    }
}

/// Initialize the native style metrics
#[no_mangle]
pub extern "C" fn sixtyfps_init_native_style_metrics(self_: &NativeStyleMetrics) {
    let layout_spacing = cpp!(unsafe [] -> f32 as "float" {
        ensure_initialized();
        return qApp->style()->pixelMetric(QStyle::PM_LayoutHorizontalSpacing);
    });
    self_.layout_spacing.set(layout_spacing.max(0.0));
    let text_cursor_width = cpp!(unsafe [] -> f32 as "float" {
        return qApp->style()->pixelMetric(QStyle::PM_TextCursorWidth);
    });
    self_.text_cursor_width.set(text_cursor_width.max(0.0));
    let window_background = cpp!(unsafe[] -> u32 as "QRgb" {
        return qApp->palette().color(QPalette::Window).rgba();
    });
    self_.window_background.set(Color::from_argb_encoded(window_background));
    let default_text_color = cpp!(unsafe[] -> u32 as "QRgb" {
        return qApp->palette().color(QPalette::WindowText).rgba();
    });
    self_.default_text_color.set(Color::from_argb_encoded(default_text_color));
    let textedit_text_color = cpp!(unsafe[] -> u32 as "QRgb" {
        return qApp->palette().color(QPalette::Text).rgba();
    });
    self_.textedit_text_color.set(Color::from_argb_encoded(textedit_text_color));
    let textedit_text_color_disabled = cpp!(unsafe[] -> u32 as "QRgb" {
        return qApp->palette().color(QPalette::Disabled, QPalette::Text).rgba();
    });
    self_.textedit_text_color_disabled.set(Color::from_argb_encoded(textedit_text_color_disabled));
    let textedit_background = cpp!(unsafe[] -> u32 as "QRgb" {
        return qApp->palette().color(QPalette::Base).rgba();
    });
    self_.textedit_background.set(Color::from_argb_encoded(textedit_background));
    let textedit_background_disabled = cpp!(unsafe[] -> u32 as "QRgb" {
        return qApp->palette().color(QPalette::Disabled, QPalette::Base).rgba();
    });
    self_.textedit_background_disabled.set(Color::from_argb_encoded(textedit_background_disabled));
    let placeholder_color = cpp!(unsafe[] -> u32 as "QRgb" {
        return qApp->palette().color(QPalette::PlaceholderText).rgba();
    });
    self_.placeholder_color.set(Color::from_argb_encoded(placeholder_color));
    let placeholder_color_disabled = cpp!(unsafe[] -> u32 as "QRgb" {
        return qApp->palette().color(QPalette::Disabled, QPalette::PlaceholderText).rgba();
    });
    self_.placeholder_color_disabled.set(Color::from_argb_encoded(placeholder_color_disabled));
}
