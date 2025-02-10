// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::input::FocusEventResult;
use i_slint_core::items::ScrollBarPolicy;

use super::*;

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct NativeScrollView {
    pub horizontal_max: Property<LogicalLength>,
    pub horizontal_page_size: Property<LogicalLength>,
    pub horizontal_value: Property<LogicalLength>,
    pub vertical_max: Property<LogicalLength>,
    pub vertical_page_size: Property<LogicalLength>,
    pub vertical_value: Property<LogicalLength>,
    pub cached_rendering_data: CachedRenderingData,
    pub native_padding_left: Property<LogicalLength>,
    pub native_padding_right: Property<LogicalLength>,
    pub native_padding_top: Property<LogicalLength>,
    pub native_padding_bottom: Property<LogicalLength>,
    pub enabled: Property<bool>,
    pub has_focus: Property<bool>,
    pub vertical_scrollbar_policy: Property<ScrollBarPolicy>,
    pub horizontal_scrollbar_policy: Property<ScrollBarPolicy>,
    data: Property<NativeSliderData>,
    widget_ptr: std::cell::Cell<SlintTypeErasedWidgetPtr>,
    animation_tracker: Property<i32>,
    // TODO: allocate two widgets for each scrollbar and a tracker for each as well,
    // for animated scrollbars...
}

impl Item for NativeScrollView {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {
        let animation_tracker_property_ptr = Self::FIELD_OFFSETS.animation_tracker.apply_pin(self);
        self.widget_ptr.set(cpp! { unsafe [animation_tracker_property_ptr as "void*"] -> SlintTypeErasedWidgetPtr as "std::unique_ptr<SlintTypeErasedWidget>"  {
            return make_unique_animated_widget<QWidget>(animation_tracker_property_ptr);
        }});

        let paddings = Rc::pin(Property::default());

