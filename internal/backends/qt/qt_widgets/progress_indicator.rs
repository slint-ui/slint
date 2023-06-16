// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

use i_slint_core::input::FocusEventResult;

use super::*;

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct NativeProgressIndicator {
    pub x: Property<LogicalLength>,
    pub y: Property<LogicalLength>,
    pub width: Property<LogicalLength>,
    pub height: Property<LogicalLength>,
    pub indeterminate: Property<bool>,
    pub progress: Property<f32>,
    pub minimum: Property<f32>,
    pub maximum: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for NativeProgressIndicator {
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
        let indeterminate = self.indeterminate() as bool;
        let progress = if indeterminate { 0 } else { self.progress() as i32 };
        let min = if indeterminate { 0 } else { self.minimum() as i32 };
        let max = if indeterminate { 0 } else { self.maximum() as i32 };

        let size = cpp!(unsafe [
            progress as "int",
            min as "int",
            max as "int"
        ] -> qttypes::QSize as "QSize" {
            ensure_initialized();
            QStyleOptionProgressBar option;
            option.maximum = max;
            option.minimum = min;
            option.progress = progress;
            option.textVisible = false;

            int chunkWidth = qApp->style()->pixelMetric(QStyle::PM_ProgressBarChunkWidth, &option, nullptr);
            auto size = QSize(chunkWidth * 10, option.fontMetrics.height() + 10);
            return qApp->style()->sizeFromContents(QStyle::CT_ProgressBar, &option, size, nullptr);
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

    fn_render! { this dpr size painter widget _initial_state =>
        let indeterminate = this.indeterminate() as bool;
        let progress = if indeterminate { 0 } else { this.progress() as i32 };
        let min =  if indeterminate { 0 } else { this.minimum() as i32 };
        let max =  if indeterminate { 0 } else { this.maximum() as i32 };

        cpp!(unsafe [
            painter as "QPainterPtr*",
            widget as "QWidget*",
            size as "QSize",
            progress as "int",
            min as "int",
            max as "int",
            dpr as "float"
        ] {
            QPainter *painter_ = painter->get();
            QStyleOptionProgressBar option;
            option.rect = QRect(QPoint(), size / dpr);
            option.maximum = max;
            option.minimum = min;
            option.progress = progress;

            qApp->style()->drawControl(QStyle::CE_ProgressBar, &option, painter_, widget);
        });
    }
}

impl ItemConsts for NativeProgressIndicator {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
fn slint_get_NativeProgressIndicatorVTable() -> NativeProgressIndicatorVTable for NativeProgressIndicator
}
