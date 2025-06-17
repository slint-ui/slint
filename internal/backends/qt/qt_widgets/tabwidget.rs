// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore hframe qreal tabbar vframe

use i_slint_core::{
    input::{FocusEventResult, FocusReason},
    platform::PointerEventButton,
};

use super::*;

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct NativeTabWidget {
    pub width: Property<LogicalLength>,
    pub height: Property<LogicalLength>,
    pub cached_rendering_data: CachedRenderingData,
    pub content_min_height: Property<LogicalLength>,
    pub content_min_width: Property<LogicalLength>,
    pub tabbar_preferred_height: Property<LogicalLength>,
    pub tabbar_preferred_width: Property<LogicalLength>,
    pub current_index: Property<i32>,
    pub current_focused: Property<i32>,

    // outputs
    pub content_x: Property<LogicalLength>,
    pub content_y: Property<LogicalLength>,
    pub content_height: Property<LogicalLength>,
    pub content_width: Property<LogicalLength>,
    pub tabbar_x: Property<LogicalLength>,
    pub tabbar_y: Property<LogicalLength>,
    pub tabbar_height: Property<LogicalLength>,
    pub tabbar_width: Property<LogicalLength>,

    widget_ptr: std::cell::Cell<SlintTypeErasedWidgetPtr>,
    animation_tracker: Property<i32>,
}

impl Item for NativeTabWidget {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {
        let animation_tracker_property_ptr = Self::FIELD_OFFSETS.animation_tracker.apply_pin(self);
        self.widget_ptr.set(cpp! { unsafe [animation_tracker_property_ptr as "void*"] -> SlintTypeErasedWidgetPtr as "std::unique_ptr<SlintTypeErasedWidget>" {
            return make_unique_animated_widget<QTabWidget>(animation_tracker_property_ptr);
        }});

        #[derive(Default, Clone)]
        #[repr(C)]
        struct TabWidgetMetrics {
            content_start: qttypes::qreal,
            content_size: qttypes::qreal,
            tabbar_start: qttypes::qreal,
            tabbar_size: qttypes::qreal,
        }
        cpp! {{ struct TabWidgetMetrics { qreal content_start, content_size, tabbar_start, tabbar_size; }; }}

        #[repr(C)]
        #[derive(FieldOffsets, Default)]
        #[pin]
        struct TabBarSharedData {
            width: Property<LogicalLength>,
            height: Property<LogicalLength>,
            tabbar_preferred_height: Property<LogicalLength>,
            tabbar_preferred_width: Property<LogicalLength>,
            horizontal_metrics: Property<TabWidgetMetrics>,
            vertical_metrics: Property<TabWidgetMetrics>,
        }
        let shared_data = Rc::pin(TabBarSharedData::default());
        macro_rules! link {
            ($prop:ident) => {
                Property::link_two_way(
                    Self::FIELD_OFFSETS.$prop.apply_pin(self),
                    TabBarSharedData::FIELD_OFFSETS.$prop.apply_pin(shared_data.as_ref()),
                );
            };
        }
        link!(width);
        link!(height);
        link!(tabbar_preferred_width);
        link!(tabbar_preferred_height);

        let shared_data_weak = pin_weak::rc::PinWeak::downgrade(shared_data.clone());

        let query_tabbar_metrics = move |orientation: Orientation| {
            let shared_data = shared_data_weak.upgrade().unwrap();

            let (size, tabbar_size) = match orientation {
                Orientation::Horizontal => (
                    qttypes::QSizeF {
                        width: TabBarSharedData::FIELD_OFFSETS
                            .width
                            .apply_pin(shared_data.as_ref())
                            .get()
                            .get() as _,
                        height: (std::i32::MAX / 2) as _,
                    },
                    qttypes::QSizeF {
                        width: TabBarSharedData::FIELD_OFFSETS
                            .tabbar_preferred_width
                            .apply_pin(shared_data.as_ref())
                            .get()
                            .get() as _,
                        height: (std::i32::MAX / 2) as _,
                    },
                ),
                Orientation::Vertical => (
                    qttypes::QSizeF {
                        width: (std::i32::MAX / 2) as _,
                        height: TabBarSharedData::FIELD_OFFSETS
                            .height
                            .apply_pin(shared_data.as_ref())
                            .get()
                            .get() as _,
                    },
                    qttypes::QSizeF {
                        width: (std::i32::MAX / 2) as _,
                        height: TabBarSharedData::FIELD_OFFSETS
                            .tabbar_preferred_height
                            .apply_pin(shared_data.as_ref())
                            .get()
                            .get() as _,
                    },
                ),
            };

            let horizontal: bool = matches!(orientation, Orientation::Horizontal);

            cpp!(unsafe [horizontal as "bool", size as "QSizeF", tabbar_size as "QSizeF"] -> TabWidgetMetrics as "TabWidgetMetrics" {
                ensure_initialized();
                QStyleOptionTabWidgetFrame option;
                auto style = qApp->style();
                option.lineWidth = style->pixelMetric(QStyle::PM_DefaultFrameWidth, 0, nullptr);
                option.shape = QTabBar::RoundedNorth;
                option.rect = QRect(QPoint(), size.toSize());
                option.tabBarSize = tabbar_size.toSize();
                option.tabBarRect = QRect(QPoint(), option.tabBarSize);
                option.rightCornerWidgetSize = QSize(0, 0);
                option.leftCornerWidgetSize = QSize(0, 0);
                QRectF contentsRect = style->subElementRect(QStyle::SE_TabWidgetTabContents, &option, nullptr);
                QRectF tabbarRect = style->subElementRect(QStyle::SE_TabWidgetTabBar, &option, nullptr);
                if (horizontal) {
                    return {contentsRect.x(), contentsRect.width(), tabbarRect.x(), tabbarRect.width()};
                } else {
                    return {contentsRect.y(), contentsRect.height(), tabbarRect.y(), tabbarRect.height()};
                }
            })
        };

