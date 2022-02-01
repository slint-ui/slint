// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

use super::*;
#[repr(C)]
#[derive(FieldOffsets, Default, SixtyFPSElement)]
#[pin]
pub struct NativeGroupBox {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub enabled: Property<bool>,
    pub title: Property<SharedString>,
    pub cached_rendering_data: CachedRenderingData,
    pub native_padding_left: Property<f32>,
    pub native_padding_right: Property<f32>,
    pub native_padding_top: Property<f32>,
    pub native_padding_bottom: Property<f32>,
}

#[repr(C)]
#[derive(FieldOffsets, Default)]
#[pin]
struct GroupBoxData {
    title: Property<SharedString>,
    paddings: Property<qttypes::QMargins>,
}

impl Item for NativeGroupBox {
    fn init(self: Pin<&Self>, _window: &WindowRc) {
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
                QStyleOptionGroupBox option;
                option.text = text;
                option.lineWidth = 1;
                option.midLineWidth = 0;
                option.subControls = QStyle::SC_GroupBoxFrame;
                if (!text.isEmpty()) {
                    option.subControls |= QStyle::SC_GroupBoxLabel;
                }
                 // Just some size big enough to be sure that the frame fits in it
                option.rect = QRect(0, 0, 10000, 10000);
                option.textColor = QColor(qApp->style()->styleHint(
                    QStyle::SH_GroupBox_TextLabelColor, &option));
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
                margins.left as _
            }
        });

        self.native_padding_right.set_binding({
            let shared_data = shared_data.clone();
            move || {
                let margins =
                    GroupBoxData::FIELD_OFFSETS.paddings.apply_pin(shared_data.as_ref()).get();
                margins.right as _
            }
        });

        self.native_padding_top.set_binding({
            let shared_data = shared_data.clone();
            move || {
                let margins =
                    GroupBoxData::FIELD_OFFSETS.paddings.apply_pin(shared_data.as_ref()).get();
                margins.top as _
            }
        });

        self.native_padding_bottom.set_binding({
            move || {
                let margins =
                    GroupBoxData::FIELD_OFFSETS.paddings.apply_pin(shared_data.as_ref()).get();
                margins.bottom as _
            }
        });
    }

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layout_info(self: Pin<&Self>, orientation: Orientation, _window: &WindowRc) -> LayoutInfo {
        LayoutInfo {
            min: match orientation {
                Orientation::Horizontal => self.native_padding_left() + self.native_padding_right(),
                Orientation::Vertical => self.native_padding_top() + self.native_padding_bottom(),
            },
            stretch: 1.,
            ..LayoutInfo::default()
        }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &WindowRc,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardEvent
    }

    fn input_event(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &WindowRc,
        _self_rc: &slint_core_internal::items::ItemRc,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &WindowRc) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &WindowRc) {}

    fn_render! { this dpr size painter widget initial_state =>
        let text: qttypes::QString =
            this.title().as_str().into();
        let enabled = this.enabled();

        cpp!(unsafe [
            painter as "QPainter*",
            widget as "QWidget*",
            text as "QString",
            enabled as "bool",
            size as "QSize",
            dpr as "float",
            initial_state as "int"
        ] {
            QStyleOptionGroupBox option;
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
            qApp->style()->drawComplexControl(QStyle::CC_GroupBox, &option, painter, widget);
        });
    }
}

impl ItemConsts for NativeGroupBox {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
fn sixtyfps_get_NativeGroupBoxVTable() -> NativeGroupBoxVTable for NativeGroupBox
}
