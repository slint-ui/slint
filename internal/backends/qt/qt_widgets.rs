// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

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

use qttypes::QPainter;

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
    ($this:ident $dpr:ident $size:ident $painter:ident $widget:ident $initial_state:ident => $($tt:tt)*) => {
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
                painter.save();
                let $widget = cpp!(unsafe [painter as "QPainter*"] -> * const () as "QWidget*" {
                    return painter->device()->devType() == QInternal::Widget ? static_cast<QWidget *>(painter->device()) : nullptr;
                });
                let $painter = painter;
                $($tt)*
                $painter.restore();
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
                        let $widget: * const () = core::ptr::null();
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

    /// Make sure there is an instance of QApplication.
    /// The `from_qt_backend` argument specifies if we know that we are running
    /// the Qt backend, or if we are just drawing widgets
    void ensure_initialized(bool from_qt_backend = false)
    {
        if (qApp) {
            return;
        }
        if (!from_qt_backend) {
            // When not using the Qt backend, Qt is not in control of the event loop
            // so we should set this flag.
            QCoreApplication::setAttribute(Qt::AA_PluginApplication, true);
        }
        static int argc  = 1;
        static char argv[] = "sixtyfps";
        static char *argv2[] = { argv };
        // Leak the QApplication, otherwise it crashes on exit
        // (because the QGuiApplication destructor access some Q_GLOBAL_STATIC which are already gone)
        new QApplication(argc, argv2);
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

mod stylemetrics;
pub use stylemetrics::*;