        shared_data.horizontal_metrics.set_binding({
            let query_tabbar_metrics = query_tabbar_metrics.clone();
            move || query_tabbar_metrics(Orientation::Horizontal)
        });
        shared_data
            .vertical_metrics
            .set_binding(move || query_tabbar_metrics(Orientation::Vertical));

        macro_rules! bind {
            ($prop:ident = $field1:ident.$field2:ident) => {
                let shared_data = shared_data.clone();
                self.$prop.set_binding(move || {
                    let metrics = TabBarSharedData::FIELD_OFFSETS
                        .$field1
                        .apply_pin(shared_data.as_ref())
                        .get();
                    LogicalLength::new(metrics.$field2 as f32)
                });
            };
        }
        bind!(content_x = horizontal_metrics.content_start);
        bind!(content_y = vertical_metrics.content_start);
        bind!(content_width = horizontal_metrics.content_size);
        bind!(content_height = vertical_metrics.content_size);
        bind!(tabbar_x = horizontal_metrics.tabbar_start);
        bind!(tabbar_y = vertical_metrics.tabbar_start);
        bind!(tabbar_width = horizontal_metrics.tabbar_size);
        bind!(tabbar_height = vertical_metrics.tabbar_size);
    }

    fn layout_info(
        self: Pin<&Self>,
        orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> LayoutInfo {
        let (content_size, tabbar_size) = match orientation {
            Orientation::Horizontal => (
                qttypes::QSizeF {
                    width: self.content_min_width().get() as _,
                    height: (std::i32::MAX / 2) as _,
                },
                qttypes::QSizeF {
                    width: self.tabbar_preferred_width().get() as _,
                    height: (std::i32::MAX / 2) as _,
                },
            ),
            Orientation::Vertical => (
                qttypes::QSizeF {
                    width: (std::i32::MAX / 2) as _,
                    height: self.content_min_height().get() as _,
                },
                qttypes::QSizeF {
                    width: (std::i32::MAX / 2) as _,
                    height: self.tabbar_preferred_height().get() as _,
                },
            ),
        };
        let widget: NonNull<()> = SlintTypeErasedWidgetPtr::qwidget_ptr(&self.widget_ptr);

        let size = cpp!(unsafe [content_size as "QSizeF", tabbar_size as "QSizeF", widget as "QWidget*"] -> qttypes::QSize as "QSize" {
            ensure_initialized();

            QStyleOptionTabWidgetFrame option;
            auto style = qApp->style();
            option.lineWidth = style->pixelMetric(QStyle::PM_DefaultFrameWidth, 0, widget);
            option.shape = QTabBar::RoundedNorth;
            option.tabBarSize = tabbar_size.toSize();
            option.rightCornerWidgetSize = QSize(0, 0);
            option.leftCornerWidgetSize = QSize(0, 0);
            auto sz = QSize(qMax(content_size.width(), tabbar_size.width()),
                content_size.height() + tabbar_size.height());
            return style->sizeFromContents(QStyle::CT_TabWidget, &option, sz, widget);
        });
        LayoutInfo {
            min: match orientation {
                Orientation::Horizontal => size.width as f32,
                Orientation::Vertical => size.height as f32,
            },
            preferred: match orientation {
                Orientation::Horizontal => size.width as f32,
                Orientation::Vertical => size.height as f32,
            },
            stretch: 1.,
            ..LayoutInfo::default()
        }
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
        let tabbar_size = qttypes::QSizeF {
            width: this.tabbar_preferred_width().get() as _,
            height: this.tabbar_preferred_height().get() as _,
        };
        cpp!(unsafe [
            painter as "QPainterPtr*",
            widget as "QWidget*",
            size as "QSize",
            dpr as "float",
            tabbar_size as "QSizeF",
            initial_state as "int"
        ] {
            QStyleOptionTabWidgetFrame option;
            option.styleObject = widget;
            option.state |= QStyle::State(initial_state);
            auto style = qApp->style();
            option.lineWidth = style->pixelMetric(QStyle::PM_DefaultFrameWidth, 0, widget);
            option.shape = QTabBar::RoundedNorth;
            if (true /*enabled*/) {
                option.state |= QStyle::State_Enabled;
            } else {
                option.palette.setCurrentColorGroup(QPalette::Disabled);
            }
            option.rect = QRect(QPoint(), size / dpr);
            option.tabBarSize = tabbar_size.toSize();
            option.rightCornerWidgetSize = QSize(0, 0);
            option.leftCornerWidgetSize = QSize(0, 0);
            option.tabBarRect = style->subElementRect(QStyle::SE_TabWidgetTabBar, &option, widget);
            option.rect = style->subElementRect(QStyle::SE_TabWidgetTabPane, &option, widget);
            style->drawPrimitive(QStyle::PE_FrameTabWidget, &option, painter->get(), widget);

            /* -- we don't need to draw the base since we already draw the frame
                QStyleOptionTab tabOverlap;
                tabOverlap.shape = option.shape;
                int overlap = style->pixelMetric(QStyle::PM_TabBarBaseOverlap, &tabOverlap, widget);
                QStyleOptionTabBarBase optTabBase;
                static_cast<QStyleOption&>(optTabBase) = (option);
                optTabBase.shape = option.shape;
                optTabBase.rect = option.tabBarRect;
                if (overlap > 0) {
                    optTabBase.rect.setHeight(optTabBase.rect.height() - overlap);
                }
                optTabBase.tabBarRect = option.tabBarRect;
                optTabBase.selectedTabRect = option.selectedTabRect;
                style->drawPrimitive(QStyle::PE_FrameTabBarBase, &optTabBase, painter->get(), widget);*/
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

impl ItemConsts for NativeTabWidget {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
fn slint_get_NativeTabWidgetVTable() -> NativeTabWidgetVTable for NativeTabWidget
}

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct NativeTab {
    pub title: Property<SharedString>,
    pub icon: Property<i_slint_core::graphics::Image>,
    pub enabled: Property<bool>,
    pub pressed: Property<bool>,
    pub current: Property<i32>,
    pub current_focused: Property<i32>,
    pub tab_index: Property<i32>,
    pub num_tabs: Property<i32>,
    widget_ptr: std::cell::Cell<SlintTypeErasedWidgetPtr>,
    animation_tracker: Property<i32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for NativeTab {
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
        let text: qttypes::QString = self.title().as_str().into();
        let icon: qttypes::QPixmap =
            crate::qt_window::image_to_pixmap((&self.icon()).into(), None).unwrap_or_default();
        let tab_index: i32 = self.tab_index();
        let num_tabs: i32 = self.num_tabs();
        let widget: NonNull<()> = SlintTypeErasedWidgetPtr::qwidget_ptr(&self.widget_ptr);
        let size = cpp!(unsafe [
            text as "QString",
            icon as "QPixmap",
            tab_index as "int",
            num_tabs as "int",
            widget as "QWidget*"
        ] -> qttypes::QSize as "QSize" {
            ensure_initialized();
            QStyleOptionTab option;
            option.rect = option.fontMetrics.boundingRect(text);
            option.text = text;
            option.icon = icon;
            option.shape = QTabBar::RoundedNorth;
            option.position = num_tabs == 1 ? QStyleOptionTab::OnlyOneTab
                : tab_index == 0 ? QStyleOptionTab::Beginning
                : tab_index == num_tabs - 1 ? QStyleOptionTab::End
                : QStyleOptionTab::Middle;
            auto style = qApp->style();
            int hframe = style->pixelMetric(QStyle::PM_TabBarTabHSpace, &option, widget);
            int vframe = style->pixelMetric(QStyle::PM_TabBarTabVSpace, &option, widget);
            int padding = icon.isNull() ? 0 : 4;
            int textWidth = option.fontMetrics.size(Qt::TextShowMnemonic, text).width();
            auto iconSize = icon.isNull() ? 0 : style->pixelMetric(QStyle::PM_TabBarIconSize, nullptr, widget);
            QSize csz = QSize(textWidth + iconSize + hframe + padding, qMax(option.fontMetrics.height(), iconSize) + vframe);
            return style->sizeFromContents(QStyle::CT_TabBarTab, &option, csz, nullptr);
        });
        LayoutInfo {
            min: match orientation {
                // FIXME: the minimum width is arbitrary, Qt uses the size of two letters + ellipses
                Orientation::Horizontal => size.width.min(size.height * 2) as f32,
                Orientation::Vertical => size.height as f32,
            },
            preferred: match orientation {
                Orientation::Horizontal => size.width as f32,
                Orientation::Vertical => size.height as f32,
            },
            ..LayoutInfo::default()
        }
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
        event: &MouseEvent,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &i_slint_core::items::ItemRc,
    ) -> InputEventResult {
        let enabled = self.enabled();
        if !enabled {
            return InputEventResult::EventIgnored;
        }

        Self::FIELD_OFFSETS.pressed.apply_pin(self).set(match event {
            MouseEvent::Pressed { button, .. } => *button == PointerEventButton::Left,
            MouseEvent::Exit | MouseEvent::Released { .. } => false,
            MouseEvent::Moved { .. } => {
                return if self.pressed() {
                    InputEventResult::GrabMouse
                } else {
                    InputEventResult::EventIgnored
                }
            }
            MouseEvent::Wheel { .. } => return InputEventResult::EventIgnored,
            MouseEvent::DragMove(..) | MouseEvent::Drop(..) => {
                return InputEventResult::EventIgnored
            }
        });
        let click_on_press = cpp!(unsafe [] -> bool as "bool" {
            return qApp->style()->styleHint(QStyle::SH_TabBar_SelectMouseType, nullptr, nullptr) == QEvent::MouseButtonPress;
        });
        if matches!(event, MouseEvent::Released { button: PointerEventButton::Left, .. } if !click_on_press)
            || matches!(event, MouseEvent::Pressed { button: PointerEventButton::Left, .. } if click_on_press)
        {
            WindowInner::from_pub(window_adapter.window()).set_focus_item(
                self_rc,
                true,
                FocusReason::PointerClick,
            );
            self.current.set(self.tab_index());
            InputEventResult::EventAccepted
        } else {
            InputEventResult::GrabMouse
        }
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
        let text: qttypes::QString = this.title().as_str().into();
        let icon: qttypes::QPixmap = crate::qt_window::image_to_pixmap(
            (&this.icon()).into(),
            None,
        )
        .unwrap_or_default();
        let enabled: bool = this.enabled();
        let current: i32 = this.current();
        let current_focused: i32 = this.current_focused();
        let tab_index: i32 = this.tab_index();
        let num_tabs: i32 = this.num_tabs();

        cpp!(unsafe [
            painter as "QPainterPtr*",
            widget as "QWidget*",
            text as "QString",
            icon as "QPixmap",
            enabled as "bool",
            size as "QSize",
            down as "bool",
            dpr as "float",
            tab_index as "int",
            current as "int",
            current_focused as "int",
            num_tabs as "int",
            initial_state as "int"
        ] {
            ensure_initialized();
            QStyleOptionTab option;
            option.styleObject = widget;
            option.state |= QStyle::State(initial_state);
            option.rect = QRect(QPoint(), size / dpr);;
            option.text = text;
            option.icon = icon;
            option.shape = QTabBar::RoundedNorth;
            option.position = num_tabs == 1 ? QStyleOptionTab::OnlyOneTab
                : tab_index == 0 ? QStyleOptionTab::Beginning
                : tab_index == num_tabs - 1 ? QStyleOptionTab::End
                : QStyleOptionTab::Middle;
            /* -- does not render correctly with the fusion style because we don't draw the selected on top
                option.selectedPosition = current == tab_index - 1 ? QStyleOptionTab::NextIsSelected
                    : current == tab_index + 1 ? QStyleOptionTab::PreviousIsSelected : QStyleOptionTab::NotAdjacent;*/
            if (down)
                option.state |= QStyle::State_Sunken;
            else
                option.state |= QStyle::State_Raised;
            if (enabled) {
                option.state |= QStyle::State_Enabled;
            } else {
                option.palette.setCurrentColorGroup(QPalette::Disabled);
            }
            if (current == tab_index)
                option.state |= QStyle::State_Selected;
            if (current_focused == tab_index) {
                option.state |= QStyle::State_HasFocus | QStyle::State_KeyboardFocusChange | QStyle::State_Item;
            }
            option.features |= QStyleOptionTab::HasFrame;
            qApp->style()->drawControl(QStyle::CE_TabBarTab, &option, painter->get(), widget);
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

impl ItemConsts for NativeTab {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
fn slint_get_NativeTabVTable() -> NativeTabVTable for NativeTab
}
