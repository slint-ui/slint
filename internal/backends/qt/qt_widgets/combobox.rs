// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

use i_slint_core::input::FocusEventResult;

use super::*;

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct NativeComboBox {
    pub x: Property<LogicalLength>,
    pub y: Property<LogicalLength>,
    pub width: Property<LogicalLength>,
    pub height: Property<LogicalLength>,
    pub enabled: Property<bool>,
    pub pressed: Property<bool>,
    pub is_open: Property<bool>,
    pub current_value: Property<SharedString>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for NativeComboBox {
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
        let size = cpp!(unsafe [] -> qttypes::QSize as "QSize" {
            ensure_initialized();
            QStyleOptionComboBox option;
            // FIXME
            option.rect = option.fontMetrics.boundingRect("*************");
            option.subControls = QStyle::SC_All;
            return qApp->style()->sizeFromContents(QStyle::CT_ComboBox, &option, option.rect.size(), nullptr);
        });
        LayoutInfo {
            min: match orientation {
                Orientation::Horizontal => size.width,
                Orientation::Vertical => size.height,
            } as f32,
            ..LayoutInfo::default()
        }
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
        _event: MouseEvent,
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

    fn_render! { this dpr size painter widget initial_state =>
        let down: bool = this.pressed();
        let is_open: bool = this.is_open();
        let text: qttypes::QString =
            this.current_value().as_str().into();
        let enabled = this.enabled();
        cpp!(unsafe [
            painter as "QPainterPtr*",
            widget as "QWidget*",
            text as "QString",
            enabled as "bool",
            size as "QSize",
            down as "bool",
            is_open as "bool",
            dpr as "float",
            initial_state as "int"
        ] {
            ensure_initialized();
            QStyleOptionComboBox option;
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
            // FIXME: This is commented out to workaround #456
            if (is_open) {
            //    option.state |= QStyle::State_On;
            }
            option.subControls = QStyle::SC_All;
            qApp->style()->drawComplexControl(QStyle::CC_ComboBox, &option, painter->get(), widget);
            qApp->style()->drawControl(QStyle::CE_ComboBoxLabel, &option, painter->get(), widget);
        });
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
    pub x: Property<LogicalLength>,
    pub y: Property<LogicalLength>,
    pub width: Property<LogicalLength>,
    pub height: Property<LogicalLength>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for NativeComboBoxPopup {
    fn init(self: Pin<&Self>, _window_adapter: &Rc<dyn WindowAdapter>) {}

    fn geometry(self: Pin<&Self>) -> LogicalRect {
        LogicalRect::new(
            LogicalPoint::from_lengths(self.x(), self.y()),
            LogicalSize::from_lengths(self.width(), self.height()),
        )
    }

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
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
            QStyleOptionFrame option;
            option.state |= QStyle::State(initial_state);
            option.lineWidth = 0;
            option.midLineWidth = 0;
            option.rect = QRect(QPoint(), size / dpr);
            option.state |= QStyle::State_Sunken | QStyle::State_Enabled;

            auto style = qApp->style();

            if (style->styleHint(QStyle::SH_ComboBox_Popup, &option, widget)) {
                style->drawPrimitive(QStyle::PE_PanelMenu, &option, painter->get(), widget);
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
}

impl ItemConsts for NativeComboBoxPopup {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
fn slint_get_NativeComboBoxPopupVTable() -> NativeComboBoxPopupVTable for NativeComboBoxPopup
}
