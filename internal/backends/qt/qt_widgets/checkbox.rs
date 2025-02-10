// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::{
    input::{FocusEventResult, KeyEventType},
    platform::PointerEventButton,
};

use super::*;

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct NativeCheckBox {
    pub enabled: Property<bool>,
    pub has_focus: Property<bool>,
    pub toggled: Callback<VoidArg>,
    pub text: Property<SharedString>,
    pub has_hover: Property<bool>,
    pub checked: Property<bool>,
    widget_ptr: std::cell::Cell<SlintTypeErasedWidgetPtr>,
    animation_tracker: Property<i32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for NativeCheckBox {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {
        let animation_tracker_property_ptr = Self::FIELD_OFFSETS.animation_tracker.apply_pin(self);
        self.widget_ptr.set(cpp! { unsafe [animation_tracker_property_ptr as "void*"] -> SlintTypeErasedWidgetPtr as "std::unique_ptr<SlintTypeErasedWidget>"  {
            return make_unique_animated_widget<QCheckBox>(animation_tracker_property_ptr);
        }})
    }

    fn layout_info(
        self: Pin<&Self>,
        orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
        let text: qttypes::QString = self.text().as_str().into();
        let widget: NonNull<()> = SlintTypeErasedWidgetPtr::qwidget_ptr(&self.widget_ptr);
        let size = cpp!(unsafe [
            text as "QString",
            widget as "QWidget*"
        ] -> qttypes::QSize as "QSize" {
            ensure_initialized();
            QStyleOptionButton option;
            option.rect = option.fontMetrics.boundingRect(text);
            option.text = std::move(text);
            return qApp->style()->sizeFromContents(QStyle::CT_CheckBox, &option, option.rect.size(), widget);
        });
        match orientation {
            Orientation::Horizontal => LayoutInfo {
                min: size.width as f32,
                preferred: size.width as f32,
                stretch: 1.,
                ..LayoutInfo::default()
            },
            Orientation::Vertical => LayoutInfo {
                min: size.height as f32,
                preferred: size.height as f32,
                max: size.height as f32,
                ..LayoutInfo::default()
            },
        }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        event: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        Self::FIELD_OFFSETS.has_hover.apply_pin(self).set(!matches!(event, MouseEvent::Exit));
        InputEventFilterResult::ForwardEvent
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &i_slint_core::items::ItemRc,
    ) -> InputEventResult {
        if matches!(event, MouseEvent::Exit) {
            Self::FIELD_OFFSETS.has_hover.apply_pin(self).set(false);
        }
        if !self.enabled() {
            return InputEventResult::EventIgnored;
        }
        if let MouseEvent::Released { position, button, .. } = event {
            let geo = self_rc.geometry();
            if button == PointerEventButton::Left
                && LogicalRect::new(LogicalPoint::default(), geo.size).contains(position)
            {
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
        let has_hover = this.has_hover();
        let text: qttypes::QString = this.text().as_str().into();

        cpp!(unsafe [
            painter as "QPainterPtr*",
            widget as "QWidget*",
            enabled as "bool",
            text as "QString",
            size as "QSize",
            checked as "bool",
            has_focus as "bool",
            has_hover as "bool",
            dpr as "float",
            initial_state as "int"
        ] {
            QStyleOptionButton option;
            option.styleObject = widget;
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
            if (has_hover) {
                option.state |= QStyle::State_MouseOver;
            }
            qApp->style()->drawControl(QStyle::CE_CheckBox, &option, painter->get(), widget);
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

impl ItemConsts for NativeCheckBox {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
    fn slint_get_NativeCheckBoxVTable() -> NativeCheckBoxVTable for NativeCheckBox
}
