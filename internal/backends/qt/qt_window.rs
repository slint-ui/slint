// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore antialiasing frameless qbrush qpointf qreal qwidgetsize svgz

use cpp::*;
use euclid::approxeq::ApproxEq;
use i_slint_core::component::{ComponentRc, ComponentRef};
use i_slint_core::graphics::euclid::num::Zero;
use i_slint_core::graphics::rendering_metrics_collector::{
    RenderingMetrics, RenderingMetricsCollector,
};
use i_slint_core::graphics::{euclid, Brush, Color, FontRequest, Image, Point, SharedImageBuffer};
use i_slint_core::input::{KeyEventType, KeyInputEvent, MouseEvent};
use i_slint_core::item_rendering::{ItemCache, ItemRenderer};
use i_slint_core::items::{
    self, FillRule, ImageRendering, InputType, ItemRc, ItemRef, Layer, MouseCursor, Opacity,
    PointerEventButton, RenderingResult, TextOverflow, TextWrap, WindowItem,
};
use i_slint_core::layout::Orientation;
use i_slint_core::lengths::{
    LogicalLength, LogicalPoint, LogicalRect, LogicalSize, LogicalVector, PhysicalPx, ScaleFactor,
};
use i_slint_core::platform::WindowEvent;
use i_slint_core::window::{WindowAdapter, WindowAdapterSealed, WindowInner};
use i_slint_core::{ImageInner, PathData, Property, SharedString};
use items::{ImageFit, TextHorizontalAlignment, TextVerticalAlignment};

use std::cell::RefCell;
use std::pin::Pin;
use std::ptr::NonNull;
use std::rc::{Rc, Weak};

use crate::key_generated;
use i_slint_core::renderer::Renderer;
use once_cell::unsync::OnceCell;

cpp! {{
    #include <QtWidgets/QtWidgets>
    #include <QtWidgets/QGraphicsScene>
    #include <QtWidgets/QGraphicsBlurEffect>
    #include <QtWidgets/QGraphicsPixmapItem>
    #include <QtGui/QAccessible>
    #include <QtGui/QPainter>
    #include <QtGui/QPaintEngine>
    #include <QtGui/QPainterPath>
    #include <QtGui/QWindow>
    #include <QtGui/QResizeEvent>
    #include <QtGui/QTextLayout>
    #include <QtGui/QImageReader>
    #include <QtGui/QCursor>
    #include <QtCore/QBasicTimer>
    #include <QtCore/QTimer>
    #include <QtCore/QPointer>
    #include <QtCore/QBuffer>
    #include <QtCore/QEvent>
    #include <QtCore/QFileInfo>

    #include <memory>

    void ensure_initialized(bool from_qt_backend);

    using QPainterPtr = std::unique_ptr<QPainter>;

    struct TimerHandler : QObject {
        QBasicTimer timer;
        static TimerHandler& instance() {
            static TimerHandler instance;
            return instance;
        }

        void timerEvent(QTimerEvent *event) override {
            if (event->timerId() != timer.timerId()) {
                QObject::timerEvent(event);
                return;
            }
            timer.stop();
            rust!(Slint_timerEvent [] { timer_event() });
        }

    };

    struct SlintWidget : QWidget {
        void *rust_window;
        bool isMouseButtonDown = false;
        QPoint ime_position;

        SlintWidget() {
            setMouseTracking(true);
            setFocusPolicy(Qt::StrongFocus);
        }

        void paintEvent(QPaintEvent *) override {
            auto painter = std::unique_ptr<QPainter>(new QPainter(this));
            painter->setClipRect(rect());
            painter->setRenderHints(QPainter::Antialiasing | QPainter::SmoothPixmapTransform);
            QPainterPtr *painter_ptr = &painter;
            rust!(Slint_paintEvent [rust_window: &QtWindow as "void*", painter_ptr: &mut QPainterPtr as "QPainterPtr*"] {
                rust_window.paint_event(std::mem::take(painter_ptr))
            });
        }

        void resizeEvent(QResizeEvent *event) override {
            QSize size = event->size();
            rust!(Slint_resizeEvent [rust_window: &QtWindow as "void*", size: qttypes::QSize as "QSize"] {
                rust_window.resize_event(size)
            });
        }

        void mousePressEvent(QMouseEvent *event) override {
            isMouseButtonDown = true;
            QPoint pos = event->pos();
            int button = event->button();
            rust!(Slint_mousePressEvent [rust_window: &QtWindow as "void*", pos: qttypes::QPoint as "QPoint", button: u32 as "int" ] {
                let position = LogicalPoint::new(pos.x as _, pos.y as _);
                let button = from_qt_button(button);
                rust_window.mouse_event(MouseEvent::Pressed{ position, button })
            });
        }
        void mouseReleaseEvent(QMouseEvent *event) override {
            // HACK: Qt on windows is a bit special when clicking on the window
            //       close button and when the resulting close event is ignored.
            //       In that case a release event that was not preceded by
            //       a press event is sent on Windows.
            //       This confuses Slint, so eat this event.
            //
            //       One example is a popup is shown in the close event that
            //       then ignores the the close request to ask the user what to
            //       do. The stray release event will then close the popup
            //       straight away
            if (!isMouseButtonDown) {
                return;
            }
            isMouseButtonDown = false;

            QPoint pos = event->pos();
            int button = event->button();
            rust!(Slint_mouseReleaseEvent [rust_window: &QtWindow as "void*", pos: qttypes::QPoint as "QPoint", button: u32 as "int" ] {
                let position = LogicalPoint::new(pos.x as _, pos.y as _);
                let button = from_qt_button(button);
                rust_window.mouse_event(MouseEvent::Released{ position, button })
            });
            if (auto p = dynamic_cast<const SlintWidget*>(parent())) {
                // FIXME: better way to close the popup
                void *parent_window = p->rust_window;
                rust!(Slint_mouseReleaseEventPopup [parent_window: &QtWindow as "void*", pos: qttypes::QPoint as "QPoint"] {
                    parent_window.close_popup();
                });
            }
        }
        void mouseMoveEvent(QMouseEvent *event) override {
            QPoint pos = event->pos();
            rust!(Slint_mouseMoveEvent [rust_window: &QtWindow as "void*", pos: qttypes::QPoint as "QPoint"] {
                let position = LogicalPoint::new(pos.x as _, pos.y as _);
                rust_window.mouse_event(MouseEvent::Moved{position})
            });
        }
        void wheelEvent(QWheelEvent *event) override {
            QPointF pos = event->position();
            QPoint delta = event->pixelDelta();
            if (delta.isNull()) {
                delta = event->angleDelta();
            }
            rust!(Slint_mouseWheelEvent [rust_window: &QtWindow as "void*", pos: qttypes::QPointF as "QPointF", delta: qttypes::QPoint as "QPoint"] {
                let position = LogicalPoint::new(pos.x as _, pos.y as _);
                rust_window.mouse_event(MouseEvent::Wheel{position, delta_x: delta.x as _, delta_y: delta.y as _})
            });
        }
        void leaveEvent(QEvent *) override {
            rust!(Slint_mouseLeaveEvent [rust_window: &QtWindow as "void*"] {
                rust_window.mouse_event(MouseEvent::Exit)
            });
        }

        void keyPressEvent(QKeyEvent *event) override {
            QString text =  event->text();
            int key = event->key();
            rust!(Slint_keyPress [rust_window: &QtWindow as "void*", key: i32 as "int", text: qttypes::QString as "QString"] {
                rust_window.key_event(key, text.clone(), false);
            });
        }
        void keyReleaseEvent(QKeyEvent *event) override {
            QString text =  event->text();
            int key = event->key();
            rust!(Slint_keyRelease [rust_window: &QtWindow as "void*", key: i32 as "int", text: qttypes::QString as "QString"] {
                rust_window.key_event(key, text.clone(), true);
            });
        }

        void customEvent(QEvent *event) override {
            if (event->type() == QEvent::User) {
                rust!(Slint_updateWindowProps [rust_window: &QtWindow as "void*"] {
                    WindowInner::from_pub(&rust_window.window).update_window_properties()
                });
            } else {
                QWidget::customEvent(event);
            }
        }

        void changeEvent(QEvent *event) override {
            if (event->type() == QEvent::ActivationChange) {
                bool active = isActiveWindow();
                rust!(Slint_updateWindowActivation [rust_window: &QtWindow as "void*", active: bool as "bool"] {
                    WindowInner::from_pub(&rust_window.window).set_active(active)
                });
            } else if (event->type() == QEvent::PaletteChange || event->type() == QEvent::StyleChange) {
                bool dark_color_scheme = qApp->palette().color(QPalette::Window).valueF() < 0.5;
                rust!(Slint_updateWindowDarkColorScheme [rust_window: &QtWindow as "void*", dark_color_scheme: bool as "bool"] {
                    if let Some(ds) = rust_window.dark_color_scheme.get() {
                        ds.as_ref().set(dark_color_scheme);
                    }
                });
            }
            QWidget::changeEvent(event);
        }

        void closeEvent(QCloseEvent *event) override {
            bool accepted = rust!(Slint_requestClose [rust_window: &QtWindow as "void*"] -> bool as "bool" {
                return WindowInner::from_pub(&rust_window.window).request_close();
            });
            if (accepted) {
                event->accept();
            } else {
                event->ignore();
            }
        }

        QSize sizeHint() const override {
            auto preferred_size = rust!(Slint_sizeHint [rust_window: &QtWindow as "void*"] -> qttypes::QSize as "QSize" {
                let component_rc = WindowInner::from_pub(&rust_window.window).component();
                let component = ComponentRc::borrow_pin(&component_rc);
                let layout_info_h = component.as_ref().layout_info(Orientation::Horizontal);
                let layout_info_v = component.as_ref().layout_info(Orientation::Vertical);
                qttypes::QSize {
                    width: layout_info_h.preferred_bounded() as _,
                    height: layout_info_v.preferred_bounded() as _,
                }
            });
            if (!preferred_size.isEmpty()) {
                return preferred_size;
            } else {
                return QWidget::sizeHint();
            }
        }

        QVariant inputMethodQuery(Qt::InputMethodQuery query) const override {
            switch (query) {
            case Qt::ImCursorRectangle: {
                return QRect(ime_position.x(), ime_position.y(), 1, 1);
            }
            default: break;
            }
            return QWidget::inputMethodQuery(query);
        }

        void inputMethodEvent(QInputMethodEvent *event) override {
            QString commit_string = event->commitString();
            QString preedit_string = event->preeditString();
            int replacement_start = event->replacementStart();
            int replacement_length = qMax(0, event->replacementLength());
            if (replacement_start < 0) {
                // Not sure if this can happen yet, but this way we can safely cast to usize below.
                replacement_start = 0;
                replacement_length = 0;
            }

            int preedit_cursor = -1;
            for (const QInputMethodEvent::Attribute &attribute: event->attributes()) {
                if (attribute.type == QInputMethodEvent::Cursor) {
                    if (attribute.length > 0) {
                        preedit_cursor = attribute.start;
                    }
                }
            }
            event->accept();
            rust!(Slint_inputMethodEvent [rust_window: &QtWindow as "void*", commit_string: qttypes::QString as "QString",
                preedit_string: qttypes::QString as "QString", replacement_start: i32 as "int", replacement_length: i32 as "int",
                preedit_cursor: i32 as "int"] {
                    let runtime_window = WindowInner::from_pub(&rust_window.window);

                    if !preedit_string.is_empty() {
                        let event = KeyInputEvent {
                            event_type: KeyEventType::UpdateComposition,
                            text: preedit_string.to_string().into(),
                            preedit_selection_start: replacement_start as usize,
                            preedit_selection_end: replacement_start as usize + replacement_length as usize,
                            ..Default::default()
                        };
                        runtime_window.process_key_input(event);
                    }

                    if !commit_string.is_empty() {
                        let event = KeyInputEvent {
                            event_type: KeyEventType::CommitComposition,
                            text: commit_string.to_string().into(),
                            ..Default::default()
                        };
                        runtime_window.process_key_input(event);
                    }
                });
        }
    };

    // Helper function used for the TextInput layouting
    //
    // if line_for_y_pos > 0, then the function will return the line at this y position
    static int do_text_layout(QTextLayout &layout, int flags, const QRectF &rect, int line_for_y_pos = -1) {
        QTextOption options;
        options.setWrapMode((flags & Qt::TextWordWrap) ? QTextOption::WordWrap : QTextOption::NoWrap);
        options.setFlags(QTextOption::IncludeTrailingSpaces);
        layout.setTextOption(options);
        layout.setCacheEnabled(true);
        QFontMetrics fm(layout.font());
        int leading = fm.leading();
        qreal height = 0;
        layout.beginLayout();
        int count = 0;
        while(1) {
            auto line = layout.createLine();
            if (!line.isValid())
                break;
            line.setLineWidth(rect.width());
            height += leading;
            line.setPosition(QPointF(0, height));
            height += line.height();
            if (line_for_y_pos >= 0 && height > line_for_y_pos) {
                return count;
            }
            count++;
        }
        layout.endLayout();
        if (flags & Qt::AlignVCenter) {
            layout.setPosition(QPointF(0, (rect.height() - height) / 2.));
        } else if (flags & Qt::AlignBottom) {
            layout.setPosition(QPointF(0, rect.height() - height));
        }
        return -1;
    }
}}

