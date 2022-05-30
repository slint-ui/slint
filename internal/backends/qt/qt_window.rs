// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cspell:ignore corelib SFPS QWIDGETSIZE pixmap qpointf qreal Antialiasing ARGB Rgba

use cpp::*;
use euclid::approxeq::ApproxEq;
use i_slint_core::graphics::rendering_metrics_collector::{
    RenderingMetrics, RenderingMetricsCollector,
};
use i_slint_core::graphics::{
    Brush, Color, FontRequest, Image, Point, Rect, RenderingCache, SharedImageBuffer, Size,
};
use i_slint_core::input::{KeyEvent, KeyEventType, MouseEvent};
use i_slint_core::item_rendering::{CachedRenderingData, ItemRenderer};
use i_slint_core::items::{
    self, FillRule, ImageRendering, InputType, ItemRc, ItemRef, Layer, MouseCursor, Opacity,
    PointerEventButton, RenderingResult, TextOverflow, TextWrap,
};
use i_slint_core::layout::Orientation;
use i_slint_core::window::{PlatformWindow, PopupWindow, PopupWindowLocation, WindowRc};
use i_slint_core::{component::ComponentRc, SharedString};
use i_slint_core::{ImageInner, PathData, Property};
use items::{ImageFit, TextHorizontalAlignment, TextVerticalAlignment};

use std::cell::RefCell;
use std::pin::Pin;
use std::ptr::NonNull;
use std::rc::{Rc, Weak};

use crate::key_generated;

