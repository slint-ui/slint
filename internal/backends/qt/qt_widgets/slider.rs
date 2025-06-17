// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::{
    input::{key_codes, FocusEventResult, FocusReason, KeyEventType},
    items::PointerEventButton,
};

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
    pub orientation: Property<Orientation>,
    pub enabled: Property<bool>,
    pub has_focus: Property<bool>,
    pub value: Property<f32>,
    pub minimum: Property<f32>,
    pub maximum: Property<f32>,
    pub step: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
    data: Property<NativeSliderData>,
    pub changed: Callback<FloatArg>,
    pub released: Callback<FloatArg>,
    widget_ptr: std::cell::Cell<SlintTypeErasedWidgetPtr>,
    animation_tracker: Property<i32>,
}

cpp! {{
void initQSliderOptions(QStyleOptionSlider &option, bool pressed, bool enabled, int active_controls, int minimum, int maximum, int value, bool vertical) {
    option.subControls = QStyle::SC_SliderGroove | QStyle::SC_SliderHandle;
    option.activeSubControls = { active_controls };
    if (vertical) {
        option.orientation = Qt::Vertical;
    } else {
        option.orientation = Qt::Horizontal;
        option.state |= QStyle::State_Horizontal;
    }
    option.maximum = maximum;
    option.minimum = minimum;
    option.sliderPosition = value;
    option.sliderValue = value;
    if (enabled) {
        option.state |= QStyle::State_Enabled;
    } else {
        option.palette.setCurrentColorGroup(QPalette::Disabled);
    }
    if (pressed) {
        option.state |= QStyle::State_Sunken | QStyle::State_MouseOver;
    }
}
}}

impl Item for NativeSlider {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {
        let animation_tracker_property_ptr = Self::FIELD_OFFSETS.animation_tracker.apply_pin(self);
        self.widget_ptr.set(cpp! { unsafe [animation_tracker_property_ptr as "void*"] -> SlintTypeErasedWidgetPtr as "std::unique_ptr<SlintTypeErasedWidget>" {
            return make_unique_animated_widget<QSlider>(animation_tracker_property_ptr);
        }})
    }