cpp_class!(
    /// Wrapper around a pointer to a QPainter.
    // We can't use [`qttypes::QPainter`] because it is not sound <https://github.com/woboq/qmetaobject-rs/issues/267>
    pub unsafe struct QPainterPtr as "QPainterPtr"
);
impl QPainterPtr {
    pub fn restore(&mut self) {
        cpp!(unsafe [self as "QPainterPtr*"] {
            (*self)->restore();
        });
    }

    pub fn save(&mut self) {
        cpp!(unsafe [self as "QPainterPtr*"] {
            (*self)->save();
        });
    }
}

cpp_class! {pub unsafe struct QPainterPath as "QPainterPath"}

impl QPainterPath {
    /*
    pub fn reserve(&mut self, size: usize) {
        cpp! { unsafe [self as "QPainterPath*", size as "long long"] {
            self->reserve(size);
        }}
    }*/

    pub fn move_to(&mut self, to: qttypes::QPointF) {
        cpp! { unsafe [self as "QPainterPath*", to as "QPointF"] {
            self->moveTo(to);
        }}
    }
    pub fn line_to(&mut self, to: qttypes::QPointF) {
        cpp! { unsafe [self as "QPainterPath*", to as "QPointF"] {
            self->lineTo(to);
        }}
    }
    pub fn quad_to(&mut self, ctrl: qttypes::QPointF, to: qttypes::QPointF) {
        cpp! { unsafe [self as "QPainterPath*", ctrl as "QPointF", to as "QPointF"] {
            self->quadTo(ctrl, to);
        }}
    }
    pub fn cubic_to(
        &mut self,
        ctrl1: qttypes::QPointF,
        ctrl2: qttypes::QPointF,
        to: qttypes::QPointF,
    ) {
        cpp! { unsafe [self as "QPainterPath*", ctrl1 as "QPointF", ctrl2 as "QPointF", to as "QPointF"] {
            self->cubicTo(ctrl1, ctrl2, to);
        }}
    }

    pub fn close(&mut self) {
        cpp! { unsafe [self as "QPainterPath*"] {
            self->closeSubpath();
        }}
    }

    pub fn set_fill_rule(&mut self, rule: key_generated::Qt_FillRule) {
        cpp! { unsafe [self as "QPainterPath*", rule as "Qt::FillRule" ] {
            self->setFillRule(rule);
        }}
    }
}

fn into_qbrush(
    brush: i_slint_core::Brush,
    width: qttypes::qreal,
    height: qttypes::qreal,
) -> qttypes::QBrush {
    /// Mangle the position to work around the fact that Qt merge stop at equal position
    fn mangle_position(position: f32, idx: usize, count: usize) -> f32 {
        // Add or subtract a small amount to make sure each stop is different but still in [0..1].
        // It is possible that we swap stops that are both really really close to 0.54321+ε,
        // but that is really unlikely
        if position < 0.54321 + 67.8 * f32::EPSILON {
            position + f32::EPSILON * idx as f32
        } else {
            position - f32::EPSILON * (count - idx - 1) as f32
        }
    }
    match brush {
        i_slint_core::Brush::SolidColor(color) => {
            let color: u32 = color.as_argb_encoded();
            cpp!(unsafe [color as "QRgb"] -> qttypes::QBrush as "QBrush" {
                return QBrush(QColor::fromRgba(color));
            })
        }
        i_slint_core::Brush::LinearGradient(g) => {
            let (start, end) = i_slint_core::graphics::line_for_angle(g.angle());
            let p1 = qttypes::QPointF { x: start.x as _, y: start.y as _ };
            let p2 = qttypes::QPointF { x: end.x as _, y: end.y as _ };
            cpp_class!(unsafe struct QLinearGradient as "QLinearGradient");
            let mut qlg = cpp! {
                unsafe [p1 as "QPointF", p2 as "QPointF"] -> QLinearGradient as "QLinearGradient" {
                    QLinearGradient qlg(p1, p2);
                    qlg.setCoordinateMode(QGradient::ObjectMode);
                    return qlg;
                }
            };
            let count = g.stops().count();
            for (idx, s) in g.stops().enumerate() {
                let pos: f32 = mangle_position(s.position, idx, count);
                let color: u32 = s.color.as_argb_encoded();
                cpp! {unsafe [mut qlg as "QLinearGradient", pos as "float", color as "QRgb"] {
                    qlg.setColorAt(pos, QColor::fromRgba(color));
                }};
            }
            cpp! {unsafe [qlg as "QLinearGradient"] -> qttypes::QBrush as "QBrush" {
                return QBrush(qlg);
            }}
        }
        i_slint_core::Brush::RadialGradient(g) => {
            cpp_class!(unsafe struct QRadialGradient as "QRadialGradient");
            let mut qrg = cpp! {
                unsafe [width as "qreal", height as "qreal"] -> QRadialGradient as "QRadialGradient" {
                    QRadialGradient qrg(width / 2, height / 2, (width + height) / 4);
                    return qrg;
                }
            };
            let count = g.stops().count();
            for (idx, s) in g.stops().enumerate() {
                let pos: f32 = mangle_position(s.position, idx, count);
                let color: u32 = s.color.as_argb_encoded();
                cpp! {unsafe [mut qrg as "QRadialGradient", pos as "float", color as "QRgb"] {
                    qrg.setColorAt(pos, QColor::fromRgba(color));
                }};
            }
            cpp! {unsafe [qrg as "QRadialGradient"] -> qttypes::QBrush as "QBrush" {
                return QBrush(qrg);
            }}
        }
        _ => qttypes::QBrush::default(),
    }
}

fn from_qt_button(qt_button: u32) -> PointerEventButton {
    match qt_button {
        1 => PointerEventButton::Left,
        2 => PointerEventButton::Right,
        4 => PointerEventButton::Middle,
        _ => PointerEventButton::None,
    }
}

/// Given a position offset and an object of a given type that has x,y,width,height properties,
/// create a QRectF that fits it.
macro_rules! get_geometry {
    ($ty:ty, $obj:expr) => {{
        type Ty = $ty;
        let width = Ty::FIELD_OFFSETS.width.apply_pin($obj).get();
        let height = Ty::FIELD_OFFSETS.height.apply_pin($obj).get();
        if width <= LogicalLength::zero() || height <= LogicalLength::zero() {
            return Default::default();
        };
        qttypes::QRectF { x: 0., y: 0., width: width.get() as _, height: height.get() as _ }
    }};
}

fn adjust_rect_and_border_for_inner_drawing(rect: &mut qttypes::QRectF, border_width: &mut f32) {
    // If the border width exceeds the width, just fill the rectangle.
    *border_width = border_width.min((rect.width as f32) / 2.);
    // adjust the size so that the border is drawn within the geometry
    rect.x += *border_width as f64 / 2.;
    rect.y += *border_width as f64 / 2.;
    rect.width -= *border_width as f64;
    rect.height -= *border_width as f64;
}

struct QtItemRenderer<'a> {
    painter: QPainterPtr,
    cache: &'a ItemCache<qttypes::QPixmap>,
    window: &'a i_slint_core::api::Window,
    metrics: RenderingMetrics,
}

