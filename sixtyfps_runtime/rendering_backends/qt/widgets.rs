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
use sixtyfps_corelib::component::{ComponentRefPin, ComponentVTable};
use sixtyfps_corelib::eventloop::ComponentWindow;
use sixtyfps_corelib::graphics::{HighLevelRenderingPrimitive, Rect, RenderingVariable, Resource};
use sixtyfps_corelib::input::{
    FocusEvent, InputEventResult, KeyEvent, KeyEventResult, MouseEvent, MouseEventType,
};
use sixtyfps_corelib::item_rendering::CachedRenderingData;
use sixtyfps_corelib::items::{Item, ItemConsts, ItemVTable};
use sixtyfps_corelib::layout::LayoutInfo;
use sixtyfps_corelib::rtti::*;
use sixtyfps_corelib::{ItemVTable_static, Property, SharedArray, SharedString, Signal};
use sixtyfps_corelib_macros::*;
use std::rc::Rc;

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
    #include <QtCore/QDebug>

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

    std::tuple<QImage, QRect> offline_style_rendering_image(QSize size, float dpr)
    {
        ensure_initialized();
        QImage img(size, QImage::Format_ARGB32_Premultiplied);
        img.setDevicePixelRatio(dpr);
        img.fill(Qt::transparent);
        return std::make_tuple(img, QRect(0, 0, size.width() / dpr, size.height() / dpr));
    }

    QWidget *global_widget()
    {
#if defined(Q_WS_MAC)
        static QWidget widget;
        return &widget;
#else
        return nullptr;
#endif
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
    fn init(self: Pin<&Self>, _window: &ComponentWindow) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            Self::FIELD_OFFSETS.x.apply_pin(self).get(),
            Self::FIELD_OFFSETS.y.apply_pin(self).get(),
            Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            Self::FIELD_OFFSETS.height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(
        self: Pin<&Self>,
        window: &ComponentWindow,
    ) -> HighLevelRenderingPrimitive {
        let down: bool = Self::FIELD_OFFSETS.pressed.apply_pin(self).get();
        let text: qttypes::QString = Self::FIELD_OFFSETS.text.apply_pin(self).get().as_str().into();
        let size: qttypes::QSize = get_size!(self);
        let dpr = window.scale_factor();

        let img = cpp!(unsafe [
            text as "QString",
            size as "QSize",
            down as "bool",
            dpr as "float"
        ] -> qttypes::QImage as "QImage" {
            auto [img, rect] = offline_style_rendering_image(size, dpr);
            QPainter p(&img);
            QStyleOptionButton option;
            option.text = std::move(text);
            option.rect = rect;
            if (down)
                option.state |= QStyle::State_Sunken;
            qApp->style()->drawControl(QStyle::CE_PushButton, &option, &p, global_widget());
            return img;
        });
        return HighLevelRenderingPrimitive::Image { source: to_resource(img) };
    }

    fn rendering_variables(
        self: Pin<&Self>,
        _window: &ComponentWindow,
    ) -> SharedArray<RenderingVariable> {
        SharedArray::default()
    }

    fn layouting_info(self: Pin<&Self>, window: &ComponentWindow) -> LayoutInfo {
        let text: qttypes::QString = Self::FIELD_OFFSETS.text.apply_pin(self).get().as_str().into();
        let dpr = window.scale_factor();
        let size = cpp!(unsafe [
            text as "QString",
            dpr as "float"
        ] -> qttypes::QSize as "QSize" {
            ensure_initialized();
            QStyleOptionButton option;
            option.rect = option.fontMetrics.boundingRect(text);
            option.text = std::move(text);
            return qApp->style()->sizeFromContents(QStyle::CT_PushButton, &option, option.rect.size(), nullptr) * dpr;
        });
        LayoutInfo {
            min_width: size.width as f32,
            min_height: size.height as f32,
            max_width: size.width as f32,
            max_height: size.height as f32,
            ..LayoutInfo::default()
        }
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        _window: &ComponentWindow,
        _app_component: ComponentRefPin,
    ) -> InputEventResult {
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

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &ComponentWindow) {}
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
    fn init(self: Pin<&Self>, _window: &ComponentWindow) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            Self::FIELD_OFFSETS.x.apply_pin(self).get(),
            Self::FIELD_OFFSETS.y.apply_pin(self).get(),
            Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            Self::FIELD_OFFSETS.height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(
        self: Pin<&Self>,
        window: &ComponentWindow,
    ) -> HighLevelRenderingPrimitive {
        let checked: bool = Self::FIELD_OFFSETS.checked.apply_pin(self).get();
        let text: qttypes::QString = Self::FIELD_OFFSETS.text.apply_pin(self).get().as_str().into();
        let size: qttypes::QSize = get_size!(self);
        let dpr = window.scale_factor();

        let img = cpp!(unsafe [
            text as "QString",
            size as "QSize",
            checked as "bool",
            dpr as "float"
        ] -> qttypes::QImage as "QImage" {
            auto [img, rect] = offline_style_rendering_image(size, dpr);
            QPainter p(&img);
            QStyleOptionButton option;
            option.text = std::move(text);
            option.rect = rect;
            option.state |= checked ? QStyle::State_On : QStyle::State_Off;
            qApp->style()->drawControl(QStyle::CE_CheckBox, &option, &p, global_widget());
            return img;
        });
        return HighLevelRenderingPrimitive::Image { source: to_resource(img) };
    }

    fn rendering_variables(
        self: Pin<&Self>,
        _window: &ComponentWindow,
    ) -> SharedArray<RenderingVariable> {
        SharedArray::default()
    }

    fn layouting_info(self: Pin<&Self>, window: &ComponentWindow) -> LayoutInfo {
        let text: qttypes::QString = Self::FIELD_OFFSETS.text.apply_pin(self).get().as_str().into();
        let dpr = window.scale_factor();
        let size = cpp!(unsafe [
            text as "QString",
            dpr as "float"
        ] -> qttypes::QSize as "QSize" {
            ensure_initialized();
            QStyleOptionButton option;
            option.rect = option.fontMetrics.boundingRect(text);
            option.text = std::move(text);
            return qApp->style()->sizeFromContents(QStyle::CT_PushButton, &option, option.rect.size(), nullptr) * dpr;
        });
        LayoutInfo {
            min_width: size.width as f32,
            min_height: size.height as f32,
            max_height: size.height as f32,
            ..LayoutInfo::default()
        }
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        _window: &ComponentWindow,
        _app_component: ComponentRefPin,
    ) -> InputEventResult {
        if matches!(event.what, MouseEventType::MouseReleased) {
            Self::FIELD_OFFSETS
                .checked
                .apply_pin(self)
                .set(!Self::FIELD_OFFSETS.checked.apply_pin(self).get());
            Self::FIELD_OFFSETS.toggled.apply_pin(self).emit(&())
        }
        InputEventResult::EventAccepted
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &ComponentWindow) {}
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
    fn init(self: Pin<&Self>, _window: &ComponentWindow) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            Self::FIELD_OFFSETS.x.apply_pin(self).get(),
            Self::FIELD_OFFSETS.y.apply_pin(self).get(),
            Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            Self::FIELD_OFFSETS.height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(
        self: Pin<&Self>,
        window: &ComponentWindow,
    ) -> HighLevelRenderingPrimitive {
        let value: i32 = Self::FIELD_OFFSETS.value.apply_pin(self).get();
        let size: qttypes::QSize = get_size!(self);
        let dpr = window.scale_factor();
        let data = Self::FIELD_OFFSETS.data.apply_pin(self).get();
        let active_controls = data.active_controls;
        let pressed = data.pressed;

        let img = cpp!(unsafe [
            value as "int",
            size as "QSize",
            active_controls as "int",
            pressed as "bool",
            dpr as "float"
        ] -> qttypes::QImage as "QImage" {
            auto [img, rect] = offline_style_rendering_image(size, dpr);
            QPainter p(&img);
            auto style = qApp->style();
            QStyleOptionSpinBox option;
            option.rect = rect;
            initQSpinBoxOptions(option, pressed, active_controls);
            style->drawComplexControl(QStyle::CC_SpinBox, &option, &p, nullptr);

            auto text_rect = style->subControlRect(QStyle::CC_SpinBox, &option, QStyle::SC_SpinBoxEditField, global_widget());
            p.drawText(text_rect, QString::number(value));
            return img;
        });
        return HighLevelRenderingPrimitive::Image { source: to_resource(img) };
    }

    fn rendering_variables(
        self: Pin<&Self>,
        _window: &ComponentWindow,
    ) -> SharedArray<RenderingVariable> {
        SharedArray::default()
    }

    fn layouting_info(self: Pin<&Self>, window: &ComponentWindow) -> LayoutInfo {
        //let value: i32 = Self::FIELD_OFFSETS.value.apply_pin(self).get();
        let data = Self::FIELD_OFFSETS.data.apply_pin(self).get();
        let active_controls = data.active_controls;
        let pressed = data.pressed;
        let dpr = window.scale_factor();

        let size = cpp!(unsafe [
            //value as "int",
            active_controls as "int",
            pressed as "bool",
            dpr as "float"
        ] -> qttypes::QSize as "QSize" {
            ensure_initialized();
            auto style = qApp->style();

            QStyleOptionSpinBox option;
            initQSpinBoxOptions(option, pressed, active_controls);

            auto content = option.fontMetrics.boundingRect("0000");

            return style->sizeFromContents(QStyle::CT_SpinBox, &option, content.size(), nullptr) * dpr;
        });
        LayoutInfo {
            min_width: size.width as f32,
            min_height: size.height as f32,
            max_height: size.height as f32,
            ..LayoutInfo::default()
        }
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        _window: &ComponentWindow,
        _app_component: ComponentRefPin,
    ) -> InputEventResult {
        let size: qttypes::QSize = get_size!(self);
        let mut data = Self::FIELD_OFFSETS.data.apply_pin(self).get();
        let active_controls = data.active_controls;
        let pressed = data.pressed;

        let pos = qttypes::QPoint { x: event.pos.x as _, y: event.pos.y as _ };

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

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &ComponentWindow) {}
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
    fn init(self: Pin<&Self>, _window: &ComponentWindow) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            Self::FIELD_OFFSETS.x.apply_pin(self).get(),
            Self::FIELD_OFFSETS.y.apply_pin(self).get(),
            Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            Self::FIELD_OFFSETS.height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(
        self: Pin<&Self>,
        window: &ComponentWindow,
    ) -> HighLevelRenderingPrimitive {
        let value = Self::FIELD_OFFSETS.value.apply_pin(self).get() as i32;
        let min = Self::FIELD_OFFSETS.min.apply_pin(self).get() as i32;
        let max = Self::FIELD_OFFSETS.max.apply_pin(self).get() as i32;
        let size: qttypes::QSize = get_size!(self);
        let dpr = window.scale_factor();
        let data = Self::FIELD_OFFSETS.data.apply_pin(self).get();
        let active_controls = data.active_controls;
        let pressed = data.pressed;

        let img = cpp!(unsafe [
            value as "int",
            min as "int",
            max as "int",
            size as "QSize",
            active_controls as "int",
            pressed as "bool",
            dpr as "float"
        ] -> qttypes::QImage as "QImage" {
            auto [img, rect] = offline_style_rendering_image(size, dpr);
            QPainter p(&img);
            QStyleOptionSlider option;
            option.rect = rect;
            initQSliderOptions(option, pressed, active_controls, min, max, value);
            auto style = qApp->style();
            style->drawComplexControl(QStyle::CC_Slider, &option, &p, global_widget());
            return img;
        });
        return HighLevelRenderingPrimitive::Image { source: to_resource(img) };
    }

    fn rendering_variables(
        self: Pin<&Self>,
        _window: &ComponentWindow,
    ) -> SharedArray<RenderingVariable> {
        SharedArray::default()
    }

    fn layouting_info(self: Pin<&Self>, window: &ComponentWindow) -> LayoutInfo {
        let value = Self::FIELD_OFFSETS.value.apply_pin(self).get() as i32;
        let min = Self::FIELD_OFFSETS.min.apply_pin(self).get() as i32;
        let max = Self::FIELD_OFFSETS.max.apply_pin(self).get() as i32;
        let data = Self::FIELD_OFFSETS.data.apply_pin(self).get();
        let active_controls = data.active_controls;
        let pressed = data.pressed;
        let dpr = window.scale_factor();

        let size = cpp!(unsafe [
            value as "int",
            min as "int",
            max as "int",
            active_controls as "int",
            pressed as "bool",
            dpr as "float"
        ] -> qttypes::QSize as "QSize" {
            ensure_initialized();
            QStyleOptionSlider option;
            initQSliderOptions(option, pressed, active_controls, min, max, value);
            auto style = qApp->style();
            auto thick = style->pixelMetric(QStyle::PM_SliderThickness, &option, nullptr);
            return style->sizeFromContents(QStyle::CT_Slider, &option, QSize(0, thick), nullptr) * dpr;
        });
        LayoutInfo {
            min_width: size.width as f32,
            min_height: size.height as f32,
            max_height: size.height as f32,
            ..LayoutInfo::default()
        }
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        _window: &ComponentWindow,
        _app_component: ComponentRefPin,
    ) -> InputEventResult {
        let size: qttypes::QSize = get_size!(self);
        let value = Self::FIELD_OFFSETS.value.apply_pin(self).get() as f32;
        let min = Self::FIELD_OFFSETS.min.apply_pin(self).get() as f32;
        let max = Self::FIELD_OFFSETS.max.apply_pin(self).get() as f32;
        let mut data = Self::FIELD_OFFSETS.data.apply_pin(self).get();
        let active_controls = data.active_controls;
        let pressed = data.pressed;
        let pos = qttypes::QPoint { x: event.pos.x as _, y: event.pos.y as _ };

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

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &ComponentWindow) {}
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

#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
struct GroupBoxData {
    title: Property<SharedString>,
    paddings: Property<qttypes::QMargins>,
}

impl Item for NativeGroupBox {
    fn init(self: Pin<&Self>, window: &ComponentWindow) {
        let shared_data = Rc::pin(GroupBoxData::default());

        Property::link_two_way(
            Self::FIELD_OFFSETS.title.apply_pin(self),
            GroupBoxData::FIELD_OFFSETS.title.apply_pin(shared_data.as_ref()),
        );

        shared_data.paddings.set_binding({
            let window_weak = Rc::downgrade(&window.0.clone());
            let shared_data_weak = pin_weak::rc::PinWeak::downgrade(shared_data.clone());
            move || {
                let shared_data = shared_data_weak.upgrade().unwrap();

                let text: qttypes::QString = GroupBoxData::FIELD_OFFSETS.title.apply_pin(shared_data.as_ref()).get().as_str().into();
                let dpr = window_weak.upgrade().unwrap().scale_factor();

                cpp!(unsafe [
                    text as "QString",
                    dpr as "float"
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
                        qRound((contentsRect.left() + hs) * dpr),
                        qRound((contentsRect.top() + vs) * dpr),
                        qRound((option.rect.right() - contentsRect.right() + hs) * dpr),
                        qRound((option.rect.bottom() - contentsRect.bottom() + vs) * dpr) };
                })
            }
        });

        self.native_padding_left.set_binding({
            let shared_data = shared_data.clone();
            move || {
                let margins =
                    GroupBoxData::FIELD_OFFSETS.paddings.apply_pin(shared_data.as_ref()).get();
                margins.left as _
            }
        });

        self.native_padding_right.set_binding({
            let shared_data = shared_data.clone();
            move || {
                let margins =
                    GroupBoxData::FIELD_OFFSETS.paddings.apply_pin(shared_data.as_ref()).get();
                margins.right as _
            }
        });

        self.native_padding_top.set_binding({
            let shared_data = shared_data.clone();
            move || {
                let margins =
                    GroupBoxData::FIELD_OFFSETS.paddings.apply_pin(shared_data.as_ref()).get();
                margins.top as _
            }
        });

        self.native_padding_bottom.set_binding({
            let shared_data = shared_data.clone();
            move || {
                let margins =
                    GroupBoxData::FIELD_OFFSETS.paddings.apply_pin(shared_data.as_ref()).get();
                margins.bottom as _
            }
        });
    }

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            Self::FIELD_OFFSETS.x.apply_pin(self).get(),
            Self::FIELD_OFFSETS.y.apply_pin(self).get(),
            Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            Self::FIELD_OFFSETS.height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(
        self: Pin<&Self>,
        window: &ComponentWindow,
    ) -> HighLevelRenderingPrimitive {
        let text: qttypes::QString =
            Self::FIELD_OFFSETS.title.apply_pin(self).get().as_str().into();
        let size: qttypes::QSize = get_size!(self);
        let dpr = window.scale_factor();

        let img = cpp!(unsafe [
            text as "QString",
            size as "QSize",
            dpr as "float"
        ] -> qttypes::QImage as "QImage" {
            auto [img, rect] = offline_style_rendering_image(size, dpr);
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
            qApp->style()->drawComplexControl(QStyle::CC_GroupBox, &option, &p, global_widget());
            return img;
        });
        return HighLevelRenderingPrimitive::Image { source: to_resource(img) };
    }

    fn rendering_variables(
        self: Pin<&Self>,
        _window: &ComponentWindow,
    ) -> SharedArray<RenderingVariable> {
        SharedArray::default()
    }

    fn layouting_info(self: Pin<&Self>, _window: &ComponentWindow) -> LayoutInfo {
        let left = Self::FIELD_OFFSETS.native_padding_left.apply_pin(self).get();
        let right = Self::FIELD_OFFSETS.native_padding_right.apply_pin(self).get();
        let top = Self::FIELD_OFFSETS.native_padding_top.apply_pin(self).get();
        let bottom = Self::FIELD_OFFSETS.native_padding_bottom.apply_pin(self).get();
        LayoutInfo { min_width: left + right, min_height: top + bottom, ..LayoutInfo::default() }
    }

    fn input_event(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &ComponentWindow,
        _app_component: ComponentRefPin,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &ComponentWindow) {}
}

