// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use i_slint_core::input::FocusEventResult;

use super::*;

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct NativeStandardListViewItem {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub item: Property<i_slint_core::model::StandardListViewItem>,
    pub index: Property<i32>,
    pub is_selected: Property<bool>,
    pub cached_rendering_data: CachedRenderingData,
    pub has_hover: Property<bool>,
}

impl Item for NativeStandardListViewItem {
    fn init(self: Pin<&Self>, _window_adapter: &Rc<dyn WindowAdapter>) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layout_info(
        self: Pin<&Self>,
        orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
        let index: i32 = self.index();
        let item = self.item();
        let text: qttypes::QString = item.text.as_str().into();

        let s = cpp!(unsafe [
            index as "int",
            text as "QString"
        ] -> qttypes::QSize as "QSize" {
            ensure_initialized();

            QStyleOptionViewItem option;
            option.decorationPosition = QStyleOptionViewItem::Left;
            option.decorationAlignment = Qt::AlignCenter;
            option.displayAlignment = Qt::AlignLeft|Qt::AlignVCenter;
            option.showDecorationSelected = qApp->style()->styleHint(QStyle::SH_ItemView_ShowDecorationSelected, nullptr, nullptr);
            if (index % 2) {
                option.features |= QStyleOptionViewItem::Alternate;
            }
            option.features |= QStyleOptionViewItem::HasDisplay;
            option.text = text;
            return qApp->style()->sizeFromContents(QStyle::CT_ItemViewItem, &option, QSize{}, nullptr);
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
        let is_selected: bool = this.is_selected();
        let has_hover: bool = this.has_hover();
        let item = this.item();
        let text: qttypes::QString = item.text.as_str().into();
        cpp!(unsafe [
            painter as "QPainterPtr*",
            widget as "QWidget*",
            size as "QSize",
            dpr as "float",
            index as "int",
            is_selected as "bool",
            has_hover as "bool",
            text as "QString",
            initial_state as "int"
        ] {
            QStyleOptionViewItem option;
            option.state |= QStyle::State(initial_state);
            option.rect = QRect(QPoint(), size / dpr);
            option.state = QStyle::State_Enabled;
            if (is_selected) {
                option.state |= QStyle::State_Selected;
            }
            if (has_hover) {
                option.state |= QStyle::State_MouseOver;
            }
            option.decorationPosition = QStyleOptionViewItem::Left;
            option.decorationAlignment = Qt::AlignCenter;
            option.displayAlignment = Qt::AlignLeft|Qt::AlignVCenter;
            option.showDecorationSelected = qApp->style()->styleHint(QStyle::SH_ItemView_ShowDecorationSelected, nullptr, nullptr);
            if (index % 2) {
                option.features |= QStyleOptionViewItem::Alternate;
            }
            option.features |= QStyleOptionViewItem::HasDisplay;
            option.text = text;
            // CE_ItemViewItem in QCommonStyle calls setClipRect on the painter and replace the clips. So we need to cheat.
            auto engine = (*painter)->paintEngine();
            auto old_clip = engine->systemClip();
            auto new_clip = old_clip & ((*painter)->clipRegion() * (*painter)->transform());
            if (new_clip.isEmpty()) return;
            engine->setSystemClip(new_clip);

            qApp->style()->drawPrimitive(QStyle::PE_PanelItemViewRow, &option, painter->get(), widget);
            qApp->style()->drawControl(QStyle::CE_ItemViewItem, &option, painter->get(), widget);
            engine->setSystemClip(old_clip);
        });
    }
}

impl ItemConsts for NativeStandardListViewItem {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
fn slint_get_NativeStandardListViewItemVTable() -> NativeStandardListViewItemVTable for NativeStandardListViewItem
}