impl ItemRenderer for QtItemRenderer<'_> {
    fn draw_rectangle(&mut self, rect_: Pin<&items::Rectangle>, _: &ItemRc) {
        let rect: qttypes::QRectF = get_geometry!(items::Rectangle, rect_);
        let brush: qttypes::QBrush = into_qbrush(rect_.background(), rect.width, rect.height);
        let painter: &mut QPainterPtr = &mut self.painter;
        cpp! { unsafe [painter as "QPainterPtr*", brush as "QBrush", rect as "QRectF"] {
            (*painter)->fillRect(rect, brush);
        }}
    }

    fn draw_border_rectangle(&mut self, rect: std::pin::Pin<&items::BorderRectangle>, _: &ItemRc) {
        Self::draw_rectangle_impl(
            &mut self.painter,
            get_geometry!(items::BorderRectangle, rect),
            rect.background(),
            rect.border_color(),
            rect.border_width().get(),
            rect.border_radius().get(),
        );
    }

    fn draw_image(&mut self, image: Pin<&items::ImageItem>, item_rc: &ItemRc) {
        let dest_rect: qttypes::QRectF = get_geometry!(items::ImageItem, image);
        self.draw_image_impl(
            item_rc,
            items::ImageItem::FIELD_OFFSETS.source.apply_pin(image),
            dest_rect,
            None,
            items::ImageItem::FIELD_OFFSETS.width.apply_pin(image),
            items::ImageItem::FIELD_OFFSETS.height.apply_pin(image),
            image.image_fit(),
            image.image_rendering(),
            None,
        );
    }

    fn draw_clipped_image(&mut self, image: Pin<&items::ClippedImage>, item_rc: &ItemRc) {
        let dest_rect: qttypes::QRectF = get_geometry!(items::ClippedImage, image);
        let source_rect = qttypes::QRectF {
            x: image.source_clip_x() as _,
            y: image.source_clip_y() as _,
            width: image.source_clip_width() as _,
            height: image.source_clip_height() as _,
        };
        self.draw_image_impl(
            item_rc,
            items::ClippedImage::FIELD_OFFSETS.source.apply_pin(image),
            dest_rect,
            Some(source_rect),
            items::ClippedImage::FIELD_OFFSETS.width.apply_pin(image),
            items::ClippedImage::FIELD_OFFSETS.height.apply_pin(image),
            image.image_fit(),
            image.image_rendering(),
            Some(items::ClippedImage::FIELD_OFFSETS.colorize.apply_pin(image)),
        );
    }

    fn draw_text(&mut self, text: std::pin::Pin<&items::Text>, _: &ItemRc) {
        let rect: qttypes::QRectF = get_geometry!(items::Text, text);
        let fill_brush: qttypes::QBrush = into_qbrush(text.color(), rect.width, rect.height);
        let mut string: qttypes::QString = text.text().as_str().into();
        let font: QFont = get_font(text.font_request(WindowInner::from_pub(&self.window)));
        let flags = match text.horizontal_alignment() {
            TextHorizontalAlignment::Left => key_generated::Qt_AlignmentFlag_AlignLeft,
            TextHorizontalAlignment::Center => key_generated::Qt_AlignmentFlag_AlignHCenter,
            TextHorizontalAlignment::Right => key_generated::Qt_AlignmentFlag_AlignRight,
        } | match text.vertical_alignment() {
            TextVerticalAlignment::Top => key_generated::Qt_AlignmentFlag_AlignTop,
            TextVerticalAlignment::Center => key_generated::Qt_AlignmentFlag_AlignVCenter,
            TextVerticalAlignment::Bottom => key_generated::Qt_AlignmentFlag_AlignBottom,
        } | match text.wrap() {
            TextWrap::NoWrap => 0,
            TextWrap::WordWrap => key_generated::Qt_TextFlag_TextWordWrap,
        };
        let elide = text.overflow() == TextOverflow::Elide;
        let painter: &mut QPainterPtr = &mut self.painter;
        cpp! { unsafe [painter as "QPainterPtr*", rect as "QRectF", fill_brush as "QBrush", mut string as "QString", flags as "int", font as "QFont", elide as "bool"] {
            (*painter)->setFont(font);
            (*painter)->setPen(QPen(fill_brush, 0));
            (*painter)->setBrush(Qt::NoBrush);
            if (!elide) {
                (*painter)->drawText(rect, flags, string);
            } else if (!(flags & Qt::TextWordWrap)) {
                QString elided;
                QFontMetrics fm(font);
                while (!string.isEmpty()) {
                    int pos = string.indexOf('\n');
                    if (pos < 0) {
                        elided += fm.elidedText(string, Qt::ElideRight, rect.width());
                        break;
                    }
                    QString line = string.left(pos);
                    elided += fm.elidedText(line, Qt::ElideRight, rect.width());
                    elided += '\n';
                    string = string.mid(pos + 1);
                }
                (*painter)->drawText(rect, flags, elided);
            } else {
                // elide and word wrap: we need to add the ellipsis manually on the last line
                string.replace(QChar('\n'), QChar::LineSeparator);
                QString elided = string;
                QFontMetrics fm(font);
                QTextLayout layout(string, font);
                QTextOption options;
                options.setWrapMode(QTextOption::WordWrap);
                layout.setTextOption(options);
                layout.setCacheEnabled(true);
                layout.beginLayout();
                int leading = fm.leading();
                qreal height = 0;
                int last_line_begin = 0, last_line_size = 0;
                while (true) {
                    auto line = layout.createLine();
                    if (!line.isValid()) {
                        last_line_begin = string.size();
                        break;
                    }
                    line.setLineWidth(rect.width());
                    height += leading + line.height();
                    if (height > rect.height()) {
                        break;
                    }
                    last_line_begin = line.textStart();
                    last_line_size = line.textLength();
                }
                if (last_line_begin < string.size()) {
                    elided = string.left(last_line_begin);
                    QString to_elide = QStringView(string).mid(last_line_begin, last_line_size).trimmed() % QStringView(QT_UNICODE_LITERAL("…"));
                    elided += fm.elidedText(to_elide, Qt::ElideRight, rect.width());
                }
                (*painter)->drawText(rect, flags, elided);
            }
        }}
    }

    fn draw_text_input(&mut self, text_input: std::pin::Pin<&items::TextInput>, _: &ItemRc) {
        let rect: qttypes::QRectF = get_geometry!(items::TextInput, text_input);
        let fill_brush: qttypes::QBrush = into_qbrush(text_input.color(), rect.width, rect.height);

        let font: QFont = get_font(
            text_input.font_request(&WindowInner::from_pub(&self.window).window_adapter()),
        );
        let flags = match text_input.horizontal_alignment() {
            TextHorizontalAlignment::Left => key_generated::Qt_AlignmentFlag_AlignLeft,
            TextHorizontalAlignment::Center => key_generated::Qt_AlignmentFlag_AlignHCenter,
            TextHorizontalAlignment::Right => key_generated::Qt_AlignmentFlag_AlignRight,
        } | match text_input.vertical_alignment() {
            TextVerticalAlignment::Top => key_generated::Qt_AlignmentFlag_AlignTop,
            TextVerticalAlignment::Center => key_generated::Qt_AlignmentFlag_AlignVCenter,
            TextVerticalAlignment::Bottom => key_generated::Qt_AlignmentFlag_AlignBottom,
        } | match text_input.wrap() {
            TextWrap::NoWrap => 0,
            TextWrap::WordWrap => key_generated::Qt_TextFlag_TextWordWrap,
        };

        let visual_representation = text_input.visual_representation();

        let text = &visual_representation.text;
        let mut string: qttypes::QString = text.as_str().into();

        if let InputType::Password = text_input.input_type() {
            cpp! { unsafe [mut string as "QString"] {
                string.fill(QChar(qApp->style()->styleHint(QStyle::SH_LineEdit_PasswordCharacter, nullptr, nullptr)));
            }}
        }

        // convert byte offsets to offsets in Qt UTF-16 encoded string, as that's
        // what QTextLayout expects.

        let (
            cursor_position_as_offset,
            anchor_position_as_offset,
            selection_foreground_color,
            selection_background_color,
            underline_selection,
        ): (usize, usize, u32, u32, bool) = if !visual_representation.preedit_range.is_empty() {
            (
                visual_representation.preedit_range.start,
                visual_representation.preedit_range.end,
                Color::default().as_argb_encoded(),
                Color::default().as_argb_encoded(),
                true,
            )
        } else {
            (
                text_input.cursor_position(&text),
                text_input.anchor_position(&text),
                text_input.selection_foreground_color().as_argb_encoded(),
                text_input.selection_background_color().as_argb_encoded(),
                false,
            )
        };

        let cursor_position: i32 = if cursor_position_as_offset > 0 {
            utf8_byte_offset_to_utf16_units(text.as_str(), cursor_position_as_offset) as i32
        } else {
            0
        };
        let anchor_position: i32 = if anchor_position_as_offset > 0 {
            utf8_byte_offset_to_utf16_units(text.as_str(), anchor_position_as_offset) as i32
        } else {
            0
        };

        let text_cursor_width: f32 = if visual_representation.cursor_position.is_some() {
            text_input.text_cursor_width().get()
        } else {
            0.
        };

        let single_line: bool = text_input.single_line();

        let painter: &mut QPainterPtr = &mut self.painter;
        cpp! { unsafe [
                painter as "QPainterPtr*",
                rect as "QRectF",
                fill_brush as "QBrush",
                selection_foreground_color as "QRgb",
                selection_background_color as "QRgb",
                underline_selection as "bool",
                mut string as "QString",
                flags as "int",
                single_line as "bool",
                font as "QFont",
                cursor_position as "int",
                anchor_position as "int",
                text_cursor_width as "float"] {
            if (!single_line) {
                string.replace(QChar('\n'), QChar::LineSeparator);
            }
            QTextLayout layout(string, font);
            do_text_layout(layout, flags, rect);
            (*painter)->setPen(QPen(fill_brush, 0));
            QVector<QTextLayout::FormatRange> selections;
            if (anchor_position != cursor_position) {
                QTextCharFormat fmt;
                if (qAlpha(selection_background_color) != 0) {
                    fmt.setBackground(QColor::fromRgba(selection_background_color));
                }
                if (qAlpha(selection_background_color) != 0) {
                    fmt.setForeground(QColor::fromRgba(selection_foreground_color));
                }
                if (underline_selection) {
                    fmt.setFontUnderline(true);
                }
                selections << QTextLayout::FormatRange{
                    std::min(anchor_position, cursor_position),
                    std::abs(anchor_position - cursor_position),
                    fmt
                };
            }
            layout.draw(painter->get(), rect.topLeft(), selections);
            if (text_cursor_width > 0) {
                layout.drawCursor(painter->get(), rect.topLeft(), cursor_position, text_cursor_width);
            }
        }}
    }

    fn draw_path(&mut self, path: Pin<&items::Path>, _: &ItemRc) {
        let elements = path.elements();
        if matches!(elements, PathData::None) {
            return;
        }
        let rect: qttypes::QRectF = get_geometry!(items::Path, path);
        let fill_brush: qttypes::QBrush = into_qbrush(path.fill(), rect.width, rect.height);
        let stroke_brush: qttypes::QBrush = into_qbrush(path.stroke(), rect.width, rect.height);
        let stroke_width: f32 = path.stroke_width().get();
        let (offset, path_events) = path.fitted_path_events();
        let pos = qttypes::QPoint { x: offset.x as _, y: offset.y as _ };
        let mut painter_path = QPainterPath::default();

        painter_path.set_fill_rule(match path.fill_rule() {
            FillRule::Nonzero => key_generated::Qt_FillRule_WindingFill,
            FillRule::Evenodd => key_generated::Qt_FillRule_OddEvenFill,
        });

        for x in path_events.iter() {
            fn to_qpointf(p: Point) -> qttypes::QPointF {
                qttypes::QPointF { x: p.x as _, y: p.y as _ }
            }
            match x {
                lyon_path::Event::Begin { at } => {
                    painter_path.move_to(to_qpointf(at));
                }
                lyon_path::Event::Line { from: _, to } => {
                    painter_path.line_to(to_qpointf(to));
                }
                lyon_path::Event::Quadratic { from: _, ctrl, to } => {
                    painter_path.quad_to(to_qpointf(ctrl), to_qpointf(to));
                }

                lyon_path::Event::Cubic { from: _, ctrl1, ctrl2, to } => {
                    painter_path.cubic_to(to_qpointf(ctrl1), to_qpointf(ctrl2), to_qpointf(to));
                }
                lyon_path::Event::End { last: _, first: _, close } => {
                    // FIXME: are we supposed to do something with last and first?
                    if close {
                        painter_path.close()
                    }
                }
            }
        }

        let painter: &mut QPainterPtr = &mut self.painter;
        cpp! { unsafe [
                painter as "QPainterPtr*",
                pos as "QPoint",
                mut painter_path as "QPainterPath",
                fill_brush as "QBrush",
                stroke_brush as "QBrush",
                stroke_width as "float"] {
            (*painter)->save();
            auto cleanup = qScopeGuard([&] { (*painter)->restore(); });
            (*painter)->translate(pos);
            (*painter)->setPen(stroke_width > 0 ? QPen(stroke_brush, stroke_width) : Qt::NoPen);
            (*painter)->setBrush(fill_brush);
            (*painter)->drawPath(painter_path);
        }}
    }

    fn draw_box_shadow(&mut self, box_shadow: Pin<&items::BoxShadow>, item_rc: &ItemRc) {
        let pixmap : qttypes::QPixmap = self.cache.get_or_update_cache_entry( item_rc, || {
                let shadow_rect = get_geometry!(items::BoxShadow, box_shadow);

                let source_size = qttypes::QSize {
                    width: shadow_rect.width.ceil() as _,
                    height: shadow_rect.height.ceil() as _,
                };

                let mut source_image =
                    qttypes::QImage::new(source_size, qttypes::ImageFormat::ARGB32_Premultiplied);
                source_image.fill(qttypes::QColor::from_rgba_f(0., 0., 0., 0.));

                let img = &mut source_image;
                let mut painter_ = cpp!(unsafe [img as "QImage*"] -> QPainterPtr as "QPainterPtr" {
                    return std::make_unique<QPainter>(img);
                });

                Self::draw_rectangle_impl(
                    &mut painter_,
                    qttypes::QRectF { x: 0., y: 0., width: shadow_rect.width, height: shadow_rect.height },
                    Brush::SolidColor(box_shadow.color()),
                    Brush::default(),
                    0.,
                    box_shadow.border_radius().get(),
                );

                drop(painter_);

                let blur_radius = box_shadow.blur().get();

                if blur_radius > 0. {
                    cpp! {
                    unsafe[img as "QImage*", blur_radius as "float"] -> qttypes::QPixmap as "QPixmap" {
                        class PublicGraphicsBlurEffect : public QGraphicsBlurEffect {
                        public:
                            // Make public what's protected
                            using QGraphicsBlurEffect::draw;
                        };

                        // Need a scene for the effect source private to draw()
                        QGraphicsScene scene;

                        auto pixmap_item = scene.addPixmap(QPixmap::fromImage(*img));

                        auto blur_effect = new PublicGraphicsBlurEffect;
                        blur_effect->setBlurRadius(blur_radius);
                        blur_effect->setBlurHints(QGraphicsBlurEffect::QualityHint);

                        // takes ownership of the effect and registers the item with
                        // the effect as source.
                        pixmap_item->setGraphicsEffect(blur_effect);

                        QImage blurred_scene(img->width() + 2 * blur_radius, img->height() + 2 * blur_radius, QImage::Format_ARGB32_Premultiplied);
                        blurred_scene.fill(Qt::transparent);

                        QPainter p(&blurred_scene);
                        p.translate(blur_radius, blur_radius);
                        blur_effect->draw(&p);
                        p.end();

                        return QPixmap::fromImage(blurred_scene);
                    }}
                } else {
                    cpp! { unsafe[img as "QImage*"] -> qttypes::QPixmap as "QPixmap" {
                        return QPixmap::fromImage(*img);
                    }}
                }
            });

        let blur_radius = box_shadow.blur();

        let shadow_offset = qttypes::QPointF {
            x: (box_shadow.offset_x() - blur_radius).get() as f64,
            y: (box_shadow.offset_y() - blur_radius).get() as f64,
        };

        let painter: &mut QPainterPtr = &mut self.painter;
        cpp! { unsafe [
                painter as "QPainterPtr*",
                shadow_offset as "QPointF",
                pixmap as "QPixmap"
            ] {
            (*painter)->drawPixmap(shadow_offset, pixmap);
        }}
    }

    fn visit_opacity(&mut self, opacity_item: Pin<&Opacity>, item_rc: &ItemRc) -> RenderingResult {
        let opacity = opacity_item.opacity();
        if Opacity::need_layer(item_rc, opacity) {
            self.render_and_blend_layer(opacity, item_rc)
        } else {
            self.apply_opacity(opacity);
            self.cache.release(item_rc);
            RenderingResult::ContinueRenderingChildren
        }
    }

    fn visit_layer(&mut self, layer_item: Pin<&Layer>, self_rc: &ItemRc) -> RenderingResult {
        if layer_item.cache_rendering_hint() {
            self.render_and_blend_layer(1.0, self_rc)
        } else {
            RenderingResult::ContinueRenderingChildren
        }
    }

    fn combine_clip(
        &mut self,
        rect: LogicalRect,
        radius: LogicalLength,
        border_width: LogicalLength,
    ) -> bool {
        let mut border_width: f32 = border_width.get();
        let mut clip_rect = qttypes::QRectF {
            x: rect.min_x() as _,
            y: rect.min_y() as _,
            width: rect.width() as _,
            height: rect.height() as _,
        };
        adjust_rect_and_border_for_inner_drawing(&mut clip_rect, &mut border_width);
        let painter: &mut QPainterPtr = &mut self.painter;
        cpp! { unsafe [painter as "QPainterPtr*", clip_rect as "QRectF", radius as "float"] -> bool as "bool" {
            if (radius <= 0) {
                (*painter)->setClipRect(clip_rect, Qt::IntersectClip);
            } else {
                QPainterPath path;
                path.addRoundedRect(clip_rect, radius, radius);
                (*painter)->setClipPath(path, Qt::IntersectClip);
            }
            return !(*painter)->clipBoundingRect().isEmpty();
        }}
    }

    fn get_current_clip(&self) -> LogicalRect {
        let painter: &QPainterPtr = &self.painter;
        let res = cpp! { unsafe [painter as "const QPainterPtr*" ] -> qttypes::QRectF as "QRectF" {
            return (*painter)->clipBoundingRect();
        }};
        LogicalRect::new(
            LogicalPoint::new(res.x as _, res.y as _),
            LogicalSize::new(res.width as _, res.height as _),
        )
    }

    fn save_state(&mut self) {
        self.painter.save()
    }

    fn restore_state(&mut self) {
        self.painter.restore()
    }

    fn scale_factor(&self) -> f32 {
        1.
        /* cpp! { unsafe [painter as "QPainterPtr*"] -> f32 as "float" {
            return (*painter)->paintEngine()->paintDevice()->devicePixelRatioF();
        }} */
    }

    fn draw_cached_pixmap(
        &mut self,
        _item_rc: &ItemRc,
        update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    ) {
        update_fn(&mut |width: u32, height: u32, data: &[u8]| {
            let data = data.as_ptr();
            let painter: &mut QPainterPtr = &mut self.painter;
            cpp! { unsafe [painter as "QPainterPtr*",  width as "int", height as "int", data as "const unsigned char *"] {
                QImage img(data, width, height, width * 4, QImage::Format_RGBA8888_Premultiplied);
                (*painter)->drawImage(QPoint(), img);
            }}
        })
    }

    fn draw_string(&mut self, string: &str, color: Color) {
        let fill_brush: qttypes::QBrush = into_qbrush(color.into(), 1., 1.);
        let mut string: qttypes::QString = string.into();
        let font: QFont = get_font(Default::default());
        let painter: &mut QPainterPtr = &mut self.painter;
        cpp! { unsafe [painter as "QPainterPtr*", fill_brush as "QBrush", mut string as "QString", font as "QFont"] {
            (*painter)->setFont(font);
            (*painter)->setPen(QPen(fill_brush, 0));
            (*painter)->setBrush(Qt::NoBrush);
            (*painter)->drawText(0, QFontMetrics((*painter)->font()).ascent(), string);
        }}
    }

    fn window(&self) -> &i_slint_core::api::Window {
        &self.window
    }

    fn as_any(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(&mut self.painter)
    }

    fn translate(&mut self, distance: LogicalVector) {
        let x: f32 = distance.x;
        let y: f32 = distance.y;
        let painter: &mut QPainterPtr = &mut self.painter;
        cpp! { unsafe [painter as "QPainterPtr*", x as "float", y as "float"] {
            (*painter)->translate(x, y);
        }}
    }

    fn rotate(&mut self, angle_in_degrees: f32) {
        let painter: &mut QPainterPtr = &mut self.painter;
        cpp! { unsafe [painter as "QPainterPtr*", angle_in_degrees as "float"] {
            (*painter)->rotate(angle_in_degrees);
        }}
    }

    fn apply_opacity(&mut self, opacity: f32) {
        let painter: &mut QPainterPtr = &mut self.painter;
        cpp! { unsafe [painter as "QPainterPtr*", opacity as "float"] {
            (*painter)->setOpacity((*painter)->opacity() * opacity);
        }}
    }

    fn metrics(&self) -> RenderingMetrics {
        self.metrics.clone()
    }
}

