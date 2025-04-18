// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::input::FocusEventResult;

use super::*;

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct NativeComboBox {
    pub enabled: Property<bool>,
    pub has_focus: Property<bool>,
    pub pressed: Property<bool>,
    pub has_hover: Property<bool>,
    pub is_open: Property<bool>,
    pub current_value: Property<SharedString>,
    widget_ptr: std::cell::Cell<SlintTypeErasedWidgetPtr>,
    animation_tracker: Property<i32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for NativeComboBox {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {
        let animation_tracker_property_ptr = Self::FIELD_OFFSETS.animation_tracker.apply_pin(self);
        self.widget_ptr.set(cpp! { unsafe [animation_tracker_property_ptr as "void*"] -> SlintTypeErasedWidgetPtr as "std::unique_ptr<SlintTypeErasedWidget>"  {
            return make_unique_animated_widget<QComboBox>(animation_tracker_property_ptr);
        }})
    }

    fn layout_info(
        self: Pin<&Self>,
        orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> LayoutInfo {
        let widget: NonNull<()> = SlintTypeErasedWidgetPtr::qwidget_ptr(&self.widget_ptr);
        let size = cpp!(unsafe [widget as "QWidget*"] -> qttypes::QSize as "QSize" {
            ensure_initialized();
            QStyleOptionComboBox option;
            // FIXME
            option.rect = option.fontMetrics.boundingRect("******************");
            option.subControls = QStyle::SC_All;
            return qApp->style()->sizeFromContents(QStyle::CT_ComboBox, &option, option.rect.size(), widget);
        });
        let min = match orientation {
            Orientation::Horizontal => size.width,
            Orientation::Vertical => size.height,
        } as f32;
        LayoutInfo { min, preferred: min, ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        event: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        Self::FIELD_OFFSETS.has_hover.apply_pin(self).set(!matches!(event, MouseEvent::Exit));
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &i_slint_core::items::ItemRc,
    ) -> InputEventResult {
        if matches!(event, MouseEvent::Exit) {
            Self::FIELD_OFFSETS.has_hover.apply_pin(self).set(false);
        }
        InputEventResult::EventIgnored
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
        let down: bool = this.pressed();
        let is_open: bool = this.is_open();
        let text: qttypes::QString =
            this.current_value().as_str().into();
        let enabled = this.enabled();
        let has_focus = this.has_focus();
        let has_hover = this.has_hover();
        cpp!(unsafe [
            painter as "QPainterPtr*",
            widget as "QWidget*",
            text as "QString",
            enabled as "bool",
            size as "QSize",
            down as "bool",
            is_open as "bool",
            has_focus as "bool",
            has_hover as "bool",
            dpr as "float",
            initial_state as "int"
        ] {
            ensure_initialized();
            QStyleOptionComboBox option;
            option.styleObject = widget;
            option.state |= QStyle::State(initial_state);
            option.currentText = std::move(text);
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
            if (has_focus) {
                option.state |= QStyle::State_HasFocus | QStyle::State_KeyboardFocusChange | QStyle::State_Item;
            }
            if (has_hover) {
                option.state |= QStyle::State_MouseOver;
            }
            // FIXME: This is commented out to workaround #456
            if (is_open) {
            //    option.state |= QStyle::State_On;
            }
            option.subControls = QStyle::SC_All;
            qApp->style()->drawComplexControl(QStyle::CC_ComboBox, &option, painter->get(), widget);
            qApp->style()->drawControl(QStyle::CE_ComboBoxLabel, &option, painter->get(), widget);
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

impl ItemConsts for NativeComboBox {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
fn slint_get_NativeComboBoxVTable() -> NativeComboBoxVTable for NativeComboBox
}

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct NativeComboBoxPopup {
    widget_ptr: std::cell::Cell<SlintTypeErasedWidgetPtr>,
    animation_tracker: Property<i32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for NativeComboBoxPopup {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {
        let animation_tracker_property_ptr = Self::FIELD_OFFSETS.animation_tracker.apply_pin(self);
        self.widget_ptr.set(cpp! { unsafe [animation_tracker_property_ptr as "void*"] -> SlintTypeErasedWidgetPtr as "std::unique_ptr<SlintTypeErasedWidget>"  {
            return make_unique_animated_widget<QWidget>(animation_tracker_property_ptr);
        }})
    }

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> LayoutInfo {
        Default::default()
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &i_slint_core::items::ItemRc,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
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

    fn_render! { _this dpr size painter widget initial_state =>
        cpp!(unsafe [
            painter as "QPainterPtr*",
            widget as "QWidget*",
            size as "QSize",
            dpr as "float",
            initial_state as "int"
        ] {
            ensure_initialized();
            QStyleOptionComboBox cb_option;
            QStyleOptionFrame option;
            option.styleObject = widget;
            option.state |= QStyle::State(initial_state);
            option.lineWidth = 0;
            option.midLineWidth = 0;
            option.rect = QRect(QPoint(), size / dpr);
            option.state |= QStyle::State_Sunken | QStyle::State_Enabled;

            auto style = qApp->style();
            painter->get()->fillRect(option.rect, option.palette.window());

            if (style->styleHint(QStyle::SH_ComboBox_Popup, &cb_option, widget)) {
                style->drawPrimitive(QStyle::PE_PanelMenu, &option, painter->get(), widget);
                auto vm = style->pixelMetric(QStyle::PM_MenuVMargin, &option, widget);
                auto hm = style->pixelMetric(QStyle::PM_MenuHMargin, &option, widget);
                painter->get()->fillRect(option.rect.adjusted(hm, vm, -hm, -vm), option.palette.window());
            } else {
                option.lineWidth = 1;
            }
            auto frameStyle = style->styleHint(QStyle::SH_ComboBox_PopupFrameStyle, &option, widget);
            if ((frameStyle & QFrame::Shadow_Mask) == QFrame::Sunken)
                option.state |= QStyle::State_Sunken;
            else if ((frameStyle & QFrame::Shadow_Mask) == QFrame::Raised)
                option.state |= QStyle::State_Raised;
            option.frameShape = QFrame::Shape(frameStyle & QFrame::Shape_Mask);
            style->drawControl(QStyle::CE_ShapedFrame, &option, painter->get(), widget);
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

impl ItemConsts for NativeComboBoxPopup {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
fn slint_get_NativeComboBoxPopupVTable() -> NativeComboBoxPopupVTable for NativeComboBoxPopup
}
