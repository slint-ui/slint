// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::input::FocusEventResult;
use i_slint_core::items::InputType;

use super::*;

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct NativeLineEdit {
    pub cached_rendering_data: CachedRenderingData,
    pub native_padding_left: Property<LogicalLength>,
    pub native_padding_right: Property<LogicalLength>,
    pub native_padding_top: Property<LogicalLength>,
    pub native_padding_bottom: Property<LogicalLength>,
    pub has_focus: Property<bool>,
    pub enabled: Property<bool>,
    pub input_type: Property<InputType>,
    widget_ptr: std::cell::Cell<SlintTypeErasedWidgetPtr>,
    animation_tracker: Property<i32>,
}

impl Item for NativeLineEdit {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {
        let animation_tracker_property_ptr = Self::FIELD_OFFSETS.animation_tracker.apply_pin(self);
        self.widget_ptr.set(cpp! { unsafe [animation_tracker_property_ptr as "void*"] -> SlintTypeErasedWidgetPtr as "std::unique_ptr<SlintTypeErasedWidget>"  {
            return make_unique_animated_widget<QLineEdit>(animation_tracker_property_ptr);
        }});

        let paddings = Rc::pin(Property::default());

        paddings.as_ref().set_binding(move || {
            cpp!(unsafe [] -> qttypes::QMargins as "QMargins" {
                ensure_initialized();
                QStyleOptionFrame option;
                option.state |= QStyle::State_Enabled;
                option.lineWidth = 1;
                option.midLineWidth = 0;
                // Just some size big enough to be sure that the frame fits in it
                option.rect = QRect(0, 0, 10000, 10000);
                QRect contentsRect = qApp->style()->subElementRect(
                    QStyle::SE_LineEditContents, &option);

                // ### remove extra margins

                return {
                    (2 + contentsRect.left()),
                    (4 + contentsRect.top()),
                    (2 + option.rect.right() - contentsRect.right()),
                    (4 + option.rect.bottom() - contentsRect.bottom())
                };
            })
        });

        self.native_padding_left.set_binding({
            let paddings = paddings.clone();
            move || LogicalLength::new(paddings.as_ref().get().left as _)
        });
        self.native_padding_right.set_binding({
            let paddings = paddings.clone();
            move || LogicalLength::new(paddings.as_ref().get().right as _)
        });
        self.native_padding_top.set_binding({
            let paddings = paddings.clone();
            move || LogicalLength::new(paddings.as_ref().get().top as _)
        });
        self.native_padding_bottom.set_binding({
            let paddings = paddings;
            move || LogicalLength::new(paddings.as_ref().get().bottom as _)
        });
    }

    fn layout_info(
        self: Pin<&Self>,
        orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
        let min = match orientation {
            Orientation::Horizontal => self.native_padding_left() + self.native_padding_right(),
            Orientation::Vertical => self.native_padding_top() + self.native_padding_bottom(),
        }
        .get();
        LayoutInfo {
            min,
            preferred: min,
            stretch: if orientation == Orientation::Horizontal { 1. } else { 0. },
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

    fn_render! { this dpr size painter widget initial_state =>
        let has_focus: bool = this.has_focus();
        let enabled: bool = this.enabled();

        cpp!(unsafe [
            painter as "QPainterPtr*",
            widget as "QWidget*",
            size as "QSize",
            dpr as "float",
            enabled as "bool",
            has_focus as "bool",
            initial_state as "int"
        ] {
            QStyleOptionFrame option;
            option.styleObject = widget;
            option.state |= QStyle::State(initial_state);
            option.rect = QRect(QPoint(), size / dpr);
            option.lineWidth = 1;
            option.midLineWidth = 0;
            if (enabled) {
                option.state |= QStyle::State_Enabled;
                if (has_focus)
                    option.state |= QStyle::State_HasFocus;
            } else {
                option.palette.setCurrentColorGroup(QPalette::Disabled);
            }
            qApp->style()->drawPrimitive(QStyle::PE_PanelLineEdit, &option, painter->get(), widget);
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

impl ItemConsts for NativeLineEdit {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
fn slint_get_NativeLineEditVTable() -> NativeLineEditVTable for NativeLineEdit
}