fn shared_image_buffer_to_pixmap(buffer: &SharedImageBuffer) -> Option<qttypes::QPixmap> {
    let (format, bytes_per_line, buffer_ptr) = match buffer {
        SharedImageBuffer::RGBA8(img) => {
            (qttypes::ImageFormat::RGBA8888, img.stride() * 4, img.as_bytes().as_ptr())
        }
        SharedImageBuffer::RGBA8Premultiplied(img) => (
            qttypes::ImageFormat::RGBA8888_Premultiplied,
            img.stride() * 4,
            img.as_bytes().as_ptr(),
        ),
        SharedImageBuffer::RGB8(img) => {
            (qttypes::ImageFormat::RGB888, img.stride() * 3, img.as_bytes().as_ptr())
        }
    };
    let width: i32 = buffer.width() as _;
    let height: i32 = buffer.height() as _;
    let pixmap = cpp! { unsafe [format as "QImage::Format", width as "int", height as "int", bytes_per_line as "uint32_t", buffer_ptr as "const uchar *"] -> qttypes::QPixmap as "QPixmap" {
        QImage img(buffer_ptr, width, height, bytes_per_line, format);
        return QPixmap::fromImage(img);
    } };
    return Some(pixmap);
}

pub(crate) fn image_to_pixmap(
    image: &ImageInner,
    source_size: Option<euclid::Size2D<u32, PhysicalPx>>,
) -> Option<qttypes::QPixmap> {
    shared_image_buffer_to_pixmap(&image.render_to_buffer(source_size)?)
}

