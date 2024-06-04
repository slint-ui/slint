// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::input::FocusEventResult;

use super::*;

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct NativeProgressIndicator {
    pub indeterminate: Property<bool>,
    pub progress: Property<f32>,
    widget_ptr: std::cell::Cell<SlintTypeErasedWidgetPtr>,
    animation_tracker: Property<i32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for NativeProgressIndicator {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {
        let animation_tracker_property_ptr = Self::FIELD_OFFSETS.animation_tracker.apply_pin(self);
        self.widget_ptr.set(
            cpp! { unsafe [animation_tracker_property_ptr as "void*"] -> SlintTypeErasedWidgetPtr as "std::unique_ptr<SlintTypeErasedWidget>"  {
                return make_unique_animated_widget<QProgressBar>(animation_tracker_property_ptr);
            }},
        )
    }

    fn layout_info(
        self: Pin<&Self>,
        orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
        let indeterminate = self.indeterminate();
        let progress =
            if indeterminate { 0 } else { (self.progress().max(0.0).min(1.0) * 100.) as i32 };
        let widget: NonNull<()> = SlintTypeErasedWidgetPtr::qwidget_ptr(&self.widget_ptr);

        let size = cpp!(unsafe [
            progress as "int",
            widget as "QWidget*"
        ] -> qttypes::QSize as "QSize" {
            ensure_initialized();
            QStyleOptionProgressBar option;
            option.maximum = 100;
            option.minimum = 0;
            option.progress = progress;
            option.textVisible = false;
            option.state |= QStyle::State_Horizontal;

            int chunkWidth = qApp->style()->pixelMetric(QStyle::PM_ProgressBarChunkWidth, &option, widget);
            auto size = QSize(chunkWidth * 10, option.fontMetrics.height() + 10);
            return qApp->style()->sizeFromContents(QStyle::CT_ProgressBar, &option, size, widget);
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

    fn_render! { this dpr size painter widget initial_state =>
        let indeterminate = this.indeterminate();
        let progress = if indeterminate { -1 } else { (this.progress().max(0.0).min(1.0) * 100.) as i32 };

        cpp!(unsafe [
            painter as "QPainterPtr*",
            widget as "QWidget*",
            size as "QSize",
            progress as "int",
            dpr as "float",
            initial_state as "int"
        ] {
            QPainter *painter_ = painter->get();
            QStyleOptionProgressBar option;
            option.styleObject = widget;
            option.state |= QStyle::State(initial_state) | QStyle::State_Horizontal |  QStyle::State_Enabled;
            option.rect = QRect(QPoint(), size / dpr);
            option.maximum = progress < 0 ? 0 : 100;
            option.minimum = 0;
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
