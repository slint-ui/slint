/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use super::*;

#[repr(C)]
#[derive(FieldOffsets, Default, SixtyFPSElement)]
#[pin]
pub struct NativeTabWidget {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
    pub content_min_height: Property<f32>,
    pub content_min_width: Property<f32>,
    pub tabbar_preferred_height: Property<f32>,
    pub tabbar_preferred_width: Property<f32>,

    // outputs
    pub content_x: Property<f32>,
    pub content_y: Property<f32>,
    pub content_height: Property<f32>,
    pub content_width: Property<f32>,
    pub tabbar_x: Property<f32>,
    pub tabbar_y: Property<f32>,
    pub tabbar_height: Property<f32>,
    pub tabbar_width: Property<f32>,
}

impl Item for NativeTabWidget {
    fn init(self: Pin<&Self>, _window: &WindowRc) {
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
            width: Property<f32>,
            height: Property<f32>,
            tabbar_preferred_height: Property<f32>,
            tabbar_preferred_width: Property<f32>,
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
                            .get() as _,
                        height: (std::i32::MAX / 2) as _,
                    },
                    qttypes::QSizeF {
                        width: TabBarSharedData::FIELD_OFFSETS
                            .tabbar_preferred_width
                            .apply_pin(shared_data.as_ref())
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
                            .get() as _,
                    },
                    qttypes::QSizeF {
                        width: (std::i32::MAX / 2) as _,
                        height: TabBarSharedData::FIELD_OFFSETS
                            .tabbar_preferred_height
                            .apply_pin(shared_data.as_ref())
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
                    metrics.$field2 as f32
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

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layouting_info(
        self: Pin<&Self>,
        orientation: Orientation,
        _window: &WindowRc,
    ) -> LayoutInfo {
        let (content_size, tabbar_size) = match orientation {
            Orientation::Horizontal => (
                qttypes::QSizeF {
                    width: self.content_min_width() as _,
                    height: (std::i32::MAX / 2) as _,
                },
                qttypes::QSizeF {
                    width: self.tabbar_preferred_width() as _,
                    height: (std::i32::MAX / 2) as _,
                },
            ),
            Orientation::Vertical => (
                qttypes::QSizeF {
                    width: (std::i32::MAX / 2) as _,
                    height: self.content_min_height() as _,
                },
                qttypes::QSizeF {
                    width: (std::i32::MAX / 2) as _,
                    height: self.tabbar_preferred_height() as _,
                },
            ),
        };

        let size = cpp!(unsafe [content_size as "QSizeF", tabbar_size as "QSizeF"] -> qttypes::QSize as "QSize" {
            ensure_initialized();

            QStyleOptionTabWidgetFrame option;
            auto style = qApp->style();
            option.lineWidth = style->pixelMetric(QStyle::PM_DefaultFrameWidth, 0, nullptr);
            option.shape = QTabBar::RoundedNorth;
            option.tabBarSize = tabbar_size.toSize();
            option.rightCornerWidgetSize = QSize(0, 0);
            option.leftCornerWidgetSize = QSize(0, 0);
            auto sz = QSize(qMax(content_size.width(), tabbar_size.width()),
                content_size.height() + tabbar_size.height());
            return style->sizeFromContents(QStyle::CT_TabWidget, &option, sz, nullptr);
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
        _self_rc: &sixtyfps_corelib::items::ItemRc,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &WindowRc) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &WindowRc) {}

    fn_render! { this dpr size painter initial_state =>
        let tabbar_size = qttypes::QSizeF {
            width: this.tabbar_preferred_width() as _,
            height: this.tabbar_preferred_height() as _,
        };
        cpp!(unsafe [
            painter as "QPainter*",
            size as "QSize",
            dpr as "float",
            tabbar_size as "QSizeF",
            initial_state as "int"
        ] {
            QStyleOptionTabWidgetFrame option;
            option.state |= QStyle::State(initial_state);
            auto style = qApp->style();
            option.lineWidth = style->pixelMetric(QStyle::PM_DefaultFrameWidth, 0, nullptr);
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
            option.tabBarRect = style->subElementRect(QStyle::SE_TabWidgetTabBar, &option, nullptr);
            option.rect = style->subElementRect(QStyle::SE_TabWidgetTabPane, &option, nullptr);
            style->drawPrimitive(QStyle::PE_FrameTabWidget, &option, painter, nullptr);

            /* -- we don't need to draw the base since we already draw the frame
                QStyleOptionTab tabOverlap;
                tabOverlap.shape = option.shape;
                int overlap = style->pixelMetric(QStyle::PM_TabBarBaseOverlap, &tabOverlap, nullptr);
                QStyleOptionTabBarBase optTabBase;
                static_cast<QStyleOption&>(optTabBase) = (option);
                optTabBase.shape = option.shape;
                optTabBase.rect = option.tabBarRect;
                if (overlap > 0) {
                    optTabBase.rect.setHeight(optTabBase.rect.height() - overlap);
                }
                optTabBase.tabBarRect = option.tabBarRect;
                optTabBase.selectedTabRect = option.selectedTabRect;
                style->drawPrimitive(QStyle::PE_FrameTabBarBase, &optTabBase, painter, nullptr);*/
        });
    }
}

impl ItemConsts for NativeTabWidget {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
fn sixtyfps_get_NativeTabWidgetVTable() -> NativeTabWidgetVTable for NativeTabWidget
}

#[repr(C)]
#[derive(FieldOffsets, Default, SixtyFPSElement)]
#[pin]
pub struct NativeTab {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub title: Property<SharedString>,
    pub icon: Property<sixtyfps_corelib::graphics::Image>,
    pub enabled: Property<bool>,
    pub pressed: Property<bool>,
    pub current: Property<i32>,
    pub num_tabs: Property<i32>,
    pub tab_index: Property<i32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for NativeTab {
    fn init(self: Pin<&Self>, _window: &WindowRc) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layouting_info(
        self: Pin<&Self>,
        orientation: Orientation,
        _window: &WindowRc,
    ) -> LayoutInfo {
        let text: qttypes::QString = self.title().as_str().into();
        let icon: qttypes::QPixmap = crate::qt_window::load_image_from_resource(
            (&self.icon()).into(),
            None,
            Default::default(),
        )
        .unwrap_or_default();
        let tab_index: i32 = self.tab_index();
        let num_tabs: i32 = self.num_tabs();
        let size = cpp!(unsafe [
            text as "QString",
            icon as "QPixmap",
            tab_index as "int",
            num_tabs as "int"
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
            int hframe = style->pixelMetric(QStyle::PM_TabBarTabHSpace, &option, nullptr);
            int vframe = style->pixelMetric(QStyle::PM_TabBarTabVSpace, &option, nullptr);
            int padding = icon.isNull() ? 0 : 4;
            int textWidth = option.fontMetrics.size(Qt::TextShowMnemonic, text).width();
            auto iconSize = icon.isNull() ? 0 : style->pixelMetric(QStyle::PM_TabBarIconSize, nullptr, nullptr);
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
        _: MouseEvent,
        _window: &WindowRc,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardEvent
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        _window: &WindowRc,
        _self_rc: &sixtyfps_corelib::items::ItemRc,
    ) -> InputEventResult {
        let enabled = self.enabled();
        if !enabled {
            return InputEventResult::EventIgnored;
        }

        Self::FIELD_OFFSETS.pressed.apply_pin(self).set(match event {
            MouseEvent::MousePressed { .. } => true,
            MouseEvent::MouseExit | MouseEvent::MouseReleased { .. } => false,
            MouseEvent::MouseMoved { .. } => {
                return if self.pressed() {
                    InputEventResult::GrabMouse
                } else {
                    InputEventResult::EventIgnored
                }
            }
            MouseEvent::MouseWheel { .. } => return InputEventResult::EventIgnored,
        });
        let click_on_press = cpp!(unsafe [] -> bool as "bool" {
            return qApp->style()->styleHint(QStyle::SH_TabBar_SelectMouseType, nullptr, nullptr) == QEvent::MouseButtonPress;
        });
        if matches!(event, MouseEvent::MouseReleased { .. } if !click_on_press)
            || matches!(event, MouseEvent::MousePressed { .. } if click_on_press)
        {
            self.current.set(self.tab_index());
            InputEventResult::EventAccepted
        } else {
            InputEventResult::GrabMouse
        }
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &WindowRc) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &WindowRc) {}

    fn_render! { this dpr size painter initial_state =>
        let down: bool = this.pressed();
        let text: qttypes::QString = this.title().as_str().into();
        let icon: qttypes::QPixmap = crate::qt_window::load_image_from_resource(
            (&this.icon()).into(),
            None,
            Default::default(),
        )
        .unwrap_or_default();
        let enabled: bool = this.enabled();
        let current: i32 = this.current();
        let tab_index: i32 = this.tab_index();
        let num_tabs: i32 = this.num_tabs();

        cpp!(unsafe [
            painter as "QPainter*",
            text as "QString",
            icon as "QPixmap",
            enabled as "bool",
            size as "QSize",
            down as "bool",
            dpr as "float",
            tab_index as "int",
            current as "int",
            num_tabs as "int",
            initial_state as "int"
        ] {
            ensure_initialized();
            QStyleOptionTab option;
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
            qApp->style()->drawControl(QStyle::CE_TabBarTab, &option, painter, nullptr);
        });
    }
}

impl ItemConsts for NativeTab {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
fn sixtyfps_get_NativeTabVTable() -> NativeTabVTable for NativeTab
}