/// Changes the source or the destination rectangle to respect the image fit
fn adjust_to_image_fit(
    image_fit: ImageFit,
    source_rect: &mut qttypes::QRectF,
    dest_rect: &mut qttypes::QRectF,
) {
    match image_fit {
        i_slint_core::items::ImageFit::Fill => (),
        i_slint_core::items::ImageFit::Cover => {
            let ratio = qttypes::qreal::max(
                dest_rect.width / source_rect.width,
                dest_rect.height / source_rect.height,
            );
            if source_rect.width > dest_rect.width / ratio {
                source_rect.x += (source_rect.width - dest_rect.width / ratio) / 2.;
                source_rect.width = dest_rect.width / ratio;
            }
            if source_rect.height > dest_rect.height / ratio {
                source_rect.y += (source_rect.height - dest_rect.height / ratio) / 2.;
                source_rect.height = dest_rect.height / ratio;
            }
        }
        i_slint_core::items::ImageFit::Contain => {
            let ratio = qttypes::qreal::min(
                dest_rect.width / source_rect.width,
                dest_rect.height / source_rect.height,
            );
            if dest_rect.width > source_rect.width * ratio {
                dest_rect.x += (dest_rect.width - source_rect.width * ratio) / 2.;
                dest_rect.width = source_rect.width * ratio;
            }
            if dest_rect.height > source_rect.height * ratio {
                dest_rect.y += (dest_rect.height - source_rect.height * ratio) / 2.;
                dest_rect.height = source_rect.height * ratio;
            }
        }
    };
}

impl QtItemRenderer<'_> {
    fn draw_image_impl(
        &mut self,
        item_rc: &ItemRc,
        source_property: Pin<&Property<Image>>,
        dest_rect: qttypes::QRectF,
        source_rect: Option<qttypes::QRectF>,
        target_width: std::pin::Pin<&Property<LogicalLength>>,
        target_height: std::pin::Pin<&Property<LogicalLength>>,
        image_fit: ImageFit,
        rendering: ImageRendering,
        colorize_property: Option<Pin<&Property<Brush>>>,
    ) {
        // Caller ensured that zero/negative width/height resulted in an early return via get_geometry!.
        debug_assert!(target_width.get() > LogicalLength::zero());
        debug_assert!(target_height.get() > LogicalLength::zero());

        let pixmap: qttypes::QPixmap = self.cache.get_or_update_cache_entry(item_rc, || {
            let source = source_property.get();
            let origin = source.size();
            let source: &ImageInner = (&source).into();

            // Query target_width/height here again to ensure that changes will invalidate the item rendering cache.
            let t =
                euclid::size2(target_width.get().get(), target_height.get().get()).cast::<f64>();

            let source_size = if source.is_svg() {
                let has_source_clipping = source_rect.map_or(false, |rect| {
                    rect.is_valid()
                        && (rect.x != 0.
                            || rect.y != 0.
                            || !rect.width.approx_eq(&t.width)
                            || !rect.height.approx_eq(&t.height))
                });
                if has_source_clipping {
                    // Source size & clipping is not implemented yet
                    None
                } else {
                    Some(i_slint_core::graphics::fit_size(image_fit, t.cast(), origin).cast())
                }
            } else {
                None
            };

            image_to_pixmap(source, source_size).map_or_else(
                Default::default,
                |mut pixmap: qttypes::QPixmap| {
                    let colorize = colorize_property.map_or(Brush::default(), |c| c.get());
                    if !colorize.is_transparent() {
                        let brush: qttypes::QBrush =
                            into_qbrush(colorize, dest_rect.width, dest_rect.height);
                        cpp!(unsafe [mut pixmap as "QPixmap", brush as "QBrush"] {
                            QPainter p(&pixmap);
                            p.setCompositionMode(QPainter::CompositionMode_SourceIn);
                            p.fillRect(QRect(QPoint(), pixmap.size()), brush);
                        });
                    }
                    pixmap
                },
            )
        });

        let image_size = pixmap.size();
        let mut source_rect = source_rect.filter(|r| r.is_valid()).unwrap_or(qttypes::QRectF {
            x: 0.,
            y: 0.,
            width: image_size.width as _,
            height: image_size.height as _,
        });
        let mut dest_rect = dest_rect;
        adjust_to_image_fit(image_fit, &mut source_rect, &mut dest_rect);
        let painter: &mut QPainterPtr = &mut self.painter;
        let smooth: bool = rendering == ImageRendering::Smooth;
        cpp! { unsafe [
                painter as "QPainterPtr*",
                pixmap as "QPixmap",
                source_rect as "QRectF",
                dest_rect as "QRectF",
                smooth as "bool"] {
            (*painter)->save();
            (*painter)->setRenderHint(QPainter::SmoothPixmapTransform, smooth);
            (*painter)->drawPixmap(dest_rect, pixmap, source_rect);
            (*painter)->restore();
        }};
    }

    fn draw_rectangle_impl(
        painter: &mut QPainterPtr,
        mut rect: qttypes::QRectF,
        brush: Brush,
        border_color: Brush,
        mut border_width: f32,
        border_radius: f32,
    ) {
        let brush: qttypes::QBrush = into_qbrush(brush, rect.width, rect.height);
        let border_color: qttypes::QBrush = into_qbrush(border_color, rect.width, rect.height);
        adjust_rect_and_border_for_inner_drawing(&mut rect, &mut border_width);
        cpp! { unsafe [painter as "QPainterPtr*", brush as "QBrush",  border_color as "QBrush", border_width as "float", border_radius as "float", rect as "QRectF"] {
            (*painter)->setPen(border_width > 0 ? QPen(border_color, border_width) : Qt::NoPen);
            (*painter)->setBrush(brush);
            if (border_radius > 0) {
                (*painter)->drawRoundedRect(rect, border_radius, border_radius);
            } else {
                (*painter)->drawRect(rect);
            }
        }}
    }

    fn render_layer(
        &mut self,
        item_rc: &ItemRc,
        layer_size_fn: &dyn Fn() -> qttypes::QSize,
    ) -> qttypes::QPixmap {
        self.cache.get_or_update_cache_entry(item_rc,  || {
            let layer_size: qttypes::QSize = layer_size_fn();
            let mut layer_image = qttypes::QImage::new(layer_size, qttypes::ImageFormat::ARGB32_Premultiplied);
            layer_image.fill(qttypes::QColor::from_rgba_f(0., 0., 0., 0.));

            *self.metrics.layers_created.as_mut().unwrap() += 1;

            let img_ref: &mut qttypes::QImage = &mut layer_image;
            let mut layer_painter = cpp!(unsafe [img_ref as "QImage*"] -> QPainterPtr as "QPainterPtr" {
                auto painter = std::make_unique<QPainter>(img_ref);
                painter->setClipRect(0, 0, img_ref->width(), img_ref->height());
                return painter;
            });

            std::mem::swap(&mut self.painter, &mut layer_painter);

            i_slint_core::item_rendering::render_item_children(
                self,
                &item_rc.component(),
                item_rc.index() as isize,
            );

            std::mem::swap(&mut self.painter, &mut layer_painter);
            drop(layer_painter);

            qttypes::QPixmap::from(layer_image)
        })
    }

    fn render_and_blend_layer(&mut self, alpha_tint: f32, self_rc: &ItemRc) -> RenderingResult {
        let current_clip = self.get_current_clip();
        let mut layer_image = self.render_layer(self_rc, &|| {
            // We don't need to include the size of the opacity item itself, since it has no content.
            let children_rect = i_slint_core::properties::evaluate_no_tracking(|| {
                self_rc.geometry().union(
                    &i_slint_core::item_rendering::item_children_bounding_rect(
                        &self_rc.component(),
                        self_rc.index() as isize,
                        &current_clip,
                    ),
                )
            });
            qttypes::QSize {
                width: children_rect.size.width as _,
                height: children_rect.size.height as _,
            }
        });
        self.save_state();
        self.apply_opacity(alpha_tint);
        {
            let painter: &mut QPainterPtr = &mut self.painter;
            let layer_image_ref: &mut qttypes::QPixmap = &mut layer_image;
            cpp! { unsafe [
                    painter as "QPainterPtr*",
                    layer_image_ref as "QPixmap*"
                ] {
                (*painter)->drawPixmap(0, 0, *layer_image_ref);
            }}
        }
        self.restore_state();
        RenderingResult::ContinueRenderingWithoutChildren
    }
}

cpp_class!(unsafe struct QWidgetPtr as "std::unique_ptr<QWidget>");

pub struct QtWindow {
    widget_ptr: QWidgetPtr,
    pub(crate) window: i_slint_core::api::Window,
    self_weak: Weak<Self>,

    rendering_metrics_collector: RefCell<Option<Rc<RenderingMetricsCollector>>>,

    cache: ItemCache<qttypes::QPixmap>,

    tree_structure_changed: RefCell<bool>,

    dark_color_scheme: OnceCell<Pin<Box<Property<bool>>>>,
}

impl QtWindow {
    pub fn new() -> Rc<Self> {
        let rc = Rc::new_cyclic(|self_weak| {
            let window_ptr = self_weak.clone().into_raw();
            let widget_ptr = cpp! {unsafe [window_ptr as "void*"] -> QWidgetPtr as "std::unique_ptr<QWidget>" {
                ensure_initialized(true);
                auto widget = std::make_unique<SlintWidget>();

                auto accessibility = new Slint_accessible_window(widget.get(), window_ptr);
                QAccessible::registerAccessibleInterface(accessibility);

                return widget;
            }};

            QtWindow {
                widget_ptr,
                window: i_slint_core::api::Window::new(self_weak.clone() as _),
                self_weak: self_weak.clone(),
                rendering_metrics_collector: Default::default(),
                cache: Default::default(),
                tree_structure_changed: RefCell::new(false),
                dark_color_scheme: Default::default(),
            }
        });
        let widget_ptr = rc.widget_ptr();
        let rust_window = Rc::as_ptr(&rc);
        cpp! {unsafe [widget_ptr as "SlintWidget*", rust_window as "void*"]  {
            widget_ptr->rust_window = rust_window;
        }};
        ALL_WINDOWS.with(|aw| aw.borrow_mut().push(rc.self_weak.clone()));
        rc
    }

    /// Return the QWidget*
    fn widget_ptr(&self) -> NonNull<()> {
        unsafe { std::mem::transmute_copy::<QWidgetPtr, NonNull<_>>(&self.widget_ptr) }
    }