cpp! {{
    #include <QtWidgets/QtWidgets>
    #include <QtWidgets/QGraphicsScene>
    #include <QtWidgets/QGraphicsBlurEffect>
    #include <QtWidgets/QGraphicsPixmapItem>
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
                let pos = Point::new(pos.x as _, pos.y as _);
                let button = from_qt_button(button);
                rust_window.mouse_event(MouseEvent::MousePressed{ pos, button })
            });
        }
        void mouseReleaseEvent(QMouseEvent *event) override {
            // HACK: Qt on windows is a bit special when clicking on the window
            //       close button and when the resulting close event is ignored.
            //       In that case a release event that was not preceeded by
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
                let pos = Point::new(pos.x as _, pos.y as _);
                let button = from_qt_button(button);
                rust_window.mouse_event(MouseEvent::MouseReleased{ pos, button })
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
                let pos = Point::new(pos.x as _, pos.y as _);
                rust_window.mouse_event(MouseEvent::MouseMoved{pos})
            });
        }
        void wheelEvent(QWheelEvent *event) override {
            QPointF pos = event->position();
            QPoint delta = event->pixelDelta();
            if (delta.isNull()) {
                delta = event->angleDelta();
            }
            rust!(Slint_mouseWheelEvent [rust_window: &QtWindow as "void*", pos: qttypes::QPointF as "QPointF", delta: qttypes::QPoint as "QPoint"] {
                let pos = Point::new(pos.x as _, pos.y as _);
                let delta = Point::new(delta.x as _, delta.y as _);
                rust_window.mouse_event(MouseEvent::MouseWheel{pos, delta})
            });
        }
        void leaveEvent(QEvent *) override {
            rust!(Slint_mouseLeaveEvent [rust_window: &QtWindow as "void*"] {
                rust_window.mouse_event(MouseEvent::MouseExit)
            });
        }

        void keyPressEvent(QKeyEvent *event) override {
            uint modifiers = uint(event->modifiers());
            QString text =  event->text();
            int key = event->key();
            rust!(Slint_keyPress [rust_window: &QtWindow as "void*", key: i32 as "int", text: qttypes::QString as "QString", modifiers: u32 as "uint"] {
                rust_window.key_event(key, text.clone(), modifiers, false);
            });
        }
        void keyReleaseEvent(QKeyEvent *event) override {
            uint modifiers = uint(event->modifiers());
            QString text =  event->text();
            int key = event->key();
            rust!(Slint_keyRelease [rust_window: &QtWindow as "void*", key: i32 as "int", text: qttypes::QString as "QString", modifiers: u32 as "uint"] {
                rust_window.key_event(key, text.clone(), modifiers, true);
            });
        }

        void customEvent(QEvent *event) override {
            if (event->type() == QEvent::User) {
                rust!(Slint_updateWindowProps [rust_window: &QtWindow as "void*"]{
                   if let Some(window) = rust_window.self_weak.upgrade() { window.update_window_properties() }
                });
            } else {
                QWidget::customEvent(event);
            }
        }

        void changeEvent(QEvent *event) override {
            if (event->type() == QEvent::ActivationChange) {
                bool active = isActiveWindow();
                rust!(Slint_updateWindowActivation [rust_window: &QtWindow as "void*", active: bool as "bool"]{
                    if let Some(window) = rust_window.self_weak.upgrade() { window.set_active(active) }
                 });
            }
            QWidget::changeEvent(event);
        }

        void closeEvent(QCloseEvent *event) override {
            bool accepted = rust!(Slint_requestClose [rust_window: &QtWindow as "void*"] -> bool as "bool" {
                if let Some(window) = rust_window.self_weak.upgrade() {
                    return window.request_close();
                }
                true
            });
            if (accepted) {
                event->accept();
            } else {
                event->ignore();
            }
        }

        QSize sizeHint() const override {
            auto preferred_size = rust!(Slint_sizeHint [rust_window: &QtWindow as "void*"] -> qttypes::QSize as "QSize" {
                let component_rc = rust_window.self_weak.upgrade().unwrap().component();
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
        // Add or substract a small amount to make sure each stop is different but still in [0..1].
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
        1 => PointerEventButton::left,
        2 => PointerEventButton::right,
        4 => PointerEventButton::middle,
        _ => PointerEventButton::none,
    }
}

/// Given a position offset and an object of a given type that has x,y,width,height properties,
/// create a QRectF that fits it.
macro_rules! get_geometry {
    ($ty:ty, $obj:expr) => {{
        type Ty = $ty;
        let width = Ty::FIELD_OFFSETS.width.apply_pin($obj).get();
        let height = Ty::FIELD_OFFSETS.height.apply_pin($obj).get();
        if width <= 0. || height <= 0. {
            return Default::default();
        };
        qttypes::QRectF { x: 0., y: 0., width: width as _, height: height as _ }
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

#[derive(Clone)]
enum QtRenderingCacheItem {
    Pixmap(qttypes::QPixmap),
    Invalid,
}

impl Default for QtRenderingCacheItem {
    fn default() -> Self {
        Self::Invalid
    }
}

type QtRenderingCache = Rc<RefCell<RenderingCache<QtRenderingCacheItem>>>;

struct QtItemRenderer {
    painter: QPainterPtr,
    cache: QtRenderingCache,
    default_font_properties: FontRequest,
    window: WindowRc,
    metrics: RenderingMetrics,
}

impl ItemRenderer for QtItemRenderer {
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
            rect.border_width(),
            rect.border_radius(),
        );
    }

    fn draw_image(&mut self, image: Pin<&items::ImageItem>, _: &ItemRc) {
        let dest_rect: qttypes::QRectF = get_geometry!(items::ImageItem, image);
        self.draw_image_impl(
            &image.cached_rendering_data,
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

    fn draw_clipped_image(&mut self, image: Pin<&items::ClippedImage>, _: &ItemRc) {
        let dest_rect: qttypes::QRectF = get_geometry!(items::ClippedImage, image);
        let source_rect = qttypes::QRectF {
            x: image.source_clip_x() as _,
            y: image.source_clip_y() as _,
            width: image.source_clip_width() as _,
            height: image.source_clip_height() as _,
        };
        self.draw_image_impl(
            &image.cached_rendering_data,
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
        let font: QFont =
            get_font(text.unresolved_font_request().merge(&self.default_font_properties));
        let flags = match text.horizontal_alignment() {
            TextHorizontalAlignment::left => key_generated::Qt_AlignmentFlag_AlignLeft,
            TextHorizontalAlignment::center => key_generated::Qt_AlignmentFlag_AlignHCenter,
            TextHorizontalAlignment::right => key_generated::Qt_AlignmentFlag_AlignRight,
        } | match text.vertical_alignment() {
            TextVerticalAlignment::top => key_generated::Qt_AlignmentFlag_AlignTop,
            TextVerticalAlignment::center => key_generated::Qt_AlignmentFlag_AlignVCenter,
            TextVerticalAlignment::bottom => key_generated::Qt_AlignmentFlag_AlignBottom,
        } | match text.wrap() {
            TextWrap::no_wrap => 0,
            TextWrap::word_wrap => key_generated::Qt_TextFlag_TextWordWrap,
        };
        let elide = text.overflow() == TextOverflow::elide;
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
        let selection_foreground_color: u32 =
            text_input.selection_foreground_color().as_argb_encoded();
        let selection_background_color: u32 =
            text_input.selection_background_color().as_argb_encoded();

        let text = text_input.text();
        let mut string: qttypes::QString = text.as_str().into();

        if let InputType::password = text_input.input_type() {
            cpp! { unsafe [mut string as "QString"] {
                string.fill(QChar(qApp->style()->styleHint(QStyle::SH_LineEdit_PasswordCharacter, nullptr, nullptr)));
            }}
        }

        let font: QFont =
            get_font(text_input.unresolved_font_request().merge(&self.default_font_properties));
        let flags = match text_input.horizontal_alignment() {
            TextHorizontalAlignment::left => key_generated::Qt_AlignmentFlag_AlignLeft,
            TextHorizontalAlignment::center => key_generated::Qt_AlignmentFlag_AlignHCenter,
            TextHorizontalAlignment::right => key_generated::Qt_AlignmentFlag_AlignRight,
        } | match text_input.vertical_alignment() {
            TextVerticalAlignment::top => key_generated::Qt_AlignmentFlag_AlignTop,
            TextVerticalAlignment::center => key_generated::Qt_AlignmentFlag_AlignVCenter,
            TextVerticalAlignment::bottom => key_generated::Qt_AlignmentFlag_AlignBottom,
        } | match text_input.wrap() {
            TextWrap::no_wrap => 0,
            TextWrap::word_wrap => key_generated::Qt_TextFlag_TextWordWrap,
        };

        // convert byte offsets to offsets in Qt UTF-16 encoded string, as that's
        // what QTextLayout expects.
        let cursor_position_as_offset: i32 = text_input.cursor_position();
        let anchor_position_as_offset: i32 = text_input.anchor_position();
        let cursor_position: i32 = if cursor_position_as_offset > 0 {
            utf8_byte_offset_to_utf16_units(text.as_str(), cursor_position_as_offset as usize)
                as i32
        } else {
            0
        };
        let anchor_position: i32 = if anchor_position_as_offset > 0 {
            utf8_byte_offset_to_utf16_units(text.as_str(), anchor_position_as_offset as usize)
                as i32
        } else {
            0
        };

        let text_cursor_width: f32 = if text_input.cursor_visible() && text_input.enabled() {
            text_input.text_cursor_width()
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
                fmt.setBackground(QColor::fromRgba(selection_background_color));
                fmt.setForeground(QColor::fromRgba(selection_foreground_color));
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
        let stroke_width: f32 = path.stroke_width();
        let (offset, path_events) = path.fitted_path_events();
        let pos = qttypes::QPoint { x: offset.x as _, y: offset.y as _ };
        let mut painter_path = QPainterPath::default();

        painter_path.set_fill_rule(match path.fill_rule() {
            FillRule::nonzero => key_generated::Qt_FillRule_WindingFill,
            FillRule::evenodd => key_generated::Qt_FillRule_OddEvenFill,
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

    fn draw_box_shadow(&mut self, box_shadow: Pin<&items::BoxShadow>, _: &ItemRc) {
        let cached_shadow_pixmap = box_shadow
            .cached_rendering_data
            .get_or_update(&self.cache, || {
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
                    box_shadow.border_radius(),
                );

                drop(painter_);

                let blur_radius = box_shadow.blur();

                let shadow_pixmap = if blur_radius > 0. {
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
                };
                QtRenderingCacheItem::Pixmap(shadow_pixmap)
            });

        let pixmap: &qttypes::QPixmap = match &cached_shadow_pixmap {
            QtRenderingCacheItem::Pixmap(pixmap) => pixmap,
            _ => return,
        };

        let blur_radius = box_shadow.blur();

        let shadow_offset = qttypes::QPointF {
            x: (box_shadow.offset_x() - blur_radius) as f64,
            y: (box_shadow.offset_y() - blur_radius) as f64,
        };

        let painter: &mut QPainterPtr = &mut self.painter;
        cpp! { unsafe [
                painter as "QPainterPtr*",
                shadow_offset as "QPointF",
                pixmap as "QPixmap*"
            ] {
            (*painter)->drawPixmap(shadow_offset, *pixmap);
        }}
    }

    fn visit_opacity(&mut self, opacity_item: Pin<&Opacity>, self_rc: &ItemRc) -> RenderingResult {
        let opacity = opacity_item.opacity();
        if Opacity::need_layer(self_rc, opacity) {
            self.render_and_blend_layer(&opacity_item.cached_rendering_data, opacity, self_rc)
        } else {
            self.apply_opacity(opacity);
            opacity_item.cached_rendering_data.release(&mut self.cache.borrow_mut());
            RenderingResult::ContinueRenderingChildren
        }
    }

    fn visit_layer(&mut self, layer_item: Pin<&Layer>, self_rc: &ItemRc) -> RenderingResult {
        if layer_item.cache_rendering_hint() {
            self.render_and_blend_layer(&layer_item.cached_rendering_data, 1.0, self_rc)
        } else {
            RenderingResult::ContinueRenderingChildren
        }
    }

    fn combine_clip(&mut self, rect: Rect, radius: f32, mut border_width: f32) {
        let mut clip_rect = qttypes::QRectF {
            x: rect.min_x() as _,
            y: rect.min_y() as _,
            width: rect.width() as _,
            height: rect.height() as _,
        };
        adjust_rect_and_border_for_inner_drawing(&mut clip_rect, &mut border_width);
        let painter: &mut QPainterPtr = &mut self.painter;
        cpp! { unsafe [painter as "QPainterPtr*", clip_rect as "QRectF", radius as "float"] {
            if (radius <= 0) {
                (*painter)->setClipRect(clip_rect, Qt::IntersectClip);
            } else {
                QPainterPath path;
                path.addRoundedRect(clip_rect, radius, radius);
                (*painter)->setClipPath(path, Qt::IntersectClip);
            }
        }}
    }

    fn get_current_clip(&self) -> Rect {
        let painter: &QPainterPtr = &self.painter;
        let res = cpp! { unsafe [painter as "const QPainterPtr*" ] -> qttypes::QRectF as "QRectF" {
            return (*painter)->clipBoundingRect();
        }};
        Rect::new(Point::new(res.x as _, res.y as _), Size::new(res.width as _, res.height as _))
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
        _item_cache: &i_slint_core::item_rendering::CachedRenderingData,
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
        let font: QFont = get_font(self.default_font_properties.clone());
        let painter: &mut QPainterPtr = &mut self.painter;
        cpp! { unsafe [painter as "QPainterPtr*", fill_brush as "QBrush", mut string as "QString", font as "QFont"] {
            (*painter)->setFont(font);
            (*painter)->setPen(QPen(fill_brush, 0));
            (*painter)->setBrush(Qt::NoBrush);
            (*painter)->drawText(0, QFontMetrics((*painter)->font()).ascent(), string);
        }}
    }

    fn window(&self) -> WindowRc {
        self.window.clone()
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        &mut self.painter
    }

    fn translate(&mut self, x: f32, y: f32) {
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

pub(crate) fn load_image_from_resource(
    resource: &ImageInner,
    source_size: Option<qttypes::QSize>,
    image_fit: ImageFit,
) -> Option<qttypes::QPixmap> {
    let (is_path, data, format) = match resource {
        ImageInner::None => return None,
        ImageInner::AbsoluteFilePath(path) => {
            (true, qttypes::QByteArray::from(path.as_str()), Default::default())
        }
        ImageInner::EmbeddedData { data, format } => (
            false,
            qttypes::QByteArray::from(data.as_slice()),
            qttypes::QByteArray::from(format.as_slice()),
        ),
        ImageInner::EmbeddedImage(buffer) => {
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
        ImageInner::StaticTextures { .. } => todo!(),
    };
    let size_requested = is_svg(resource) && source_size.is_some();
    let source_size = source_size.unwrap_or_default();
    debug_assert_eq!(ImageFit::contain as i32, 1);
    debug_assert_eq!(ImageFit::cover as i32, 2);
    Some(cpp! { unsafe [
            data as "QByteArray",
            is_path as "bool",
            format as "QByteArray",
            size_requested as "bool",
            source_size as "QSize",
            image_fit as "int"] -> qttypes::QPixmap as "QPixmap" {
        QImageReader reader;
        QBuffer buffer;
        if (is_path) {
            reader.setFileName(QString::fromUtf8(data));
        } else {
            buffer.setBuffer(const_cast<QByteArray *>(&data));
            reader.setDevice(&buffer);
        }
        if (!reader.canRead()) {
            QString fileName = reader.fileName();
            if (!fileName.isEmpty()) {
                qWarning("Error loading image \"%s\": %s", QFile::encodeName(fileName).constData(), qPrintable(reader.errorString()));
            } else {
                qWarning("Error loading image of format %s: %s", format.constData(), qPrintable(reader.errorString()));
            }
            return QPixmap();
        }
        if (size_requested) {
            if (reader.supportsOption(QImageIOHandler::ScaledSize)) {
                auto target_size = source_size;
                if (image_fit == 1) { //ImageFit::contain
                    QSizeF s = reader.size();
                    target_size = (s * qMin(source_size.width() / s.width(), source_size.height() / s.height())).toSize();
                } else if (image_fit == 2) { //ImageFit::cover
                    QSizeF s = reader.size();
                    target_size = (s * qMax(source_size.width() / s.width(), source_size.height() / s.height())).toSize();
                }
                reader.setScaledSize(target_size);
            }
        }
        return QPixmap::fromImageReader(&reader);
    }})
}

/// Changes the source or the destination rectangle to respect the image fit
fn adjust_to_image_fit(
    image_fit: ImageFit,
    source_rect: &mut qttypes::QRectF,
    dest_rect: &mut qttypes::QRectF,
) {
    match image_fit {
        i_slint_core::items::ImageFit::fill => (),
        i_slint_core::items::ImageFit::cover => {
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
        i_slint_core::items::ImageFit::contain => {
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

/// Return true if this image is a SVG that is scalable
fn is_svg(resource: &ImageInner) -> bool {
    match resource {
        ImageInner::None => false,
        ImageInner::AbsoluteFilePath(path) => {
            path.as_str().ends_with(".svg") || path.as_str().ends_with(".svgz")
        }
        ImageInner::EmbeddedData { format, .. } => {
            format.as_slice() == b"svg" || format.as_slice() == b"svgz"
        }
        ImageInner::EmbeddedImage { .. } => false,
        ImageInner::StaticTextures { .. } => false,
    }
}

impl QtItemRenderer {
    fn draw_image_impl(
        &mut self,
        item_cache: &CachedRenderingData,
        source_property: Pin<&Property<Image>>,
        dest_rect: qttypes::QRectF,
        source_rect: Option<qttypes::QRectF>,
        target_width: std::pin::Pin<&Property<f32>>,
        target_height: std::pin::Pin<&Property<f32>>,
        image_fit: ImageFit,
        rendering: ImageRendering,
        colorize_property: Option<Pin<&Property<Brush>>>,
    ) {
        // Caller ensured that zero/negative width/height resulted in an early return via get_geometry!.
        debug_assert!(target_width.get() > 0.);
        debug_assert!(target_height.get() > 0.);

        let cached = item_cache.get_or_update(&self.cache, || {
            // Query target_width/height here again to ensure that changes will invalidate the item rendering cache.
            let target_width = target_width.get() as f64;
            let target_height = target_height.get() as f64;

            let has_source_clipping = source_rect.map_or(false, |rect| {
                rect.is_valid()
                    && (rect.x != 0.
                        || rect.y != 0.
                        || !rect.width.approx_eq(&target_width)
                        || !rect.height.approx_eq(&target_height))
            });
            let source_size = if !has_source_clipping {
                Some(qttypes::QSize { width: target_width as u32, height: target_height as u32 })
            } else {
                // Source size & clipping is not implemented yet
                None
            };

            load_image_from_resource((&source_property.get()).into(), source_size, image_fit)
                .map_or(QtRenderingCacheItem::Invalid, |mut pixmap: qttypes::QPixmap| {
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
                    QtRenderingCacheItem::Pixmap(pixmap)
                })
        });
        let pixmap: &qttypes::QPixmap = match &cached {
            QtRenderingCacheItem::Pixmap(pixmap) => pixmap,
            _ => return,
        };
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
        let smooth: bool = rendering == ImageRendering::smooth;
        cpp! { unsafe [
                painter as "QPainterPtr*",
                pixmap as "QPixmap*",
                source_rect as "QRectF",
                dest_rect as "QRectF",
                smooth as "bool"] {
            (*painter)->save();
            (*painter)->setRenderHint(QPainter::SmoothPixmapTransform, smooth);
            (*painter)->drawPixmap(dest_rect, *pixmap, source_rect);
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
        item_cache: &CachedRenderingData,
        item_rc: &ItemRc,
        layer_size_fn: &dyn Fn() -> qttypes::QSize,
    ) -> Option<qttypes::QPixmap> {
        let cache_entry = item_cache.get_or_update(&self.cache.clone(), || {
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

            QtRenderingCacheItem::Pixmap(qttypes::QPixmap::from(layer_image))
        });
        match &cache_entry {
            QtRenderingCacheItem::Pixmap(pixmap) => Some(pixmap.clone()),
            _ => None,
        }
    }

    fn render_and_blend_layer(
        &mut self,
        item_cache: &CachedRenderingData,
        alpha_tint: f32,
        self_rc: &ItemRc,
    ) -> RenderingResult {
        let current_clip = self.get_current_clip();
        if let Some(mut layer_image) = self.render_layer(item_cache, self_rc, &|| {
            // We don't need to include the size of the opacity item itself, since it has no content.
            let children_rect = i_slint_core::properties::evaluate_no_tracking(|| {
                let self_ref = self_rc.borrow();
                self_ref.as_ref().geometry().union(
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
        }) {
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
        }
        RenderingResult::ContinueRenderingWithoutChildren
    }
}

cpp_class!(unsafe struct QWidgetPtr as "std::unique_ptr<QWidget>");

pub struct QtWindow {
    widget_ptr: QWidgetPtr,
    pub(crate) self_weak: Weak<i_slint_core::window::Window>,

    rendering_metrics_collector: Option<Rc<RenderingMetricsCollector>>,

    cache: QtRenderingCache,
}

impl QtWindow {
    pub fn new(window_weak: &Weak<i_slint_core::window::Window>) -> Rc<Self> {
        let widget_ptr = cpp! {unsafe [] -> QWidgetPtr as "std::unique_ptr<QWidget>" {
            ensure_initialized(true);
            return std::make_unique<SlintWidget>();
        }};
        let rc = Rc::new(QtWindow {
            widget_ptr,
            self_weak: window_weak.clone(),
            rendering_metrics_collector: RenderingMetricsCollector::new(window_weak.clone()),
            cache: Default::default(),
        });
        let self_weak = Rc::downgrade(&rc);
        let widget_ptr = rc.widget_ptr();
        let rust_window = Rc::as_ptr(&rc);
        cpp! {unsafe [widget_ptr as "SlintWidget*", rust_window as "void*"]  {
            widget_ptr->rust_window = rust_window;
        }};
        ALL_WINDOWS.with(|aw| aw.borrow_mut().push(self_weak));
        rc
    }

    /// Return the QWidget*
    fn widget_ptr(&self) -> NonNull<()> {
        unsafe { std::mem::transmute_copy::<QWidgetPtr, NonNull<_>>(&self.widget_ptr) }
    }

    fn paint_event(&self, painter: QPainterPtr) {
        let runtime_window = self.self_weak.upgrade().unwrap();
        runtime_window.clone().draw_contents(|components| {
            i_slint_core::animations::update_animations();
            let cache = self.cache.clone();
            let mut renderer = QtItemRenderer {
                painter,
                cache,
                default_font_properties: self.default_font_properties(),
                window: runtime_window,
                metrics: RenderingMetrics { layers_created: Some(0) },
            };

            for (component, origin) in components {
                i_slint_core::item_rendering::render_component_items(
                    component,
                    &mut renderer,
                    *origin,
                );
            }

            if let Some(collector) = &self.rendering_metrics_collector {
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
    }

    fn resize_event(&self, size: qttypes::QSize) {
        self.self_weak
            .upgrade()
            .unwrap()
            .set_window_item_geometry(size.width as _, size.height as _);
    }

    fn mouse_event(&self, event: MouseEvent) {
        self.self_weak.upgrade().unwrap().process_mouse_input(event);
        timer_event();
    }

    fn key_event(&self, key: i32, text: qttypes::QString, qt_modifiers: u32, released: bool) {
        i_slint_core::animations::update_animations();
        let text: String = text.into();
        let modifiers = i_slint_core::input::KeyboardModifiers {
            control: (qt_modifiers & key_generated::Qt_KeyboardModifier_ControlModifier) != 0,
            alt: (qt_modifiers & key_generated::Qt_KeyboardModifier_AltModifier) != 0,
            shift: (qt_modifiers & key_generated::Qt_KeyboardModifier_ShiftModifier) != 0,
            meta: (qt_modifiers & key_generated::Qt_KeyboardModifier_MetaModifier) != 0,
        };

        let text = qt_key_to_string(key as key_generated::Qt_Key, text);

        let event = KeyEvent {
            event_type: if released { KeyEventType::KeyReleased } else { KeyEventType::KeyPressed },
            text,
            modifiers,
        };
        self.self_weak.upgrade().unwrap().process_key_input(&event);

        timer_event();
    }

    fn default_font_properties(&self) -> FontRequest {
        self.self_weak.upgrade().unwrap().default_font_properties()
    }

    fn close_popup(&self) {
        self.self_weak.upgrade().unwrap().close_popup();
    }
}

#[allow(unused)]
impl PlatformWindow for QtWindow {
    fn show(self: Rc<Self>) {
        let component_rc = self.self_weak.upgrade().unwrap().component();
        let component = ComponentRc::borrow_pin(&component_rc);
        let root_item = component.as_ref().get_item_ref(0);
        if let Some(window_item) = ItemRef::downcast_pin(root_item) {
            self.apply_window_properties(window_item);
        }

        let widget_ptr = self.widget_ptr();
        cpp! {unsafe [widget_ptr as "QWidget*"] {
            widget_ptr->show();
        }};
        if let Some(collector) = &self.rendering_metrics_collector {
            let qt_platform_name = cpp! {unsafe [] -> qttypes::QString as "QString" {
                return QGuiApplication::platformName();
            }};
            collector.start(&format!("Qt backend (platform {})", qt_platform_name));
        }
    }

    fn hide(self: Rc<Self>) {
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
            width: window_item.width().ceil() as _,
            height: window_item.height().ceil() as _,
        };
        if size.width == 0 || size.height == 0 {
            let existing_size = cpp!(unsafe [widget_ptr as "QWidget*"] -> qttypes::QSize as "QSize" {
                return widget_ptr->size();
            });
            if size.width == 0 {
                window_item.width.set(existing_size.width as _);
                size.width = existing_size.width;
            }
            if size.height == 0 {
                window_item.height.set(existing_size.height as _);
                size.height = existing_size.height;
            }
        }
        let background: u32 = window_item.background().as_argb_encoded();

        match (&window_item.icon()).into() {
            &ImageInner::AbsoluteFilePath(ref path) => {
                let icon_name: qttypes::QString = path.as_str().into();
                cpp! {unsafe [widget_ptr as "QWidget*", icon_name as "QString"] {
                    widget_ptr->setWindowIcon(QIcon(icon_name));
                }};
            }
            &ImageInner::None => (),
            r => {
                if let Some(pixmap) = load_image_from_resource(r, None, ImageFit::contain) {
                    cpp! {unsafe [widget_ptr as "QWidget*", pixmap as "QPixmap"] {
                        widget_ptr->setWindowIcon(QIcon(pixmap));
                    }};
                }
            }
        };

        cpp! {unsafe [widget_ptr as "QWidget*",  title as "QString", size as "QSize", background as "QRgb", no_frame as "bool"] {
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
            pal.setColor(QPalette::Window, QColor::fromRgba(background));
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

    fn free_graphics_resources<'a>(&self, items: &mut dyn Iterator<Item = Pin<ItemRef<'a>>>) {
        for item in items {
            let cached_rendering_data = item.cached_rendering_data_offset();
            cached_rendering_data.release(&mut self.cache.borrow_mut());
        }
    }

    fn show_popup(&self, popup: &i_slint_core::component::ComponentRc, position: Point) {
        let window = i_slint_core::window::Window::new(|window| QtWindow::new(window));
        let popup_window: &QtWindow =
            <dyn std::any::Any>::downcast_ref(window.as_ref().as_any()).unwrap();
        window.set_component(popup);

        let runtime_window = self.self_weak.upgrade().unwrap();
        let size = runtime_window.set_active_popup(PopupWindow {
            location: PopupWindowLocation::TopLevel(window.clone()),
            component: popup.clone(),
        });

        let size = qttypes::QSize { width: size.width as _, height: size.height as _ };

        let popup_ptr = popup_window.widget_ptr();
        let pos = qttypes::QPoint { x: position.x as _, y: position.y as _ };
        let widget_ptr = self.widget_ptr();
        cpp! {unsafe [widget_ptr as "QWidget*", popup_ptr as "QWidget*", pos as "QPoint", size as "QSize"] {
            popup_ptr->setParent(widget_ptr, Qt::Popup);
            popup_ptr->setGeometry(QRect(pos + widget_ptr->mapToGlobal(QPoint(0,0)), size));
            popup_ptr->show();
        }};
    }

    fn set_mouse_cursor(&self, cursor: MouseCursor) {
        let widget_ptr = self.widget_ptr();
        //unidirectional resize cursors are replaced with bidirectional ones
        let cursor_shape = match cursor {
            MouseCursor::default => key_generated::Qt_CursorShape_ArrowCursor,
            MouseCursor::none => key_generated::Qt_CursorShape_BlankCursor,
            MouseCursor::help => key_generated::Qt_CursorShape_WhatsThisCursor,
            MouseCursor::pointer => key_generated::Qt_CursorShape_PointingHandCursor,
            MouseCursor::progress => key_generated::Qt_CursorShape_BusyCursor,
            MouseCursor::wait => key_generated::Qt_CursorShape_WaitCursor,
            MouseCursor::crosshair => key_generated::Qt_CursorShape_CrossCursor,
            MouseCursor::text => key_generated::Qt_CursorShape_IBeamCursor,
            MouseCursor::alias => key_generated::Qt_CursorShape_DragLinkCursor,
            MouseCursor::copy => key_generated::Qt_CursorShape_DragCopyCursor,
            MouseCursor::r#move => key_generated::Qt_CursorShape_DragMoveCursor,
            MouseCursor::no_drop => key_generated::Qt_CursorShape_ForbiddenCursor,
            MouseCursor::not_allowed => key_generated::Qt_CursorShape_ForbiddenCursor,
            MouseCursor::grab => key_generated::Qt_CursorShape_OpenHandCursor,
            MouseCursor::grabbing => key_generated::Qt_CursorShape_ClosedHandCursor,
            MouseCursor::col_resize => key_generated::Qt_CursorShape_SplitHCursor,
            MouseCursor::row_resize => key_generated::Qt_CursorShape_SplitVCursor,
            MouseCursor::n_resize => key_generated::Qt_CursorShape_SizeVerCursor,
            MouseCursor::e_resize => key_generated::Qt_CursorShape_SizeHorCursor,
            MouseCursor::s_resize => key_generated::Qt_CursorShape_SizeVerCursor,
            MouseCursor::w_resize => key_generated::Qt_CursorShape_SizeHorCursor,
            MouseCursor::ne_resize => key_generated::Qt_CursorShape_SizeBDiagCursor,
            MouseCursor::nw_resize => key_generated::Qt_CursorShape_SizeFDiagCursor,
            MouseCursor::se_resize => key_generated::Qt_CursorShape_SizeFDiagCursor,
            MouseCursor::sw_resize => key_generated::Qt_CursorShape_SizeBDiagCursor,
            MouseCursor::ew_resize => key_generated::Qt_CursorShape_SizeHorCursor,
            MouseCursor::ns_resize => key_generated::Qt_CursorShape_SizeVerCursor,
            MouseCursor::nesw_resize => key_generated::Qt_CursorShape_SizeBDiagCursor,
            MouseCursor::nwse_resize => key_generated::Qt_CursorShape_SizeFDiagCursor,
        };
        cpp! {unsafe [widget_ptr as "QWidget*", cursor_shape as "Qt::CursorShape"] {
            widget_ptr->setCursor(QCursor{cursor_shape});
        }};
    }

    fn text_size(
        &self,
        font_request: i_slint_core::graphics::FontRequest,
        text: &str,
        max_width: Option<f32>,
    ) -> Size {
        get_font(font_request.merge(&self.default_font_properties())).text_size(text, max_width)
    }

    fn text_input_byte_offset_for_position(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        pos: Point,
    ) -> usize {
        if pos.y < 0. {
            return 0;
        }
        let rect: qttypes::QRectF = get_geometry!(items::TextInput, text_input);
        let pos = qttypes::QPointF { x: pos.x as _, y: pos.y as _ };
        let font: QFont =
            get_font(text_input.unresolved_font_request().merge(&self.default_font_properties()));
        let string = qttypes::QString::from(text_input.text().as_str());
        let flags = match text_input.horizontal_alignment() {
            TextHorizontalAlignment::left => key_generated::Qt_AlignmentFlag_AlignLeft,
            TextHorizontalAlignment::center => key_generated::Qt_AlignmentFlag_AlignHCenter,
            TextHorizontalAlignment::right => key_generated::Qt_AlignmentFlag_AlignRight,
        } | match text_input.vertical_alignment() {
            TextVerticalAlignment::top => key_generated::Qt_AlignmentFlag_AlignTop,
            TextVerticalAlignment::center => key_generated::Qt_AlignmentFlag_AlignVCenter,
            TextVerticalAlignment::bottom => key_generated::Qt_AlignmentFlag_AlignBottom,
        } | match text_input.wrap() {
            TextWrap::no_wrap => 0,
            TextWrap::word_wrap => key_generated::Qt_TextFlag_TextWordWrap,
        };
        let single_line: bool = text_input.single_line();
        let is_password: bool = matches!(text_input.input_type(), InputType::password);
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
    ) -> Rect {
        let rect: qttypes::QRectF = get_geometry!(items::TextInput, text_input);
        let font: QFont =
            get_font(text_input.unresolved_font_request().merge(&self.default_font_properties()));
        let text = text_input.text();
        let mut string = qttypes::QString::from(text.as_str());
        let offset: u32 = utf8_byte_offset_to_utf16_units(text.as_str(), byte_offset) as _;
        let flags = match text_input.horizontal_alignment() {
            TextHorizontalAlignment::left => key_generated::Qt_AlignmentFlag_AlignLeft,
            TextHorizontalAlignment::center => key_generated::Qt_AlignmentFlag_AlignHCenter,
            TextHorizontalAlignment::right => key_generated::Qt_AlignmentFlag_AlignRight,
        } | match text_input.vertical_alignment() {
            TextVerticalAlignment::top => key_generated::Qt_AlignmentFlag_AlignTop,
            TextVerticalAlignment::center => key_generated::Qt_AlignmentFlag_AlignVCenter,
            TextVerticalAlignment::bottom => key_generated::Qt_AlignmentFlag_AlignBottom,
        } | match text_input.wrap() {
            TextWrap::no_wrap => 0,
            TextWrap::word_wrap => key_generated::Qt_TextFlag_TextWordWrap,
        };
        let single_line: bool = text_input.single_line();
        let r = cpp! { unsafe [font as "QFont", mut string as "QString", offset as "int", flags as "int", rect as "QRectF", single_line as "bool"]
                -> qttypes::QPointF as "QPointF" {
            if (!single_line) {
                string.replace(QChar('\n'), QChar::LineSeparator);
            }
            QTextLayout layout(string, font);
            do_text_layout(layout, flags, rect);

            QTextLine textLine = layout.lineForTextPosition(offset);
            if (!textLine.isValid())
                return QPointF();
            return QPointF(textLine.x() + textLine.cursorToX(offset), textLine.y());
        }};

        let font_size = cpp! { unsafe [font as "QFont"]
                -> i32 as "int" {
            return QFontInfo(font).pixelSize();
        }};

        Rect::new(Point::new(r.x as _, r.y as _), Size::new(1.0, font_size as f32))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn get_font(request: FontRequest) -> QFont {
    let family: qttypes::QString = request.family.unwrap_or_default().as_str().into();
    let pixel_size: f32 = request.pixel_size.unwrap_or(0.);
    let weight: i32 = request.weight.unwrap_or(0);
    let letter_spacing: f32 = request.letter_spacing.unwrap_or_default();
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
    fn text_size(&self, text: &str, max_width: Option<f32>) -> i_slint_core::graphics::Size {
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
        i_slint_core::graphics::Size::new(size.width as _, size.height as _)
    }
}

thread_local! {
    // FIXME: currently the window are never removed
    static ALL_WINDOWS: RefCell<Vec<Weak<QtWindow>>> = Default::default();
}

/// Called by C++'s TimerHandler::timerEvent, or every time a timer might have been started
pub(crate) fn timer_event() {
    i_slint_core::animations::update_animations();
    i_slint_core::timers::TimerList::maybe_activate_timers();

    i_slint_core::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
        if !driver.has_active_animations() {
            return;
        }

        ALL_WINDOWS.with(|windows| {
            for x in windows.borrow().iter() {
                if let Some(x) = x.upgrade() {
                    x.request_redraw();
                }
            }
        });
    });

    let mut timeout = i_slint_core::timers::TimerList::next_timeout().map(|instant| {
        let now = std::time::Instant::now();
        let instant: std::time::Instant = instant.into();
        if instant > now {
            instant.duration_since(now).as_millis() as i32
        } else {
            0
        }
    });
    if i_slint_core::animations::CURRENT_ANIMATION_DRIVER
        .with(|driver| driver.has_active_animations())
    {
        timeout = timeout.map(|x| x.max(16)).or(Some(16));
    };
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
                let mut buffer = [0; 6];
                Some(i_slint_core::SharedString::from(char.encode_utf8(&mut buffer) as &str))
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
    pub extern "C" fn slint_qt_get_widget(window: &i_slint_core::window::WindowRc) -> *mut c_void {
        <dyn std::any::Any>::downcast_ref(window.as_any())
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
