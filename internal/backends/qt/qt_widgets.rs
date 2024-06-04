// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

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
use cpp::{cpp, cpp_class};
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
use std::ptr::NonNull;
use std::rc::Rc;

type ItemRendererRef<'a> = &'a mut dyn ItemRenderer;

/// Helper macro to get the size from the width and height property,
/// and return Default::default in case the size is too small
macro_rules! get_size {
    ($self:ident) => {{
        let geo = $self.geometry();
        let width = geo.width();
        let height = geo.height();
        if width < 1. || height < 1. {
            return Default::default();
        };
        qttypes::QSize { width: width as _, height: height as _ }
    }};
}

macro_rules! fn_render {
    ($this:ident $dpr:ident $size:ident $painter:ident $widget:ident $initial_state:ident => $($tt:tt)*) => {
        fn render(self: Pin<&Self>, backend: &mut &mut dyn ItemRenderer, item_rc: &ItemRc, size: LogicalSize) -> RenderingResult {
            self.animation_tracker();
            let $dpr: f32 = backend.scale_factor();

            let active: bool = backend.window().active();
            // This should include self.enabled() as well, but not every native widget
            // has that property right now.
            let $initial_state = cpp!(unsafe [ active as "bool" ] -> i32 as "int" {
                QStyle::State state(QStyle::State_None);
                if (active)
                    state |= QStyle::State_Active;
                return (int)state;
            });

            let $widget: NonNull<()> = SlintTypeErasedWidgetPtr::qwidget_ptr(&self.widget_ptr);

            if let Some(painter) = backend.as_any().and_then(|any| <dyn std::any::Any>::downcast_mut::<QPainterPtr>(any)) {
                let width = size.width * $dpr;
                let height = size.height * $dpr;
                if width < 1. || height < 1. {
                    return Default::default();
                };
                let $size = qttypes::QSize { width: width as _, height: height as _ };
                let $this = self;
                let _workaround = unsafe { $crate::qt_widgets::PainterClipWorkaround::new(painter) };
                painter.save();
                let $painter = painter;
                $($tt)*
                $painter.restore();
            } else {
                // Fallback: this happen when the Qt backend is not used and the gl backend is used instead
                backend.draw_cached_pixmap(
                    item_rc,
                    &|callback| {
                        let geo = item_rc.geometry();
                        let width = geo.width() * $dpr;
                        let height = geo.height() * $dpr;
                        if width < 1. || height < 1. {
                            return Default::default();
                        };
                        let $size = qttypes::QSize { width: width as _, height: height as _ };
                        let mut imgarray = QImageWrapArray::new($size, $dpr);
                        let img = &mut imgarray.img;
                        let mut painter = cpp!(unsafe [img as "QImage*"] -> QPainterPtr as "std::unique_ptr<QPainter>" {
                            auto painter = std::make_unique<QPainter>(img);
                            painter->setRenderHints(QPainter::Antialiasing | QPainter::SmoothPixmapTransform);
                            return painter;
                        });
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

    static bool g_lastWindowClosed = false; // Wohoo, global to track window closure when using processEvents().

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

        static QByteArray executable = rust!(Slint_get_executable_name [] -> qttypes::QByteArray as "QByteArray" {
            std::env::args().next().unwrap_or_default().as_bytes().into()
        });

        static int argc  = 1;
        static char *argv[] = { executable.data() };
        // Leak the QApplication, otherwise it crashes on exit
        // (because the QGuiApplication destructor access some Q_GLOBAL_STATIC which are already gone)
        new QApplication(argc, argv);
        qApp->setQuitOnLastWindowClosed(false);
    }

    // HACK ALERT: This struct declaration is duplicated in api/cpp/bindgen.rs - keep in sync.
    struct SlintTypeErasedWidget
    {
        virtual ~SlintTypeErasedWidget() = 0;
        SlintTypeErasedWidget() = default;
        SlintTypeErasedWidget(const SlintTypeErasedWidget&) = delete;
        SlintTypeErasedWidget& operator=(const SlintTypeErasedWidget&) = delete;

        virtual void *qwidget() = 0;
    };

    SlintTypeErasedWidget::~SlintTypeErasedWidget() = default;

    template <typename Base>
    struct SlintAnimatedWidget: public Base, public SlintTypeErasedWidget {
        void *animation_update_property_ptr;
        bool event(QEvent *event) override {
            // QEvent::StyleAnimationUpdate is sent by QStyleAnimation used by Qt builtin styles
            // And we hacked some attribute so that QWidget::update() will emit UpdateLater
            if (event->type() == QEvent::StyleAnimationUpdate  || event->type() == QEvent::UpdateLater) {
                rust!(Slint_AnimatedWidget_update [animation_update_property_ptr: Pin<&Property<i32>> as "void*"] {
                    animation_update_property_ptr.set(animation_update_property_ptr.get() + 1);
                });
                event->accept();
                return true;
            } else {
                return Base::event(event);
            }
        }
        // This seemingly useless cast is needed to adjust the this pointer correctly to point to Base.
        void *qwidget() override { return static_cast<QWidget*>(this); }
    };

    template <typename Base>
    std::unique_ptr<SlintTypeErasedWidget> make_unique_animated_widget(void *animation_update_property_ptr)
    {
        ensure_initialized();
        auto ptr = std::make_unique<SlintAnimatedWidget<Base>>();
        // For our hacks to work, we need to have some invisible parent widget.
        static QWidget globalParent;
        ptr->setParent(&globalParent);
        // Let Qt thinks the widget is visible even if it isn't so update() from animation is forwarded
        ptr->setAttribute(Qt::WA_WState_Visible, true);
        // Hack so update() send a UpdateLater event
        ptr->setAttribute(Qt::WA_WState_InPaintEvent, true);
        ptr->animation_update_property_ptr = animation_update_property_ptr;
        return ptr;
    }
}}

cpp_class!(pub unsafe struct SlintTypeErasedWidgetPtr as "std::unique_ptr<SlintTypeErasedWidget>");

impl SlintTypeErasedWidgetPtr {
    fn qwidget_ptr(this: &std::cell::Cell<Self>) -> NonNull<()> {
        let widget_ptr: *mut SlintTypeErasedWidgetPtr = this.as_ptr();
        cpp!(unsafe [widget_ptr as "std::unique_ptr<SlintTypeErasedWidget>*"] -> NonNull<()> as "void*" {
            return (*widget_ptr)->qwidget();
        })
    }
}

cpp! {{
    // Some style function calls setClipRect or setClipRegion on the painter and replace the clips.
    // eg CE_ItemViewItem, CE_Header, or CC_GroupBox in QCommonStyle (#3541).
    // We do workaround that by setting the clip as a system clip so it cant be overwritten
    struct PainterClipWorkaround {
        QPainter *painter;
        QRegion old_clip;
        explicit PainterClipWorkaround(QPainter *painter) : painter(painter) {
            auto engine = painter->paintEngine();
            old_clip = engine->systemClip();
            auto new_clip = painter->clipRegion() * painter->transform();
            if (!old_clip.isNull())
                new_clip &= old_clip;
            engine->setSystemClip(new_clip);
        }
        ~PainterClipWorkaround() {
            auto engine = painter->paintEngine();
            engine->setSystemClip(old_clip);
            // Qt is seriously bugged, setSystemClip will be scaled by the scale factor
            auto actual_clip = engine->systemClip();
            if (actual_clip != old_clip) {
                QSizeF s2 = actual_clip.boundingRect().size();
                QSizeF s1 = old_clip.boundingRect().size();
                engine->setSystemClip(old_clip * QTransform::fromScale(s1.width() / s2.width(), s1.height() / s2.height()));
            }
        }
        PainterClipWorkaround(const PainterClipWorkaround&) = delete;
        PainterClipWorkaround& operator=(const PainterClipWorkaround&) = delete;
    };
}}
cpp_class!(pub(crate) unsafe struct PainterClipWorkaround as "PainterClipWorkaround");
impl PainterClipWorkaround {
    /// Safety: the painter must outlive us
    pub unsafe fn new(painter: &QPainterPtr) -> Self {
        cpp!(unsafe [painter as "const QPainterPtr*"] -> PainterClipWorkaround as "PainterClipWorkaround" {
            return PainterClipWorkaround(painter->get());
        })
    }
}

mod button;
pub use button::*;

mod checkbox;
pub use checkbox::*;

mod spinbox;
pub use spinbox::*;

mod slider;
pub use slider::*;

mod progress_indicator;
pub use progress_indicator::*;

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

mod palette;
pub use palette::*;

mod tableheadersection;
pub use tableheadersection::*;