    fn paint_event(&self, painter: QPainterPtr) {
        let runtime_window = WindowInner::from_pub(&self.window);
        runtime_window.draw_contents(|components| {
            i_slint_core::animations::update_animations();
            let mut renderer = QtItemRenderer {
                painter,
                cache: &self.cache,
                window: &self.window,
                metrics: RenderingMetrics { layers_created: Some(0) },
            };

            for (component, origin) in components {
                i_slint_core::item_rendering::render_component_items(
                    component,
                    &mut renderer,
                    *origin,
                );
            }

            if let Some(collector) = &*self.rendering_metrics_collector.borrow() {
                collector.measure_frame_rendered(&mut renderer);
            }

            i_slint_core::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
                if !driver.has_active_animations() {
                    return;
                }
                let widget_ptr = self.widget_ptr();
                cpp! {unsafe [widget_ptr as "QWidget*"] {
                    // FIXME: using QTimer -::singleShot is not optimal. We should use Qt animation timer
                    QTimer::singleShot(16, [widget_ptr = QPointer<QWidget>(widget_ptr)] {
                        if (widget_ptr)
                            widget_ptr->update();
                    });
                    //return widget_ptr->update();
                }}
            });
        });

        // Update the accessibility tree (if the component tree has changed)
        if self.tree_structure_changed.replace(false) {
            let widget_ptr = self.widget_ptr();
            cpp! { unsafe [widget_ptr as "QWidget*"] {
                auto accessible = dynamic_cast<Slint_accessible_window*>(QAccessible::queryAccessibleInterface(widget_ptr));
                if (accessible->isUsed()) { accessible->updateAccessibilityTree(); }
            }};
        }
    }

    fn resize_event(&self, size: qttypes::QSize) {
        self.window.set_size(i_slint_core::api::PhysicalSize::new(size.width, size.height));
    }

    fn mouse_event(&self, event: MouseEvent) {
        WindowInner::from_pub(&self.window).process_mouse_input(event);
        timer_event();
    }

    fn key_event(&self, key: i32, text: qttypes::QString, released: bool) {
        i_slint_core::animations::update_animations();
        let text: String = text.into();

        let text = qt_key_to_string(key as key_generated::Qt_Key, text);

        for ch in text.chars() {
            let event = if released {
                WindowEvent::KeyReleased { text: ch }
            } else {
                WindowEvent::KeyPressed { text: ch }
            };
            self.window.dispatch_event(event)
        }

        timer_event();
    }

    fn close_popup(&self) {
        WindowInner::from_pub(&self.window).close_popup();
    }
}

impl WindowAdapter for QtWindow {
    fn window(&self) -> &i_slint_core::api::Window {
        &self.window
    }
}

impl WindowAdapterSealed for QtWindow {
    fn show(&self) {
        let component_rc = WindowInner::from_pub(&self.window).component();
        let component = ComponentRc::borrow_pin(&component_rc);
        let root_item = component.as_ref().get_item_ref(0);
        if let Some(window_item) = ItemRef::downcast_pin::<WindowItem>(root_item) {
            if window_item.width() <= LogicalLength::zero() {
                window_item.width.set(LogicalLength::new(
                    component.as_ref().layout_info(Orientation::Horizontal).preferred_bounded(),
                ))
            }
            if window_item.height() <= LogicalLength::zero() {
                window_item.height.set(LogicalLength::new(
                    component.as_ref().layout_info(Orientation::Vertical).preferred_bounded(),
                ))
            }

            self.apply_window_properties(window_item);
        }

        let widget_ptr = self.widget_ptr();
        cpp! {unsafe [widget_ptr as "QWidget*"] {
            widget_ptr->show();
        }};
        let qt_platform_name = cpp! {unsafe [] -> qttypes::QString as "QString" {
            return QGuiApplication::platformName();
        }};
        *self.rendering_metrics_collector.borrow_mut() = RenderingMetricsCollector::new(
            self.self_weak.clone(),
            &format!("Qt backend (platform {})", qt_platform_name),
        );
    }

    fn hide(&self) {
        self.rendering_metrics_collector.take();
        let widget_ptr = self.widget_ptr();
        cpp! {unsafe [widget_ptr as "QWidget*"] {
            widget_ptr->hide();
            // Since we don't call close(), this will force Qt to recompute wether there are any
            // visible windows, and ends the application if needed
            QEventLoopLocker();
        }};
    }

    fn request_redraw(&self) {
        let widget_ptr = self.widget_ptr();
        cpp! {unsafe [widget_ptr as "QWidget*"] {
            return widget_ptr->update();
        }}
    }

    fn request_window_properties_update(&self) {
        let widget_ptr = self.widget_ptr();
        cpp! {unsafe [widget_ptr as "SlintWidget*"]  {
            QCoreApplication::postEvent(widget_ptr, new QEvent(QEvent::User));
        }};
    }

    /// Apply windows property such as title to the QWidget*
    fn apply_window_properties(&self, window_item: Pin<&items::WindowItem>) {
        let widget_ptr = self.widget_ptr();
        let title: qttypes::QString = window_item.title().as_str().into();
        let no_frame = window_item.no_frame();
        let mut size = qttypes::QSize {
            width: window_item.width().get().ceil() as _,
            height: window_item.height().get().ceil() as _,
        };
        if size.width == 0 || size.height == 0 {
            let existing_size = cpp!(unsafe [widget_ptr as "QWidget*"] -> qttypes::QSize as "QSize" {
                return widget_ptr->size();
            });
            if size.width == 0 {
                window_item.width.set(LogicalLength::new(existing_size.width as _));
                size.width = existing_size.width;
            }
            if size.height == 0 {
                window_item.height.set(LogicalLength::new(existing_size.height as _));
                size.height = existing_size.height;
            }
        }

        let background =
            into_qbrush(window_item.background(), size.width.into(), size.height.into());

        match (&window_item.icon()).into() {
            &ImageInner::None => (),
            r => {
                if let Some(pixmap) = image_to_pixmap(r, None) {
                    cpp! {unsafe [widget_ptr as "QWidget*", pixmap as "QPixmap"] {
                        widget_ptr->setWindowIcon(QIcon(pixmap));
                    }};
                }
            }
        };

        cpp! {unsafe [widget_ptr as "QWidget*",  title as "QString", size as "QSize", background as "QBrush", no_frame as "bool"] {
            if (size != widget_ptr->size()) {
                widget_ptr->resize(size.expandedTo({1, 1}));
            }
            widget_ptr->setWindowFlag(Qt::FramelessWindowHint, no_frame);
            widget_ptr->setWindowTitle(title);
            auto pal = widget_ptr->palette();

            #if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
            // If the background color is the same as what NativeStyleMetrics supplied from QGuiApplication::palette().color(QPalette::Window),
            // then the setColor (implicitly setBrush) call will not detach the palette. However it will set the resolveMask, which due to the
            // lack of a detach changes QGuiApplicationPrivate::app_pal's resolve mask and thus breaks future theme based palette changes.
            // Therefore we force a detach.
            // https://bugreports.qt.io/browse/QTBUG-98762
            {
                pal.setResolveMask(~pal.resolveMask());
                pal.setResolveMask(~pal.resolveMask());
            }
            #endif
            pal.setBrush(QPalette::Window, background);
            widget_ptr->setPalette(pal);
        }};
    }

    /// Set the min/max sizes on the QWidget
    fn apply_geometry_constraint(
        &self,
        constraints_h: i_slint_core::layout::LayoutInfo,
        constraints_v: i_slint_core::layout::LayoutInfo,
    ) {
        let widget_ptr = self.widget_ptr();
        let min_width: f32 = constraints_h.min.min(constraints_h.max);
        let min_height: f32 = constraints_v.min.min(constraints_v.max);
        let mut max_width: f32 = constraints_h.max.max(constraints_h.min);
        let mut max_height: f32 = constraints_v.max.max(constraints_v.min);
        cpp! {unsafe [widget_ptr as "QWidget*",  min_width as "float", min_height as "float", mut max_width as "float", mut max_height as "float"] {
            widget_ptr->setMinimumSize(QSize(min_width, min_height));
            if (max_width > QWIDGETSIZE_MAX)
                max_width = QWIDGETSIZE_MAX;
            if (max_height > QWIDGETSIZE_MAX)
                max_height = QWIDGETSIZE_MAX;
            widget_ptr->setMaximumSize(QSize(max_width, max_height).expandedTo({1,1}));
        }};
    }

    fn register_component(&self) {
        self.tree_structure_changed.replace(true);
    }

