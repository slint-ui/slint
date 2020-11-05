/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

/*!

This module contains all the native Qt widgetimplementation that forwards to QStyle.

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

struct QImageWrapArray {
    /// The image reference the array, so the array must outlive the image without being detached or accessed
    img: qttypes::QImage,
    array: SharedArray<u32>,
}

impl QImageWrapArray {
    pub fn new(size: qttypes::QSize, dpr: f32) -> Self {
        let mut array = SharedArray::default();
        array.resize((size.width * size.height) as usize, 0u32);
        let array_ptr = array.as_slice_mut().as_mut_ptr();
        let img = cpp!(unsafe [size as "QSize", array_ptr as "uchar*", dpr as "float"] -> qttypes::QImage as "QImage" {
            QImage img(array_ptr, size.width(), size.height(), size.width() * 4, QImage::Format_ARGB32_Premultiplied);
            img.setDevicePixelRatio(dpr);
            return img;
        });
        QImageWrapArray { img, array }
    }

    pub fn to_resource(self) -> Resource {
        let size = self.img.size();
        drop(self.img);
        Resource::EmbeddedRgbaImage { width: size.width, height: size.height, data: self.array }
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
    pub enabled: Property<bool>,
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
        let enabled = Self::FIELD_OFFSETS.enabled.apply_pin(self).get();
        let size: qttypes::QSize = get_size!(self);
        let dpr = window.scale_factor();

        let mut imgarray = QImageWrapArray::new(size, dpr);
        let img = &mut imgarray.img;

        cpp!(unsafe [
            img as "QImage*",
            text as "QString",
            enabled as "bool",
            size as "QSize",
            down as "bool",
            dpr as "float"
        ] {
            QPainter p(img);
            QStyleOptionButton option;
            option.text = std::move(text);
            option.rect = QRect(QPoint(), size / dpr);
            if (down)
                option.state |= QStyle::State_Sunken;
            else
                option.state |= QStyle::State_Raised;
            if (enabled)
                option.state |= QStyle::State_Enabled;
            qApp->style()->drawControl(QStyle::CE_PushButton, &option, &p, nullptr);
        });
        return HighLevelRenderingPrimitive::Image { source: imgarray.to_resource() };
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
            ..LayoutInfo::default()
        }
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        _window: &ComponentWindow,
        _app_component: ComponentRefPin,
    ) -> InputEventResult {
        let enabled = Self::FIELD_OFFSETS.enabled.apply_pin(self).get();
        if !enabled {
            return InputEventResult::EventIgnored;
        }

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
    pub enabled: Property<bool>,
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
        let enabled = Self::FIELD_OFFSETS.enabled.apply_pin(self).get();
        let text: qttypes::QString = Self::FIELD_OFFSETS.text.apply_pin(self).get().as_str().into();
        let size: qttypes::QSize = get_size!(self);
        let dpr = window.scale_factor();

        let mut imgarray = QImageWrapArray::new(size, dpr);
        let img = &mut imgarray.img;

        cpp!(unsafe [
            img as "QImage*",
            enabled as "bool",
            text as "QString",
            size as "QSize",
            checked as "bool",
            dpr as "float"
        ] {
            QPainter p(img);
            QStyleOptionButton option;
            option.text = std::move(text);
            option.rect = QRect(QPoint(), size / dpr);
            option.state |= checked ? QStyle::State_On : QStyle::State_Off;
            if (enabled)
                option.state |= QStyle::State_Enabled;
            qApp->style()->drawControl(QStyle::CE_CheckBox, &option, &p, nullptr);
        });
        return HighLevelRenderingPrimitive::Image { source: imgarray.to_resource() };
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
            return qApp->style()->sizeFromContents(QStyle::CT_CheckBox, &option, option.rect.size(), nullptr) * dpr;
        });
        LayoutInfo {
            min_width: size.width as f32,
            min_height: size.height as f32,
            max_height: size.height as f32,
            horizontal_stretch: 1.,
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

#[derive(Default, Copy, Clone, Debug, PartialEq)]
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
    pub enabled: Property<bool>,
    pub value: Property<i32>,
    pub cached_rendering_data: CachedRenderingData,
    data: Property<NativeSpinBoxData>,
}

cpp! {{
void initQSpinBoxOptions(QStyleOptionSpinBox &option, bool pressed, bool enabled, int active_controls) {
    auto style = qApp->style();
    option.activeSubControls = QStyle::SC_None;
    option.subControls = QStyle::SC_SpinBoxEditField | QStyle::SC_SpinBoxUp | QStyle::SC_SpinBoxDown;
    if (style->styleHint(QStyle::SH_SpinBox_ButtonsInsideFrame, nullptr, nullptr))
        option.subControls |= QStyle::SC_SpinBoxFrame;
    option.activeSubControls = {active_controls};
    if (enabled)
        option.state |= QStyle::State_Enabled;
    option.state |= QStyle::State_Active;
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
        let enabled = Self::FIELD_OFFSETS.enabled.apply_pin(self).get();
        let size: qttypes::QSize = get_size!(self);
        let dpr = window.scale_factor();
        let data = Self::FIELD_OFFSETS.data.apply_pin(self).get();
        let active_controls = data.active_controls;
        let pressed = data.pressed;

        let mut imgarray = QImageWrapArray::new(size, dpr);
        let img = &mut imgarray.img;
        cpp!(unsafe [
            img as "QImage*",
            value as "int",
            enabled as "bool",
            size as "QSize",
            active_controls as "int",
            pressed as "bool",
            dpr as "float"
        ] {
            QPainter p(img);
            auto style = qApp->style();
            QStyleOptionSpinBox option;
            option.rect = QRect(QPoint(), size / dpr);
            initQSpinBoxOptions(option, pressed, enabled, active_controls);
            style->drawComplexControl(QStyle::CC_SpinBox, &option, &p, nullptr);

            auto text_rect = style->subControlRect(QStyle::CC_SpinBox, &option, QStyle::SC_SpinBoxEditField, nullptr);
            p.drawText(text_rect, QString::number(value));
        });
        return HighLevelRenderingPrimitive::Image { source: imgarray.to_resource() };
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
        let enabled = Self::FIELD_OFFSETS.enabled.apply_pin(self).get();
        let dpr = window.scale_factor();

        let size = cpp!(unsafe [
            //value as "int",
            active_controls as "int",
            pressed as "bool",
            enabled as "bool",
            dpr as "float"
        ] -> qttypes::QSize as "QSize" {
            ensure_initialized();
            auto style = qApp->style();

            QStyleOptionSpinBox option;
            initQSpinBoxOptions(option, pressed, enabled, active_controls);

            auto content = option.fontMetrics.boundingRect("0000");

            return style->sizeFromContents(QStyle::CT_SpinBox, &option, content.size(), nullptr) * dpr;
        });
        LayoutInfo {
            min_width: size.width as f32,
            min_height: size.height as f32,
            max_height: size.height as f32,
            horizontal_stretch: 1.,
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
        let enabled = Self::FIELD_OFFSETS.enabled.apply_pin(self).get();
        let mut data = Self::FIELD_OFFSETS.data.apply_pin(self).get();
        let active_controls = data.active_controls;
        let pressed = data.pressed;

        let pos = qttypes::QPoint { x: event.pos.x as _, y: event.pos.y as _ };

        let new_control = cpp!(unsafe [
            pos as "QPoint",
            size as "QSize",
            enabled as "bool",
            active_controls as "int",
            pressed as "bool"
        ] -> u32 as "int" {
            ensure_initialized();
            auto style = qApp->style();

            QStyleOptionSpinBox option;
            option.rect = { QPoint{}, size };
            initQSpinBoxOptions(option, pressed, enabled, active_controls);

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

#[derive(Default, Copy, Clone, Debug, PartialEq)]
#[repr(C)]
struct NativeSliderData {
    active_controls: u32,
    /// For sliders, this is a bool, For scroll area: 1 == horizontal, 2 == vertical
    pressed: u8,
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
    pub enabled: Property<bool>,
    pub value: Property<f32>,
    pub min: Property<f32>,
    pub max: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
    data: Property<NativeSliderData>,
}

cpp! {{
void initQSliderOptions(QStyleOptionSlider &option, bool pressed, bool enabled, int active_controls, int minimum, int maximum, int value) {
    option.subControls = QStyle::SC_SliderGroove | QStyle::SC_SliderHandle;
    option.activeSubControls = { active_controls };
    option.orientation = Qt::Horizontal;
    option.maximum = maximum;
    option.minimum = minimum;
    option.sliderPosition = value;
    option.sliderValue = value;
    if (enabled)
        option.state |= QStyle::State_Enabled;
    option.state |= QStyle::State_Active | QStyle::State_Horizontal;
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
        let enabled = Self::FIELD_OFFSETS.enabled.apply_pin(self).get();
        let value = Self::FIELD_OFFSETS.value.apply_pin(self).get() as i32;
        let min = Self::FIELD_OFFSETS.min.apply_pin(self).get() as i32;
        let max = Self::FIELD_OFFSETS.max.apply_pin(self).get() as i32;
        let size: qttypes::QSize = get_size!(self);
        let dpr = window.scale_factor();
        let data = Self::FIELD_OFFSETS.data.apply_pin(self).get();
        let active_controls = data.active_controls;
        let pressed = data.pressed;

        let mut imgarray = QImageWrapArray::new(size, dpr);
        let img = &mut imgarray.img;

        cpp!(unsafe [
            img as "QImage*",
            enabled as "bool",
            value as "int",
            min as "int",
            max as "int",
            size as "QSize",
            active_controls as "int",
            pressed as "bool",
            dpr as "float"
        ] {
            QPainter p(img);
            QStyleOptionSlider option;
            option.rect = QRect(QPoint(), size / dpr);
            initQSliderOptions(option, pressed, enabled, active_controls, min, max, value);
            auto style = qApp->style();
            style->drawComplexControl(QStyle::CC_Slider, &option, &p, nullptr);
        });
        return HighLevelRenderingPrimitive::Image { source: imgarray.to_resource() };
    }

    fn rendering_variables(
        self: Pin<&Self>,
        _window: &ComponentWindow,
    ) -> SharedArray<RenderingVariable> {
        SharedArray::default()
    }

    fn layouting_info(self: Pin<&Self>, window: &ComponentWindow) -> LayoutInfo {
        let enabled = Self::FIELD_OFFSETS.enabled.apply_pin(self).get();
        let value = Self::FIELD_OFFSETS.value.apply_pin(self).get() as i32;
        let min = Self::FIELD_OFFSETS.min.apply_pin(self).get() as i32;
        let max = Self::FIELD_OFFSETS.max.apply_pin(self).get() as i32;
        let data = Self::FIELD_OFFSETS.data.apply_pin(self).get();
        let active_controls = data.active_controls;
        let pressed = data.pressed;
        let dpr = window.scale_factor();

        let size = cpp!(unsafe [
            enabled as "bool",
            value as "int",
            min as "int",
            max as "int",
            active_controls as "int",
            pressed as "bool",
            dpr as "float"
        ] -> qttypes::QSize as "QSize" {
            ensure_initialized();
            QStyleOptionSlider option;
            initQSliderOptions(option, pressed, enabled, active_controls, min, max, value);
            auto style = qApp->style();
            auto thick = style->pixelMetric(QStyle::PM_SliderThickness, &option, nullptr);
            return style->sizeFromContents(QStyle::CT_Slider, &option, QSize(0, thick), nullptr) * dpr;
        });
        LayoutInfo {
            min_width: size.width as f32,
            min_height: size.height as f32,
            max_height: size.height as f32,
            horizontal_stretch: 1.,
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
        let enabled = Self::FIELD_OFFSETS.enabled.apply_pin(self).get();
        let value = Self::FIELD_OFFSETS.value.apply_pin(self).get() as f32;
        let min = Self::FIELD_OFFSETS.min.apply_pin(self).get() as f32;
        let max = Self::FIELD_OFFSETS.max.apply_pin(self).get() as f32;
        let mut data = Self::FIELD_OFFSETS.data.apply_pin(self).get();
        let active_controls = data.active_controls;
        let pressed: bool = data.pressed != 0;
        let pos = qttypes::QPoint { x: event.pos.x as _, y: event.pos.y as _ };

        let new_control = cpp!(unsafe [
            pos as "QPoint",
            size as "QSize",
            enabled as "bool",
            value as "float",
            min as "float",
            max as "float",
            active_controls as "int",
            pressed as "bool"
        ] -> u32 as "int" {
            ensure_initialized();
            QStyleOptionSlider option;
            initQSliderOptions(option, pressed, enabled, active_controls, min, max, value);
            auto style = qApp->style();
            option.rect = { QPoint{}, size };
            return style->hitTestComplexControl(QStyle::CC_Slider, &option, pos, nullptr);
        });
        let result = match event.what {
            MouseEventType::MousePressed => {
                data.pressed_x = event.pos.x as f32;
                data.pressed = 1;
                data.pressed_val = value;
                InputEventResult::GrabMouse
            }
            MouseEventType::MouseExit | MouseEventType::MouseReleased => {
                data.pressed = 0;
                InputEventResult::EventAccepted
            }
            MouseEventType::MouseMoved => {
                if data.pressed != 0 {
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
    pub enabled: Property<bool>,
    pub title: Property<SharedString>,
    pub cached_rendering_data: CachedRenderingData,
    pub native_padding_left: Property<f32>,
    pub native_padding_right: Property<f32>,
    pub native_padding_top: Property<f32>,
    pub native_padding_bottom: Property<f32>,
}

#[repr(C)]
#[derive(FieldOffsets, Default)]
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
        let enabled = Self::FIELD_OFFSETS.enabled.apply_pin(self).get();
        let size: qttypes::QSize = get_size!(self);
        let dpr = window.scale_factor();

        let mut imgarray = QImageWrapArray::new(size, dpr);
        let img = &mut imgarray.img;

        cpp!(unsafe [
            img as "QImage*",
            text as "QString",
            enabled as "bool",
            size as "QSize",
            dpr as "float"
        ] {
            QPainter p(img);
            QStyleOptionGroupBox option;
            if (enabled)
                option.state |= QStyle::State_Enabled;
            option.rect = QRect(QPoint(), size / dpr);
            option.text = text;
            option.lineWidth = 1;
            option.midLineWidth = 0;
            option.subControls = QStyle::SC_GroupBoxFrame;
            if (!text.isEmpty()) {
                option.subControls |= QStyle::SC_GroupBoxLabel;
            }
            option.textColor = QColor(qApp->style()->styleHint(
                QStyle::SH_GroupBox_TextLabelColor, &option));
            qApp->style()->drawComplexControl(QStyle::CC_GroupBox, &option, &p, nullptr);
        });
        return HighLevelRenderingPrimitive::Image { source: imgarray.to_resource() };
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
        LayoutInfo {
            min_width: left + right,
            min_height: top + bottom,
            horizontal_stretch: 1.,
            vertical_stretch: 1.,
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
    pub enabled: Property<bool>,
}

impl Item for NativeLineEdit {
    fn init(self: Pin<&Self>, window: &ComponentWindow) {
        let paddings = Rc::pin(Property::default());

        paddings.as_ref().set_binding({
            let window_weak = Rc::downgrade(&window.0.clone());
            move || {
                let dpr = window_weak.upgrade().unwrap().scale_factor();

                cpp!(unsafe [
                    dpr as "float"
                ] -> qttypes::QMargins as "QMargins" {
                    ensure_initialized();
                    QStyleOptionFrame option;
                    option.state |= QStyle::State_Enabled;
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
                })
            }
        });

        self.native_padding_left.set_binding({
            let paddings = paddings.clone();
            move || paddings.as_ref().get().left as _
        });
        self.native_padding_right.set_binding({
            let paddings = paddings.clone();
            move || paddings.as_ref().get().right as _
        });
        self.native_padding_top.set_binding({
            let paddings = paddings.clone();
            move || paddings.as_ref().get().top as _
        });
        self.native_padding_bottom.set_binding({
            let paddings = paddings.clone();
            move || paddings.as_ref().get().bottom as _
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
        let size: qttypes::QSize = get_size!(self);
        let dpr = window.scale_factor();
        let focused: bool = Self::FIELD_OFFSETS.focused.apply_pin(self).get();
        let enabled: bool = Self::FIELD_OFFSETS.enabled.apply_pin(self).get();

        let mut imgarray = QImageWrapArray::new(size, dpr);
        let img = &mut imgarray.img;

        cpp!(unsafe [
            img as "QImage*",
            size as "QSize",
            dpr as "float",
            enabled as "bool",
            focused as "bool"
        ] {
            QPainter p(img);
            QStyleOptionFrame option;
            option.rect = QRect(QPoint(), size / dpr);
            option.lineWidth = 1;
            option.midLineWidth = 0;
            if (focused)
                option.state |= QStyle::State_HasFocus;
            if (enabled)
                option.state |= QStyle::State_Enabled;
            qApp->style()->drawPrimitive(QStyle::PE_PanelLineEdit, &option, &p, nullptr);
        });
        return HighLevelRenderingPrimitive::Image { source: imgarray.to_resource() };
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
        LayoutInfo {
            min_width: left + right,
            min_height: top + bottom,
            horizontal_stretch: 1.,
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
pub struct NativeScrollView {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub horizontal_max: Property<f32>,
    pub horizontal_page_size: Property<f32>,
    pub horizontal_value: Property<f32>,
    pub vertical_max: Property<f32>,
    pub vertical_page_size: Property<f32>,
    pub vertical_value: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
    pub native_padding_left: Property<f32>,
    pub native_padding_right: Property<f32>,
    pub native_padding_top: Property<f32>,
    pub native_padding_bottom: Property<f32>,
    data: Property<NativeSliderData>,
}

impl Item for NativeScrollView {
    fn init(self: Pin<&Self>, window: &ComponentWindow) {
        let paddings = Rc::pin(Property::default());

        paddings.as_ref().set_binding({
            let window_weak = Rc::downgrade(&window.0.clone());
            move || {
                let dpr = window_weak.upgrade().unwrap().scale_factor();

                cpp!(unsafe [
                    dpr as "float"
                ] -> qttypes::QMargins as "QMargins" {
                    ensure_initialized();
                    QStyleOptionSlider option;
                    initQSliderOptions(option, false, true, 0, 0, 1000, 1000);

                    int extent = qApp->style()->pixelMetric(QStyle::PM_ScrollBarExtent, &option, nullptr);
                    int sliderMin = qApp->style()->pixelMetric(QStyle::PM_ScrollBarSliderMin, &option, nullptr);
                    auto horizontal_size = qApp->style()->sizeFromContents(QStyle::CT_ScrollBar, &option, QSize(extent * 2 + sliderMin, extent), nullptr);
                    option.state ^= QStyle::State_Horizontal;
                    option.orientation = Qt::Vertical;
                    extent = qApp->style()->pixelMetric(QStyle::PM_ScrollBarExtent, &option, nullptr);
                    sliderMin = qApp->style()->pixelMetric(QStyle::PM_ScrollBarSliderMin, &option, nullptr);
                    auto vertical_size = qApp->style()->sizeFromContents(QStyle::CT_ScrollBar, &option, QSize(extent, extent * 2 + sliderMin), nullptr);

                    /*int hscrollOverlap = hbar->style()->pixelMetric(QStyle::PM_ScrollView_ScrollBarOverlap, &opt, hbar);
                    int vscrollOverlap = vbar->style()->pixelMetric(QStyle::PM_ScrollView_ScrollBarOverlap, &opt, vbar);*/

                    QStyleOptionFrame frameOption;
                    frameOption.rect = QRect(QPoint(), QSize(1000, 1000));
                    frameOption.frameShape = QFrame::StyledPanel;
                    frameOption.lineWidth = 1;
                    frameOption.midLineWidth = 0;
                    QRect cr = qApp->style()->subElementRect(QStyle::SE_ShapedFrameContents, &frameOption, nullptr);
                    return {
                        qRound(cr.left() * dpr),
                        qRound(cr.top() * dpr),
                        qRound((vertical_size.width() + frameOption.rect.right() - cr.right()) * dpr),
                        qRound((horizontal_size.height() + frameOption.rect.bottom() - cr.bottom()) * dpr) };
                })
            }
        });

        self.native_padding_left.set_binding({
            let paddings = paddings.clone();
            move || paddings.as_ref().get().left as _
        });
        self.native_padding_right.set_binding({
            let paddings = paddings.clone();
            move || paddings.as_ref().get().right as _
        });
        self.native_padding_top.set_binding({
            let paddings = paddings.clone();
            move || paddings.as_ref().get().top as _
        });
        self.native_padding_bottom.set_binding({
            let paddings = paddings.clone();
            move || paddings.as_ref().get().bottom as _
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
        let size: qttypes::QSize = get_size!(self);
        let dpr = window.scale_factor();

        let mut imgarray = QImageWrapArray::new(size, dpr);

        let data = Self::FIELD_OFFSETS.data.apply_pin(self).get();
        let left = Self::FIELD_OFFSETS.native_padding_left.apply_pin(self).get();
        let right = Self::FIELD_OFFSETS.native_padding_right.apply_pin(self).get();
        let top = Self::FIELD_OFFSETS.native_padding_top.apply_pin(self).get();
        let bottom = Self::FIELD_OFFSETS.native_padding_bottom.apply_pin(self).get();
        let corner_rect = qttypes::QRectF {
            x: ((size.width as f32 - (right - left)) / dpr) as _,
            y: ((size.height as f32 - (bottom - top)) / dpr) as _,
            width: ((right - left) / dpr) as _,
            height: ((bottom - top) / dpr) as _,
        };
        let img: &mut qttypes::QImage = &mut imgarray.img;
        cpp!(unsafe [img as "QImage*", corner_rect as "QRectF"] {
            ensure_initialized();
            QStyleOptionFrame frameOption;
            frameOption.frameShape = QFrame::StyledPanel;
            frameOption.lineWidth = 1;
            frameOption.midLineWidth = 0;
            frameOption.rect = corner_rect.toAlignedRect();
            QPainter p(img);
            qApp->style()->drawPrimitive(QStyle::PE_PanelScrollAreaCorner, &frameOption, &p, nullptr);
            frameOption.rect = QRect(QPoint(), corner_rect.toAlignedRect().topLeft());
            qApp->style()->drawControl(QStyle::CE_ShapedFrame, &frameOption, &p, nullptr);
        });

        let draw_scrollbar = |horizontal: bool,
                              rect: qttypes::QRectF,
                              value: i32,
                              page_size: i32,
                              max: i32,
                              active_controls: u32,
                              pressed: bool| {
            cpp!(unsafe [
                img as "QImage*",
                value as "int",
                page_size as "int",
                max as "int",
                rect as "QRectF",
                active_controls as "int",
                pressed as "bool",
                dpr as "float",
                horizontal as "bool"
            ] {
                auto r = rect.toAlignedRect();
                // The mac style ignores painter translations (due to CGContextRef redirection) as well as
                // option.rect's top-left - hence this hack with an intermediate buffer.
            #if defined(Q_OS_MAC)
                QImage scrollbar_image(r.size(), QImage::Format_ARGB32_Premultiplied);
                scrollbar_image.fill(Qt::transparent);
                QPainter p(&scrollbar_image);
            #else
                QPainter p(img);
                p.translate(r.topLeft()); // There is bugs in the styles if the scrollbar is not in (0,0)
            #endif
                QStyleOptionSlider option;
                option.rect = QRect(QPoint(), r.size());
                initQSliderOptions(option, pressed, true, active_controls, 0, max / dpr, -value / dpr);
                option.subControls = QStyle::SC_All;
                option.pageStep = page_size / dpr;

                if (!horizontal) {
                    option.state ^= QStyle::State_Horizontal;
                    option.orientation = Qt::Vertical;
                }

                auto style = qApp->style();
                style->drawComplexControl(QStyle::CC_ScrollBar, &option, &p, nullptr);
                p.end();
            #if defined(Q_OS_MAC)
                p.begin(img);
                p.drawImage(r.topLeft(), scrollbar_image);
            #endif
            });
        };

        draw_scrollbar(
            false,
            qttypes::QRectF {
                x: ((size.width as f32 - right + left) / dpr) as _,
                y: 0.,
                width: ((right - left) / dpr) as _,
                height: ((size.height as f32 - bottom + top) / dpr) as _,
            },
            Self::FIELD_OFFSETS.vertical_value.apply_pin(self).get() as i32,
            Self::FIELD_OFFSETS.vertical_page_size.apply_pin(self).get() as i32,
            Self::FIELD_OFFSETS.vertical_max.apply_pin(self).get() as i32,
            data.active_controls,
            data.pressed == 2,
        );
        draw_scrollbar(
            true,
            qttypes::QRectF {
                x: 0.,
                y: ((size.height as f32 - bottom + top) / dpr) as _,
                width: ((size.width as f32 - right + left) / dpr) as _,
                height: ((bottom - top) / dpr) as _,
            },
            Self::FIELD_OFFSETS.horizontal_value.apply_pin(self).get() as i32,
            Self::FIELD_OFFSETS.horizontal_page_size.apply_pin(self).get() as i32,
            Self::FIELD_OFFSETS.horizontal_max.apply_pin(self).get() as i32,
            data.active_controls,
            data.pressed == 1,
        );

        return HighLevelRenderingPrimitive::Image { source: imgarray.to_resource() };
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
        LayoutInfo {
            min_width: left + right,
            min_height: top + bottom,
            horizontal_stretch: 1.,
            vertical_stretch: 1.,
            ..LayoutInfo::default()
        }
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        window: &ComponentWindow,
        _app_component: ComponentRefPin,
    ) -> InputEventResult {
        let dpr = window.scale_factor();
        let size: qttypes::QSize = get_size!(self);
        let mut data = Self::FIELD_OFFSETS.data.apply_pin(self).get();
        let active_controls = data.active_controls;
        let pressed = data.pressed;
        let left = Self::FIELD_OFFSETS.native_padding_left.apply_pin(self).get();
        let right = Self::FIELD_OFFSETS.native_padding_right.apply_pin(self).get();
        let top = Self::FIELD_OFFSETS.native_padding_top.apply_pin(self).get();
        let bottom = Self::FIELD_OFFSETS.native_padding_bottom.apply_pin(self).get();

        let mut handle_scrollbar = |horizontal: bool,
                                    pos: qttypes::QPoint,
                                    size: qttypes::QSize,
                                    value_prop: Pin<&Property<f32>>,
                                    page_size: i32,
                                    max: i32| {
            let pressed: bool = data.pressed != 0;
            let value: i32 = value_prop.get() as i32;
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
                initQSliderOptions(option, pressed, true, active_controls, 0, max / dpr, -value / dpr);
                option.pageStep = page_size / dpr;
                if (!horizontal) {
                    option.state ^= QStyle::State_Horizontal;
                    option.orientation = Qt::Vertical;
                }
                auto style = qApp->style();
                option.rect = { QPoint{}, size / dpr };
                return style->hitTestComplexControl(QStyle::CC_ScrollBar, &option, pos / dpr, nullptr);
            });

            #[allow(non_snake_case)]
            let SC_ScrollBarSlider =
                cpp!(unsafe []->u32 as "int" { return QStyle::SC_ScrollBarSlider;});

            let (pos, size) = if horizontal { (pos.x, size.width) } else { (pos.y, size.height) };

            let result = match event.what {
                MouseEventType::MousePressed => {
                    data.pressed = if horizontal { 1 } else { 2 };
                    if new_control == SC_ScrollBarSlider {
                        data.pressed_x = pos as f32;
                        data.pressed_val = -value as f32;
                    }
                    data.active_controls = new_control;
                    InputEventResult::GrabMouse
                }
                MouseEventType::MouseExit => {
                    data.pressed = 0;
                    InputEventResult::EventIgnored
                }
                MouseEventType::MouseReleased => {
                    data.pressed = 0;
                    let new_val = cpp!(unsafe [active_controls as "int", value as "int", max as "int", page_size as "int", dpr as "float"] -> i32 as "int" {
                        switch (active_controls) {
                            case QStyle::SC_ScrollBarAddPage:
                                return -value + page_size;
                            case QStyle::SC_ScrollBarSubPage:
                                return -value - page_size;
                            case QStyle::SC_ScrollBarAddLine:
                                return -value + 3. * dpr;
                            case QStyle::SC_ScrollBarSubLine:
                                return -value - 3. * dpr;
                            case QStyle::SC_ScrollBarFirst:
                                return 0;
                            case QStyle::SC_ScrollBarLast:
                                return max;
                            default:
                                return -value;
                        }
                    });
                    value_prop.set(-(new_val.min(max).max(0) as f32));
                    InputEventResult::EventIgnored
                }
                MouseEventType::MouseMoved => {
                    if data.pressed != 0 && data.active_controls == SC_ScrollBarSlider {
                        let max = max as f32;
                        let new_val = data.pressed_val
                            + ((pos as f32) - data.pressed_x) * (max + (page_size as f32))
                                / size as f32;
                        value_prop.set(-new_val.min(max).max(0.));
                        InputEventResult::GrabMouse
                    } else {
                        InputEventResult::EventAccepted
                    }
                }
            };
            self.data.set(data);
            result
        };

        if pressed == 2 || (pressed == 0 && event.pos.x > (size.width as f32 - right)) {
            handle_scrollbar(
                false,
                qttypes::QPoint {
                    x: (event.pos.x - (size.width as f32 - right)) as _,
                    y: (event.pos.y - top) as _,
                },
                qttypes::QSize {
                    width: (right - left) as _,
                    height: (size.height as f32 - (bottom + top)) as _,
                },
                Self::FIELD_OFFSETS.vertical_value.apply_pin(self),
                Self::FIELD_OFFSETS.vertical_page_size.apply_pin(self).get() as i32,
                Self::FIELD_OFFSETS.vertical_max.apply_pin(self).get() as i32,
            )
        } else if pressed == 1 || event.pos.y > (size.height as f32 - bottom) {
            handle_scrollbar(
                true,
                qttypes::QPoint {
                    x: (event.pos.x - left) as _,
                    y: (event.pos.y - (size.height as f32 - bottom)) as _,
                },
                qttypes::QSize {
                    width: (size.width as f32 - (right + left)) as _,
                    height: (bottom - top) as _,
                },
                Self::FIELD_OFFSETS.horizontal_value.apply_pin(self),
                Self::FIELD_OFFSETS.horizontal_page_size.apply_pin(self).get() as i32,
                Self::FIELD_OFFSETS.horizontal_max.apply_pin(self).get() as i32,
            )
        } else {
            Default::default()
        }
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &ComponentWindow) {}
}

