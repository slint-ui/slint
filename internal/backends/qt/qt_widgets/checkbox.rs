// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use i_slint_core::input::{FocusEventResult, KeyEventType};

use super::*;

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct NativeCheckBox {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub enabled: Property<bool>,
    pub has_focus: Property<bool>,
    pub toggled: Callback<VoidArg>,
    pub text: Property<SharedString>,
    pub checked: Property<bool>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for NativeCheckBox {
    fn init(self: Pin<&Self>, _window_adapter: &Rc<dyn WindowAdapter>) {}

    fn geometry(self: Pin<&Self>) -> LogicalRect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layout_info(
        self: Pin<&Self>,
        orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
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
        if !self.enabled() {
            return InputEventResult::EventIgnored;
        }
        if let MouseEvent::Released { position, .. } = event {
            if euclid::rect(0., 0., self.width(), self.height()).contains(position) {
                Self::FIELD_OFFSETS.checked.apply_pin(self).set(!self.checked());
                Self::FIELD_OFFSETS.toggled.apply_pin(self).call(&())
            }
        }
        InputEventResult::EventAccepted
    }

    fn key_event(
        self: Pin<&Self>,
        event: &KeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        match event.event_type {
            KeyEventType::KeyPressed if event.text == " " || event.text == "\n" => {
                Self::FIELD_OFFSETS.checked.apply_pin(self).set(!self.checked());
                Self::FIELD_OFFSETS.toggled.apply_pin(self).call(&());
                KeyEventResult::EventAccepted
            }
            KeyEventType::KeyPressed => KeyEventResult::EventIgnored,
            KeyEventType::KeyReleased => KeyEventResult::EventIgnored,
            KeyEventType::UpdateComposition | KeyEventType::CommitComposition => {
                KeyEventResult::EventIgnored
            }
        }
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
                .set(event == &FocusEvent::FocusIn || event == &FocusEvent::WindowReceivedFocus);
            FocusEventResult::FocusAccepted
        } else {
            FocusEventResult::FocusIgnored
        }
    }

    fn_render! { this dpr size painter widget initial_state =>
        let checked: bool = this.checked();
        let enabled = this.enabled();
        let has_focus = this.has_focus();
        let text: qttypes::QString = this.text().as_str().into();

        cpp!(unsafe [
            painter as "QPainterPtr*",
            widget as "QWidget*",
            enabled as "bool",
            text as "QString",
            size as "QSize",
            checked as "bool",
            has_focus as "bool",
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
            if (has_focus) {
                option.state |= QStyle::State_HasFocus | QStyle::State_KeyboardFocusChange | QStyle::State_Item;
            }
            qApp->style()->drawControl(QStyle::CE_CheckBox, &option, painter->get(), widget);
        });
    }
}

impl ItemConsts for NativeCheckBox {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
    fn slint_get_NativeCheckBoxVTable() -> NativeCheckBoxVTable for NativeCheckBox
}