        paddings.as_ref().set_binding(move || {
        cpp!(unsafe [] -> qttypes::QMargins as "QMargins" {
            ensure_initialized();
            QStyleOptionSlider option;
            initQSliderOptions(option, false, true, 0, 0, 1000, 1000, false);

            int extent = qApp->style()->pixelMetric(QStyle::PM_ScrollBarExtent, &option, nullptr);
            int sliderMin = qApp->style()->pixelMetric(QStyle::PM_ScrollBarSliderMin, &option, nullptr);
            auto horizontal_size = qApp->style()->sizeFromContents(QStyle::CT_ScrollBar, &option, QSize(extent * 2 + sliderMin, extent), nullptr);
            option.state ^= QStyle::State_Horizontal;
            option.orientation = Qt::Vertical;
            extent = qApp->style()->pixelMetric(QStyle::PM_ScrollBarExtent, &option, nullptr);
            sliderMin = qApp->style()->pixelMetric(QStyle::PM_ScrollBarSliderMin, &option, nullptr);
            auto vertical_size = qApp->style()->sizeFromContents(QStyle::CT_ScrollBar, &option, QSize(extent, extent * 2 + sliderMin), nullptr);

            QStyleOptionFrame frameOption;
            frameOption.rect = QRect(QPoint(), QSize(1000, 1000));
            frameOption.frameShape = QFrame::StyledPanel;
            frameOption.lineWidth = 1;
            frameOption.midLineWidth = 0;
            QRect cr = qApp->style()->subElementRect(QStyle::SE_ShapedFrameContents, &frameOption, nullptr);
            return {
                cr.left(),
                cr.top(),
                (vertical_size.width() + frameOption.rect.right() - cr.right()),
                (horizontal_size.height() + frameOption.rect.bottom() - cr.bottom())
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
        LayoutInfo { min, preferred: min, stretch: 1., ..LayoutInfo::default() }
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
        event: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &i_slint_core::items::ItemRc,
    ) -> InputEventResult {
        let size: qttypes::QSize = get_size!(self_rc);
        let mut data = self.data();
        let active_controls = data.active_controls;
        let pressed = data.pressed;
        let left = self.native_padding_left().get();
        let right = self.native_padding_right().get();
        let top = self.native_padding_top().get();
        let bottom = self.native_padding_bottom().get();

        let mut handle_scrollbar = |horizontal: bool,
                                    pos: qttypes::QPoint,
                                    size: qttypes::QSize,
                                    value_prop: Pin<&Property<LogicalLength>>,
                                    page_size: i32,
                                    max: i32| {
            let pressed: bool = data.pressed != 0;
            let value: i32 = value_prop.get().get() as i32;
            let new_control = cpp!(unsafe [
                pos as "QPoint",
                value as "int",
                page_size as "int",
                max as "int",
                size as "QSize",
                active_controls as "int",
                pressed as "bool",
                horizontal as "bool"
            ] -> u32 as "int" {
                ensure_initialized();
                QStyleOptionSlider option;
                initQSliderOptions(option, pressed, true, active_controls, 0, max, -value, false);
                option.pageStep = page_size;
                if (!horizontal) {
                    option.state ^= QStyle::State_Horizontal;
                    option.orientation = Qt::Vertical;
                }
                auto style = qApp->style();
                option.rect = { QPoint{}, size };
                return style->hitTestComplexControl(QStyle::CC_ScrollBar, &option, pos, nullptr);
            });

            #[allow(non_snake_case)]
            let SC_ScrollBarSlider =
                cpp!(unsafe []->u32 as "int" { return QStyle::SC_ScrollBarSlider;});

            let (pos, size) = if horizontal { (pos.x, size.width) } else { (pos.y, size.height) };

            let result = match event {
                MouseEvent::Pressed { .. } => {
                    data.pressed = if horizontal { 1 } else { 2 };
                    if new_control == SC_ScrollBarSlider {
                        data.pressed_x = pos as f32;
                        data.pressed_val = -value as f32;
                    }
                    data.active_controls = new_control;
                    InputEventResult::GrabMouse
                }
                MouseEvent::Exit => {
                    data.pressed = 0;
                    InputEventResult::EventIgnored
                }
                MouseEvent::Released { .. } => {
                    data.pressed = 0;
                    let new_val = cpp!(unsafe [active_controls as "int", value as "int", max as "int", page_size as "int"] -> i32 as "int" {
                        switch (active_controls) {
                            case QStyle::SC_ScrollBarAddPage:
                                return -value + page_size;
                            case QStyle::SC_ScrollBarSubPage:
                                return -value - page_size;
                            case QStyle::SC_ScrollBarAddLine:
                                return -value + 3.;
                            case QStyle::SC_ScrollBarSubLine:
                                return -value - 3.;
                            case QStyle::SC_ScrollBarFirst:
                                return 0;
                            case QStyle::SC_ScrollBarLast:
                                return max;
                            default:
                                return -value;
                        }
                    });
                    value_prop.set(LogicalLength::new(-(new_val.min(max).max(0) as f32)));
                    InputEventResult::EventIgnored
                }
                MouseEvent::Moved { .. } => {
                    if data.pressed != 0 && data.active_controls == SC_ScrollBarSlider {
                        let max = max as f32;
                        let new_val = data.pressed_val
                            + ((pos as f32) - data.pressed_x) * (max + (page_size as f32))
                                / size as f32;
                        value_prop.set(LogicalLength::new(-new_val.min(max).max(0.)));
                        InputEventResult::GrabMouse
                    } else {
                        InputEventResult::EventAccepted
                    }
                }
                MouseEvent::Wheel { delta_x, delta_y, .. } => {
                    if horizontal {
                        let max = max as f32;
                        let new_val = value as f32 + delta_x;
                        value_prop.set(LogicalLength::new(new_val.min(0.).max(-max)));
                    } else {
                        let max = max as f32;
                        let new_val = value as f32 + delta_y;
                        value_prop.set(LogicalLength::new(new_val.min(0.).max(-max)));
                    }
                    InputEventResult::EventAccepted
                }
            };
            self.data.set(data);
            result
        };

        let pos = event.position().unwrap_or_default();

        if pressed == 2 || (pressed == 0 && pos.x > (size.width as f32 - right)) {
            handle_scrollbar(
                false,
                qttypes::QPoint {
                    x: (pos.x - (size.width as f32 - right)) as _,
                    y: (pos.y - top) as _,
                },
                qttypes::QSize {
                    width: (right - left) as _,
                    height: (size.height as f32 - (bottom + top)) as _,
                },
                Self::FIELD_OFFSETS.vertical_value.apply_pin(self),
                self.vertical_page_size().get() as i32,
                self.vertical_max().get() as i32,
            )
        } else if pressed == 1 || pos.y > (size.height as f32 - bottom) {
            handle_scrollbar(
                true,
                qttypes::QPoint {
                    x: (pos.x - left) as _,
                    y: (pos.y - (size.height as f32 - bottom)) as _,
                },
                qttypes::QSize {
                    width: (size.width as f32 - (right + left)) as _,
                    height: (bottom - top) as _,
                },
                Self::FIELD_OFFSETS.horizontal_value.apply_pin(self),
                self.horizontal_page_size().get() as i32,
                self.horizontal_max().get() as i32,
            )
        } else {
            Default::default()
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

        let data = this.data();
        let margins = qttypes::QMargins {
            left: this.native_padding_left().get() as _,
            top: this.native_padding_top().get() as _,
            right: this.native_padding_right().get() as _,
            bottom: this.native_padding_bottom().get() as _,
        };
        let enabled: bool = this.enabled();
        let has_focus: bool = this.has_focus();
        let vertical_bar_visible = (this.vertical_scrollbar_policy() == ScrollBarPolicy::AlwaysOn) || ((this.vertical_scrollbar_policy() == ScrollBarPolicy::AsNeeded) && (this.vertical_max().get() > 0.0));
        let horizontal_bar_visible = (this.horizontal_scrollbar_policy() == ScrollBarPolicy::AlwaysOn) || ((this.horizontal_scrollbar_policy() == ScrollBarPolicy::AsNeeded) && (this.horizontal_max().get() > 0.0));
        let scrollbar_bar_visible = vertical_bar_visible || horizontal_bar_visible;
        let frame_around_contents = cpp!(unsafe [
            painter as "QPainterPtr*",
            widget as "QWidget*",
            size as "QSize",
            dpr as "float",
            margins as "QMargins",
            enabled as "bool",
            has_focus as "bool",
            initial_state as "int",
            scrollbar_bar_visible as "bool"
        ] -> bool as "bool" {
            ensure_initialized();
            QStyleOptionFrame frameOption;
            frameOption.styleObject = widget;
            frameOption.state |= QStyle::State(initial_state);
            frameOption.frameShape = QFrame::StyledPanel;

            frameOption.lineWidth = 1;
            frameOption.midLineWidth = 0;
            frameOption.rect = QRect(QPoint(), size / dpr);
            frameOption.state |= QStyle::State_Sunken;
            if (enabled) {
                frameOption.state |= QStyle::State_Enabled;
            } else {
                frameOption.palette.setCurrentColorGroup(QPalette::Disabled);
            }
            if (has_focus)
                frameOption.state |= QStyle::State_HasFocus;
            //int scrollOverlap = qApp->style()->pixelMetric(QStyle::PM_ScrollView_ScrollBarOverlap, &frameOption, nullptr);
            bool foac = qApp->style()->styleHint(QStyle::SH_ScrollView_FrameOnlyAroundContents, &frameOption, widget);
            // this assume that the frame size is the same on both side, so that the scrollbar width is (right-left)
            QSize corner_size = QSize(margins.right() - margins.left(), margins.bottom() - margins.top());
            if (foac) {
                frameOption.rect = QRect(QPoint(), (size / dpr) - corner_size);
                qApp->style()->drawControl(QStyle::CE_ShapedFrame, &frameOption, painter->get(), widget);
                frameOption.rect = QRect(frameOption.rect.bottomRight() + QPoint(1, 1), corner_size);
                if (scrollbar_bar_visible) {
                    qApp->style()->drawPrimitive(QStyle::PE_PanelScrollAreaCorner, &frameOption, painter->get(), widget);
                }
            } else {
                qApp->style()->drawControl(QStyle::CE_ShapedFrame, &frameOption, painter->get(), widget);
                frameOption.rect = QRect(frameOption.rect.bottomRight() + QPoint(1, 1) - QPoint(margins.right(), margins.bottom()), corner_size);
                if (scrollbar_bar_visible) {
                    qApp->style()->drawPrimitive(QStyle::PE_PanelScrollAreaCorner, &frameOption, painter->get(), widget);
                }
            }
            return foac;
        });

        let draw_scrollbar = |horizontal: bool,
                              rect: qttypes::QRectF,
                              value: i32,
                              page_size: i32,
                              max: i32,
                              active_controls: u32,
                              pressed: bool,
                              initial_state: i32| {
            cpp!(unsafe [
                painter as "QPainterPtr*",
                widget as "QWidget*",
                value as "int",
                page_size as "int",
                max as "int",
                rect as "QRectF",
                active_controls as "int",
                pressed as "bool",
                dpr as "float",
                horizontal as "bool",
                has_focus as "bool",
                initial_state as "int"
            ] {
                QPainter *painter_ = painter->get();
                auto r = rect.toAlignedRect();
                // The mac style may crash on invalid rectangles (#595)
                if (!r.isValid())
                    return;
                // The mac style ignores painter translations (due to CGContextRef redirection) as well as
                // option.rect's top-left - hence this hack with an intermediate buffer.
            #if defined(Q_OS_MAC)
                QImage scrollbar_image(r.size(), QImage::Format_ARGB32_Premultiplied);
                scrollbar_image.fill(Qt::transparent);
                {QPainter p(&scrollbar_image); QPainter *painter_ = &p;
            #else
                painter_->save();
                auto cleanup = qScopeGuard([&] { painter_->restore(); });
                painter_->translate(r.topLeft()); // There is bugs in the styles if the scrollbar is not in (0,0)
            #endif
                QStyleOptionSlider option;
                option.state |= QStyle::State(initial_state);
                option.rect = QRect(QPoint(), r.size());
                initQSliderOptions(option, pressed, true, active_controls, 0, max / dpr, -value / dpr, false);
                option.subControls = QStyle::SC_All;
                option.pageStep = page_size / dpr;
                if (has_focus)
                    option.state |= QStyle::State_HasFocus;

                if (!horizontal) {
                    option.state ^= QStyle::State_Horizontal;
                    option.orientation = Qt::Vertical;
                }

                auto style = qApp->style();
                style->drawComplexControl(QStyle::CC_ScrollBar, &option, painter_, widget);
            #if defined(Q_OS_MAC)
                }
                (painter_)->drawImage(r.topLeft(), scrollbar_image);
            #endif
            });
        };

        let scrollbars_width = (margins.right - margins.left) as f32;
        let scrollbars_height = (margins.bottom - margins.top) as f32;
        if vertical_bar_visible {
            draw_scrollbar(
                false,
                qttypes::QRectF {
                    x: ((size.width as f32 / dpr) - if frame_around_contents { scrollbars_width } else { margins.right as _ }) as _,
                    y: (if frame_around_contents { 0 } else { margins.top }) as _,
                    width: scrollbars_width as _,
                    height: ((size.height as f32 / dpr) - if frame_around_contents { scrollbars_height } else { (margins.bottom + margins.top) as f32 }) as _,
                },
                this.vertical_value().get() as i32,
                this.vertical_page_size().get() as i32,
                this.vertical_max().get() as i32,
                data.active_controls,
                data.pressed == 2,
                initial_state
            );
        }
        if horizontal_bar_visible {
            draw_scrollbar(
                true,
                qttypes::QRectF {
                    x: (if frame_around_contents { 0 } else { margins.left }) as _,
                    y: ((size.height as f32 / dpr) - if frame_around_contents { scrollbars_height } else { margins.bottom as _ }) as _,
                    width: ((size.width as f32 / dpr) - if frame_around_contents { scrollbars_width } else { (margins.left + margins.right) as _ }) as _,
                    height: (scrollbars_height) as _,
                },
                this.horizontal_value().get() as i32,
                this.horizontal_page_size().get() as i32,
                this.horizontal_max().get() as i32,
                data.active_controls,
                data.pressed == 1,
                initial_state
            );
        }
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

impl ItemConsts for NativeScrollView {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
fn slint_get_NativeScrollViewVTable() -> NativeScrollViewVTable for NativeScrollView
}