impl ItemConsts for NativeScrollView {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! { #[no_mangle] pub static NativeScrollViewVTable for NativeScrollView }

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
    pub is_selected: Property<bool>,
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
        let is_selected: bool = Self::FIELD_OFFSETS.is_selected.apply_pin(self).get();
        let item = Self::FIELD_OFFSETS.item.apply_pin(self).get();
        let text: qttypes::QString = item.text.as_str().into();

        let mut imgarray = QImageWrapArray::new(size, dpr);
        let img = &mut imgarray.img;

        cpp!(unsafe [
            img as "QImage*",
            size as "QSize",
            dpr as "float",
            index as "int",
            is_selected as "bool",
            text as "QString"
        ] {
            QPainter p(img);
            QStyleOptionViewItem option;
            option.rect = QRect(QPoint(), size / dpr);
            option.state = QStyle::State_Enabled | QStyle::State_Active;
            if (is_selected) {
                option.state |= QStyle::State_Selected;
            }
            option.decorationPosition = QStyleOptionViewItem::Left;
            option.decorationAlignment = Qt::AlignCenter;
            option.displayAlignment = Qt::AlignLeft|Qt::AlignVCenter;
            option.showDecorationSelected = qApp->style()->styleHint(QStyle::SH_ItemView_ShowDecorationSelected, nullptr, nullptr);
            if (index % 2) {
                option.features |= QStyleOptionViewItem::Alternate;
            }
            option.features |= QStyleOptionViewItem::HasDisplay;
            option.text = text;
            qApp->style()->drawPrimitive(QStyle::PE_PanelItemViewRow, &option, &p, nullptr);
            qApp->style()->drawControl(QStyle::CE_ItemViewItem, &option, &p, nullptr);
        });
        return HighLevelRenderingPrimitive::Image { source: imgarray.to_resource() };
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
            option.showDecorationSelected = qApp->style()->styleHint(QStyle::SH_ItemView_ShowDecorationSelected, nullptr, nullptr);
            if (index % 2) {
                option.features |= QStyleOptionViewItem::Alternate;
            }
            option.features |= QStyleOptionViewItem::HasDisplay;
            option.text = text;
            return qApp->style()->sizeFromContents(QStyle::CT_ItemViewItem, &option, QSize{}, nullptr);
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

#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem)]
#[pin]
pub struct NativeComboBox {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub enabled: Property<bool>,
    pub pressed: Property<bool>,
    pub is_open: Property<bool>,
    pub current_value: Property<SharedString>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for NativeComboBox {
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
        let is_open: bool = Self::FIELD_OFFSETS.is_open.apply_pin(self).get();
        let text: qttypes::QString =
            Self::FIELD_OFFSETS.current_value.apply_pin(self).get().as_str().into();
        let enabled = Self::FIELD_OFFSETS.enabled.apply_pin(self).get();
        let size: qttypes::QSize = get_size!(self);
        let dpr = window.scale_factor();

        let mut imgarray = QImageWrapArray::new(size, dpr);
        let img = &mut imgarray.img;

        cpp!(unsafe [
            img as "QImage*",
            text as "QString",
            enabled as "bool",
            size as "QSize",
            down as "bool",
            is_open as "bool",
            dpr as "float"
        ] {
            QPainter p(img);
            QStyleOptionComboBox option;
            option.currentText = std::move(text);
            option.rect = QRect(QPoint(), size / dpr);
            if (down)
                option.state |= QStyle::State_Sunken;
            else
                option.state |= QStyle::State_Raised;
            if (enabled)
                option.state |= QStyle::State_Enabled;
            if (is_open)
                option.state |= QStyle::State_On;
            option.subControls = QStyle::SC_All;
            qApp->style()->drawComplexControl(QStyle::CC_ComboBox, &option, &p, nullptr);
            qApp->style()->drawControl(QStyle::CE_ComboBoxLabel, &option, &p, nullptr);
        });
        return HighLevelRenderingPrimitive::Image { source: imgarray.to_resource() };
    }

