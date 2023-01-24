// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use i_slint_core::{input::FocusEventResult, items::PointerEventButton};

use super::*;

#[derive(Default, Copy, Clone, Debug, PartialEq)]
#[repr(C)]
// Also used by the NativeScrollView
pub(super) struct NativeSliderData {
    pub active_controls: u32,
    /// For sliders, this is a bool, For scroll area: 1 == horizontal, 2 == vertical
    pub pressed: u8,
    pub pressed_x: f32,
    pub pressed_val: f32,
}

type FloatArg = (f32,);

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct NativeSlider {
    pub x: Property<LogicalLength>,
    pub y: Property<LogicalLength>,
    pub width: Property<LogicalLength>,
    pub height: Property<LogicalLength>,
    pub enabled: Property<bool>,
    pub value: Property<f32>,
    pub minimum: Property<f32>,
    pub maximum: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
    data: Property<NativeSliderData>,
    pub changed: Callback<FloatArg>,
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
    if (enabled) {
        option.state |= QStyle::State_Enabled;
    } else {
        option.palette.setCurrentColorGroup(QPalette::Disabled);
    }
    option.state |= QStyle::State_Horizontal;
    if (pressed) {
        option.state |= QStyle::State_Sunken | QStyle::State_MouseOver;
    }
}
}}

impl Item for NativeSlider {
    fn init(self: Pin<&Self>, _window_adapter: &Rc<dyn WindowAdapter>) {}

    fn geometry(self: Pin<&Self>) -> LogicalRect {
        LogicalRect::new(
            LogicalPoint::from_lengths(self.x(), self.y()),
            LogicalSize::from_lengths(self.width(), self.height()),
        )
    }

    fn layout_info(
        self: Pin<&Self>,
        orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
        let enabled = self.enabled();
        let value = self.value() as i32;
        let min = self.minimum() as i32;
        let max = self.maximum() as i32;
        let data = self.data();
        let active_controls = data.active_controls;
        let pressed = data.pressed;

        let size = cpp!(unsafe [
            enabled as "bool",
            value as "int",
            min as "int",
            max as "int",
            active_controls as "int",
            pressed as "bool"
        ] -> qttypes::QSize as "QSize" {
            ensure_initialized();
            QStyleOptionSlider option;
            initQSliderOptions(option, pressed, enabled, active_controls, min, max, value);
            auto style = qApp->style();
            auto thick = style->pixelMetric(QStyle::PM_SliderThickness, &option, nullptr);
            return style->sizeFromContents(QStyle::CT_Slider, &option, QSize(0, thick), nullptr);
        });
        match orientation {
            Orientation::Horizontal => {
                LayoutInfo { min: size.width as f32, stretch: 1., ..LayoutInfo::default() }
            }
            Orientation::Vertical => LayoutInfo {
                min: size.height as f32,
                max: size.height as f32,
                ..LayoutInfo::default()
            },
        }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardEvent
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &i_slint_core::items::ItemRc,
    ) -> InputEventResult {
        let size: qttypes::QSize = get_size!(self);
        let enabled = self.enabled();
        let value = self.value() as f32;
        let min = self.minimum() as f32;
        let max = self.maximum() as f32;
        let mut data = self.data();
        let active_controls = data.active_controls;
        let pressed: bool = data.pressed != 0;
        let pos = event
            .position()
            .map(|p| qttypes::QPoint { x: p.x as _, y: p.y as _ })
            .unwrap_or_default();

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
        let result = match event {
            _ if !enabled => {
                data.pressed = 0;
                InputEventResult::EventIgnored
            }
            MouseEvent::Pressed {
                position: pos,
                button: PointerEventButton::Left,
                click_count: 0,
            } => {
                data.pressed_x = pos.x as f32;
                data.pressed = 1;
                data.pressed_val = value;
                InputEventResult::GrabMouse
            }
            MouseEvent::Exit | MouseEvent::Released { button: PointerEventButton::Left, .. } => {
                data.pressed = 0;
                InputEventResult::EventAccepted
            }
            MouseEvent::Moved { position: pos } => {
                if data.pressed != 0 {
                    // FIXME: use QStyle::subControlRect to find out the actual size of the groove
                    let new_val = data.pressed_val
                        + ((pos.x as f32) - data.pressed_x) * (max - min) / size.width as f32;
                    let new_val = new_val.max(min).min(max);
                    self.value.set(new_val);
                    Self::FIELD_OFFSETS.changed.apply_pin(self).call(&(new_val,));
                    InputEventResult::GrabMouse
                } else {
                    InputEventResult::EventIgnored
                }
            }
            MouseEvent::Wheel { delta_x, delta_y, .. } => {
                let new_val = value + delta_x + delta_y;
                let new_val = new_val.max(min).min(max);
                self.value.set(new_val);
                Self::FIELD_OFFSETS.changed.apply_pin(self).call(&(new_val,));
                InputEventResult::EventAccepted
            }
            MouseEvent::Pressed { button, .. } | MouseEvent::Released { button, .. } => {
                debug_assert_ne!(button, PointerEventButton::Left);
                InputEventResult::EventIgnored
            }
        };
        data.active_controls = new_control;

        self.data.set(data);
        result
    }

    fn key_event(
        self: Pin<&Self>,
        _: &KeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(
        self: Pin<&Self>,
        _: &FocusEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> FocusEventResult {
        FocusEventResult::FocusIgnored
    }

    fn_render! { this dpr size painter widget initial_state =>
        let enabled = this.enabled();
        let value = this.value() as i32;
        let min = this.minimum() as i32;
        let max = this.maximum() as i32;
        let data = this.data();
        let active_controls = data.active_controls;
        let pressed = data.pressed;

        cpp!(unsafe [
            painter as "QPainterPtr*",
            widget as "QWidget*",
            enabled as "bool",
            value as "int",
            min as "int",
            max as "int",
            size as "QSize",
            active_controls as "int",
            pressed as "bool",
            dpr as "float",
            initial_state as "int"
        ] {
            QStyleOptionSlider option;
            option.state |= QStyle::State(initial_state);
            option.rect = QRect(QPoint(), size / dpr);
            initQSliderOptions(option, pressed, enabled, active_controls, min, max, value);
            auto style = qApp->style();
            style->drawComplexControl(QStyle::CC_Slider, &option, painter->get(), widget);
        });
    }
}

impl ItemConsts for NativeSlider {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
fn slint_get_NativeSliderVTable() -> NativeSliderVTable for NativeSlider
}
