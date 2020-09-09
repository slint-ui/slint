/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#![allow(non_upper_case_globals)]
use const_field_offset::FieldOffsets;
use core::pin::Pin;
use cpp::cpp;
use sixtyfps_corelib::graphics::{HighLevelRenderingPrimitive, Rect, RenderingVariable, Resource};
use sixtyfps_corelib::input::{InputEventResult, MouseEvent, MouseEventType};
use sixtyfps_corelib::item_rendering::CachedRenderingData;
use sixtyfps_corelib::items::{Item, ItemConsts, ItemVTable};
use sixtyfps_corelib::layout::LayoutInfo;
use sixtyfps_corelib::rtti::*;
use sixtyfps_corelib::{ItemVTable_static, Property, SharedArray, SharedString, Signal};
use sixtyfps_corelib_macros::*;

use crate::qttypes;

/// Helper macro to get the size from the width and height property,
/// and return Default::default in case the size is too small
macro_rules! get_size {
    ($self:ident) => {{
        let width = Self::FIELD_OFFSETS.width.apply_pin($self).get();
        let height = Self::FIELD_OFFSETS.height.apply_pin($self).get();
        if width < 1. || height < 1. {
            return Default::default();
        };
        qttypes::QSize { width: width as _, height: height as _ }
    }};
}

fn to_resource(image: qttypes::QImage) -> Resource {
    let size = image.size();
    Resource::EmbeddedRgbaImage {
        width: size.width,
        height: size.height,
        data: SharedArray::from(image.data()),
    }
}

cpp! {{
    #include <QtWidgets/QApplication>
    #include <QtWidgets/QStyle>
    #include <QtWidgets/QStyleOption>
    #include <QtWidgets/QStyleFactory>
    #include <QtGui/QPainter>

    void ensure_initialized()
    {
        static auto app [[maybe_unused]]  = []{
            QCoreApplication::setAttribute(Qt::AA_PluginApplication, true);
            static int argc  = 1;
            static char argv[] = "sixtyfps";
            static char *argv2[] = { argv };
            // Leak the QApplication, otherwise it crashes on exit
            // (because the QGuiApplication destructor access some Q_GLOBAL_STATIC which are already gone)
            return new QApplication(argc, argv2);
        }();
    }

    std::tuple<QImage, QRect> offline_style_rendering_image(QSize size)
    {
        ensure_initialized();
        QImage img(size, QImage::Format_ARGB32);
        img.fill(Qt::transparent);
        return std::make_tuple(img, img.rect());
    }
}}