    fn rendering_variables(
        self: Pin<&Self>,
        _window: &ComponentWindow,
    ) -> SharedArray<RenderingVariable> {
        SharedArray::default()
    }

    fn layouting_info(self: Pin<&Self>, window: &ComponentWindow) -> LayoutInfo {
        let text: qttypes::QString =
            Self::FIELD_OFFSETS.current_value.apply_pin(self).get().as_str().into();
        let dpr = window.scale_factor();
        let size = cpp!(unsafe [
            text as "QString",
            dpr as "float"
        ] -> qttypes::QSize as "QSize" {
            ensure_initialized();
            QStyleOptionButton option;
            // FIXME
            option.rect = option.fontMetrics.boundingRect("*****************");
            option.text = std::move(text);
            return qApp->style()->sizeFromContents(QStyle::CT_ComboBox, &option, option.rect.size(), nullptr) * dpr;
        });
        LayoutInfo {
            min_width: size.width as f32,
            min_height: size.height as f32,
            ..LayoutInfo::default()
        }
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        _window: &ComponentWindow,
        _app_component: ComponentRefPin,
    ) -> InputEventResult {
        let enabled = Self::FIELD_OFFSETS.enabled.apply_pin(self).get();
        if !enabled {
            return InputEventResult::EventIgnored;
        }
        // FIXME: this is the input event of a button, but we need to do the proper hit test

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
            Self::FIELD_OFFSETS.is_open.apply_pin(self).set(true);
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

impl ItemConsts for NativeComboBox {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

ItemVTable_static! { #[no_mangle] pub static NativeComboBoxVTable for NativeComboBox }

#[repr(C)]
#[derive(FieldOffsets, BuiltinItem)]
#[pin]
pub struct NativeStyleMetrics {
    pub layout_spacing: Property<f32>,
    pub layout_padding: Property<f32>,
}

impl Default for NativeStyleMetrics {
    fn default() -> Self {
        let s = NativeStyleMetrics {
            layout_spacing: Default::default(),
            layout_padding: Default::default(),
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
}
