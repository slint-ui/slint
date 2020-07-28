#![allow(non_upper_case_globals)]
use const_field_offset::FieldOffsets;
use core::pin::Pin;
use corelib_macro::*;
#[cfg(have_qt)]
use cpp::cpp;
#[cfg(have_qt)]
use sixtyfps_corelib::abi::datastructures::Resource;
use sixtyfps_corelib::abi::datastructures::{
    CachedRenderingData, Item, ItemConsts, ItemVTable, LayoutInfo, MouseEvent, MouseEventType,
    Rect, RenderingPrimitive,
};
#[cfg(feature = "rtti")]
use sixtyfps_corelib::rtti::*;
use sixtyfps_corelib::{ItemVTable_static, Property, SharedString, Signal};

#[cfg(have_qt)]
mod qttypes;

#[cfg(have_qt)]
fn to_resource(image: qttypes::QImage) -> Resource {
    let size = image.size();
    Resource::EmbeddedDataOwned {
        width: size.width,
        height: size.height,
        data: sixtyfps_corelib::abi::sharedarray::SharedArray::from(image.data()),
    }
}

#[cfg(have_qt)]
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
        #[cfg(have_qt)]
        {
            let down: bool = Self::field_offsets().pressed.apply_pin(self).get();
            let text: qttypes::QString =
                Self::field_offsets().text.apply_pin(self).get().as_str().into();
            let size: qttypes::QSize = qttypes::QSize {
                width: Self::field_offsets().width.apply_pin(self).get() as _,
                height: Self::field_offsets().height.apply_pin(self).get() as _,
            };

            #[cfg(have_qt)]
            let img = cpp!(unsafe [
                text as "QString",
                size as "QSize",
                down as "bool"
            ] -> qttypes::QImage as "QImage" {
                ensure_initialized();
                QImage img(size, QImage::Format_ARGB32);
                img.fill(Qt::transparent);
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
            return RenderingPrimitive::Image {
                x: Self::field_offsets().x.apply_pin(self).get(),
                y: Self::field_offsets().y.apply_pin(self).get(),
                source: to_resource(img),
            };
        }
        #[cfg(not(have_qt))]
        RenderingPrimitive::NoContents
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
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::field_offsets().cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! { #[no_mangle] pub static QtStyleButtonVTable for QtStyleButton }

#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
pub struct QtStyleCheckBox {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub toggled: Signal<()>,
    pub text: Property<SharedString>,
    pub checked: Property<bool>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for QtStyleCheckBox {
    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            Self::field_offsets().x.apply_pin(self).get(),
            Self::field_offsets().y.apply_pin(self).get(),
            Self::field_offsets().width.apply_pin(self).get(),
            Self::field_offsets().height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(self: Pin<&Self>) -> RenderingPrimitive {
        #[cfg(have_qt)]
        {
            let checked: bool = Self::field_offsets().checked.apply_pin(self).get();
            let text: qttypes::QString =
                Self::field_offsets().text.apply_pin(self).get().as_str().into();
            let size: qttypes::QSize = qttypes::QSize {
                width: Self::field_offsets().width.apply_pin(self).get() as _,
                height: Self::field_offsets().height.apply_pin(self).get() as _,
            };

            #[cfg(have_qt)]
            let img = cpp!(unsafe [
                text as "QString",
                size as "QSize",
                checked as "bool"
            ] -> qttypes::QImage as "QImage" {
                ensure_initialized();
                QImage img(size, QImage::Format_ARGB32);
                img.fill(Qt::transparent);
                // Note: i wonder if it would be possible to paint directly in the cairo context
                QPainter p(&img);
                QStyleOptionButton option;
                option.text = std::move(text);
                option.rect = QRect(img.rect());
                option.state |= checked ? QStyle::State_On : QStyle::State_Off;
                qApp->style()->drawControl(QStyle::CE_CheckBox, &option, &p, nullptr);
                return img;
            });
            return RenderingPrimitive::Image {
                x: Self::field_offsets().x.apply_pin(self).get(),
                y: Self::field_offsets().y.apply_pin(self).get(),
                source: to_resource(img),
            };
        }
        #[cfg(not(have_qt))]
        RenderingPrimitive::NoContents
    }

    fn layouting_info(self: Pin<&Self>) -> LayoutInfo {
        todo!()
    }

    fn input_event(self: Pin<&Self>, event: MouseEvent) {
        if matches!(event.what, MouseEventType::MouseReleased) {
            Self::field_offsets()
                .checked
                .apply_pin(self)
                .set(!Self::field_offsets().checked.apply_pin(self).get());
            Self::field_offsets().toggled.apply_pin(self).emit(())
        }
    }
}

impl ItemConsts for QtStyleCheckBox {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::field_offsets().cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! { #[no_mangle] pub static QtStyleCheckBoxVTable for QtStyleCheckBox }
