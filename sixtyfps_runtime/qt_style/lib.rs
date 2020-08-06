#![allow(non_upper_case_globals)]
#![cfg_attr(not(have_qt), allow(unused))]
#![recursion_limit = "256"]
use const_field_offset::FieldOffsets;
use core::pin::Pin;
#[cfg(have_qt)]
use cpp::cpp;
use sixtyfps_corelib::abi::datastructures::{Item, ItemConsts, ItemVTable};
use sixtyfps_corelib::graphics::{HighLevelRenderingPrimitive, Rect, RenderingVariable, Resource};
use sixtyfps_corelib::input::{InputEventResult, MouseEvent, MouseEventType};
use sixtyfps_corelib::item_rendering::CachedRenderingData;
use sixtyfps_corelib::layout::LayoutInfo;
#[cfg(feature = "rtti")]
use sixtyfps_corelib::rtti::*;
use sixtyfps_corelib::{ItemVTable_static, Property, SharedArray, SharedString, Signal};
use sixtyfps_corelib_macros::*;

#[cfg(have_qt)]
mod qttypes;

#[cfg(have_qt)]
fn to_resource(image: qttypes::QImage) -> Resource {
    let size = image.size();
    Resource::EmbeddedRgbaImage {
        width: size.width,
        height: size.height,
        data: SharedArray::from(image.data()),
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
            Self::FIELD_OFFSETS.x.apply_pin(self).get(),
            Self::FIELD_OFFSETS.y.apply_pin(self).get(),
            Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            Self::FIELD_OFFSETS.height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(self: Pin<&Self>) -> HighLevelRenderingPrimitive {
        #[cfg(have_qt)]
        {
            let down: bool = Self::FIELD_OFFSETS.pressed.apply_pin(self).get();
            let text: qttypes::QString =
                Self::FIELD_OFFSETS.text.apply_pin(self).get().as_str().into();
            let size: qttypes::QSize = qttypes::QSize {
                width: Self::FIELD_OFFSETS.width.apply_pin(self).get() as _,
                height: Self::FIELD_OFFSETS.height.apply_pin(self).get() as _,
            };

            let img = cpp!(unsafe [
                text as "QString",
                size as "QSize",
                down as "bool"
            ] -> qttypes::QImage as "QImage" {
                ensure_initialized();
                QImage img(size, QImage::Format_ARGB32);
                img.fill(Qt::transparent);
                QPainter p(&img);
                QStyleOptionButton option;
                option.text = std::move(text);
                option.rect = QRect(img.rect());
                if (down)
                    option.state |= QStyle::State_Sunken;
                qApp->style()->drawControl(QStyle::CE_PushButton, &option, &p, nullptr);
                return img;
            });
            return HighLevelRenderingPrimitive::Image { source: to_resource(img) };
        }
        #[cfg(not(have_qt))]
        HighLevelRenderingPrimitive::NoContents
    }

    fn rendering_variables(self: Pin<&Self>) -> SharedArray<RenderingVariable> {
        SharedArray::from(&[])
    }

    fn layouting_info(self: Pin<&Self>) -> LayoutInfo {
        #[cfg(have_qt)]
        {
            let text: qttypes::QString =
                Self::FIELD_OFFSETS.text.apply_pin(self).get().as_str().into();
            let size = cpp!(unsafe [
                text as "QString"
            ] -> qttypes::QSize as "QSize" {
                ensure_initialized();
                QStyleOptionButton option;
                option.rect = option.fontMetrics.boundingRect(text);
                option.text = std::move(text);
                return qApp->style()->sizeFromContents(QStyle::CT_PushButton, &option, option.rect.size(), nullptr);
            });
            LayoutInfo {
                min_width: size.width as f32,
                min_height: size.height as f32,
                ..LayoutInfo::default()
            }
        }
        #[cfg(not(have_qt))]
        LayoutInfo::default()
    }

    fn input_event(self: Pin<&Self>, event: MouseEvent) -> InputEventResult {
        Self::FIELD_OFFSETS.pressed.apply_pin(self).set(match event.what {
            MouseEventType::MousePressed => true,
            MouseEventType::MouseExit | MouseEventType::MouseReleased => false,
            MouseEventType::MouseMoved => return InputEventResult::EventAccepted,
        });
        if matches!(event.what, MouseEventType::MouseReleased) {
            Self::FIELD_OFFSETS.clicked.apply_pin(self).emit(())
        }
        InputEventResult::GrabMouse
    }
}

impl ItemConsts for QtStyleButton {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
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
            Self::FIELD_OFFSETS.x.apply_pin(self).get(),
            Self::FIELD_OFFSETS.y.apply_pin(self).get(),
            Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            Self::FIELD_OFFSETS.height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(self: Pin<&Self>) -> HighLevelRenderingPrimitive {
        #[cfg(have_qt)]
        {
            let checked: bool = Self::FIELD_OFFSETS.checked.apply_pin(self).get();
            let text: qttypes::QString =
                Self::FIELD_OFFSETS.text.apply_pin(self).get().as_str().into();
            let size: qttypes::QSize = qttypes::QSize {
                width: Self::FIELD_OFFSETS.width.apply_pin(self).get() as _,
                height: Self::FIELD_OFFSETS.height.apply_pin(self).get() as _,
            };

            let img = cpp!(unsafe [
                text as "QString",
                size as "QSize",
                checked as "bool"
            ] -> qttypes::QImage as "QImage" {
                ensure_initialized();
                QImage img(size, QImage::Format_ARGB32);
                img.fill(Qt::transparent);
                QPainter p(&img);
                QStyleOptionButton option;
                option.text = std::move(text);
                option.rect = QRect(img.rect());
                option.state |= checked ? QStyle::State_On : QStyle::State_Off;
                qApp->style()->drawControl(QStyle::CE_CheckBox, &option, &p, nullptr);
                return img;
            });
            return HighLevelRenderingPrimitive::Image { source: to_resource(img) };
        }
        #[cfg(not(have_qt))]
        HighLevelRenderingPrimitive::NoContents
    }

    fn rendering_variables(self: Pin<&Self>) -> SharedArray<RenderingVariable> {
        SharedArray::from(&[])
    }

    fn layouting_info(self: Pin<&Self>) -> LayoutInfo {
        #[cfg(have_qt)]
        {
            let text: qttypes::QString =
                Self::FIELD_OFFSETS.text.apply_pin(self).get().as_str().into();
            let size = cpp!(unsafe [
                text as "QString"
            ] -> qttypes::QSize as "QSize" {
                ensure_initialized();
                QStyleOptionButton option;
                option.rect = option.fontMetrics.boundingRect(text);
                option.text = std::move(text);
                return qApp->style()->sizeFromContents(QStyle::CT_PushButton, &option, option.rect.size(), nullptr);
            });
            LayoutInfo {
                min_width: size.width as f32,
                min_height: size.height as f32,
                max_height: size.height as f32,
                ..LayoutInfo::default()
            }
        }
        #[cfg(not(have_qt))]
        LayoutInfo::default()
    }

    fn input_event(self: Pin<&Self>, event: MouseEvent) -> InputEventResult {
        if matches!(event.what, MouseEventType::MouseReleased) {
            Self::FIELD_OFFSETS
                .checked
                .apply_pin(self)
                .set(!Self::FIELD_OFFSETS.checked.apply_pin(self).get());
            Self::FIELD_OFFSETS.toggled.apply_pin(self).emit(())
        }
        InputEventResult::GrabMouse
    }
}

impl ItemConsts for QtStyleCheckBox {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! { #[no_mangle] pub static QtStyleCheckBoxVTable for QtStyleCheckBox }

#[derive(Default, Copy, Clone, Debug)]
#[repr(C)]
struct QtStyleSpinBoxData {
    active_controls: u32,
    pressed: bool,
}

#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
pub struct QtStyleSpinBox {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub value: Property<i32>,
    pub cached_rendering_data: CachedRenderingData,
    data: Property<QtStyleSpinBoxData>,
}

#[cfg(have_qt)]
cpp! {{
void initQSpinBoxOptions(QStyleOptionSpinBox &option, bool pressed, int active_controls) {
    auto style = qApp->style();
    option.activeSubControls = QStyle::SC_None;
    option.subControls = QStyle::SC_SpinBoxEditField | QStyle::SC_SpinBoxUp | QStyle::SC_SpinBoxDown;
    if (style->styleHint(QStyle::SH_SpinBox_ButtonsInsideFrame, nullptr, nullptr))
        option.subControls |= QStyle::SC_SpinBoxFrame;
    option.activeSubControls = {active_controls};
    option.state = QStyle::State_Enabled | QStyle::State_Active;
    if (pressed) {
        option.state |= QStyle::State_Sunken | QStyle::State_MouseOver;
    }
    /*if (active_controls) {
        option.state |= QStyle::State_MouseOver;
    }*/
    option.stepEnabled = QAbstractSpinBox::StepDownEnabled | QAbstractSpinBox::StepUpEnabled;
    option.frame = true;
}
}}

impl Item for QtStyleSpinBox {
    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            Self::FIELD_OFFSETS.x.apply_pin(self).get(),
            Self::FIELD_OFFSETS.y.apply_pin(self).get(),
            Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            Self::FIELD_OFFSETS.height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(self: Pin<&Self>) -> HighLevelRenderingPrimitive {
        #[cfg(have_qt)]
        {
            let value: i32 = Self::FIELD_OFFSETS.value.apply_pin(self).get();
            let size: qttypes::QSize = qttypes::QSize {
                width: Self::FIELD_OFFSETS.width.apply_pin(self).get() as _,
                height: Self::FIELD_OFFSETS.height.apply_pin(self).get() as _,
            };
            let data = Self::FIELD_OFFSETS.data.apply_pin(self).get();
            let active_controls = data.active_controls;
            let pressed = data.pressed;

            let img = cpp!(unsafe [
                value as "int",
                size as "QSize",
                active_controls as "int",
                pressed as "bool"
            ] -> qttypes::QImage as "QImage" {
                ensure_initialized();
                QImage img(size, QImage::Format_ARGB32);
                img.fill(Qt::transparent);
                QPainter p(&img);
                auto style = qApp->style();
                QStyleOptionSpinBox option;
                option.rect = img.rect();
                initQSpinBoxOptions(option, pressed, active_controls);
                style->drawComplexControl(QStyle::CC_SpinBox, &option, &p, nullptr);

                auto text_rect = style->subControlRect(QStyle::CC_SpinBox, &option, QStyle::SC_SpinBoxEditField, nullptr);
                p.drawText(text_rect, QString::number(value));
                return img;
            });
            return HighLevelRenderingPrimitive::Image { source: to_resource(img) };
        }
        #[cfg(not(have_qt))]
        HighLevelRenderingPrimitive::NoContents
    }

    fn rendering_variables(self: Pin<&Self>) -> SharedArray<RenderingVariable> {
        SharedArray::from(&[])
    }

    fn layouting_info(self: Pin<&Self>) -> LayoutInfo {
        #[cfg(have_qt)]
        {
            //let value: i32 = Self::FIELD_OFFSETS.value.apply_pin(self).get();
            let data = Self::FIELD_OFFSETS.data.apply_pin(self).get();
            let active_controls = data.active_controls;
            let pressed = data.pressed;

            let size = cpp!(unsafe [
                //value as "int",
                active_controls as "int",
                pressed as "bool"
            ] -> qttypes::QSize as "QSize" {
                ensure_initialized();
                auto style = qApp->style();

                QStyleOptionSpinBox option;
                initQSpinBoxOptions(option, pressed, active_controls);

                auto content = option.fontMetrics.boundingRect("0000");

                return style->sizeFromContents(QStyle::CT_SpinBox, &option, content.size(), nullptr)
                    .expandedTo(QApplication::globalStrut());
            });
            LayoutInfo {
                min_width: size.width as f32,
                min_height: size.height as f32,
                max_height: size.height as f32,
                ..LayoutInfo::default()
            }
        }
        #[cfg(not(have_qt))]
        LayoutInfo::default()
    }

    fn input_event(self: Pin<&Self>, event: MouseEvent) -> InputEventResult {
        #[cfg(have_qt)]
        {
            let size: qttypes::QSize = qttypes::QSize {
                width: Self::FIELD_OFFSETS.width.apply_pin(self).get() as _,
                height: Self::FIELD_OFFSETS.height.apply_pin(self).get() as _,
            };
            let mut data = Self::FIELD_OFFSETS.data.apply_pin(self).get();
            let active_controls = data.active_controls;
            let pressed = data.pressed;

            let pos = qttypes::QPoint { x: event.pos.x as u32, y: event.pos.y as u32 };

            let new_control = cpp!(unsafe [
                pos as "QPoint",
                size as "QSize",
                active_controls as "int",
                pressed as "bool"
            ] -> u32 as "int" {
                ensure_initialized();
                auto style = qApp->style();

                QStyleOptionSpinBox option;
                option.rect = { QPoint{}, size };
                initQSpinBoxOptions(option, pressed, active_controls);

                return style->hitTestComplexControl(QStyle::CC_SpinBox, &option, pos, nullptr);
            });
            let changed = new_control != active_controls
                || match event.what {
                    MouseEventType::MousePressed => {
                        data.pressed = true;
                        true
                    }
                    MouseEventType::MouseExit | MouseEventType::MouseReleased => {
                        data.pressed = false;
                        if new_control
                            == cpp!(unsafe []->u32 as "int" { return QStyle::SC_SpinBoxUp;})
                        {
                            self.value.set(Self::FIELD_OFFSETS.value.apply_pin(self).get() + 1);
                        }
                        if new_control
                            == cpp!(unsafe []->u32 as "int" { return QStyle::SC_SpinBoxDown;})
                        {
                            self.value.set(Self::FIELD_OFFSETS.value.apply_pin(self).get() - 1);
                        }
                        true
                    }
                    MouseEventType::MouseMoved => false,
                };
            data.active_controls = new_control;
            if changed {
                self.data.set(data);
            }
        }
        InputEventResult::GrabMouse
    }
}

impl ItemConsts for QtStyleSpinBox {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! { #[no_mangle] pub static QtStyleSpinBoxVTable for QtStyleSpinBox }
