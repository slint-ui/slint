// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

use super::*;

#[repr(C)]
#[derive(FieldOffsets, Default, SixtyFPSElement)]
#[pin]
pub struct NativeCheckBox {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub enabled: Property<bool>,
    pub toggled: Callback<VoidArg>,
    pub text: Property<SharedString>,
    pub checked: Property<bool>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for NativeCheckBox {
    fn init(self: Pin<&Self>, _window: &WindowRc) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layout_info(self: Pin<&Self>, orientation: Orientation, _window: &WindowRc) -> LayoutInfo {
        let text: qttypes::QString = self.text().as_str().into();
        let size = cpp!(unsafe [
            text as "QString"
        ] -> qttypes::QSize as "QSize" {
            ensure_initialized();
            QStyleOptionButton option;
            option.rect = option.fontMetrics.boundingRect(text);
            option.text = std::move(text);
            return qApp->style()->sizeFromContents(QStyle::CT_CheckBox, &option, option.rect.size(), nullptr);
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
        _window: &WindowRc,
        _self_rc: &sixtyfps_corelib::items::ItemRc,
    ) -> InputEventResult {
        if !self.enabled() {
            return InputEventResult::EventIgnored;
        }
        if let MouseEvent::MouseReleased { pos, .. } = event {
            if euclid::rect(0., 0., self.width(), self.height()).contains(pos) {
                Self::FIELD_OFFSETS.checked.apply_pin(self).set(!self.checked());
                Self::FIELD_OFFSETS.toggled.apply_pin(self).call(&())
            }
        }
        InputEventResult::EventAccepted
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &WindowRc) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &WindowRc) {}

    fn_render! { this dpr size painter widget initial_state =>
        let checked: bool = this.checked();
        let enabled = this.enabled();
        let text: qttypes::QString = this.text().as_str().into();

        cpp!(unsafe [
            painter as "QPainter*",
            widget as "QWidget*",
            enabled as "bool",
            text as "QString",
            size as "QSize",
            checked as "bool",
            dpr as "float",
            initial_state as "int"
        ] {
            QStyleOptionButton option;
            option.state |= QStyle::State(initial_state);
            option.text = std::move(text);
            option.rect = QRect(QPoint(), size / dpr);
            option.state |= checked ? QStyle::State_On : QStyle::State_Off;
            if (enabled) {
                option.state |= QStyle::State_Enabled;
            } else {
                option.palette.setCurrentColorGroup(QPalette::Disabled);
            }
            qApp->style()->drawControl(QStyle::CE_CheckBox, &option, painter, widget);
        });
    }
}

impl ItemConsts for NativeCheckBox {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
    fn sixtyfps_get_NativeCheckBoxVTable() -> NativeCheckBoxVTable for NativeCheckBox
}