    fn layout_info(
        self: Pin<&Self>,
        orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> LayoutInfo {
        let enabled = self.enabled();
        // Slint slider supports floating point ranges, while Qt uses integer. To support (0..1) ranges
        // of values, scale up a little, before truncating to integer values.
        let value = (self.value() * 1024.0) as i32;
        let min = (self.minimum() * 1024.0) as i32;
        let max = (self.maximum() * 1024.0) as i32;
        let data = self.data();
        let active_controls = data.active_controls;
        let pressed = data.pressed;
        let vertical = self.orientation() == Orientation::Vertical;
        let widget: NonNull<()> = SlintTypeErasedWidgetPtr::qwidget_ptr(&self.widget_ptr);

        let size = cpp!(unsafe [
            enabled as "bool",
            value as "int",
            min as "int",
            max as "int",
            active_controls as "int",
            pressed as "bool",
            vertical as "bool",
            widget as "QWidget*"
        ] -> qttypes::QSize as "QSize" {
            ensure_initialized();
            QStyleOptionSlider option;
            initQSliderOptions(option, pressed, enabled, active_controls, min, max, value, vertical);
            auto style = qApp->style();
            auto thick = style->pixelMetric(QStyle::PM_SliderThickness, &option, widget);
            return style->sizeFromContents(QStyle::CT_Slider, &option, QSize(0, thick), widget);
        });
        let (width, height) = (size.width as f32, size.height as f32);
        match orientation {
            Orientation::Horizontal => {
                if !vertical {
                    LayoutInfo {
                        min: width,
                        preferred: width,
                        stretch: 1.,
                        ..LayoutInfo::default()
                    }
                } else {
                    LayoutInfo {
                        min: height,
                        preferred: height,
                        max: height,
                        ..LayoutInfo::default()
                    }
                }
            }
            Orientation::Vertical => {
                if !vertical {
                    LayoutInfo {
                        min: height,
                        preferred: height,
                        max: height,
                        ..LayoutInfo::default()
                    }
                } else {
                    LayoutInfo {
                        min: width,
                        preferred: width,
                        stretch: 1.,
                        ..LayoutInfo::default()
                    }
                }
            }
        }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: &MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardEvent
    }

    #[allow(clippy::unnecessary_cast)] // MouseEvent uses Coord
    fn input_event(
        self: Pin<&Self>,
        event: &MouseEvent,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &i_slint_core::items::ItemRc,
    ) -> InputEventResult {
        let size: qttypes::QSize = get_size!(self_rc);
        let enabled = self.enabled();
        // Slint slider supports floating point ranges, while Qt uses integer. To support (0..1) ranges
        // of values, scale up a little, before truncating to integer values.
        let value = (self.value() * 1024.0) as i32;
        let min = (self.minimum() * 1024.0) as i32;
        let max = (self.maximum() * 1024.0) as i32;
        let mut data = self.data();
        let active_controls = data.active_controls;
        let pressed: bool = data.pressed != 0;
        let vertical = self.orientation() == Orientation::Vertical;
        let pos = event
            .position()
            .map(|p| qttypes::QPoint { x: p.x as _, y: p.y as _ })
            .unwrap_or_default();
        let widget: NonNull<()> = SlintTypeErasedWidgetPtr::qwidget_ptr(&self.widget_ptr);

        let new_control = cpp!(unsafe [
            pos as "QPoint",
            size as "QSize",
            enabled as "bool",
            value as "int",
            min as "int",
            max as "int",
            active_controls as "int",
            pressed as "bool",
            vertical as "bool",
            widget as "QWidget*"
        ] -> u32 as "int" {
            ensure_initialized();
            QStyleOptionSlider option;
            initQSliderOptions(option, pressed, enabled, active_controls, min, max, value, vertical);
            auto style = qApp->style();
            option.rect = { QPoint{}, size };
            return style->hitTestComplexControl(QStyle::CC_Slider, &option, pos, widget);
        });
        let result = match event {
            _ if !enabled => {
                data.pressed = 0;
                InputEventResult::EventIgnored
            }
            MouseEvent::Pressed {
                position: pos,
                button: PointerEventButton::Left,
                click_count: _,
            } => {
                if !self.has_focus() {
                    WindowInner::from_pub(window_adapter.window()).set_focus_item(
                        self_rc,
                        true,
                        FocusReason::PointerClick,
                    );
                }
                data.pressed_x = if vertical { pos.y as f32 } else { pos.x as f32 };
                data.pressed = 1;
                data.pressed_val = self.value();
                InputEventResult::GrabMouse
            }
            MouseEvent::Exit | MouseEvent::Released { button: PointerEventButton::Left, .. } => {
                if data.pressed != 0 {
                    Self::FIELD_OFFSETS.released.apply_pin(self).call(&(self.value(),));
                }
                data.pressed = 0;
                InputEventResult::EventAccepted
            }
            MouseEvent::Moved { position: pos } => {
                let (coord, size) =
                    if vertical { (pos.y, size.height) } else { (pos.x, size.width) };
                if data.pressed != 0 {
                    // FIXME: use QStyle::subControlRect to find out the actual size of the groove
                    let new_val = data.pressed_val
                        + ((coord as f32) - data.pressed_x) * (self.maximum() - self.minimum())
                            / size as f32;
                    self.set_value(new_val);
                    InputEventResult::GrabMouse
                } else {
                    InputEventResult::EventIgnored
                }
            }
            MouseEvent::Wheel { delta_x, delta_y, .. } => {
                let new_val = self.value() + delta_x + delta_y;
                self.set_value(new_val);
                InputEventResult::EventAccepted
            }
            MouseEvent::Pressed { button, .. } | MouseEvent::Released { button, .. } => {
                debug_assert_ne!(*button, PointerEventButton::Left);
                InputEventResult::EventIgnored
            }
            MouseEvent::DragMove(..) | MouseEvent::Drop(..) => InputEventResult::EventIgnored,
        };
        data.active_controls = new_control;

        self.data.set(data);
        result
    }

    fn key_event(
        self: Pin<&Self>,
        event: &KeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        if self.enabled() {
            let Some(keycode) = event.text.chars().next() else {
                return KeyEventResult::EventIgnored;
            };
            let vertical = self.orientation() == Orientation::Vertical;

            if (!vertical && keycode == key_codes::RightArrow)
                || (vertical && keycode == key_codes::DownArrow)
            {
                if event.event_type == KeyEventType::KeyPressed {
                    self.set_value(self.value() + self.step());
                } else if event.event_type == KeyEventType::KeyReleased {
                    Self::FIELD_OFFSETS.released.apply_pin(self).call(&(self.value(),));
                }
                return KeyEventResult::EventAccepted;
            }
            if (!vertical && keycode == key_codes::LeftArrow)
                || (vertical && keycode == key_codes::UpArrow)
            {
                if event.event_type == KeyEventType::KeyPressed {
                    self.set_value(self.value() - self.step());
                } else if event.event_type == KeyEventType::KeyReleased {
                    Self::FIELD_OFFSETS.released.apply_pin(self).call(&(self.value(),));
                }
                return KeyEventResult::EventAccepted;
            }
            if keycode == key_codes::Home {
                if event.event_type == KeyEventType::KeyPressed {
                    self.set_value(self.minimum());
                } else if event.event_type == KeyEventType::KeyReleased {
                    Self::FIELD_OFFSETS.released.apply_pin(self).call(&(self.value(),));
                }
                return KeyEventResult::EventAccepted;
            }
            if keycode == key_codes::End {
                if event.event_type == KeyEventType::KeyPressed {
                    self.set_value(self.maximum());
                } else if event.event_type == KeyEventType::KeyReleased {
                    Self::FIELD_OFFSETS.released.apply_pin(self).call(&(self.value(),));
                }
                return KeyEventResult::EventAccepted;
            }
        }
        KeyEventResult::EventIgnored
    }

    fn focus_event(
        self: Pin<&Self>,
        event: &FocusEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> FocusEventResult {
        if self.enabled() {
            Self::FIELD_OFFSETS
                .has_focus
                .apply_pin(self)
                .set(matches!(event, FocusEvent::FocusIn(_)));
            FocusEventResult::FocusAccepted
        } else {
            FocusEventResult::FocusIgnored
        }
    }

    fn_render! { this dpr size painter widget initial_state =>
        let enabled = this.enabled();
        let has_focus = this.has_focus();
        // Slint slider supports floating point ranges, while Qt uses integer. To support (0..1) ranges
        // of values, scale up a little, before truncating to integer values.
        let value = (this.value() * 1024.0) as i32;
        let min = (this.minimum() * 1024.0) as i32;
        let max = (this.maximum() * 1024.0) as i32;
        let data = this.data();
        let active_controls = data.active_controls;
        let pressed = data.pressed;
        let vertical = this.orientation() == Orientation::Vertical;

        cpp!(unsafe [
            painter as "QPainterPtr*",
            widget as "QWidget*",
            enabled as "bool",
            has_focus as "bool",
            value as "int",
            min as "int",
            max as "int",
            size as "QSize",
            active_controls as "int",
            pressed as "bool",
            vertical as "bool",
            dpr as "float",
            initial_state as "int"
        ] {
            QStyleOptionSlider option;
            option.styleObject = widget;
            option.state |= QStyle::State(initial_state);
            if (has_focus) {
                option.state |= QStyle::State_HasFocus | QStyle::State_KeyboardFocusChange | QStyle::State_Item;
            }
            option.rect = QRect(QPoint(), size / dpr);
            initQSliderOptions(option, pressed, enabled, active_controls, min, max, value, vertical);
            auto style = qApp->style();
            style->drawComplexControl(QStyle::CC_Slider, &option, painter->get(), widget);
        });
    }

    fn bounding_rect(
        self: core::pin::Pin<&Self>,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        geometry: LogicalRect,
    ) -> LogicalRect {
        geometry
    }

    fn clips_children(self: core::pin::Pin<&Self>) -> bool {
        false
    }
}

impl NativeSlider {
    fn set_value(self: Pin<&Self>, new_val: f32) {
        let new_val = new_val.max(self.minimum()).min(self.maximum());
        self.value.set(new_val);
        Self::FIELD_OFFSETS.changed.apply_pin(self).call(&(new_val,));
    }
}

impl ItemConsts for NativeSlider {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
fn slint_get_NativeSliderVTable() -> NativeSliderVTable for NativeSlider
}
