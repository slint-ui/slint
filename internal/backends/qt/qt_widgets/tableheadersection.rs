// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

use i_slint_core::{input::FocusEventResult, items::SortOrder};

use super::*;

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct NativeTableHeaderSection {
    pub x: Property<LogicalLength>,
    pub y: Property<LogicalLength>,
    pub width: Property<LogicalLength>,
    pub height: Property<LogicalLength>,
    pub item: Property<i_slint_core::model::TableColumn>,
    pub index: Property<i32>,
    pub cached_rendering_data: CachedRenderingData,
    pub has_hover: Property<bool>,
}

impl Item for NativeTableHeaderSection {
    fn init(self: Pin<&Self>) {}

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
        let index: i32 = self.index();
        let item = self.item();
        let text: qttypes::QString = item.title.as_str().into();

        let s = cpp!(unsafe [
            index as "int",
            text as "QString"
        ] -> qttypes::QSize as "QSize" {
            ensure_initialized();

            QStyleOptionHeader option;
            option.state |= QStyle::State_Horizontal;
            option.section = index;

            option.text = text;

            option.textAlignment = Qt::AlignCenter | Qt::AlignVCenter;
            return qApp->style()->sizeFromContents(QStyle::CT_HeaderSection, &option, QSize{}, nullptr);
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
            #else
                // CE_Header in QCommonStyle calls setClipRect on the painter and replace the clips. So we need to cheat.
                auto engine = (*painter)->paintEngine();
                auto old_clip = engine->systemClip();
                auto new_clip = (*painter)->clipRegion() * (*painter)->transform();
                if (!old_clip.isNull())
                    new_clip &= old_clip;
                engine->setSystemClip(new_clip);
            #endif

            QStyleOptionHeader option;
            option.state |= QStyle::State(initial_state);
            option.state |= QStyle::State_Horizontal;
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
            #else
                engine->setSystemClip(old_clip);
                // Qt is seriously bugged, setSystemClip will be scaled by the scale factor
                auto actual_clip = engine->systemClip();
                if (actual_clip != old_clip) {
                    QSizeF s2 = actual_clip.boundingRect().size();
                    QSizeF s1 = old_clip.boundingRect().size();
                    engine->setSystemClip(old_clip * QTransform::fromScale(s1.width() / s2.width(), s1.height() / s2.height()));
                }
            #endif
        });
    }
}

impl ItemConsts for NativeTableHeaderSection {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
fn slint_get_NativeTableHeaderSectionVTable() -> NativeTableHeaderSectionVTable for NativeTableHeaderSection
}
