// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::input::FocusEventResult;

use super::*;

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct NativeGroupBox {
    pub enabled: Property<bool>,
    pub title: Property<SharedString>,
    pub cached_rendering_data: CachedRenderingData,
    pub native_padding_left: Property<LogicalLength>,
    pub native_padding_right: Property<LogicalLength>,
    pub native_padding_top: Property<LogicalLength>,
    pub native_padding_bottom: Property<LogicalLength>,
    widget_ptr: std::cell::Cell<SlintTypeErasedWidgetPtr>,
    animation_tracker: Property<i32>,
}

#[repr(C)]
#[derive(FieldOffsets, Default)]
#[pin]
struct GroupBoxData {
    title: Property<SharedString>,
    paddings: Property<qttypes::QMargins>,
}

cpp! {{
    QStyleOptionGroupBox create_group_box_option(QString title) {
        QStyleOptionGroupBox option;
        option.text = title;
        option.lineWidth = 1;
        option.midLineWidth = 0;
        option.subControls = QStyle::SC_GroupBoxFrame;
        if (!title.isEmpty()) {
            option.subControls |= QStyle::SC_GroupBoxLabel;
        }
        option.textColor = QColor(qApp->style()->styleHint(
            QStyle::SH_GroupBox_TextLabelColor, &option));

        return option;
    }
}}

fn minimum_group_box_size(title: qttypes::QString) -> qttypes::QSize {
    cpp!(unsafe [title as "QString"] -> qttypes::QSize as "QSize" {
        ensure_initialized();

        QStyleOptionGroupBox option = create_group_box_option(title);

        QFontMetrics metrics = option.fontMetrics;
        int baseWidth = metrics.horizontalAdvance(title) + metrics.horizontalAdvance(QLatin1Char(' '));
        int baseHeight = metrics.height();

        return qApp->style()->sizeFromContents(QStyle::CT_GroupBox, &option, QSize(baseWidth, baseHeight), nullptr);
    })
}

impl Item for NativeGroupBox {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {
        let animation_tracker_property_ptr = Self::FIELD_OFFSETS.animation_tracker.apply_pin(self);
        self.widget_ptr.set(cpp! { unsafe [animation_tracker_property_ptr as "void*"] -> SlintTypeErasedWidgetPtr as "std::unique_ptr<SlintTypeErasedWidget>"  {
            return make_unique_animated_widget<QGroupBox>(animation_tracker_property_ptr);
        }});

        let shared_data = Rc::pin(GroupBoxData::default());

        Property::link_two_way(
            Self::FIELD_OFFSETS.title.apply_pin(self),
            GroupBoxData::FIELD_OFFSETS.title.apply_pin(shared_data.as_ref()),
        );

        shared_data.paddings.set_binding({
            let shared_data_weak = pin_weak::rc::PinWeak::downgrade(shared_data.clone());
            move || {
                let shared_data = shared_data_weak.upgrade().unwrap();

                let text: qttypes::QString = GroupBoxData::FIELD_OFFSETS.title.apply_pin(shared_data.as_ref()).get().as_str().into();

                cpp!(unsafe [
                    text as "QString"
                ] -> qttypes::QMargins as "QMargins" {
                    ensure_initialized();
                    QStyleOptionGroupBox option = create_group_box_option(text);

                    // Just some size big enough to be sure that the frame fits in it
                    option.rect = QRect(0, 0, 10000, 10000);
                    QRect contentsRect = qApp->style()->subControlRect(
                        QStyle::CC_GroupBox, &option, QStyle::SC_GroupBoxContents);
                    //QRect elementRect = qApp->style()->subElementRect(
                    //    QStyle::SE_GroupBoxLayoutItem, &option);

                    auto hs = qApp->style()->pixelMetric(QStyle::PM_LayoutHorizontalSpacing, &option);
                    auto vs = qApp->style()->pixelMetric(QStyle::PM_LayoutVerticalSpacing, &option);

                    return {
                        (contentsRect.left() + hs),
                        (contentsRect.top() + vs),
                        (option.rect.right() - contentsRect.right() + hs),
                        (option.rect.bottom() - contentsRect.bottom() + vs)
                    };
                })
            }
        });

        self.native_padding_left.set_binding({
            let shared_data = shared_data.clone();
            move || {
                let margins =
                    GroupBoxData::FIELD_OFFSETS.paddings.apply_pin(shared_data.as_ref()).get();
                LogicalLength::new(margins.left as _)
            }
        });

        self.native_padding_right.set_binding({
            let shared_data = shared_data.clone();
            move || {
                let margins =
                    GroupBoxData::FIELD_OFFSETS.paddings.apply_pin(shared_data.as_ref()).get();
                LogicalLength::new(margins.right as _)
            }
        });

        self.native_padding_top.set_binding({
            let shared_data = shared_data.clone();
            move || {
                let margins =
                    GroupBoxData::FIELD_OFFSETS.paddings.apply_pin(shared_data.as_ref()).get();
                LogicalLength::new(margins.top as _)
            }
        });

        self.native_padding_bottom.set_binding({
            move || {
                let margins =
                    GroupBoxData::FIELD_OFFSETS.paddings.apply_pin(shared_data.as_ref()).get();
                LogicalLength::new(margins.bottom as _)
            }
        });
    }

    fn layout_info(
        self: Pin<&Self>,
        orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> LayoutInfo {
        let text: qttypes::QString = self.title().as_str().into();

        let size = minimum_group_box_size(text);

        let min = match orientation {
            Orientation::Horizontal => size.width as f32,
            Orientation::Vertical => size.height as f32,
        };
        LayoutInfo { min, preferred: min, stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: &MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardEvent
    }

    fn input_event(
        self: Pin<&Self>,
        _: &MouseEvent,
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
        let text: qttypes::QString =
            this.title().as_str().into();
        let enabled = this.enabled();

        cpp!(unsafe [
            painter as "QPainterPtr*",
            widget as "QWidget*",
            text as "QString",
            enabled as "bool",
            size as "QSize",
            dpr as "float",
            initial_state as "int"
        ] {
            if (auto groupbox = qobject_cast<QGroupBox *>(widget)) {
                // If not set, the style may render incorrectly
                // https://github.com/qt/qtbase/blob/5be45ff6a6e157d45b0010a4f09d3a11e62fddce/src/widgets/styles/qfusionstyle.cpp#L441
                groupbox->setTitle(text);
            }
            QStyleOptionGroupBox option;
            option.styleObject = widget;
            option.state |= QStyle::State(initial_state);
            if (enabled) {
                option.state |= QStyle::State_Enabled;
            } else {
                option.palette.setCurrentColorGroup(QPalette::Disabled);
            }
            option.rect = QRect(QPoint(), size / dpr);
            option.text = text;
            option.lineWidth = 1;
            option.midLineWidth = 0;
            option.subControls = QStyle::SC_GroupBoxFrame;
            if (!text.isEmpty()) {
                option.subControls |= QStyle::SC_GroupBoxLabel;
            }
            option.textColor = QColor(qApp->style()->styleHint(
                QStyle::SH_GroupBox_TextLabelColor, &option));
            qApp->style()->drawComplexControl(QStyle::CC_GroupBox, &option, painter->get(), widget);
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

impl ItemConsts for NativeGroupBox {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
fn slint_get_NativeGroupBoxVTable() -> NativeGroupBoxVTable for NativeGroupBox
}