impl ItemConsts for NativeGroupBox {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! { #[no_mangle] pub static NativeGroupBoxVTable for NativeGroupBox }

#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
pub struct NativeLineEdit {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
    pub native_padding_left: Property<f32>,
    pub native_padding_right: Property<f32>,
    pub native_padding_top: Property<f32>,
    pub native_padding_bottom: Property<f32>,
    pub focused: Property<bool>,
}

impl Item for NativeLineEdit {
    fn init(self: Pin<&Self>, _window: &ComponentWindow) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            Self::FIELD_OFFSETS.x.apply_pin(self).get(),
            Self::FIELD_OFFSETS.y.apply_pin(self).get(),
            Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            Self::FIELD_OFFSETS.height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(
        self: Pin<&Self>,
        window: &ComponentWindow,
    ) -> HighLevelRenderingPrimitive {
        let size: qttypes::QSize = get_size!(self);
        let dpr = window.scale_factor();
        let focused = Self::FIELD_OFFSETS.focused.apply_pin(self).get();

        let img = cpp!(unsafe [
            size as "QSize",
            dpr as "float",
            focused as "bool"
        ] -> qttypes::QImage as "QImage" {
            auto [img, rect] = offline_style_rendering_image(size, dpr);
            QPainter p(&img);
            QStyleOptionFrame option;
            option.rect = rect;
            option.lineWidth = 1;
            option.midLineWidth = 0;
            if (focused)
                option.state |= QStyle::State_HasFocus;
            qApp->style()->drawPrimitive(QStyle::PE_PanelLineEdit, &option, &p, global_widget());
            return img;
        });
        return HighLevelRenderingPrimitive::Image { source: to_resource(img) };
    }