    fn unregister_component<'a>(
        &self,
        _component: ComponentRef,
        _: &mut dyn Iterator<Item = Pin<ItemRef<'a>>>,
    ) {
        self.tree_structure_changed.replace(true);
    }

    fn create_popup(&self, geometry: LogicalRect) -> Option<Rc<dyn WindowAdapter>> {
        let popup_window = QtWindow::new();

        let size = qttypes::QSize { width: geometry.width() as _, height: geometry.height() as _ };

        let popup_ptr = popup_window.widget_ptr();
        let pos = qttypes::QPoint { x: geometry.origin.x as _, y: geometry.origin.y as _ };
        let widget_ptr = self.widget_ptr();
        cpp! {unsafe [widget_ptr as "QWidget*", popup_ptr as "QWidget*", pos as "QPoint", size as "QSize"] {
            popup_ptr->setParent(widget_ptr, Qt::Popup);
            popup_ptr->setGeometry(QRect(pos + widget_ptr->mapToGlobal(QPoint(0,0)), size));
            popup_ptr->show();
        }};
        Some(popup_window as _)
    }

    fn set_mouse_cursor(&self, cursor: MouseCursor) {
        let widget_ptr = self.widget_ptr();
        //unidirectional resize cursors are replaced with bidirectional ones
        let cursor_shape = match cursor {
            MouseCursor::Default => key_generated::Qt_CursorShape_ArrowCursor,
            MouseCursor::None => key_generated::Qt_CursorShape_BlankCursor,
            MouseCursor::Help => key_generated::Qt_CursorShape_WhatsThisCursor,
            MouseCursor::Pointer => key_generated::Qt_CursorShape_PointingHandCursor,
            MouseCursor::Progress => key_generated::Qt_CursorShape_BusyCursor,
            MouseCursor::Wait => key_generated::Qt_CursorShape_WaitCursor,
            MouseCursor::Crosshair => key_generated::Qt_CursorShape_CrossCursor,
            MouseCursor::Text => key_generated::Qt_CursorShape_IBeamCursor,
            MouseCursor::Alias => key_generated::Qt_CursorShape_DragLinkCursor,
            MouseCursor::Copy => key_generated::Qt_CursorShape_DragCopyCursor,
            MouseCursor::Move => key_generated::Qt_CursorShape_DragMoveCursor,
            MouseCursor::NoDrop => key_generated::Qt_CursorShape_ForbiddenCursor,
            MouseCursor::NotAllowed => key_generated::Qt_CursorShape_ForbiddenCursor,
            MouseCursor::Grab => key_generated::Qt_CursorShape_OpenHandCursor,
            MouseCursor::Grabbing => key_generated::Qt_CursorShape_ClosedHandCursor,
            MouseCursor::ColResize => key_generated::Qt_CursorShape_SplitHCursor,
            MouseCursor::RowResize => key_generated::Qt_CursorShape_SplitVCursor,
            MouseCursor::NResize => key_generated::Qt_CursorShape_SizeVerCursor,
            MouseCursor::EResize => key_generated::Qt_CursorShape_SizeHorCursor,
            MouseCursor::SResize => key_generated::Qt_CursorShape_SizeVerCursor,
            MouseCursor::WResize => key_generated::Qt_CursorShape_SizeHorCursor,
            MouseCursor::NeResize => key_generated::Qt_CursorShape_SizeBDiagCursor,
            MouseCursor::NwResize => key_generated::Qt_CursorShape_SizeFDiagCursor,
            MouseCursor::SeResize => key_generated::Qt_CursorShape_SizeFDiagCursor,
            MouseCursor::SwResize => key_generated::Qt_CursorShape_SizeBDiagCursor,
            MouseCursor::EwResize => key_generated::Qt_CursorShape_SizeHorCursor,
            MouseCursor::NsResize => key_generated::Qt_CursorShape_SizeVerCursor,
            MouseCursor::NeswResize => key_generated::Qt_CursorShape_SizeBDiagCursor,
            MouseCursor::NwseResize => key_generated::Qt_CursorShape_SizeFDiagCursor,
        };
        cpp! {unsafe [widget_ptr as "QWidget*", cursor_shape as "Qt::CursorShape"] {
            widget_ptr->setCursor(QCursor{cursor_shape});
        }};
    }

    fn enable_input_method(&self, input: i_slint_core::items::InputType) {
        let enable: bool = matches!(input, i_slint_core::items::InputType::Text);
        let widget_ptr = self.widget_ptr();
        cpp! {unsafe [widget_ptr as "QWidget*", enable as "bool"] {
            widget_ptr->setAttribute(Qt::WA_InputMethodEnabled, enable);
        }};
    }

    fn disable_input_method(&self) {
        let widget_ptr = self.widget_ptr();
        cpp! {unsafe [widget_ptr as "QWidget*"] {
            widget_ptr->setAttribute(Qt::WA_InputMethodEnabled, false);
        }};
    }

    fn set_ime_position(&self, position: LogicalPoint) {
        let pos = qttypes::QPoint { x: position.x as _, y: position.y as _ };
        let widget_ptr = self.widget_ptr();
        cpp! {unsafe [widget_ptr as "SlintWidget*", pos as "QPoint"]  {
            widget_ptr->ime_position = pos;
            QGuiApplication::inputMethod()->update(Qt::ImQueryInput);
        }};
    }

    fn renderer(&self) -> &dyn Renderer {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn handle_focus_change(&self, _old: Option<ItemRc>, new: Option<ItemRc>) {
        let widget_ptr = self.widget_ptr();
        if let Some(ai) = accessible_item(new) {
            let item = &ai;
            cpp! {unsafe [widget_ptr as "QWidget*", item as "void*"] {
                auto accessible = QAccessible::queryAccessibleInterface(widget_ptr);
                if (auto slint_accessible = dynamic_cast<Slint_accessible*>(accessible)) {
                    slint_accessible->clearFocus();
                    slint_accessible->focusItem(item);
                }
            }};
        }
    }

    fn position(&self) -> i_slint_core::api::PhysicalPosition {
        let widget_ptr = self.widget_ptr();
        let qp = cpp! {unsafe [widget_ptr as "QWidget*"] -> qttypes::QPoint as "QPoint" {
            return widget_ptr->pos();
        }};
        // Qt returns logical coordinates, so scale those!
        i_slint_core::api::LogicalPosition::new(qp.x as _, qp.y as _)
            .to_physical(self.window().scale_factor())
    }

    fn set_position(&self, position: i_slint_core::api::WindowPosition) {
        let physical_position = position.to_physical(self.window().scale_factor());
        let widget_ptr = self.widget_ptr();
        let pos = qttypes::QPoint { x: physical_position.x as _, y: physical_position.y as _ };
        cpp! {unsafe [widget_ptr as "QWidget*", pos as "QPoint"] {
            widget_ptr->move(pos);
        }};
    }

    fn set_size(&self, size: i_slint_core::api::WindowSize) {
        let logical_size = size.to_logical(self.window().scale_factor());
        let widget_ptr = self.widget_ptr();
        let sz = qttypes::QSize {
            width: logical_size.width.round() as _,
            height: logical_size.height.round() as _,
        };
        // Qt uses logical units!
        cpp! {unsafe [widget_ptr as "QWidget*", sz as "QSize"] {
            widget_ptr->resize(sz);
        }};
    }

    fn dark_color_scheme(&self) -> bool {
        let ds = self.dark_color_scheme.get_or_init(|| {
            Box::pin(Property::new(cpp! {unsafe [] -> bool as "bool" {
                return qApp->palette().color(QPalette::Window).valueF() < 0.5;
            }}))
        });
        ds.as_ref().get()
    }

    fn is_visible(&self) -> bool {
        let widget_ptr = self.widget_ptr();
        cpp! {unsafe [widget_ptr as "QWidget*"] -> bool as "bool" {
            return widget_ptr->isVisible();
        }}
    }
}

impl Renderer for QtWindow {
    fn text_size(
        &self,
        font_request: i_slint_core::graphics::FontRequest,
        text: &str,
        max_width: Option<LogicalLength>,
        _scale_factor: ScaleFactor,
    ) -> LogicalSize {
        get_font(font_request).text_size(text, max_width.map(|logical_width| logical_width.get()))
    }

    fn text_input_byte_offset_for_position(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        pos: LogicalPoint,
    ) -> usize {
        if pos.y < 0. {
            return 0;
        }
        let rect: qttypes::QRectF = get_geometry!(items::TextInput, text_input);
        let pos = qttypes::QPointF { x: pos.x as _, y: pos.y as _ };
        let font: QFont = get_font(
            text_input.font_request(&WindowInner::from_pub(&self.window).window_adapter()),
        );
        let string = qttypes::QString::from(text_input.text().as_str());
        let flags = match text_input.horizontal_alignment() {
            TextHorizontalAlignment::Left => key_generated::Qt_AlignmentFlag_AlignLeft,
            TextHorizontalAlignment::Center => key_generated::Qt_AlignmentFlag_AlignHCenter,
            TextHorizontalAlignment::Right => key_generated::Qt_AlignmentFlag_AlignRight,
        } | match text_input.vertical_alignment() {
            TextVerticalAlignment::Top => key_generated::Qt_AlignmentFlag_AlignTop,
            TextVerticalAlignment::Center => key_generated::Qt_AlignmentFlag_AlignVCenter,
            TextVerticalAlignment::Bottom => key_generated::Qt_AlignmentFlag_AlignBottom,
        } | match text_input.wrap() {
            TextWrap::NoWrap => 0,
            TextWrap::WordWrap => key_generated::Qt_TextFlag_TextWordWrap,
        };
        let single_line: bool = text_input.single_line();
        let is_password: bool = matches!(text_input.input_type(), InputType::Password);
        cpp! { unsafe [font as "QFont", string as "QString", pos as "QPointF", flags as "int",
                rect as "QRectF", single_line as "bool", is_password as "bool"] -> usize as "size_t" {
            // we need to do the \n replacement in a copy because the original need to be kept to know the utf8 offset
            auto copy = string;
            if (is_password) {
                copy.fill(QChar(qApp->style()->styleHint(QStyle::SH_LineEdit_PasswordCharacter, nullptr, nullptr)));
            }
            if (!single_line) {
                copy.replace(QChar('\n'), QChar::LineSeparator);
            }
            QTextLayout layout(copy, font);
            auto line = do_text_layout(layout, flags, rect, pos.y());
            if (line < 0 || layout.lineCount() <= line)
                return string.toUtf8().size();
            QTextLine textLine = layout.lineAt(line);
            int cur;
            if (pos.x() > textLine.naturalTextWidth()) {
                cur = textLine.textStart() + textLine.textLength();
                // cur is one past the last character of the line (eg, the \n or space).
                // Go one back to get back on this line.
                // Unless we were at the end of the text, in which case there was no \n
                if (cur > textLine.textStart() && (cur < string.size() || string[cur-1] == '\n'))
                    cur--;
            } else {
                cur = textLine.xToCursor(pos.x());
            }
            if (cur < string.size() && string[cur].isLowSurrogate())
                cur++;
            // convert to an utf8 pos;
            return QStringView(string).left(cur).toUtf8().size();
        }}
    }

    fn text_input_cursor_rect_for_byte_offset(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        byte_offset: usize,
    ) -> LogicalRect {
        let rect: qttypes::QRectF = get_geometry!(items::TextInput, text_input);
        let font: QFont = get_font(
            text_input.font_request(&WindowInner::from_pub(&self.window).window_adapter()),
        );
        let text = text_input.text();
        let mut string = qttypes::QString::from(text.as_str());
        let offset: u32 = utf8_byte_offset_to_utf16_units(text.as_str(), byte_offset) as _;
        let flags = match text_input.horizontal_alignment() {
            TextHorizontalAlignment::Left => key_generated::Qt_AlignmentFlag_AlignLeft,
            TextHorizontalAlignment::Center => key_generated::Qt_AlignmentFlag_AlignHCenter,
            TextHorizontalAlignment::Right => key_generated::Qt_AlignmentFlag_AlignRight,
        } | match text_input.vertical_alignment() {
            TextVerticalAlignment::Top => key_generated::Qt_AlignmentFlag_AlignTop,
            TextVerticalAlignment::Center => key_generated::Qt_AlignmentFlag_AlignVCenter,
            TextVerticalAlignment::Bottom => key_generated::Qt_AlignmentFlag_AlignBottom,
        } | match text_input.wrap() {
            TextWrap::NoWrap => 0,
            TextWrap::WordWrap => key_generated::Qt_TextFlag_TextWordWrap,
        };
        let single_line: bool = text_input.single_line();
        let r = cpp! { unsafe [font as "QFont", mut string as "QString", offset as "int", flags as "int", rect as "QRectF", single_line as "bool"]
                -> qttypes::QRectF as "QRectF" {
            if (!single_line) {
                string.replace(QChar('\n'), QChar::LineSeparator);
            }
            QTextLayout layout(string, font);
            do_text_layout(layout, flags, rect);

            QTextLine textLine = layout.lineForTextPosition(offset);
            if (!textLine.isValid())
                return QRectF();
            return QRectF(textLine.x() + textLine.cursorToX(offset), layout.position().y() + textLine.y(), 1.0, textLine.height());
        }};

        LogicalRect::new(
            LogicalPoint::new(r.x as _, r.y as _),
            LogicalSize::new(r.width as _, r.height as _),
        )
    }

    fn register_font_from_memory(
        &self,
        data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let data = qttypes::QByteArray::from(data);
        cpp! {unsafe [data as "QByteArray"] {
            ensure_initialized(true);
            QFontDatabase::addApplicationFontFromData(data);
        } }
        Ok(())
    }

    fn register_font_from_path(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let encoded_path: qttypes::QByteArray = path.to_string_lossy().as_bytes().into();
        cpp! {unsafe [encoded_path as "QByteArray"] {
            ensure_initialized(true);

            QString requested_path = QFileInfo(QFile::decodeName(encoded_path)).canonicalFilePath();
            static QSet<QString> loaded_app_fonts;
            // QFontDatabase::addApplicationFont unconditionally reads the provided file from disk,
            // while we want to do this only once to avoid things like the live-review going crazy.
            if (!loaded_app_fonts.contains(requested_path)) {
                loaded_app_fonts.insert(requested_path);
                QFontDatabase::addApplicationFont(requested_path);
            }
        } }
        Ok(())
    }

