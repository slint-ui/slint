// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*!

This module contains all the native Qt widget implementation that forwards to QStyle.

Same as in i_slint_core::items, when When adding an item or a property,
it needs to be kept in sync with different place.

 - It needs to be changed in this module
 - the Widget list in lib.rs
 - In the compiler: builtins.slint
 - For the C++ code (new item only): the build.rs to export the new item, and the `using` declaration in slint.h
 - Don't forget to update the documentation
*/

#![allow(non_upper_case_globals)]

use crate::qt_window::QPainterPtr;
use const_field_offset::FieldOffsets;
use core::pin::Pin;
use cpp::cpp;
use i_slint_core::graphics::Color;
use i_slint_core::input::{
    FocusEvent, InputEventFilterResult, InputEventResult, KeyEvent, KeyEventResult, MouseEvent,
};
use i_slint_core::item_rendering::{CachedRenderingData, ItemRenderer};
use i_slint_core::items::{Item, ItemConsts, ItemRc, ItemVTable, RenderingResult, VoidArg};
use i_slint_core::layout::{LayoutInfo, Orientation};
use i_slint_core::lengths::{LogicalLength, LogicalPoint, LogicalRect, LogicalSize};
#[cfg(feature = "rtti")]
use i_slint_core::rtti::*;
use i_slint_core::window::{WindowAdapter, WindowAdapterRc, WindowInner};
use i_slint_core::{
    declare_item_vtable, Callback, ItemVTable_static, Property, SharedString, SharedVector,
};
use i_slint_core_macros::*;
use std::rc::Rc;

type ItemRendererRef<'a> = &'a mut dyn ItemRenderer;

/// Helper macro to get the size from the width and height property,
/// and return Default::default in case the size is too small
macro_rules! get_size {
    ($self:ident) => {{
        let width = $self.width().get();
        let height = $self.height().get();
        if width < 1. || height < 1. {
            return Default::default();
        };
        qttypes::QSize { width: width as _, height: height as _ }
    }};
}

macro_rules! fn_render {
    ($this:ident $dpr:ident $size:ident $painter:ident $widget:ident $initial_state:ident => $($tt:tt)*) => {
        fn render(self: Pin<&Self>, backend: &mut &mut dyn ItemRenderer, item_rc: &ItemRc) -> RenderingResult {
            let $dpr: f32 = backend.scale_factor();

            let window = backend.window();
            let active: bool = WindowInner::from_pub(window).active();
            // This should include self.enabled() as well, but not every native widget
            // has that property right now.
            let $initial_state = cpp!(unsafe [ active as "bool" ] -> i32 as "int" {
                QStyle::State state(QStyle::State_None);
                if (active)
                    state |= QStyle::State_Active;
                return (int)state;
            });

            if let Some(painter) = backend.as_any().and_then(|any| <dyn std::any::Any>::downcast_mut::<QPainterPtr>(any)) {
                let $size: qttypes::QSize = get_size!(self);
                let $this = self;
                painter.save();
                let $widget = cpp!(unsafe [painter as "QPainterPtr*"] -> * const () as "QWidget*" {
                    return (*painter)->device()->devType() == QInternal::Widget ? static_cast<QWidget *>((*painter)->device()) : nullptr;
                });
                let $painter = painter;
                $($tt)*
                $painter.restore();
            } else {
                // Fallback: this happen when the Qt backend is not used and the gl backend is used instead
                backend.draw_cached_pixmap(
                    item_rc,
                    &|callback| {
                        let width = self.width().get() * $dpr;
                        let height = self.height().get() * $dpr;
                        if width < 1. || height < 1. {
                            return Default::default();
                        };
                        let $size = qttypes::QSize { width: width as _, height: height as _ };
                        let mut imgarray = QImageWrapArray::new($size, $dpr);
                        let img = &mut imgarray.img;
                        let mut painter = cpp!(unsafe [img as "QImage*"] -> QPainterPtr as "std::unique_ptr<QPainter>" {
                            return std::make_unique<QPainter>(img);
                        });
                        let $widget: * const () = core::ptr::null();
                        let $painter = &mut painter;
                        let $this = self;
                        $($tt)*
                        drop(painter);
                        imgarray.draw(callback);
                    },
                );
            }
            RenderingResult::ContinueRenderingChildren
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

    using QPainterPtr = std::unique_ptr<QPainter>;

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
        static char argv[] = "Slint";
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

mod tableview;
pub use tableview::*;

mod tabwidget;
pub use tabwidget::*;

mod stylemetrics;
pub use stylemetrics::*;

mod tablecolumn;
pub use tablecolumn::*;