    fn rendering_variables(
        self: Pin<&Self>,
        _window: &ComponentWindow,
    ) -> SharedArray<RenderingVariable> {
        SharedArray::default()
    }

    fn layouting_info(self: Pin<&Self>, window: &ComponentWindow) -> LayoutInfo {
        let dpr = window.scale_factor();

        let paddings = cpp!(unsafe [
            dpr as "float"
        ] -> qttypes::QMargins as "QMargins" {
            ensure_initialized();
            QStyleOptionFrame option;
            option.lineWidth = 1;
            option.midLineWidth = 0;
             // Just some size big enough to be sure that the frame fitst in it
            option.rect = QRect(0, 0, 10000, 10000);
            QRect contentsRect = qApp->style()->subElementRect(
                QStyle::SE_LineEditContents, &option);

            // ### remove extra margins

            return {
                qRound((2 + contentsRect.left()) * dpr),
                qRound((4 + contentsRect.top()) * dpr),
                qRound((2 + option.rect.right() - contentsRect.right()) * dpr),
                qRound((4 + option.rect.bottom() - contentsRect.bottom()) * dpr) };
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

    fn input_event(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &ComponentWindow,
        _app_component: ComponentRefPin,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &ComponentWindow) {}
}

impl ItemConsts for NativeLineEdit {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! { #[no_mangle] pub static NativeLineEditVTable for NativeLineEdit }

#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
pub struct NativeScrollBar {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub max: Property<f32>,
    pub page_size: Property<f32>,
    pub value: Property<f32>,
    pub horizontal: Property<bool>,
    pub cached_rendering_data: CachedRenderingData,
    data: Property<NativeSliderData>,
}

impl Item for NativeScrollBar {
    fn init(self: Pin<&Self>, _window: &ComponentWindow) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            Self::FIELD_OFFSETS.x.apply_pin(self).get(),
            Self::FIELD_OFFSETS.y.apply_pin(self).get(),
            Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            Self::FIELD_OFFSETS.height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(
        self: Pin<&Self>,
        window: &ComponentWindow,
    ) -> HighLevelRenderingPrimitive {
        let size: qttypes::QSize = get_size!(self);
        let dpr = window.scale_factor();
        let value = Self::FIELD_OFFSETS.value.apply_pin(self).get() as i32;
        let max = (Self::FIELD_OFFSETS.max.apply_pin(self).get() as i32).max(0);
        let page_size = Self::FIELD_OFFSETS.page_size.apply_pin(self).get() as i32;
        let horizontal: bool = Self::FIELD_OFFSETS.horizontal.apply_pin(self).get();
        let data = Self::FIELD_OFFSETS.data.apply_pin(self).get();
        let active_controls = data.active_controls;
        let pressed = data.pressed;

        let img = cpp!(unsafe [
            value as "int",
            page_size as "int",
            max as "int",
            size as "QSize",
            active_controls as "int",
            pressed as "bool",
            dpr as "float",
            horizontal as "bool"
        ] -> qttypes::QImage as "QImage" {
            auto [img, rect] = offline_style_rendering_image(size, dpr);
            QPainter p(&img);
            QStyleOptionSlider option;
            option.rect = rect;
            initQSliderOptions(option, pressed, active_controls, 0, max / dpr, value / dpr);
            option.pageStep = page_size / dpr;

            if (!horizontal) {
                option.state ^= QStyle::State_Horizontal;
                option.orientation = Qt::Vertical;
            }

            auto style = qApp->style();
            style->drawComplexControl(QStyle::CC_Slider, &option, &p, global_widget());
            return img;
        });
        return HighLevelRenderingPrimitive::Image { source: to_resource(img) };
    }

    fn rendering_variables(
        self: Pin<&Self>,
        _window: &ComponentWindow,
    ) -> SharedArray<RenderingVariable> {
        SharedArray::default()
    }

    fn layouting_info(self: Pin<&Self>, window: &ComponentWindow) -> LayoutInfo {
        let dpr = window.scale_factor();
        let horizontal: bool = Self::FIELD_OFFSETS.horizontal.apply_pin(self).get();

        let s = cpp!(unsafe [
            horizontal as "bool"
        ] -> qttypes::QSize as "QSize" {
            ensure_initialized();

            QStyleOptionSlider option;
            // int overlap = qApp->style()->pixelMetric(QStyle::PM_ScrollView_ScrollBarOverlap, &option, global_widget());

            initQSliderOptions(option, false, 0, 0, 1000, 1000);
            if (!horizontal) {
                option.state ^= QStyle::State_Horizontal;
                option.orientation = Qt::Vertical;
            }
            int extent = qApp->style()->pixelMetric(QStyle::PM_ScrollBarExtent, &option, global_widget());
            int sliderMin = qApp->style()->pixelMetric(QStyle::PM_ScrollBarSliderMin, &option, global_widget());
            auto csize = horizontal ? QSize(extent * 2 + sliderMin, extent) : QSize(extent,extent * 2 + sliderMin) ;
            return qApp->style()->sizeFromContents(QStyle::CT_ScrollBar, &option, csize, global_widget());
        });
        let mut result = LayoutInfo {
            min_width: s.width as f32 * dpr,
            min_height: s.height as f32 * dpr,
            ..LayoutInfo::default()
        };
        if horizontal {
            result.max_height = result.min_height;
        } else {
            result.max_width = result.min_width;
        }
        result
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        window: &ComponentWindow,
        _app_component: ComponentRefPin,
    ) -> InputEventResult {
        let dpr = window.scale_factor();
        let pos = qttypes::QPoint { x: (event.pos.x / dpr) as _, y: (event.pos.y / dpr) as _ };
        let size: qttypes::QSize = get_size!(self);
        let value = Self::FIELD_OFFSETS.value.apply_pin(self).get() as i32;
        let max = (Self::FIELD_OFFSETS.max.apply_pin(self).get() as i32).max(0);
        let page_size = Self::FIELD_OFFSETS.page_size.apply_pin(self).get() as i32;
        let horizontal: bool = Self::FIELD_OFFSETS.horizontal.apply_pin(self).get();
        let mut data = Self::FIELD_OFFSETS.data.apply_pin(self).get();
        let active_controls = data.active_controls;
        let pressed = data.pressed;

        let new_control = cpp!(unsafe [
            pos as "QPoint",
            value as "int",
            page_size as "int",
            max as "int",
            size as "QSize",
            active_controls as "int",
            pressed as "bool",
            dpr as "float",
            horizontal as "bool"
        ] -> u32 as "int" {
            ensure_initialized();
            QStyleOptionSlider option;
            initQSliderOptions(option, pressed, active_controls, 0, max / dpr, value / dpr);
            option.pageStep = page_size / dpr;
            if (!horizontal) {
                option.state ^= QStyle::State_Horizontal;
                option.orientation = Qt::Vertical;
            }
            auto style = qApp->style();
            option.rect = { QPoint{}, size / dpr };
            return style->hitTestComplexControl(QStyle::CC_ScrollBar, &option, pos, nullptr);
        });

        #[allow(non_snake_case)]
        let SC_ScrollBarSlider =
            cpp!(unsafe []->u32 as "int" { return QStyle::SC_ScrollBarSlider;});

        let (pos, size) =
            if horizontal { (event.pos.x, size.width) } else { (event.pos.y, size.height) };

        let result = match event.what {
            MouseEventType::MousePressed => {
                data.pressed = true;
                if new_control == SC_ScrollBarSlider {
                    data.pressed_x = pos as f32;
                    data.pressed_val = value as f32;
                }
                data.active_controls = new_control;
                InputEventResult::GrabMouse
            }
            MouseEventType::MouseExit => {
                data.pressed = false;
                InputEventResult::EventIgnored
            }
            MouseEventType::MouseReleased => {
                data.pressed = false;
                let new_val = cpp!(unsafe [active_controls as "int", value as "int", max as "int", page_size as "int", dpr as "float"] -> i32 as "int" {
                    switch (active_controls) {
                        case QStyle::SC_ScrollBarAddPage:
                            return value + page_size;
                        case QStyle::SC_ScrollBarSubPage:
                            return value - page_size;
                        case QStyle::SC_ScrollBarAddLine:
                            return value + dpr;
                        case QStyle::SC_ScrollBarSubLine:
                            return value - dpr;
                        case QStyle::SC_ScrollBarFirst:
                            return 0;
                        case QStyle::SC_ScrollBarLast:
                            return max;
                        default:
                            return value;
                    }
                });
                self.value.set(new_val.max(0).min(max) as f32);
                InputEventResult::EventIgnored
            }
            MouseEventType::MouseMoved => {
                if data.pressed && data.active_controls == SC_ScrollBarSlider {
                    let max = max as f32;
                    let new_val =
                        data.pressed_val + ((pos as f32) - data.pressed_x) * max / size as f32;
                    self.value.set(new_val.max(0.).min(max));
                    InputEventResult::GrabMouse
                } else {
                    InputEventResult::EventAccepted
                }
            }
        };
        self.data.set(data);
        result
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &ComponentWindow) {}
}

impl ItemConsts for NativeScrollBar {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! { #[no_mangle] pub static NativeScrollBarVTable for NativeScrollBar }

#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
pub struct NativeStandardListViewItem {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub item: Property<sixtyfps_corelib::model::StandardListViewItem>,
    pub index: Property<i32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for NativeStandardListViewItem {
    fn init(self: Pin<&Self>, _window: &ComponentWindow) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(
            Self::FIELD_OFFSETS.x.apply_pin(self).get(),
            Self::FIELD_OFFSETS.y.apply_pin(self).get(),
            Self::FIELD_OFFSETS.width.apply_pin(self).get(),
            Self::FIELD_OFFSETS.height.apply_pin(self).get(),
        )
    }
    fn rendering_primitive(
        self: Pin<&Self>,
        window: &ComponentWindow,
    ) -> HighLevelRenderingPrimitive {
        let size: qttypes::QSize = get_size!(self);
        let dpr = window.scale_factor();
        let index: i32 = Self::FIELD_OFFSETS.index.apply_pin(self).get();
        let item = Self::FIELD_OFFSETS.item.apply_pin(self).get();
        let text: qttypes::QString = item.text.as_str().into();

        let img = cpp!(unsafe [
            size as "QSize",
            dpr as "float",
            index as "int",
            text as "QString"
        ] -> qttypes::QImage as "QImage" {
            auto [img, rect] = offline_style_rendering_image(size, dpr);
            QPainter p(&img);
            QStyleOptionViewItem option;
            option.rect = rect;
            option.state = QStyle::State_Enabled | QStyle::State_Active;
            option.decorationPosition = QStyleOptionViewItem::Left;
            option.decorationAlignment = Qt::AlignCenter;
            option.displayAlignment = Qt::AlignLeft|Qt::AlignVCenter;
            option.showDecorationSelected = qApp->style()->styleHint(QStyle::SH_ItemView_ShowDecorationSelected, nullptr, global_widget());
            if (index % 2) {
                option.features |= QStyleOptionViewItem::Alternate;
            }
            option.features |= QStyleOptionViewItem::HasDisplay;
            option.text = text;
            qApp->style()->drawPrimitive(QStyle::PE_PanelItemViewRow, &option, &p, global_widget());
            qApp->style()->drawControl(QStyle::CE_ItemViewItem, &option, &p, global_widget());
            return img;
        });
        return HighLevelRenderingPrimitive::Image { source: to_resource(img) };
    }

    fn rendering_variables(
        self: Pin<&Self>,
        _window: &ComponentWindow,
    ) -> SharedArray<RenderingVariable> {
        SharedArray::default()
    }

    fn layouting_info(self: Pin<&Self>, window: &ComponentWindow) -> LayoutInfo {
        let dpr = window.scale_factor();
        let index: i32 = Self::FIELD_OFFSETS.index.apply_pin(self).get();
        let item = Self::FIELD_OFFSETS.item.apply_pin(self).get();
        let text: qttypes::QString = item.text.as_str().into();

        let s = cpp!(unsafe [
            index as "int",
            text as "QString"
        ] -> qttypes::QSize as "QSize" {
            ensure_initialized();

            QStyleOptionViewItem option;
            option.decorationPosition = QStyleOptionViewItem::Left;
            option.decorationAlignment = Qt::AlignCenter;
            option.displayAlignment = Qt::AlignLeft|Qt::AlignVCenter;
            option.showDecorationSelected = qApp->style()->styleHint(QStyle::SH_ItemView_ShowDecorationSelected, nullptr, global_widget());
            if (index % 2) {
                option.features |= QStyleOptionViewItem::Alternate;
            }
            option.features |= QStyleOptionViewItem::HasDisplay;
            option.text = text;
            return qApp->style()->sizeFromContents(QStyle::CT_ItemViewItem, &option, QSize{}, global_widget());
        });
        let result = LayoutInfo {
            min_width: s.width as f32 * dpr,
            min_height: s.height as f32 * dpr,
            ..LayoutInfo::default()
        };
        result
    }

    fn input_event(
        self: Pin<&Self>,
        _event: MouseEvent,
        _window: &ComponentWindow,
        _app_component: ComponentRefPin,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &ComponentWindow) {}
}

impl ItemConsts for NativeStandardListViewItem {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! { #[no_mangle] pub static NativeStandardListViewItemVTable for NativeStandardListViewItem }
