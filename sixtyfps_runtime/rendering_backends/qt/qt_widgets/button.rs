/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use super::*;

#[repr(C)]
#[derive(FieldOffsets, Default, SixtyFPSElement)]
#[pin]
pub struct NativeButton {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub text: Property<SharedString>,
    pub icon: Property<sixtyfps_corelib::graphics::Image>,
    pub enabled: Property<bool>,
    pub pressed: Property<bool>,
    pub clicked: Callback<VoidArg>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for NativeButton {
    fn init(self: Pin<&Self>, _window: &WindowRc) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layouting_info(
        self: Pin<&Self>,
        orientation: Orientation,
        _window: &WindowRc,
    ) -> LayoutInfo {
        let mut text: qttypes::QString = self.text().as_str().into();
        let icon: qttypes::QPixmap = crate::qt_window::load_image_from_resource(
            (&self.icon()).into(),
            None,
            Default::default(),
        )
        .unwrap_or_default();
        let size = cpp!(unsafe [
            mut text as "QString",
            icon as "QPixmap"
        ] -> qttypes::QSize as "QSize" {
            ensure_initialized();
            QStyleOptionButton option;
            if (text.isEmpty())
                text = "**";
            option.rect = option.fontMetrics.boundingRect(text);
            option.text = std::move(text);
            option.icon = icon;
            auto iconSize = qApp->style()->pixelMetric(QStyle::PM_ButtonIconSize, 0, nullptr);
            option.iconSize = QSize(iconSize, iconSize);
            return qApp->style()->sizeFromContents(QStyle::CT_PushButton, &option, option.rect.size(), nullptr);
        });
        LayoutInfo {
            min: match orientation {
                Orientation::Horizontal => size.width as f32,
                Orientation::Vertical => size.height as f32,
            },
            ..LayoutInfo::default()
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
        _window: &WindowRc,
        _self_rc: &sixtyfps_corelib::items::ItemRc,
    ) -> InputEventResult {
        let enabled = self.enabled();
        if !enabled {
            return InputEventResult::EventIgnored;
        }

        Self::FIELD_OFFSETS.pressed.apply_pin(self).set(match event {
            MouseEvent::MousePressed { .. } => true,
            MouseEvent::MouseExit | MouseEvent::MouseReleased { .. } => false,
            MouseEvent::MouseMoved { .. } => {
                return if self.pressed() {
                    InputEventResult::GrabMouse
                } else {
                    InputEventResult::EventIgnored
                }
            }
            MouseEvent::MouseWheel { .. } => return InputEventResult::EventIgnored,
        });
        if let MouseEvent::MouseReleased { pos, .. } = event {
            if euclid::rect(0., 0., self.width(), self.height()).contains(pos) {
                Self::FIELD_OFFSETS.clicked.apply_pin(self).call(&());
            }
            InputEventResult::EventAccepted
        } else {
            InputEventResult::GrabMouse
        }
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &WindowRc) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &WindowRc) {}

    fn_render! { this dpr size painter initial_state =>
        let down: bool = this.pressed();
        let text: qttypes::QString = this.text().as_str().into();
        let icon : qttypes::QPixmap = crate::qt_window::load_image_from_resource(
            (&this.icon()).into(),
            None,
            Default::default(),
        )
        .unwrap_or_default();
        let enabled = this.enabled();

        cpp!(unsafe [
            painter as "QPainter*",
            text as "QString",
            icon as "QPixmap",
            enabled as "bool",
            size as "QSize",
            down as "bool",
            dpr as "float",
            initial_state as "int"
        ] {
            QStyleOptionButton option;
            option.state |= QStyle::State(initial_state);
            option.text = std::move(text);
            option.icon = icon;
            auto iconSize = qApp->style()->pixelMetric(QStyle::PM_ButtonIconSize, 0, nullptr);
            option.iconSize = QSize(iconSize, iconSize);
            option.rect = QRect(QPoint(), size / dpr);
            if (down)
                option.state |= QStyle::State_Sunken;
            else
                option.state |= QStyle::State_Raised;
            if (enabled) {
                option.state |= QStyle::State_Enabled;
            } else {
                option.palette.setCurrentColorGroup(QPalette::Disabled);
            }
            qApp->style()->drawControl(QStyle::CE_PushButton, &option, painter, nullptr);
        });
    }
}

impl ItemConsts for NativeButton {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}
