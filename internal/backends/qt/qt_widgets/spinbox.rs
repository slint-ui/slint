// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use i_slint_core::{
    input::{FocusEventResult, KeyEventType},
    items::{EventResult, KeyEventArg},
};

use super::*;

#[derive(Default, Copy, Clone, Debug, PartialEq)]
#[repr(C)]
struct NativeSpinBoxData {
    active_controls: u32,
    pressed: bool,
}

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct NativeSpinBox {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub enabled: Property<bool>,
    pub has_focus: Property<bool>,
    pub value: Property<i32>,
    pub minimum: Property<i32>,
    pub maximum: Property<i32>,
    pub cached_rendering_data: CachedRenderingData,
    pub key_pressed: Callback<KeyEventArg, EventResult>,
    pub key_released: Callback<KeyEventArg, EventResult>,
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
if (enabled) {
    option.state |= QStyle::State_Enabled;
} else {
    option.palette.setCurrentColorGroup(QPalette::Disabled);
}
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
    fn init(self: Pin<&Self>, _window: &WindowRc) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layout_info(self: Pin<&Self>, orientation: Orientation, _window: &WindowRc) -> LayoutInfo {
        //let value: i32 = self.value();
        let data = self.data();
        let active_controls = data.active_controls;
        let pressed = data.pressed;
        let enabled = self.enabled();

        let size = cpp!(unsafe [
            //value as "int",
            active_controls as "int",
            pressed as "bool",
            enabled as "bool"
        ] -> qttypes::QSize as "QSize" {
            ensure_initialized();
            auto style = qApp->style();

            QStyleOptionSpinBox option;
            initQSpinBoxOptions(option, pressed, enabled, active_controls);

            QStyleOptionFrame frame;
            frame.state = option.state;
            frame.lineWidth = style->styleHint(QStyle::SH_SpinBox_ButtonsInsideFrame, &option, nullptr) ? 0
                : style->pixelMetric(QStyle::PM_DefaultFrameWidth, &option, nullptr);
            frame.midLineWidth = 0;
            auto content = option.fontMetrics.boundingRect("0000");
            const QSize margins(2 * 2, 2 * 1); // QLineEditPrivate::verticalMargin and QLineEditPrivate::horizontalMargin
            auto line_edit_size = style->sizeFromContents(QStyle::CT_LineEdit, &frame, content.size() + margins, nullptr);
            return style->sizeFromContents(QStyle::CT_SpinBox, &option, line_edit_size, nullptr);
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
        _window: &WindowRc,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardEvent
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        window: &WindowRc,
        self_rc: &i_slint_core::items::ItemRc,
    ) -> InputEventResult {
        let size: qttypes::QSize = get_size!(self);
        let enabled = self.enabled();
        let mut data = self.data();
        let active_controls = data.active_controls;
        let pressed = data.pressed;

        let pos =
            event.pos().map(|p| qttypes::QPoint { x: p.x as _, y: p.y as _ }).unwrap_or_default();

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
            || match event {
                MouseEvent::MousePressed { .. } => {
                    data.pressed = true;
                    true
                }
                MouseEvent::MouseExit => {
                    data.pressed = false;
                    true
                }
                MouseEvent::MouseReleased { .. } => {
                    data.pressed = false;
                    if new_control == cpp!(unsafe []->u32 as "int" { return QStyle::SC_SpinBoxUp;})
                        && enabled
                    {
                        let v = self.value();
                        if v < self.maximum() {
                            self.value.set(v + 1);
                        }
                    }
                    if new_control
                        == cpp!(unsafe []->u32 as "int" { return QStyle::SC_SpinBoxDown;})
                        && enabled
                    {
                        let v = self.value();
                        if v > self.minimum() {
                            self.value.set(v - 1);
                        }
                    }
                    true
                }
                MouseEvent::MouseMoved { .. } => false,
                MouseEvent::MouseWheel { .. } => false, // TODO
            };
        data.active_controls = new_control;
        if changed {
            self.data.set(data);
        }

        if let MouseEvent::MousePressed { .. } = event {
            if !self.has_focus() {
                window.clone().set_focus_item(self_rc);
            }
        }
        InputEventResult::EventAccepted
    }

    fn key_event(self: Pin<&Self>, event: &KeyEvent, _window: &WindowRc) -> KeyEventResult {
        let r = match event.event_type {
            KeyEventType::KeyPressed => {
                Self::FIELD_OFFSETS.key_pressed.apply_pin(self).call(&(event.clone(),))
            }
            KeyEventType::KeyReleased => {
                Self::FIELD_OFFSETS.key_released.apply_pin(self).call(&(event.clone(),))
            }
        };
        match r {
            EventResult::accept => KeyEventResult::EventAccepted,
            EventResult::reject => KeyEventResult::EventIgnored,
        }
    }

    fn focus_event(self: Pin<&Self>, event: &FocusEvent, _window: &WindowRc) -> FocusEventResult {
        match event {
            FocusEvent::FocusIn | FocusEvent::WindowReceivedFocus => {
                self.has_focus.set(true);
            }
            FocusEvent::FocusOut | FocusEvent::WindowLostFocus => {
                self.has_focus.set(false);
            }
        }
        FocusEventResult::FocusAccepted
    }

    fn_render! { this dpr size painter widget initial_state =>
        let value: i32 = this.value();
        let enabled = this.enabled();
        let has_focus = this.has_focus();
        let data = this.data();
        let active_controls = data.active_controls;
        let pressed = data.pressed;

        cpp!(unsafe [
            painter as "QPainter*",
            widget as "QWidget*",
            value as "int",
            enabled as "bool",
            has_focus as "bool",
            size as "QSize",
            active_controls as "int",
            pressed as "bool",
            dpr as "float",
            initial_state as "int"
        ] {
            auto style = qApp->style();
            QStyleOptionSpinBox option;
            option.state |= QStyle::State(initial_state);
            if (enabled && has_focus) {
                option.state |= QStyle::State_HasFocus;
            }
            option.rect = QRect(QPoint(), size / dpr);
            initQSpinBoxOptions(option, pressed, enabled, active_controls);
            style->drawComplexControl(QStyle::CC_SpinBox, &option, painter, widget);

            QStyleOptionFrame frame;
            frame.state = option.state;
            frame.palette = option.palette;
            frame.lineWidth = style->styleHint(QStyle::SH_SpinBox_ButtonsInsideFrame, &option, widget) ? 0
                : style->pixelMetric(QStyle::PM_DefaultFrameWidth, &option, widget);
            frame.midLineWidth = 0;
            frame.rect = style->subControlRect(QStyle::CC_SpinBox, &option, QStyle::SC_SpinBoxEditField, widget);
            style->drawPrimitive(QStyle::PE_PanelLineEdit, &frame, painter, widget);
            QRect text_rect = qApp->style()->subElementRect(QStyle::SE_LineEditContents, &frame, widget);
            text_rect.adjust(1, 2, 1, 2);
            painter->setPen(option.palette.color(QPalette::Text));
            painter->drawText(text_rect, QString::number(value));
        });
    }
}

impl ItemConsts for NativeSpinBox {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
fn slint_get_NativeSpinBoxVTable() -> NativeSpinBoxVTable for NativeSpinBox
}
