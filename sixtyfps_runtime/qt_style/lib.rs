#![allow(non_upper_case_globals)]
use const_field_offset::FieldOffsets;
use core::pin::Pin;
use corelib_macro::*;
use cpp::cpp;
use sixtyfps_corelib::abi::datastructures::{
    CachedRenderingData, Item, ItemConsts, LayoutInfo, MouseEvent, MouseEventType, Rect,
    RenderingPrimitive, Resource,
};
#[cfg(feature = "rtti")]
use sixtyfps_corelib::rtti::*;
use sixtyfps_corelib::{Property, SharedString, Signal};

mod qttypes;

cpp! {{
    #include <QtWidgets/QApplication>
    #include <QtWidgets/QStyle>
    #include <QtWidgets/QStyleOption>
    #include <QtWidgets/QStyleFactory>
    #include <QtGui/QPainter>

    void ensure_initialized()
    {
        static int argc  = 1;
        static char argv[] = "sixtyfps";
        static char *argv2[] = { argv };
        static QApplication app(argc, argv2);
    }
}}

#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
pub struct QtStyleButton {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub text: Property<SharedString>,
    pub pressed: Property<bool>,
    pub clicked: Signal<()>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for QtStyleButton {
    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            Self::field_offsets().x.apply_pin(self).get(),
            Self::field_offsets().y.apply_pin(self).get(),
            Self::field_offsets().width.apply_pin(self).get(),
            Self::field_offsets().height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(self: Pin<&Self>) -> RenderingPrimitive {
        let down: bool = Self::field_offsets().pressed.apply_pin(self).get();
        let text: qttypes::QString =
            Self::field_offsets().text.apply_pin(self).get().as_str().into();
        let size: qttypes::QSize = qttypes::QSize {
            width: Self::field_offsets().width.apply_pin(self).get() as _,
            height: Self::field_offsets().height.apply_pin(self).get() as _,
        };

        let img = cpp!(unsafe [
            text as "QString",
            size as "QSize",
            down as "bool"
        ] -> qttypes::QImage as "QImage" {
            ensure_initialized();
            QImage img(size, QImage::Format_ARGB32);
            // Note: i wonder if it would be possible to paint directly in the cairo context
            QPainter p(&img);
            QStyleOptionButton option;
            option.text = std::move(text);
            option.rect = QRect(img.rect());
            if (down)
                option.state |= QStyle::State_Sunken;
            qApp->style()->drawControl(QStyle::CE_PushButton, &option, &p, nullptr);
            return img;
        });
        let source = Resource::EmbeddedDataOwned(
            sixtyfps_corelib::abi::sharedarray::SharedArray::from(img.data()),
        );
        RenderingPrimitive::Image { x: 0., y: 0., source }
    }

    fn layouting_info(self: Pin<&Self>) -> LayoutInfo {
        todo!()
    }

    fn input_event(self: Pin<&Self>, event: MouseEvent) {
        Self::field_offsets().pressed.apply_pin(self).set(match event.what {
            MouseEventType::MousePressed => true,
            MouseEventType::MouseReleased => false,
            MouseEventType::MouseMoved => return,
        });
        if matches!(event.what, MouseEventType::MouseReleased) {
            Self::field_offsets().clicked.apply_pin(self).emit(())
        }
    }
}

impl ItemConsts for QtStyleButton {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        QtStyleButton,
        CachedRenderingData,
    > = QtStyleButton::field_offsets().cached_rendering_data.as_unpinned_projection();
}

/*
#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
pub struct QtStyleCheckBox {
    pub toggled: Signal<()>,
    pub text: Property<SharedString>,
    pub checked: Property<bool>,
}
*/