#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
pub struct NativeButton {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub text: Property<SharedString>,
    pub pressed: Property<bool>,
    pub clicked: Signal<()>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for NativeButton {
    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            Self::FIELD_OFFSETS.x.apply_pin(self).get(),
            Self::FIELD_OFFSETS.y.apply_pin(self).get(),
            Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            Self::FIELD_OFFSETS.height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(self: Pin<&Self>) -> HighLevelRenderingPrimitive {
        let down: bool = Self::FIELD_OFFSETS.pressed.apply_pin(self).get();
        let text: qttypes::QString = Self::FIELD_OFFSETS.text.apply_pin(self).get().as_str().into();
        let size: qttypes::QSize = get_size!(self);

        let img = cpp!(unsafe [
            text as "QString",
            size as "QSize",
            down as "bool"
        ] -> qttypes::QImage as "QImage" {
            auto [img, rect] = offline_style_rendering_image(size);
            QPainter p(&img);
            QStyleOptionButton option;
            option.text = std::move(text);
            option.rect = rect;
            if (down)
                option.state |= QStyle::State_Sunken;
            qApp->style()->drawControl(QStyle::CE_PushButton, &option, &p, nullptr);
            return img;
        });
        return HighLevelRenderingPrimitive::Image { source: to_resource(img) };
    }

    fn rendering_variables(self: Pin<&Self>) -> SharedArray<RenderingVariable> {
        SharedArray::default()
    }

    fn layouting_info(self: Pin<&Self>) -> LayoutInfo {
        let text: qttypes::QString = Self::FIELD_OFFSETS.text.apply_pin(self).get().as_str().into();
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

    fn input_event(self: Pin<&Self>, event: MouseEvent) -> InputEventResult {
        Self::FIELD_OFFSETS.pressed.apply_pin(self).set(match event.what {
            MouseEventType::MousePressed => true,
            MouseEventType::MouseExit | MouseEventType::MouseReleased => false,
            MouseEventType::MouseMoved => {
                return if Self::FIELD_OFFSETS.pressed.apply_pin(self).get() {
                    InputEventResult::GrabMouse
                } else {
                    InputEventResult::EventIgnored
                }
            }
        });
        if matches!(event.what, MouseEventType::MouseReleased) {
            Self::FIELD_OFFSETS.clicked.apply_pin(self).emit(&());
            InputEventResult::EventAccepted
        } else {
            InputEventResult::GrabMouse
        }
    }
}

impl ItemConsts for NativeButton {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! { #[no_mangle] pub static NativeButtonVTable for NativeButton }

#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
pub struct NativeCheckBox {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub toggled: Signal<()>,
    pub text: Property<SharedString>,
    pub checked: Property<bool>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for NativeCheckBox {
    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            Self::FIELD_OFFSETS.x.apply_pin(self).get(),
            Self::FIELD_OFFSETS.y.apply_pin(self).get(),
            Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            Self::FIELD_OFFSETS.height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(self: Pin<&Self>) -> HighLevelRenderingPrimitive {
        let checked: bool = Self::FIELD_OFFSETS.checked.apply_pin(self).get();
        let text: qttypes::QString = Self::FIELD_OFFSETS.text.apply_pin(self).get().as_str().into();
        let size: qttypes::QSize = get_size!(self);

        let img = cpp!(unsafe [
            text as "QString",
            size as "QSize",
            checked as "bool"
        ] -> qttypes::QImage as "QImage" {
            auto [img, rect] = offline_style_rendering_image(size);
            QPainter p(&img);
            QStyleOptionButton option;
            option.text = std::move(text);
            option.rect = rect;
            option.state |= checked ? QStyle::State_On : QStyle::State_Off;
            qApp->style()->drawControl(QStyle::CE_CheckBox, &option, &p, nullptr);
            return img;
        });
        return HighLevelRenderingPrimitive::Image { source: to_resource(img) };
    }

    fn rendering_variables(self: Pin<&Self>) -> SharedArray<RenderingVariable> {
        SharedArray::default()
    }

    fn layouting_info(self: Pin<&Self>) -> LayoutInfo {
        let text: qttypes::QString = Self::FIELD_OFFSETS.text.apply_pin(self).get().as_str().into();
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

    fn input_event(self: Pin<&Self>, event: MouseEvent) -> InputEventResult {
        if matches!(event.what, MouseEventType::MouseReleased) {
            Self::FIELD_OFFSETS
                .checked
                .apply_pin(self)
                .set(!Self::FIELD_OFFSETS.checked.apply_pin(self).get());
            Self::FIELD_OFFSETS.toggled.apply_pin(self).emit(&())
        }
        InputEventResult::EventAccepted
    }
}

impl ItemConsts for NativeCheckBox {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! { #[no_mangle] pub static NativeCheckBoxVTable for NativeCheckBox }

#[derive(Default, Copy, Clone, Debug)]
#[repr(C)]
struct NativeSpinBoxData {
    active_controls: u32,
    pressed: bool,
}

#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
pub struct NativeSpinBox {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub value: Property<i32>,
    pub cached_rendering_data: CachedRenderingData,
    data: Property<NativeSpinBoxData>,
}

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

impl Item for NativeSpinBox {
    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            Self::FIELD_OFFSETS.x.apply_pin(self).get(),
            Self::FIELD_OFFSETS.y.apply_pin(self).get(),
            Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            Self::FIELD_OFFSETS.height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(self: Pin<&Self>) -> HighLevelRenderingPrimitive {
        let value: i32 = Self::FIELD_OFFSETS.value.apply_pin(self).get();
        let size: qttypes::QSize = get_size!(self);
        let data = Self::FIELD_OFFSETS.data.apply_pin(self).get();
        let active_controls = data.active_controls;
        let pressed = data.pressed;

        let img = cpp!(unsafe [
            value as "int",
            size as "QSize",
            active_controls as "int",
            pressed as "bool"
        ] -> qttypes::QImage as "QImage" {
            auto [img, rect] = offline_style_rendering_image(size);
            QPainter p(&img);
            auto style = qApp->style();
            QStyleOptionSpinBox option;
            option.rect = rect;
            initQSpinBoxOptions(option, pressed, active_controls);
            style->drawComplexControl(QStyle::CC_SpinBox, &option, &p, nullptr);

            auto text_rect = style->subControlRect(QStyle::CC_SpinBox, &option, QStyle::SC_SpinBoxEditField, nullptr);
            p.drawText(text_rect, QString::number(value));
            return img;
        });
        return HighLevelRenderingPrimitive::Image { source: to_resource(img) };
    }

    fn rendering_variables(self: Pin<&Self>) -> SharedArray<RenderingVariable> {
        SharedArray::default()
    }

    fn layouting_info(self: Pin<&Self>) -> LayoutInfo {
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

            return style->sizeFromContents(QStyle::CT_SpinBox, &option, content.size(), nullptr);
        });
        LayoutInfo {
            min_width: size.width as f32,
            min_height: size.height as f32,
            max_height: size.height as f32,
            ..LayoutInfo::default()
        }
    }

    fn input_event(self: Pin<&Self>, event: MouseEvent) -> InputEventResult {
        let size: qttypes::QSize = get_size!(self);
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
                    if new_control == cpp!(unsafe []->u32 as "int" { return QStyle::SC_SpinBoxUp;})
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
        InputEventResult::EventAccepted
    }
}

impl ItemConsts for NativeSpinBox {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! { #[no_mangle] pub static NativeSpinBoxVTable for NativeSpinBox }

#[derive(Default, Copy, Clone, Debug)]
#[repr(C)]
struct NativeSliderData {
    active_controls: u32,
    pressed: bool,
    pressed_x: f32,
    pressed_val: f32,
}

#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
pub struct NativeSlider {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub value: Property<f32>,
    pub min: Property<f32>,
    pub max: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
    data: Property<NativeSliderData>,
}

cpp! {{
void initQSliderOptions(QStyleOptionSlider &option, bool pressed, int active_controls, int minimum, int maximum, int value) {
    option.subControls = QStyle::SC_SliderGroove | QStyle::SC_SliderHandle;
    option.activeSubControls = { active_controls };
    option.orientation = Qt::Horizontal;
    option.maximum = maximum;
    option.minimum = minimum;
    option.sliderPosition = value;
    option.sliderValue = value;
    option.state = QStyle::State_Enabled | QStyle::State_Active | QStyle::State_Horizontal;
    if (pressed) {
        option.state |= QStyle::State_Sunken | QStyle::State_MouseOver;
    }
}
}}

impl Item for NativeSlider {
    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            Self::FIELD_OFFSETS.x.apply_pin(self).get(),
            Self::FIELD_OFFSETS.y.apply_pin(self).get(),
            Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            Self::FIELD_OFFSETS.height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(self: Pin<&Self>) -> HighLevelRenderingPrimitive {
        let value = Self::FIELD_OFFSETS.value.apply_pin(self).get() as i32;
        let min = Self::FIELD_OFFSETS.min.apply_pin(self).get() as i32;
        let max = Self::FIELD_OFFSETS.max.apply_pin(self).get() as i32;
        let size: qttypes::QSize = get_size!(self);
        let data = Self::FIELD_OFFSETS.data.apply_pin(self).get();
        let active_controls = data.active_controls;
        let pressed = data.pressed;

        let img = cpp!(unsafe [
            value as "int",
            min as "int",
            max as "int",
            size as "QSize",
            active_controls as "int",
            pressed as "bool"
        ] -> qttypes::QImage as "QImage" {
            auto [img, rect] = offline_style_rendering_image(size);
            QPainter p(&img);
            QStyleOptionSlider option;
            option.rect = rect;
            initQSliderOptions(option, pressed, active_controls, min, max, value);
            auto style = qApp->style();
            style->drawComplexControl(QStyle::CC_Slider, &option, &p, nullptr);
            return img;
        });
        return HighLevelRenderingPrimitive::Image { source: to_resource(img) };
    }

    fn rendering_variables(self: Pin<&Self>) -> SharedArray<RenderingVariable> {
        SharedArray::default()
    }

    fn layouting_info(self: Pin<&Self>) -> LayoutInfo {
        let value = Self::FIELD_OFFSETS.value.apply_pin(self).get() as i32;
        let min = Self::FIELD_OFFSETS.min.apply_pin(self).get() as i32;
        let max = Self::FIELD_OFFSETS.max.apply_pin(self).get() as i32;
        let data = Self::FIELD_OFFSETS.data.apply_pin(self).get();
        let active_controls = data.active_controls;
        let pressed = data.pressed;

        let size = cpp!(unsafe [
            value as "int",
            min as "int",
            max as "int",
            active_controls as "int",
            pressed as "bool"
        ] -> qttypes::QSize as "QSize" {
            ensure_initialized();
            QStyleOptionSlider option;
            initQSliderOptions(option, pressed, active_controls, min, max, value);
            auto style = qApp->style();
            auto thick = style->pixelMetric(QStyle::PM_SliderThickness, &option, nullptr);
            return style->sizeFromContents(QStyle::CT_Slider, &option, QSize(0, thick), nullptr);
        });
        LayoutInfo {
            min_width: size.width as f32,
            min_height: size.height as f32,
            max_height: size.height as f32,
            ..LayoutInfo::default()
        }
    }

    fn input_event(self: Pin<&Self>, event: MouseEvent) -> InputEventResult {
        let size: qttypes::QSize = get_size!(self);
        let value = Self::FIELD_OFFSETS.value.apply_pin(self).get() as f32;
        let min = Self::FIELD_OFFSETS.min.apply_pin(self).get() as f32;
        let max = Self::FIELD_OFFSETS.max.apply_pin(self).get() as f32;
        let mut data = Self::FIELD_OFFSETS.data.apply_pin(self).get();
        let active_controls = data.active_controls;
        let pressed = data.pressed;
        let pos = qttypes::QPoint { x: event.pos.x as u32, y: event.pos.y as u32 };

        let new_control = cpp!(unsafe [
            pos as "QPoint",
            size as "QSize",
            value as "float",
            min as "float",
            max as "float",
            active_controls as "int",
            pressed as "bool"
        ] -> u32 as "int" {
            ensure_initialized();
            QStyleOptionSlider option;
            initQSliderOptions(option, pressed, active_controls, min, max, value);
            auto style = qApp->style();
            option.rect = { QPoint{}, size };
            return style->hitTestComplexControl(QStyle::CC_Slider, &option, pos, nullptr);
        });
        let result = match event.what {
            MouseEventType::MousePressed => {
                data.pressed_x = event.pos.x as f32;
                data.pressed = true;
                data.pressed_val = value;
                InputEventResult::GrabMouse
            }
            MouseEventType::MouseExit | MouseEventType::MouseReleased => {
                data.pressed = false;
                InputEventResult::EventAccepted
            }
            MouseEventType::MouseMoved => {
                if data.pressed {
                    // FIXME: use QStyle::subControlRect to find out the actual size of the groove
                    let new_val = data.pressed_val
                        + ((event.pos.x as f32) - data.pressed_x) * (max - min) / size.width as f32;
                    self.value.set(new_val.max(min).min(max));
                    InputEventResult::GrabMouse
                } else {
                    InputEventResult::EventIgnored
                }
            }
        };
        data.active_controls = new_control;

        self.data.set(data);
        result
    }
}

impl ItemConsts for NativeSlider {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! { #[no_mangle] pub static NativeSliderVTable for NativeSlider }

#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
pub struct NativeGroupBox {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub title: Property<SharedString>,
    pub cached_rendering_data: CachedRenderingData,
    pub native_padding_left: Property<f32>,
    pub native_padding_right: Property<f32>,
    pub native_padding_top: Property<f32>,
    pub native_padding_bottom: Property<f32>,
}

impl Item for NativeGroupBox {
    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            Self::FIELD_OFFSETS.x.apply_pin(self).get(),
            Self::FIELD_OFFSETS.y.apply_pin(self).get(),
            Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            Self::FIELD_OFFSETS.height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(self: Pin<&Self>) -> HighLevelRenderingPrimitive {
        let text: qttypes::QString =
            Self::FIELD_OFFSETS.title.apply_pin(self).get().as_str().into();
        let size: qttypes::QSize = get_size!(self);

        let img = cpp!(unsafe [
            text as "QString",
            size as "QSize"
        ] -> qttypes::QImage as "QImage" {
            auto [img, rect] = offline_style_rendering_image(size);
            QPainter p(&img);
            QStyleOptionGroupBox option;
            option.rect = rect;
            option.text = text;
            option.lineWidth = 1;
            option.midLineWidth = 0;
            option.subControls = QStyle::SC_GroupBoxFrame;
            if (!text.isEmpty()) {
                option.subControls |= QStyle::SC_GroupBoxLabel;
            }
            option.textColor = QColor(qApp->style()->styleHint(
                QStyle::SH_GroupBox_TextLabelColor, &option));
            qApp->style()->drawComplexControl(QStyle::CC_GroupBox, &option, &p);
            return img;
        });
        return HighLevelRenderingPrimitive::Image { source: to_resource(img) };
    }

    fn rendering_variables(self: Pin<&Self>) -> SharedArray<RenderingVariable> {
        SharedArray::default()
    }

    fn layouting_info(self: Pin<&Self>) -> LayoutInfo {
        let text: qttypes::QString =
            Self::FIELD_OFFSETS.title.apply_pin(self).get().as_str().into();

        let paddings = cpp!(unsafe [
            text as "QString"
        ] -> qttypes::QMargins as "QMargins" {
            ensure_initialized();
            QStyleOptionGroupBox option;
            option.text = text;
            option.lineWidth = 1;
            option.midLineWidth = 0;
            option.subControls = QStyle::SC_GroupBoxFrame;
            if (!text.isEmpty()) {
                option.subControls |= QStyle::SC_GroupBoxLabel;
            }
             // Just some size big enough to be sure that the frame fitst in it
            option.rect = QRect(0, 0, 10000, 10000);
            option.textColor = QColor(qApp->style()->styleHint(
                QStyle::SH_GroupBox_TextLabelColor, &option));
            QRect contentsRect = qApp->style()->subControlRect(
                QStyle::CC_GroupBox, &option, QStyle::SC_GroupBoxContents);
            //QRect elementRect = qApp->style()->subElementRect(
            //    QStyle::SE_GroupBoxLayoutItem, &option);

            auto hs = qApp->style()->pixelMetric(QStyle::PM_LayoutHorizontalSpacing, &option);
            auto vs = qApp->style()->pixelMetric(QStyle::PM_LayoutVerticalSpacing, &option);

            return {
                contentsRect.left() + hs,
                contentsRect.top() + vs,
                option.rect.right() - contentsRect.right() + hs,
                option.rect.bottom() - contentsRect.bottom() + vs };
        });
        self.native_padding_left.set(paddings.left as _);
        self.native_padding_right.set(paddings.right as _);
        self.native_padding_top.set(paddings.top as _);
        self.native_padding_bottom.set(paddings.bottom as _);
        LayoutInfo {
            min_width: (paddings.left + paddings.right) as _,
            min_height: (paddings.top + paddings.bottom) as _,
            ..LayoutInfo::default()
        }
    }

    fn input_event(self: Pin<&Self>, _: MouseEvent) -> InputEventResult {
        InputEventResult::EventIgnored
    }
}

impl ItemConsts for NativeGroupBox {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! { #[no_mangle] pub static NativeGroupBoxVTable for NativeGroupBox }