    fn default_font_size(&self) -> LogicalLength {
        let default_font_size = cpp!(unsafe[] -> i32 as "int" {
            return QFontInfo(qApp->font()).pixelSize();
        });
        // Ideally this would return the value from another property with a binding that's updated
        // as a FontChange event is received. This is relevant for the case of using the Qt backend
        // with a non-native style.
        LogicalLength::new(default_font_size as f32)
    }

    fn free_graphics_resources(&self, component: ComponentRef) {
        // Invalidate caches:
        self.cache.component_destroyed(component);
    }
}

fn accessible_item(item: Option<ItemRc>) -> Option<ItemRc> {
    let mut current = item;
    while let Some(c) = current {
        if c.is_accessible() {
            return Some(c);
        } else {
            current = c.parent_item();
        }
    }
    None
}

fn get_font(request: FontRequest) -> QFont {
    let family: qttypes::QString = request.family.unwrap_or_default().as_str().into();
    let pixel_size: f32 = request.pixel_size.map_or(0., |logical_size| logical_size.get());
    let weight: i32 = request.weight.unwrap_or(0);
    let letter_spacing: f32 =
        request.letter_spacing.map_or(0., |logical_spacing| logical_spacing.get());
    cpp!(unsafe [family as "QString", pixel_size as "float", weight as "int", letter_spacing as "float"] -> QFont as "QFont" {
        QFont f;
        if (!family.isEmpty())
            f.setFamily(family);
        if (pixel_size > 0)
            f.setPixelSize(pixel_size);
        if (weight > 0) {
    #if QT_VERSION < QT_VERSION_CHECK(6, 0, 0)
            f.setWeight(qMin((weight-100)/8, 99));
    #else
            f.setWeight(QFont::Weight(weight));
    #endif
        }
        f.setLetterSpacing(QFont::AbsoluteSpacing, letter_spacing);
        // Mark all font properties as resolved, to avoid inheriting font properties
        // from the widget hierarchy. Later we call QPainter::setFont, which would
        // merge in unset properties (such as bold, etc.) that it retrieved from
        // the widget the painter is associated with.
    #if QT_VERSION < QT_VERSION_CHECK(6, 0, 0)
        f.resolve(QFont::AllPropertiesResolved);
    #else
        f.setResolveMask(QFont::AllPropertiesResolved);
    #endif
        return f;
    })
}

cpp_class! {pub unsafe struct QFont as "QFont"}

impl QFont {
    fn text_size(&self, text: &str, max_width: Option<f32>) -> LogicalSize {
        let string = qttypes::QString::from(text);
        let mut r = qttypes::QRectF::default();
        if let Some(max) = max_width {
            r.height = f32::MAX as _;
            r.width = max as _;
        }
        let size = cpp! { unsafe [self as "const QFont*", string as "QString", r as "QRectF"]
                -> qttypes::QSizeF as "QSizeF"{
            return QFontMetricsF(*self).boundingRect(r, r.isEmpty() ? 0 : Qt::TextWordWrap , string).size();
        }};
        LogicalSize::new(size.width as _, size.height as _)
    }
}

thread_local! {
    // FIXME: currently the window are never removed
    static ALL_WINDOWS: RefCell<Vec<Weak<QtWindow>>> = Default::default();
}

/// Called by C++'s TimerHandler::timerEvent, or every time a timer might have been started
pub(crate) fn timer_event() {
    i_slint_core::platform::update_timers_and_animations();

    let timeout = i_slint_core::timers::TimerList::next_timeout().map(|instant| {
        let now = std::time::Instant::now();
        let instant: std::time::Instant = instant.into();
        if instant > now {
            instant.duration_since(now).as_millis() as i32
        } else {
            0
        }
    });
    if let Some(timeout) = timeout {
        cpp! { unsafe [timeout as "int"] {
            ensure_initialized(true);
            TimerHandler::instance().timer.start(timeout, &TimerHandler::instance());
        }}
    }
}

mod key_codes {
    macro_rules! define_qt_key_to_string_fn {
        ($($char:literal # $name:ident # $($qt:ident)|* # $($winit:ident)|* ;)*) => {
            use crate::key_generated;
            pub fn qt_key_to_string(key: key_generated::Qt_Key) -> Option<i_slint_core::SharedString> {

                let char = match(key) {
                    $($(key_generated::$qt => $char,)*)*
                    _ => return None,
                };
                Some(char.into())
            }
        };
    }

    i_slint_common::for_each_special_keys!(define_qt_key_to_string_fn);
}

fn qt_key_to_string(key: key_generated::Qt_Key, event_text: String) -> SharedString {
    // First try to see if we received one of the non-ascii keys that we have
    // a special representation for. If that fails, try to use the provided
    // text. If that's empty, then try to see if the provided key has an ascii
    // representation. The last step is needed because modifiers may result in
    // the text to be empty otherwise, for example Ctrl+C.
    if let Some(special_key_code) = key_codes::qt_key_to_string(key) {
        return special_key_code;
    };

    // On Windows, X11 and Wayland, Ctrl+C for example sends a terminal control character,
    // which we choose not to supply to the application. Instead we fall through to translating
    // the supplied key code.
    if !event_text.is_empty() && !event_text.chars().any(|ch| ch.is_control()) {
        return event_text.into();
    }

    match key {
        key_generated::Qt_Key_Key_Space => " ",
        key_generated::Qt_Key_Key_Exclam => "!",
        key_generated::Qt_Key_Key_QuoteDbl => "\"",
        key_generated::Qt_Key_Key_NumberSign => "#",
        key_generated::Qt_Key_Key_Dollar => "$",
        key_generated::Qt_Key_Key_Percent => "%",
        key_generated::Qt_Key_Key_Ampersand => "&",
        key_generated::Qt_Key_Key_Apostrophe => "'",
        key_generated::Qt_Key_Key_ParenLeft => "(",
        key_generated::Qt_Key_Key_ParenRight => ")",
        key_generated::Qt_Key_Key_Asterisk => "*",
        key_generated::Qt_Key_Key_Plus => "+",
        key_generated::Qt_Key_Key_Comma => ",",
        key_generated::Qt_Key_Key_Minus => "-",
        key_generated::Qt_Key_Key_Period => ".",
        key_generated::Qt_Key_Key_Slash => "/",
        key_generated::Qt_Key_Key_0 => "0",
        key_generated::Qt_Key_Key_1 => "1",
        key_generated::Qt_Key_Key_2 => "2",
        key_generated::Qt_Key_Key_3 => "3",
        key_generated::Qt_Key_Key_4 => "4",
        key_generated::Qt_Key_Key_5 => "5",
        key_generated::Qt_Key_Key_6 => "6",
        key_generated::Qt_Key_Key_7 => "7",
        key_generated::Qt_Key_Key_8 => "8",
        key_generated::Qt_Key_Key_9 => "9",
        key_generated::Qt_Key_Key_Colon => ":",
        key_generated::Qt_Key_Key_Semicolon => ";",
        key_generated::Qt_Key_Key_Less => "<",
        key_generated::Qt_Key_Key_Equal => "=",
        key_generated::Qt_Key_Key_Greater => ">",
        key_generated::Qt_Key_Key_Question => "?",
        key_generated::Qt_Key_Key_At => "@",
        key_generated::Qt_Key_Key_A => "a",
        key_generated::Qt_Key_Key_B => "b",
        key_generated::Qt_Key_Key_C => "c",
        key_generated::Qt_Key_Key_D => "d",
        key_generated::Qt_Key_Key_E => "e",
        key_generated::Qt_Key_Key_F => "f",
        key_generated::Qt_Key_Key_G => "g",
        key_generated::Qt_Key_Key_H => "h",
        key_generated::Qt_Key_Key_I => "i",
        key_generated::Qt_Key_Key_J => "j",
        key_generated::Qt_Key_Key_K => "k",
        key_generated::Qt_Key_Key_L => "l",
        key_generated::Qt_Key_Key_M => "m",
        key_generated::Qt_Key_Key_N => "n",
        key_generated::Qt_Key_Key_O => "o",
        key_generated::Qt_Key_Key_P => "p",
        key_generated::Qt_Key_Key_Q => "q",
        key_generated::Qt_Key_Key_R => "r",
        key_generated::Qt_Key_Key_S => "s",
        key_generated::Qt_Key_Key_T => "t",
        key_generated::Qt_Key_Key_U => "u",
        key_generated::Qt_Key_Key_V => "v",
        key_generated::Qt_Key_Key_W => "w",
        key_generated::Qt_Key_Key_X => "x",
        key_generated::Qt_Key_Key_Y => "y",
        key_generated::Qt_Key_Key_Z => "z",
        key_generated::Qt_Key_Key_BracketLeft => "[",
        key_generated::Qt_Key_Key_Backslash => "\\",
        key_generated::Qt_Key_Key_BracketRight => "]",
        key_generated::Qt_Key_Key_AsciiCircum => "^",
        key_generated::Qt_Key_Key_Underscore => "_",
        key_generated::Qt_Key_Key_QuoteLeft => "`",
        key_generated::Qt_Key_Key_BraceLeft => "{",
        key_generated::Qt_Key_Key_Bar => "|",
        key_generated::Qt_Key_Key_BraceRight => "}",
        key_generated::Qt_Key_Key_AsciiTilde => "~",
        _ => "",
    }
    .into()
}

pub(crate) mod ffi {
    use std::ffi::c_void;

    use super::QtWindow;

    #[no_mangle]
    pub extern "C" fn slint_qt_get_widget(
        window_adapter: &i_slint_core::window::WindowAdapterRc,
    ) -> *mut c_void {
        <dyn std::any::Any>::downcast_ref(window_adapter.as_any())
            .map_or(std::ptr::null_mut(), |win: &QtWindow| {
                win.widget_ptr().cast::<c_void>().as_ptr()
            })
    }
}

fn utf8_byte_offset_to_utf16_units(str: &str, byte_offset: usize) -> usize {
    let mut current_offset = 0;
    let mut utf16_units = 0;
    for ch in str.chars() {
        if current_offset >= byte_offset {
            break;
        }
        current_offset += ch.len_utf8();
        utf16_units += ch.len_utf16();
    }
    utf16_units
}

#[test]
fn test_utf8_byte_offset_to_utf16_units() {
    assert_eq!(utf8_byte_offset_to_utf16_units("Hello", 2), 2);

    {
        let test_str = "a🚀🍌";
        assert_eq!(test_str.encode_utf16().count(), 5);

        let banana_offset = test_str.char_indices().nth(2).unwrap().0;

        assert_eq!(
            utf8_byte_offset_to_utf16_units(test_str, banana_offset),
            // 'a' encodes as one utf-16 unit, the rocket ship requires two units
            3
        );
    }
}
