// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::{input::FocusEventResult, items::SortOrder};

use super::*;

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct NativeTableHeaderSection {
    pub item: Property<i_slint_core::model::TableColumn>,
    pub index: Property<i32>,
    pub cached_rendering_data: CachedRenderingData,
    pub has_hover: Property<bool>,
    widget_ptr: std::cell::Cell<SlintTypeErasedWidgetPtr>,
    animation_tracker: Property<i32>,
}

impl Item for NativeTableHeaderSection {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {
        let animation_tracker_property_ptr = Self::FIELD_OFFSETS.animation_tracker.apply_pin(self);
        self.widget_ptr.set(cpp! { unsafe [animation_tracker_property_ptr as "void*"] -> SlintTypeErasedWidgetPtr as "std::unique_ptr<SlintTypeErasedWidget>" {
            return make_unique_animated_widget<QWidget>(animation_tracker_property_ptr);
        }});
    }

    fn layout_info(
        self: Pin<&Self>,
        orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> LayoutInfo {
        let index: i32 = self.index();
        let item = self.item();
        let text: qttypes::QString = item.title.as_str().into();
        let widget: NonNull<()> = SlintTypeErasedWidgetPtr::qwidget_ptr(&self.widget_ptr);

        let s = cpp!(unsafe [
            index as "int",
            text as "QString",
            widget as "QWidget*"
        ] -> qttypes::QSize as "QSize" {
            ensure_initialized();

            QStyleOptionHeader option;
            option.state |= QStyle::State_Horizontal;
            option.section = index;

            option.text = text;

            option.textAlignment = Qt::AlignCenter | Qt::AlignVCenter;
            return qApp->style()->sizeFromContents(QStyle::CT_HeaderSection, &option, QSize{}, widget);
        });
        let min = match orientation {
            Orientation::Horizontal => s.width,
            Orientation::Vertical => s.height,
        } as f32;
        LayoutInfo { min, preferred: min, ..LayoutInfo::default() }
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
        let index: i32 = this.index();
        let has_hover: bool = this.has_hover();
        let item = this.item();
        let text: qttypes::QString = item.title.as_str().into();
        let ascending: bool = item.sort_order == SortOrder::Ascending;
        let descending: bool = item.sort_order == SortOrder::Descending;

        cpp!(unsafe [
            painter as "QPainterPtr*",
            widget as "QWidget*",
            size as "QSize",
            dpr as "float",
            index as "int",
            has_hover as "bool",
            text as "QString",
            initial_state as "int",
            ascending as "bool",
            descending as "bool"
        ] {
            QPainter *painter_ = painter->get();

            #if defined(Q_OS_MAC)
                QImage header_image(size, QImage::Format_ARGB32_Premultiplied);
                header_image.fill(Qt::transparent);
                {QPainter p(&header_image); QPainter *painter_ = &p;
            #endif

            QStyleOptionHeader option;
            option.styleObject = widget;
            option.state |= QStyle::State(initial_state);
            option.state |= QStyle::State_Horizontal | QStyle::State_Enabled;
            option.rect = QRect(QPoint(), size / dpr);

            option.section = index;

            option.textAlignment = Qt::AlignLeft | Qt::AlignVCenter;

            if (ascending) {
                option.sortIndicator = QStyleOptionHeader::SortDown;
            } else if (descending) {
                option.sortIndicator = QStyleOptionHeader::SortUp;
            } else {
                option.sortIndicator = QStyleOptionHeader::None;
            }

            if (has_hover) {
                option.state |= QStyle::State_MouseOver;
            }

            option.text = text;

            qApp->style()->drawControl(QStyle::CE_Header, &option, painter_, widget);

            #if defined(Q_OS_MAC)
                }
                (painter_)->drawImage(QPoint(), header_image);
            #endif
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

impl ItemConsts for NativeTableHeaderSection {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
fn slint_get_NativeTableHeaderSectionVTable() -> NativeTableHeaderSectionVTable for NativeTableHeaderSection
}
